"""Build the Rust C ABI into a platform-specific Python wheel."""

import os
import shutil
import subprocess
import sys
from pathlib import Path

from setuptools import setup
from setuptools.command.build_py import build_py
from wheel.bdist_wheel import bdist_wheel


ROOT = Path(__file__).resolve().parents[2]
LIBRARY = (
    "ofdrw_ffi.dll"
    if os.name == "nt"
    else "libofdrw_ffi.dylib" if sys.platform == "darwin" else "libofdrw_ffi.so"
)


class BuildRust(build_py):
    def run(self):
        subprocess.run(
            ["cargo", "build", "--release", "--locked", "-p", "ofdrw-ffi"],
            cwd=ROOT,
            check=True,
        )
        super().run()
        target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
        if not target_dir.is_absolute():
            target_dir = ROOT / target_dir
        destination = Path(self.build_lib) / "ofdrw_rs"
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copy2(target_dir / "release" / LIBRARY, destination / LIBRARY)


class BinaryWheel(bdist_wheel):
    def finalize_options(self):
        super().finalize_options()
        self.root_is_pure = False

    def get_tag(self):
        _, _, platform = super().get_tag()
        return "py3", "none", platform


setup(cmdclass={"build_py": BuildRust, "bdist_wheel": BinaryWheel})
