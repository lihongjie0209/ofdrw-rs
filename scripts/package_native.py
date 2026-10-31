"""Assemble one platform's native library, C header and license as a ZIP."""

from pathlib import Path
import sys
import tomllib
import zipfile

from release_zip import write_entry

root = Path(__file__).resolve().parents[1]
platform, library, output = sys.argv[1:]
libraries = {
    "linux-x64": "libofdrw_ffi.so", "linux-arm64": "libofdrw_ffi.so",
    "darwin-x64": "libofdrw_ffi.dylib", "darwin-arm64": "libofdrw_ffi.dylib",
    "win32-x64": "ofdrw_ffi.dll", "win32-arm64": "ofdrw_ffi.dll",
}
library = Path(library).resolve()
if library.name != libraries.get(platform) or not library.is_file():
    raise SystemExit(f"Invalid native library for {platform}: {library}")
version = tomllib.loads((root / "crates/ofdrw/Cargo.toml").read_text())["package"]["version"]
name = f"ofdrw-native-{platform}-v{version}"
output = Path(output)
output.mkdir(parents=True, exist_ok=True)
files = [library, root / "LICENSE", root / "NOTICE"]
static = library.parent / ("ofdrw_ffi.lib" if platform.startswith("win32") else "libofdrw_ffi.a")
if not static.is_file():
    raise SystemExit(f"Static library is missing: {static}")
files.append(static)
with zipfile.ZipFile(output / f"{name}.zip", "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for file in files:
        write_entry(archive, f"{name}/{file.name}", file)
    write_entry(archive, f"{name}/include/ofdrw.h", root / "include/ofdrw.h")
    write_entry(archive, f"{name}/README.txt", (
        f"ofdrw-rs {version} for {platform}\n"
        f"Set OFDRW_LIB_PATH to the absolute path of {library.name}.\n"
        "Python and Node.js release packages already bundle native libraries.\n"
        "Go: go get github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@v" + version + "\n"
        "Documentation: https://github.com/lihongjie0209/ofdrw-rs\n"
    ).encode("utf-8"))
print(output / f"{name}.zip")
