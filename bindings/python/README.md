# ofdrw-rs for Python

Python 3.10+ bindings to the Rust OFD core through CFFI.
Install v0.1.0 from [PyPI](https://pypi.org/project/ofdrw-rs/):

```sh
python -m pip install ofdrw-rs==0.1.0
```

The wheel includes the native library; a Rust toolchain is not needed.
Wheels for all six supported platforms are also available from
[GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases/tag/v0.1.0).

```python
from ofdrw_rs import Document

with Document.new() as document:
    document.add_text_page("你好，OFD", 210, 297)
    document.save("hello.ofd")
    print(document.page_text(0))
```

Use `Document.open(path)`, `Document.from_bytes(data)` or
`Document.from_pdf_bytes(data)` to read existing content. Use `page_svg(0)`,
`page_png(0, 96)` or `to_pdf()` to export. Close documents or use a context manager.
Coordinates are millimeters and page indexes start at zero.

Set `OFDRW_LIB_PATH` to an absolute shared-library path for a custom build.
Supported behavior and limitations are documented in the
[repository README](https://github.com/lihongjie0209/ofdrw-rs#python).
