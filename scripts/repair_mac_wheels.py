"""Validate or repair historical macOS wheel labels without changing native code."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("ofdrw_wheel_platform", root / "bindings/python/wheel_platform.py")
platform = importlib.util.module_from_spec(spec)
spec.loader.exec_module(platform)
parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
parser.add_argument("--check-only", action="store_true")
parser.add_argument("--report", type=Path)
args = parser.parse_args()
changed = []
for wheel in sorted(args.directory.glob("*-macosx_*.whl")):
    expected, valid, binary = platform.macos_wheel_info(wheel)
    if valid:
        continue
    if args.check_only:
        raise SystemExit(f"macOS wheel tag does not match its native library: {wheel.name}")
    original = wheel.name
    subprocess.run([sys.executable, "-m", "wheel", "tags", "--platform-tag", expected, "--remove", str(wheel)], check=True)
    repaired = wheel.with_name(wheel.stem.rsplit("-", 1)[0] + "-" + expected + ".whl")
    _, valid, repaired_binary = platform.macos_wheel_info(repaired)
    if not valid or repaired_binary != binary:
        raise SystemExit("Wheel repair changed native code or did not fix its platform tag")
    changed.append({"old": original, "new": repaired.name})
    print(f"Repaired {original} -> {repaired.name}; native library unchanged")
if args.report:
    args.report.write_text(json.dumps(changed, indent=2) + "\n")
print("macOS wheel platform validation passed")
