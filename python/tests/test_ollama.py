import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer

from codemorph_ml.ollama_client import OllamaTeacher


class FakeHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length))
        assert body["model"] == "test-model"
        response = {"response": "fn foo() {}", "done": True}
        raw = json.dumps(response).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def log_message(self, *args):
        return


class TestOllamaClient(unittest.TestCase):
    def test_generate_candidates(self):
        server = HTTPServer(("127.0.0.1", 0), FakeHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            from codemorph_ml.ollama_client import OllamaConfig

            cfg = OllamaConfig(host=f"http://127.0.0.1:{server.server_port}", model="test-model")
            cand = OllamaTeacher(cfg).generate_candidates("translate", candidate_count=2)
            self.assertEqual(len(cand), 2)
            self.assertEqual(cand[0].text, "fn foo() {}")
            self.assertEqual(cand[0].input_hash, cand[1].input_hash)
        finally:
            server.shutdown()
            thread.join()

    def test_prompt_contains_source(self):
        from codemorph_ml.synthetic_factory import prompt_for_translation

        prompt = prompt_for_translation("int f(){return 1;}", ["arithmetic"])
        self.assertIn("int f(){return 1;}", prompt)
        self.assertIn("arithmetic", prompt)

    def test_compile_rust(self):
        from codemorph_ml.synthetic_factory import compile_rust_with_rustc

        self.assertTrue(compile_rust_with_rustc("fn main(){}"))
        self.assertFalse(compile_rust_with_rustc("fn main(){"))


if __name__ == "__main__":
    unittest.main()

