"""Run the README Rust and Go examples against the published registries."""

import io
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from urllib.request import urlopen
import zipfile
import os

root = Path(__file__).resolve().parents[1]
tag = sys.argv[1]
readme = (root / "README.md").read_text()
with tempfile.TemporaryDirectory(prefix="ofdrw-registry-consumers-") as temporary:
    work = Path(temporary)
    rust = work / "rust"
    (rust / "src").mkdir(parents=True)
    (rust / "src/main.rs").write_text(re.search(r"```rust\n(.*?)\n```", readme, re.S).group(1))
    (rust / "Cargo.toml").write_text(
        '[package]\nname="ofdrw-registry-consumer"\nversion="0.1.0"\nedition="2021"\n'
        f'\n[dependencies]\nofdrw="={tag[1:]}"\n'
    )
    subprocess.run(["cargo", "run", "--manifest-path", str(rust / "Cargo.toml")], cwd=rust, check=True)
    go = work / "go"
    go.mkdir()
    (go / "main.go").write_text(re.search(r"```go\n(.*?)\n```", readme, re.S).group(1))
    asset = f"ofdrw-native-linux-x64-{tag}"
    with urlopen(f"https://github.com/lihongjie0209/ofdrw-rs/releases/download/{tag}/{asset}.zip", timeout=60) as response:
        archive = response.read()
    library = work / "libofdrw_ffi.so"
    with zipfile.ZipFile(io.BytesIO(archive)) as package:
        library.write_bytes(package.read(f"{asset}/libofdrw_ffi.so"))
    environment = dict(os.environ, GOPROXY="direct", OFDRW_LIB_PATH=str(library), CGO_ENABLED="0")
    subprocess.run(["go", "mod", "init", "example.com/ofdrw-registry-consumer"], cwd=go, check=True)
    subprocess.run(["go", "get", f"github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@{tag}"], cwd=go, env=environment, check=True)
    subprocess.run(["go", "run", "."], cwd=go, env=environment, check=True)
    if not (rust / "hello.ofd").is_file() or not (go / "hello.ofd").is_file():
        raise SystemExit("Registry consumer did not produce its OFD document")
    print("Verified crates.io Rust and GitHub Go module installation")
