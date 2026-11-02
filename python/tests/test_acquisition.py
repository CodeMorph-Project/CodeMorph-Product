import tempfile
import unittest
from pathlib import Path

from codemorph_ml.acquisition import acquire_local_source
from codemorph_ml.provenance import LicensePolicy


class TestAcquisition(unittest.TestCase):
    def test_acquire_local_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "main.cpp").write_text("int main(){return 0;}\n")
            (root / "LICENSE").write_text("MIT License")
            records = acquire_local_source(root, LicensePolicy(allowed_licenses={"MIT"}))
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0].language, "cpp")
            self.assertEqual(records[0].license, "MIT")


if __name__ == "__main__":
    unittest.main()
EOF
PYTHONPATH="$PWD/python" python3 -m unittest python/tests/test_acquisition.py 2>&1 | tail -5