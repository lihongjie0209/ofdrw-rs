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

## 盒模型与浮动行（源码开发功能，尚未发布）

```python
with Document.new() as document:
    result = document.add_box_layout(0, [
        {"type": "box", "size": [40, 10], "padding": [2] * 4,
         "border": [0.2] * 4, "float": "left", "clear": "none"},
        {"type": "box", "size": [40, 10], "float": "right", "clear": "none"},
    ])
    placed = result["placements"][0]
    document.add_text_object(placed["page_index"], "Hello", placed["content_box"])
    document.save("boxes.ofd")
```

`size` 是内容宽高；`margin`、`border`、`padding` 均按上、右、下、左排列。
`result["next"]` 可作为下一次调用的 `start`。接口布局已测量尺寸的盒子并绘制装饰；
自动测量文本盒与嵌套混合容器见下文的源码开发接口。详见[盒模型说明](../../docs/FEATURES.md#盒模型与浮动分段)。

## 文本自动测量（源码开发功能，尚未发布）

```python
with Document.new() as document:
    spans = [{"text": "First line\nSecond line"}]
    metrics = document.measure_flowed_spans(0, spans, 40, font_size=5, line_height=6)
    print(metrics["width"], metrics["height"], metrics["lines"])
    boxes = document.add_box_layout(0, [{"type": "box", "size": [40, metrics["height"]],
        "padding": [2] * 4, "border": [0.2] * 4, "integrity": True}])
```

测量不会增加页面或资源。`width` 为最大行前进宽度，`height` 包含内部空行和段间距；
逐行 `x`、`y`、`baseline` 相对段落左上角。页面几何和 `start` 不参与测量。
将测得高度用于盒模型，再按内容矩形调用文本流式绘制；尺寸属于逻辑行框，
按行拆分的文本盒与一般嵌套容器见下文的源码开发接口。

### Intrinsic image sizes (unreleased source API)

Flow images, table-cell images and image blocks accept `intrinsic_size: true`
(`IntrinsicSize: true` in Go). Omit width/height or leave them zero: pixel
sizes are converted at 5 pixels/mm, matching Java OFDRW, regardless of embedded
DPI metadata. `fit_within: [width, height]` (`FitWithin: &[2]float64{width, height}`
in Go) optionally shrinks the image to millimeter bounds while preserving
aspect ratio. It never enlarges smaller images. Nonzero explicit dimensions
cannot be combined with intrinsic sizing. Without intrinsic sizing, both
explicit dimensions remain required. These options share the same native
validation in C and all bindings and are not included in version 0.3.0.

```python
from pathlib import Path
from ofdrw_rs import Document

with Document.new() as doc:
    doc.add_flow_blocks(0, [{
        "type": "image", "data": Path("picture.png").read_bytes(),
        "format": "PNG", "intrinsic_size": True, "fit_within": [40, 30],
    }])
    Path("picture.ofd").write_bytes(doc.to_bytes())
```

### Automatic text boxes (unreleased source API)

```python
from pathlib import Path
from ofdrw_rs import Document

with Document.new() as doc:
    result = doc.add_text_boxes(0, [{
        "layout": {"size": [40, 0], "padding": [2, 2, 2, 2],
                   "border": [0.2, 0.2, 0.2, 0.2]},
        "spans": [{"text": "Text wraps and grows this decorated box."}],
        "style": {"font_size": 4, "line_height": 5},
    }])
    Path("text-box.ofd").write_bytes(doc.to_bytes())
```

Content height replaces `layout.size[1]` / `Layout.Size[1]` automatically.
Width is clamped before wrapping, and complete lines move to the next page.
Typography can enable shaped text or paragraph keep rules; page geometry and
continuation come from the shared box options. The result contains all
fragment rectangles and `next` (`Next` in Go), reusable as `start` (`Start`).
General nested child containers use the mixed container API below. This API is available
from the development source and is not included in published version 0.3.0.

### Nested mixed containers (unreleased source API)

```python
from pathlib import Path
from ofdrw_rs import Document

with Document.new() as doc:
    result = doc.add_flow_containers(0, [{
        "layout": {"size": [40, 0], "padding": [1, 1, 1, 1]},
        "gap": 1,
        "children": [
            {"type": "paragraph", "spans": [{"text": "Heading"}],
             "style": {"font_size": 4, "line_height": 5}},
            {"type": "container", "container": {
                "layout": {"size": [30, 0], "border": [0.2, 0.2, 0.2, 0.2]},
                "children": [{"type": "paragraph", "spans": [{"text": "Nested text"}],
                              "style": {"font_size": 2, "line_height": 3}}],
            }},
        ],
    }])
    Path("containers.ofd").write_bytes(doc.to_bytes())
```

Image children accept `data` bytes or `data_base64`; font IDs in nested styles
can use the integer returned by `add_embedded_font`.

Each container has `layout`, optional `gap`, and `children`. Supported child
types are paragraph, image, container, spacer, table, line_break and page_area_fill. A table child has a
`table` grid and optional `table_style` (`font_size`, `line_height`, `font_id`,
`repeat_header`). Images accept explicit dimensions or intrinsic sizing and
fit bounds. Spacer height defaults to zero. Returned root fragments and
`next` / `Next` support continuation with shared `start` / `Start` options.

Containers support floating rows at every nesting level. Adjacent nested
boxes share rows according to float, clear and width rules, with independent
page continuation per column. Paragraphs, images, tables and spacers occupy
full rows; `gap` separates rows. Repeated table headers stay with complete
body-row groups, and header/body pairs that cannot fit a full page fail.
Fragment heights include repeated headers and the space needed to align
continuing floating columns.
The whole call rolls back on invalid children. Limits are eight container
levels and 10,000 container/child entries. This source API is not included in
published version 0.3.0.


`line_break` ends the current nonempty floating row; leading, repeated and
trailing empty breaks are ignored. `page_area_fill` consumes the current page's
remaining content area. If that area is already full, it starts and fills the
next page, so consecutive fills preserve explicit blank pages. A trailing fill
does not append another page. Fills reset row gaps and affect only their own
floating column. Both controls accept only `type`; a complete (`integrity`)
container that would span pages fails atomically.

```json
{"children": [{"type": "line_break"}, {"type": "page_area_fill"}]}
```
