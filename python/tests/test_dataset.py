import hashlib
import unittest

from codemorph_ml.dataset import DATASET_SCHEMA_VERSION, DatasetExample, Provenance, VerificationRecord
from codemorph_ml.provenance import LicensePolicy, check_mixture
from codemorph_ml.tokenizer import metrics
from codemorph_ml.training import TrainingConfig, backend_availability


def provenance():
    return Provenance(
        source="example",
        repository="example/repo",
        commit="abc",
        file_path="a.cpp",
        language="cpp",
        license="MIT",
        retrieval_date="2026-10-30",
        content_hash="hash",
    )


class TestDataset(unittest.TestCase):
    def test_valid_example(self):
        example = DatasetExample(
            schema_version=DATASET_SCHEMA_VERSION,
            id="1",
            source_language="cpp",
            target_language="rust",
            source_cpp="int f(){return 1;}",
            structured_context={},
            rust_target="fn f()->i32{1}",
            verification=VerificationRecord(
                cpp_compiles=True,
                rust_compiles=True,
                tests_passed=True,
                differential_passed=True,
                fuzz_iterations=10,
                sanitizer_status="clean",
                static_analysis_status="clean",
                benchmark_ratio=1.0,
                rust_quality=0.9,
                confidence="high",
            ),
            feature_tags=["arithmetic"],
            difficulty="easy",
            provenance=provenance(),
            teacher={"id": "teacher"},
        )
        example.validate()
        self.assertEqual(len(example.content_hash()), 64)

    def test_rejects_unknown_license_mixture(self):
        policy = LicensePolicy(allowed_licenses={"MIT"})
        self.assertEqual(check_mixture(["MIT", "unknown"], policy), ["disallowed or unknown license: unknown"])

    def test_tokenizer_metrics(self):
        m = metrics("int main(){return 0;}\n")
        self.assertGreater(m["tokens_per_loc"], 0)

    def test_backend_availability_shape(self):
        avail = backend_availability()
        self.assertIn("torch", avail)
        self.assertFalse(avail["torch"])


if __name__ == "__main__":
    unittest.main()