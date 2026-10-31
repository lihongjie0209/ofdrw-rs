import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile


path = Path(__file__).resolve().parents[1] / "release_zip.py"
spec = importlib.util.spec_from_file_location("release_zip", path)
release_zip = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release_zip)


class ReleaseZipTests(unittest.TestCase):
    def test_release_scripts_reproduce_archives_and_checksums(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            scripts = root / "scripts"
            scripts.mkdir()
            for name in ["release_zip.py", "package_native.py", "assemble_release.py",
                         "check_release_version.py"]:
                shutil.copyfile(path.parent / name, scripts / name)
            files = {
                "crates/ofdrw/Cargo.toml": '[package]\nversion = "0.2.0"\n',
                "crates/ofdrw-ffi/Cargo.toml": '[package]\nversion = "0.2.0"\n',
                "bindings/python/pyproject.toml": '[project]\nversion = "0.2.0"\n',
                "bindings/node/package.json": '{"version": "0.2.0"}',
                "bindings/go/go.mod": "module example.org/ofdrw\n",
                "bindings/go/ofdrw/doc.go": "package ofdrw\n",
                "LICENSE": "license\n", "NOTICE": "notice\n",
                "include/ofdrw.h": "/* public header */\n",
                "lib/libofdrw_ffi.so": "dynamic library fixture\n",
                "lib/libofdrw_ffi.a": "static library fixture\n",
            }
            for name, content in files.items():
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(content)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "add", "bindings/go"], cwd=root, check=True)
            output = root / "dist"
            output.mkdir()
            for platform in ["linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64", "win32-arm64"]:
                with zipfile.ZipFile(output / f"ofdrw-native-{platform}-v0.2.0.zip", "w") as archive:
                    archive.writestr("fixture", b"other platform")
            for index in range(6):
                (output / f"ofdrw_rs-0.2.0-platform{index}.whl").write_bytes(b"wheel fixture")
            for name in ["ofdrw-rs-0.2.0.tgz", "ofdrw-0.2.0.crate"]:
                (output / name).write_bytes(b"registry fixture")
            snapshots = []
            for index in range(2):
                for name in files:
                    os.utime(root / name, (1700000000 + index * 86400,) * 2)
                    os.chmod(root / name, 0o600 if index == 0 else 0o755)
                for command in [
                    ["package_native.py", "linux-x64", str(root / "lib/libofdrw_ffi.so"), str(output)],
                    ["assemble_release.py", "v0.2.0", str(output)],
                ]:
                    subprocess.run([sys.executable, str(scripts / command[0]), *command[1:]],
                                   cwd=root, check=True, capture_output=True)
                snapshots.append({file.name: file.read_bytes() for file in output.iterdir()})
            self.assertEqual(*snapshots)
            with zipfile.ZipFile(output / "ofdrw-go-v0.2.0.zip") as archive:
                self.assertEqual(archive.namelist(), ["ofdrw-go-v0.2.0/go.mod", "ofdrw-go-v0.2.0/ofdrw/doc.go"])
                self.assertEqual(archive.read("ofdrw-go-v0.2.0/ofdrw/doc.go"), b"package ofdrw\n")
            with zipfile.ZipFile(output / "ofdrw-native-linux-x64-v0.2.0.zip") as archive:
                self.assertEqual(len(archive.namelist()), 6)
                self.assertEqual(archive.read("ofdrw-native-linux-x64-v0.2.0/include/ofdrw.h"), b"/* public header */\n")

    def test_metadata_changes_do_not_change_archive_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "library.so"
            payload = bytes(range(256)) * 8192
            source.write_bytes(payload)
            archives = []
            for index, mode in enumerate([0o600, 0o755]):
                os.chmod(source, mode)
                os.utime(source, (1700000000 + index * 86400,) * 2)
                output = root / f"{index}.zip"
                with zipfile.ZipFile(output, "w") as archive:
                    release_zip.write_entry(archive, "native/library.so", source)
                    release_zip.write_entry(archive, "native/README.txt", b"usage\n")
                archives.append(output.read_bytes())
            self.assertEqual(*archives)
            with zipfile.ZipFile(root / "0.zip") as archive:
                self.assertEqual(archive.read("native/library.so"), payload)
                self.assertEqual(archive.read("native/README.txt"), b"usage\n")
                for entry in archive.infolist():
                    self.assertEqual(entry.date_time, (1980, 1, 1, 0, 0, 0))
                    self.assertEqual(entry.external_attr >> 16, 0o100644)
                    self.assertEqual(entry.create_system, 3)


if __name__ == "__main__":
    unittest.main()
