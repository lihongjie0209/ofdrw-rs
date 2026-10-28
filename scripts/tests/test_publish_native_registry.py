import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

path = Path(__file__).resolve().parents[1] / "publish_native_registry.py"
spec = importlib.util.spec_from_file_location("publisher", path)
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class RegistryIntegrityTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.first = Path(temporary.name) / "first.whl"
        self.second = Path(temporary.name) / "second.whl"
        self.first.write_bytes(b"first distribution")
        self.second.write_bytes(b"second distribution")

    def published(self):
        return {"urls": [{"filename": self.first.name,
                           "digests": {"sha256": hashlib.sha256(self.first.read_bytes()).hexdigest()}}]}

    def test_partial_python_release_uploads_only_missing_files(self):
        self.assertEqual(publisher.verify_pypi([self.first, self.second], self.published()), [self.second])

    def test_matching_python_release_needs_no_upload(self):
        self.assertEqual(publisher.verify_pypi([self.first], self.published()), [])

    def test_existing_python_filename_with_different_content_is_rejected(self):
        published = self.published()
        self.first.write_bytes(b"changed distribution")
        with self.assertRaisesRegex(SystemExit, "different bytes"):
            publisher.verify_pypi([self.first, self.second], published)

    def test_registry_404_means_no_version(self):
        with patch.object(publisher, "urlopen", side_effect=HTTPError("https://example.invalid", 404, "missing", {}, None)):
            self.assertIsNone(publisher.read_url("https://example.invalid"))

    def test_authentication_failure_is_not_treated_as_missing_version(self):
        with patch.object(publisher, "urlopen", side_effect=HTTPError("https://example.invalid", 403, "forbidden", {}, None)):
            with self.assertRaises(HTTPError):
                publisher.read_url("https://example.invalid")

    def test_cargo_checksum_selects_exact_version(self):
        data = b'{"vers":"0.1.0","cksum":"first"}\n{"vers":"0.2.0","cksum":"second"}\n'
        with patch.object(publisher, "read_url", return_value=data):
            self.assertEqual(publisher.cargo_checksum("0.2.0"), "second")
            self.assertIsNone(publisher.cargo_checksum("0.3.0"))


if __name__ == "__main__":
    unittest.main()
