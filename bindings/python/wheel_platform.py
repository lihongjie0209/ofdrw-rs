"""Check macOS wheel tags against the bundled Rust library's Mach-O header."""

from email.parser import Parser
from pathlib import Path
import re
import zipfile


def macos_platform_tag(tag, binary):
    if len(binary) < 8 or binary[:4] != b"\xcf\xfa\xed\xfe":
        raise ValueError("Expected a thin 64-bit Mach-O native library")
    cpu = int.from_bytes(binary[4:8], "little")
    architectures = {0x01000007: "x86_64", 0x0100000C: "arm64"}
    if cpu not in architectures:
        raise ValueError(f"Unsupported Mach-O CPU type: {cpu:#x}")
    match = re.fullmatch(r"macosx_(\d+)_(\d+)_(?:universal2|x86_64|arm64)", tag)
    if match is None:
        raise ValueError(f"Invalid macOS platform tag: {tag}")
    major, minor = map(int, match.groups())
    architecture = architectures[cpu]
    if architecture == "arm64" and major < 11:
        major, minor = 11, 0
    return f"macosx_{major}_{minor}_{architecture}"


def macos_wheel_info(path):
    path = Path(path)
    filename_tag = path.stem.rsplit("-", 1)[1]
    with zipfile.ZipFile(path) as archive:
        libraries = [name for name in archive.namelist() if Path(name).name == "libofdrw_ffi.dylib"]
        descriptions = [name for name in archive.namelist() if name.endswith(".dist-info/WHEEL")]
        if len(libraries) != 1 or len(descriptions) != 1:
            raise ValueError("Wheel must contain one native library and one WHEEL record")
        binary = archive.read(libraries[0])
        description = Parser().parsestr(archive.read(descriptions[0]).decode())
        tags = description.get_all("Tag", [])
    expected = macos_platform_tag(filename_tag, binary)
    valid = filename_tag == expected and tags == [f"py3-none-{expected}"]
    return expected, valid, binary
