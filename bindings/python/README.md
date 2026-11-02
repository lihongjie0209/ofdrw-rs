# ofdrw-rs for Python

Python 3.10+ bindings to the Rust OFD core through CFFI.
Install the matching version from [PyPI](https://pypi.org/project/ofdrw-rs/):

```sh
python -m pip install ofdrw-rs==0.4.0
```

Check the registry and release pages for version availability.
The wheel includes the native library; a Rust toolchain is not needed.
Wheels for all six supported platforms are also available from
[GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases/tag/v0.4.0).

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

## 混合流布局（0.4.0 起）

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

## 盒模型与浮动行（0.4.0 起）

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
自动测量文本盒与嵌套混合容器见下文的布局接口。详见[盒模型说明](../../docs/FEATURES.md#盒模型与浮动分段)。

## 文本自动测量（0.4.0 起）

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
按行拆分的文本盒与一般嵌套容器见下文的布局接口。

### Intrinsic image sizes (since 0.4.0)

Flow images, table-cell images and image blocks accept `intrinsic_size: true`
(`IntrinsicSize: true` in Go). Omit width/height or leave them zero: pixel
sizes are converted at 5 pixels/mm, matching Java OFDRW, regardless of embedded
DPI metadata. `fit_within: [width, height]` (`FitWithin: &[2]float64{width, height}`
in Go) optionally shrinks the image to millimeter bounds while preserving
aspect ratio. It never enlarges smaller images. Nonzero explicit dimensions
cannot be combined with intrinsic sizing. Without intrinsic sizing, both
explicit dimensions remain required. These options share the same native
validation in C and all bindings, available since 0.4.0.

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

### Automatic text boxes (since 0.4.0)

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
since 0.4.0.

### Nested mixed containers (since 0.4.0)

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
levels and 10,000 container/child entries. This API is available since 0.4.0.


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


### Dashed box borders (since 0.4.0)

`border_dash` (`BorderDash` in Go) uses Java `Div.setBorderDash` arguments:
`[3]` means dash/gap `[3, 3]`; `[3, 2]` means dash/gap `[3, 2]`;
`[1, 3, 2]` sets offset 1 followed by dash/gap `[3, 2]`. Longer even
sequences contain dash/gap pairs, and longer odd sequences start with the
offset. Null or an empty sequence selects a solid border. Lengths must be
positive and finite, with at most 256 lengths; offsets below 0.000001 are
omitted, matching Java. Invalid patterns fail atomically, including placeholders.

This field works in measured boxes, automatic text boxes and nested mixed
containers. Equal-width borders use one closed stroked path; different edge
widths use separate paths with horizontal edges owning the corners. Page cuts
preserve the existing top/bottom border fragmentation rules. Dash phase starts
at each emitted path; allocation, text wrapping and continuation are unchanged.

```json
{"size": [40, 10], "border": [0.5, 0.5, 0.5, 0.5], "border_dash": [1, 3, 2]}
```


### Box painting layers (since 0.4.0)

Set `layout.layer` (`Layout.Layer` in Go) to `background`, `body` or
`foreground`. Measured box items accept the same `layer` field directly.
Decoration, ordinary/shaped text, underline paths, images and table children
use the selected layer across page fragments. A nested container without a
layer inherits its parent; an explicit layer overrides it for that subtree.
Roots default to Body. Painting follows OFD Background → Body → Foreground
order independently of the order in which boxes were submitted.

Existing direct page layers of the chosen type are reused; another layer is
created when needed. Existing empty Body layers on blank pages are retained.
Layer selection does not change measured dimensions, pagination or returned
placements. Later ordinary drawing calls continue to use Body. Invalid layer
values fail atomically. The same rules apply to static and advanced container
pagination and to existing pages with nonzero canvas origins.

```python
document.add_box_layout(0, [{"type": "box", "size": [40, 10],
    "background_color": [240, 240, 240], "layer": "background"}])
```

### Fixed page containers (since 0.4.0)

Available since 0.4.0; use bindings with their matching native library.
`add_fixed_containers` draws on an existing global page and returns a placement list:

```python
from pathlib import Path
from ofdrw_rs import Document

with Document.new() as doc:
    doc.add_blank_page(80, 60)
    placements = doc.add_fixed_containers(0, [{
        "position": [10, 20],
        "container": {
            "layout": {"size": [40, 0]},
            "children": [{"type": "paragraph", "spans": [{"text": "Hello"}]}],
        },
    }])
    Path("fixed.ofd").write_bytes(doc.to_bytes())
```

Positions are absolute OFD millimeters. Zero height measures the content; a positive
height controls allocation and decorations while children may overflow. The call
never adds pages. It supports the same nested text/image/table content, offsets,
layers and placeholders as flow containers. Page-area fillers are ignored. Invalid
later content rolls back the entire call. Images accept bytes in `data`.

### Inline placeholders (since 0.4.0)

A span with empty `text`, `placeholder_width` in millimeters and `font_size` as
height reserves an indivisible inline rectangle without painting objects or
font resources. Width zero is allowed. For example:

```python
with Document.new() as doc:
    metrics = doc.measure_flowed_spans(0, [
        {"text": "Label:"},
        {"text": "", "placeholder_width": 8, "font_size": 4},
        {"text": "value"},
    ], 50)
    assert metrics["lines"][0]["text"] == "Label:value"
```

The same spans work in styled flow, text boxes, flow blocks and fixed/flow
containers. Reserved geometry participates in wrapping and maximum line height;
measured/extracted text omits placeholders. Invalid later input rolls back the
call. A leading placeholder absorbs a configured first-line indent once.
For character-count reservations use `count * (font_size + letter_spacing)` as
the width. Available since 0.4.0; use matching bindings and native library.

### Protected content ranges (unreleased source API)

Use matching source bindings and native library; this addition is outside 0.4.0.
Protect complete measured lines while placing/decorating a box:

```python
metrics = doc.measure_flowed_spans(0, [{"text": "A\nB\nC\nD"}], 8,
                                   font_size=2, line_height=3)
policies = [{"source_index": 0, "ranges": [[line["y"], line["y"] + line["height"]]
             for line in metrics["lines"]]}]
result = doc.add_box_layout_with_content_ranges(0,
    [{"type": "box", "size": [8, metrics["height"]]}], policies,
    page_size=[20, 14], margins=[2] * 4)
```

Intervals use original content coordinates and must be ordered, positive,
disjoint and inside the box. A cut inside an interval moves it intact to the
next page. The result uses ordinary box-layout fragment fields and continuation;
callers draw child content separately. Empty policies retain geometric cuts.
Invalid policies roll back the entire call.

## Device hairlines (unreleased; after 0.4.0)

Pass `line_width=0` to `add_path_object`, `add_composite_path_object`, or
`add_path_annotation` for a one-device-pixel path stroke. Existing paths
accept `set_page_path_stroke_style(0, object_id, {"line_width": 0})`.
PNG retains device width across DPI and transforms; PDF preserves a native
zero-width stroke. Use the matching unreleased native library.

## Vector canvas batches (unreleased; after 0.4.0)

Use a matching native library built from the same unreleased source.

```python
from ofdrw_rs import Document

with Document.new() as document:
    document.add_blank_page(40, 30)
    ids = document.draw_canvas(0, (0, 0, 40, 30), [
        {"type": "set_stroke_color", "color": [0, 80, 200]},
        {"type": "rect", "rect": [2, 3, 12, 8]},
        {"type": "stroke"},
    ])
    document.save("canvas.ofd")
```

The batch commits atomically and returns painted object IDs as strings.
Coordinates are millimeters; angles are degrees. Use `save`/`restore` for
paths and drawing state, and `begin_path` to clear the path. Unknown commands
and fields are rejected. Set global alpha to `None` to clear it.
See the [command reference](../../docs/FEATURES.md#有状态向量画布) for all vector operations and remaining text limitations.

### Canvas images (unreleased)

Image commands accept bytes, bytearray or memoryview in `data`. Alternatively,
use `data_base64`; providing both is an error. `draw_image_at` uses natural
pixel dimensions divided by `pixels_per_mm` (default 3.78). `draw_image_region`
converts its source rectangle from millimeters to integer pixels by truncation,
matching Java; portions outside the source are transparent. Negative target
widths/heights mirror the image, and zero dimensions collapse it.

```python
from pathlib import Path
from ofdrw_rs import Document

image = Path("picture.png").read_bytes()
with Document.new() as document:
    document.add_blank_page(40, 30)
    document.draw_canvas(0, (0, 0, 40, 30), [
        {"type": "draw_image", "data": image, "format": "png", "rect": [2, 2, 12, 8]},
        {"type": "draw_image_region", "data": image, "format": "png",
         "source": [0, 0, 1, 1], "rect": [20, 2, 8, 8], "pixels_per_mm": 2},
    ])
    document.save("canvas-images.ofd")
```

### Canvas text (unreleased)

`set_font` configures size, tracking, weight, italic, alignment and quarter-turn
reading/character directions. Omitted fields use SimSun, 1 mm, weight 400 and
start alignment. `measure_canvas_text` returns logical width, first glyph offset
and signed advances without modifying the document. These metrics exclude
kerning and shaping. Embedded fonts provide reproducible measurements.

```python
from ofdrw_rs import Document

font = {"size": 4, "align": "center"}
with Document.new() as document:
    document.add_blank_page(40, 30)
    metrics = document.measure_canvas_text(0, "Canvas", font)
    document.draw_canvas(0, (0, 0, 40, 30), [
        {"type": "set_font", "font": font},
        {"type": "fill_text", "text": "Canvas", "point": [20, 10]},
    ])
    document.save("canvas-text.ofd")
    print(metrics["width"])
```
