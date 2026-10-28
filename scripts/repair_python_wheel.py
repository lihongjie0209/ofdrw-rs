"""Repair a Linux wheel for PyPI and discard the unrepaired platform wheel."""

from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

output = Path(sys.argv[1]).resolve()
wheels = list(output.glob("*-py3-none-linux_*.whl"))
if len(wheels) != 1:
    raise SystemExit(f"Expected one Linux wheel, found {len(wheels)}")
architecture = {"x86_64": "x86_64", "aarch64": "aarch64"}.get(platform.machine())
if architecture is None:
    raise SystemExit(f"Unsupported build architecture: {platform.machine()}")
with tempfile.TemporaryDirectory(prefix="ofdrw-manylinux-") as directory:
    subprocess.run([
        sys.executable, "-m", "auditwheel", "repair", "--plat",
        f"manylinux_2_39_{architecture}", "--wheel-dir", directory, str(wheels[0]),
    ], check=True)
    repaired = list(Path(directory).glob("*.whl"))
    if len(repaired) != 1:
        raise SystemExit("auditwheel did not create exactly one wheel")
    destination = output / repaired[0].name
    shutil.move(repaired[0], destination)
    wheels[0].unlink()
    print(destination)
