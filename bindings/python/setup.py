"""Build the Rust C ABI into a platform-specific Python wheel."""

import os
import importlib.util
import shutil
import subprocess
import sys
from pathlib import Path

from setuptools import Distribution, setup
from setuptools.command.build_py import build_py
from wheel.bdist_wheel import bdist_wheel


ROOT = Path(__file__).resolve().parents[2]
_platform_spec = importlib.util.spec_from_file_location(
    "ofdrw_wheel_platform", Path(__file__).with_name("wheel_platform.py")
)
_wheel_platform = importlib.util.module_from_spec(_platform_spec)
_platform_spec.loader.exec_module(_wheel_platform)
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
        if sys.platform == "darwin":
            libraries = list(Path(self.bdist_dir).rglob(LIBRARY))
            if len(libraries) != 1:
                raise ValueError("Expected exactly one macOS native library in the wheel")
            platform = _wheel_platform.macos_platform_tag(platform, libraries[0].read_bytes())
        return "py3", "none", platform


class BinaryDistribution(Distribution):
    def has_ext_modules(self):
        # Native C ABI files must be installed into platlib for auditwheel.
        return True


setup(
    distclass=BinaryDistribution,
    cmdclass={"build_py": BuildRust, "bdist_wheel": BinaryWheel},
)
