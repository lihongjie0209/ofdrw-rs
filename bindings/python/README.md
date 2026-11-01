# ofdrw-rs for Python

Python 3.10+ bindings to the Rust OFD core through CFFI.
Install the matching version from [PyPI](https://pypi.org/project/ofdrw-rs/):

```sh
python -m pip install ofdrw-rs==0.3.0
```

Check the registry and release pages for version availability.
The wheel includes the native library; a Rust toolchain is not needed.
Wheels for all six supported platforms are also available from
[GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases/tag/v0.3.0).

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

## 混合流布局（源码开发功能，尚未发布）

```python
with Document.new() as document:
    result = document.add_flow_blocks(0, [
        {"type": "paragraph", "text": "第一段"},
        {"type": "spacer", "height": 4},
        {"type": "paragraph", "spans": [{"text": "第二段", "bold": True}]},
    ], block_gap=2)
    document.add_flow_blocks(0, [{"type": "paragraph", "text": "继续"}],
                             start=result["next"])
    document.save("mixed.ofd")
```

支持图片、跨页表格和 `page_break`；图片可传 `data` 字节或 `data_base64`。
`next` 包含全局 `page_index` 和相对页面顶端的 `y`。整次调用失败时回滚。
详见[混合流布局](../../docs/FEATURES.md#混合流布局)。
