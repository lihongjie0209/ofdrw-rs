"""Offline provenance and distribution checks for bundled Adobe resources."""
import hashlib
import json
from pathlib import Path
import re
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "crates/ofdrw/assets/cmaps"


class AdobeCmapResourcesTest(unittest.TestCase):
    def test_manifest_matches_original_git_blobs_and_runtime_resource_names(self):
        manifest = json.loads((ASSETS / "manifest.json").read_text())
        self.assertEqual(len(manifest["resources"]), 189)
        self.assertEqual(sum(item["writing_mode"] == 1 for item in manifest["resources"]), 73)
        payload = (ASSETS / "cmaps.zip").read_bytes()
        self.assertEqual(hashlib.sha256(payload).hexdigest(), manifest["archive_sha256"])
        names = re.findall(r'\("([^"]+)", "([^"]+)"\)', (ASSETS / "names.rs").read_text())
        self.assertEqual(names, [(item["name"], item["ordering"]) for item in manifest["resources"]])
        self.assertEqual([name for name, _ in names], sorted({name for name, _ in names}))
        with zipfile.ZipFile(ASSETS / "cmaps.zip") as archive:
            self.assertEqual(archive.namelist(), [name for name, _ in names])
            dependencies = set()
            for item in manifest["resources"]:
                with self.subTest(name=item["name"]):
                    data = archive.read(item["name"])
                    self.assertEqual(len(data), item["bytes"])
                    self.assertEqual(hashlib.sha256(data).hexdigest(), item["sha256"])
                    self.assertEqual(hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest(), item["git_blob_sha1"])
                    self.assertIn(b"Copyright", data)
                    self.assertRegex(data, rb"/WMode\s+" + str(item["writing_mode"]).encode() + rb"\s+def")
                    self.assertRegex(data, rb"/Ordering\s+\(" + re.escape(item["ordering"].encode()) + rb"\)")
                    dependencies.update(name.decode() for name in re.findall(rb"/(\S+)\s+usecmap", data))
            self.assertLessEqual(dependencies, {name for name, _ in names})

    def test_adobe_notice_is_retained_in_each_native_language_distribution(self):
        notice = (ASSETS / "NOTICE").read_text()
        manifest = json.loads((ASSETS / "manifest.json").read_text())
        self.assertIn(manifest["commit"], notice)
        for name in ["NOTICE", "bindings/python/NOTICE", "bindings/node/NOTICE"]:
            with self.subTest(name=name):
                self.assertIn(notice, (ROOT / name).read_text())

    def test_cid_unicode_archive_preserves_original_resources_and_notices(self):
        assets = ROOT / "crates/ofdrw/assets/cid_unicode"
        manifest = json.loads((assets / "manifest.json").read_text())
        self.assertEqual(hashlib.sha256((assets / "unicode.zip").read_bytes()).hexdigest(), manifest["archive_sha256"])
        self.assertEqual({item["ordering"] for item in manifest["resources"]}, {"CNS1", "GB1", "Japan1", "Korea1", "KR"})
        with zipfile.ZipFile(assets / "unicode.zip") as archive:
            self.assertEqual(archive.namelist(), [item["name"] for item in manifest["resources"]])
            for item in manifest["resources"]:
                with self.subTest(name=item["name"]):
                    data = archive.read(item["name"])
                    self.assertEqual(len(data), item["bytes"])
                    self.assertEqual(hashlib.sha256(data).hexdigest(), item["sha256"])
                    self.assertEqual(hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest(), item["git_blob_sha1"])
                    self.assertIn(b"Copyright", data)
                    self.assertRegex(data, rb"/CMapType\s+2\s+def")
        notice = (assets / "NOTICE").read_text()
        self.assertIn(manifest["commit"], notice)
        for name in ["NOTICE", "bindings/python/NOTICE", "bindings/node/NOTICE"]:
            self.assertIn(notice, (ROOT / name).read_text())


if __name__ == "__main__":
    unittest.main()
