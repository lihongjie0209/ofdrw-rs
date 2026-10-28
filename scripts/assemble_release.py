"""Validate the six-platform distributions and add Go source and checksums."""

import hashlib
from pathlib import Path
import subprocess
import sys
import zipfile

root = Path(__file__).resolve().parents[1]
tag, output = sys.argv[1:]
subprocess.run([sys.executable, str(root / "scripts/check_release_version.py"), tag], check=True)
version = tag[1:]
output = Path(output).resolve()
platforms = ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64", "win32-arm64"]
for platform in platforms:
    path = output / f"ofdrw-native-{platform}-{tag}.zip"
    if not path.is_file() or not zipfile.is_zipfile(path):
        raise SystemExit(f"Missing native distribution: {platform}")
if len(list(output.glob(f"ofdrw_rs-{version}-*.whl"))) != 6:
    raise SystemExit("Expected six platform-specific Python wheels")
for name in [f"ofdrw-rs-{version}.tgz", f"ofdrw-{version}.crate"]:
    if not (output / name).is_file():
        raise SystemExit(f"Missing distribution: {name}")
with zipfile.ZipFile(output / f"ofdrw-go-{tag}.zip", "w", compression=zipfile.ZIP_DEFLATED) as archive:
    paths = subprocess.check_output(["git", "ls-files", "-z", "bindings/go"], cwd=root).split(b"\0")
    for value in paths:
        if value:
            relative = Path(value.decode())
            archive.write(root / relative, f"ofdrw-go-{tag}/{relative.relative_to('bindings/go')}")
checksums = []
for path in sorted(output.iterdir()):
    if path.is_file() and path.name != "SHA256SUMS":
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        checksums.append(f"{digest}  {path.name}")
(output / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
print(f"Prepared {len(checksums)} release assets and SHA256SUMS")
