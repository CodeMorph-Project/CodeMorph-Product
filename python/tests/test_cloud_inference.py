import contextlib
import io
import json
import os
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from unittest.mock import patch

from codemorph_ml.cloud_inference import main


class CloudInferenceTests(unittest.TestCase):
    def setUp(self):
        self.requests = []
        self.response = {
            "choices": [{"message": {"content": "```rust\nfn main() {}\n```"}}]
        }
        self.response_code = 200
        test = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                test.requests.append((self.path, self.headers.get("Authorization"), payload))
                self.send_response(test.response_code)
                if test.response_code == 307:
                    self.send_header("Location", "/redirect")
                self.end_headers()
                self.wfile.write(json.dumps(test.response).encode())

            def log_message(self, *_args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(thread.join)
        self.addCleanup(server.shutdown)

        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.profile = Path(directory.name)
        (self.profile / "cloud_config.json").write_text(
            json.dumps({
                "chat_completions_url": f"http://127.0.0.1:{server.server_port}/v1/chat/completions",
                "model": "test-model",
                "api_key_env": "CODEMORPH_TEST_API_KEY",
                "options": {"max_completion_tokens": 64},
            }),
            encoding="utf-8",
        )

    def invoke(self, *, allow="1", key="test-key"):
        stdout = io.StringIO()
        with (
            patch.dict(os.environ, {
                "CODEMORPH_ALLOW_EXTERNAL": allow,
                "CODEMORPH_TEST_API_KEY": key,
            }),
            patch("sys.argv", ["cloud_inference", "--model-dir", str(self.profile)]),
            patch("sys.stdin", io.StringIO("int main() {}")),
            contextlib.redirect_stdout(stdout),
        ):
            main()
        return stdout.getvalue()

    def test_compatible_chat_completion(self):
        self.assertEqual(self.invoke(), "fn main() {}\n")
        path, authorization, payload = self.requests[0]
        self.assertEqual(path, "/v1/chat/completions")
        self.assertEqual(authorization, "Bearer test-key")
        self.assertEqual(payload["model"], "test-model")
        self.assertEqual(payload["max_completion_tokens"], 64)
        self.assertEqual(payload["messages"][-1]["content"], "int main() {}")

    def test_opt_in_and_credentials_checked_before_request(self):
        with self.assertRaisesRegex(SystemExit, "CODEMORPH_ALLOW_EXTERNAL"):
            self.invoke(allow="0")
        with self.assertRaisesRegex(SystemExit, "CODEMORPH_TEST_API_KEY"):
            self.invoke(key="")
        self.assertEqual(self.requests, [])

    def test_redirect_is_rejected(self):
        self.response_code = 307
        with self.assertRaisesRegex(SystemExit, "HTTP 307"):
            self.invoke()
        self.assertEqual(len(self.requests), 1)

    def test_truncated_completion_is_rejected(self):
        self.response["choices"][0]["finish_reason"] = "length"
        with self.assertRaisesRegex(SystemExit, "incomplete"):
            self.invoke()
