import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile

path = Path(__file__).resolve().parents[2] / "bindings/python/wheel_platform.py"
spec = importlib.util.spec_from_file_location("wheel_platform", path)
platform = importlib.util.module_from_spec(spec)
spec.loader.exec_module(platform)


def macho(cpu):
    return b"\xcf\xfa\xed\xfe" + cpu.to_bytes(4, "little") + b"native payload"


class MacWheelTests(unittest.TestCase):
    def test_intel_library_is_not_universal2(self):
        self.assertEqual(platform.macos_platform_tag("macosx_10_13_universal2", macho(0x01000007)), "macosx_10_13_x86_64")

    def test_arm_library_is_not_universal2(self):
        self.assertEqual(platform.macos_platform_tag("macosx_11_0_universal2", macho(0x0100000C)), "macosx_11_0_arm64")

    def test_arm_minimum_is_at_least_macos_11(self):
        self.assertEqual(platform.macos_platform_tag("macosx_10_13_universal2", macho(0x0100000C)), "macosx_11_0_arm64")

    def test_native_tag_is_preserved(self):
        self.assertEqual(platform.macos_platform_tag("macosx_15_0_arm64", macho(0x0100000C)), "macosx_15_0_arm64")

    def test_invalid_header_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Mach-O"):
            platform.macos_platform_tag("macosx_11_0_universal2", b"not macho")

    def test_unknown_cpu_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "CPU"):
            platform.macos_platform_tag("macosx_11_0_universal2", macho(0x01000012))

    def test_invalid_platform_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "platform"):
            platform.macos_platform_tag("linux_x86_64", macho(0x01000007))

    def test_filename_and_wheel_record_must_both_match_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = Path(directory) / "ofdrw_rs-0.1.0-py3-none-macosx_11_0_universal2.whl"
            with zipfile.ZipFile(wheel, "w") as archive:
                archive.writestr("ofdrw_rs/libofdrw_ffi.dylib", macho(0x0100000C))
                archive.writestr("ofdrw_rs-0.1.0.dist-info/WHEEL", "Tag: py3-none-macosx_11_0_universal2\n")
            expected, valid, _ = platform.macos_wheel_info(wheel)
            self.assertEqual(expected, "macosx_11_0_arm64")
            self.assertFalse(valid)


if __name__ == "__main__":
    unittest.main()
