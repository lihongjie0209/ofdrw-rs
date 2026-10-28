"""Reject a release tag unless every language package has the same version."""

import json
from pathlib import Path
import re
import sys
import tomllib

root = Path(__file__).resolve().parents[1]
tag = sys.argv[1]
if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", tag):
    raise SystemExit(f"Invalid release tag: {tag}")
expected = tag[1:]
for name in ["crates/ofdrw/Cargo.toml", "crates/ofdrw-ffi/Cargo.toml", "bindings/python/pyproject.toml"]:
    data = tomllib.loads((root / name).read_text())
    version = data.get("package", data.get("project"))["version"]
    if version != expected:
        raise SystemExit(f"{name}: version {version} does not match {tag}")
node = json.loads((root / "bindings/node/package.json").read_text())
if node["version"] != expected:
    raise SystemExit(f"Node.js version does not match {tag}")
print(f"All package versions match {tag}")
