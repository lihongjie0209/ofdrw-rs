# ofdrw Go bindings

This README targets 0.5.0. Check the [release workflow](https://github.com/lihongjie0209/ofdrw-rs/actions/workflows/release.yml) for distribution status; build matching bindings and native libraries from source while publication is pending. The previous [v0.4.0 release](https://github.com/lihongjie0209/ofdrw-rs/releases/tag/v0.4.0) remains available.

Go 1.22+ wrapper for the Rust OFD core. It loads the C ABI with purego and does
not require cgo. Use the module and native assets from the same release;
check GitHub Releases for version availability. Install the module:

```sh
go get github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@v0.5.0
```

Download native ZIPs from [GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases):

```sh
curl -fL -o native.zip https://github.com/lihongjie0209/ofdrw-rs/releases/download/v0.5.0/ofdrw-native-linux-x64-v0.5.0.zip
unzip native.zip -d native
```

Select the ZIP for your OS/CPU.
Set `OFDRW_LIB_PATH` to its absolute path: `libofdrw_ffi.so` on Linux,
`libofdrw_ffi.dylib` on macOS, or `ofdrw_ffi.dll` on Windows.

```go
package main

import (
    "log"
    "github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw"
)

func main() {
    document, err := ofdrw.New()
    if err != nil { log.Fatal(err) }
    defer document.Close()
    if err := document.AddTextPage("Hello, OFD", 210, 297); err != nil { log.Fatal(err) }
    if err := document.Save("hello.ofd"); err != nil { log.Fatal(err) }
}
```

`Open`, `FromBytes` and `FromPDF` read documents; `PageSVG`, `PagePNG` and
`ToPDF` export content. Page indexes start at zero and coordinates use millimeters.
The release workflow publishes the Go module tag `bindings/go/v0.5.0`.

## 混合流布局（0.5.0 起）

```go
style := ofdrw.FlowDocumentStyle{BlockGap: 2}
result, err := document.AddFlowBlocks(0, []ofdrw.FlowBlock{
    {Type: "paragraph", Spans: []ofdrw.FlowSpan{{Text: "First paragraph"}}},
    {Type: "spacer", Height: 4},
    {Type: "paragraph", Spans: []ofdrw.FlowSpan{{Text: "Second paragraph", Bold: true}}},
}, style)
if err != nil { log.Fatal(err) }
style.Start = result.Next
_, err = document.AddFlowBlocks(0,
    []ofdrw.FlowBlock{{Type: "paragraph", Spans: []ofdrw.FlowSpan{{Text: "Continued"}}}}, style)
if err != nil { log.Fatal(err) }
```

`FlowTable` 是流中的顶层跨页表格，`Padding: 0` 明确表示无内边距；
单元格中的 `NestedTable` 保留既有默认行为。图片使用 `Data` 字节，自动编码。
`result.Next` 可传给下一次布局的 `Start`，整次调用失败时回滚。
详见[混合流布局](../../docs/FEATURES.md#混合流布局)。

## 盒模型与浮动行（0.5.0 起）

```go
style := ofdrw.DefaultBoxLayoutStyle()
result, err := document.AddBoxLayout(0, []ofdrw.BoxLayoutItem{
    {Type: "box", LayoutBox: &ofdrw.LayoutBox{Size: [2]float64{40, 10},
        Padding: [4]float64{2, 2, 2, 2}, Float: "left", Clear: "none"}},
    {Type: "box", LayoutBox: &ofdrw.LayoutBox{Size: [2]float64{40, 10},
        Float: "right", Clear: "none"}},
}, style)
if err != nil { log.Fatal(err) }
placed := result.Placements[0]
_, err = document.AddTextObject(placed.PageIndex, "Hello", placed.ContentBox, 5, "")
if err != nil { log.Fatal(err) }
```

边距数组按上、右、下、左排列。使用 `DefaultBoxLayoutStyle()` 后可修改页面几何；
将 `result.Next` 赋给 `style.Start` 可续写。断行和换页分别使用
`BoxLayoutItem{Type: "line_break"}` 和 `BoxLayoutItem{Type: "page_break"}`。
自动测量文本盒与嵌套混合容器见下文的布局接口。
详见[盒模型说明](../../docs/FEATURES.md#盒模型与浮动分段)。

## 文本自动测量（0.5.0 起）

```go
spans := []ofdrw.FlowSpan{{Text: "First line\nSecond line"}}
textStyle := ofdrw.DefaultFlowTextStyle()
metrics, err := document.MeasureFlowedSpans(0, spans, 40, textStyle)
if err != nil { log.Fatal(err) }
_, err = document.AddBoxLayout(0, []ofdrw.BoxLayoutItem{{Type: "box",
    LayoutBox: &ofdrw.LayoutBox{Size: [2]float64{40, metrics.Height},
        Padding: [4]float64{2, 2, 2, 2}, Border: [4]float64{0.2, 0.2, 0.2, 0.2}, Integrity: true},
}}, ofdrw.DefaultBoxLayoutStyle())
if err != nil { log.Fatal(err) }
```

`FlowTextMetrics`、`FlowLineMetrics` 返回逻辑尺寸和逐行位置。测量不修改页面或资源，
页面几何和 `Start` 不参与计算；坐标和基线相对段落原点。
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

```go
image := ofdrw.FlowBlock{
    Type: "image", Data: imageBytes, Format: "PNG",
    IntrinsicSize: true, FitWithin: &[2]float64{40, 30},
}
_, err := doc.AddFlowBlocks(0, []ofdrw.FlowBlock{image}, ofdrw.FlowDocumentStyle{})
if err != nil {
    return err
}
```

### Automatic text boxes (since 0.4.0)

```go
typography := ofdrw.DefaultFlowTextStyle()
typography.FontSize, typography.LineHeight = 4, 5
result, err := doc.AddTextBoxes(0, []ofdrw.TextBox{{
    Layout: ofdrw.LayoutBox{
        Size: [2]float64{40, 0}, Padding: [4]float64{2, 2, 2, 2},
        Border: [4]float64{0.2, 0.2, 0.2, 0.2},
    },
    Spans: []ofdrw.FlowSpan{{Text: "Text wraps and grows this decorated box."}},
    Style: typography,
}}, ofdrw.DefaultBoxLayoutStyle())
if err != nil {
    return err
}
_ = result
```

Content height replaces `layout.size[1]` / `Layout.Size[1]` automatically.
Width is clamped before wrapping, and complete lines move to the next page.
Typography can enable shaped text or paragraph keep rules; page geometry and
continuation come from the shared box options. The result contains all
fragment rectangles and `next` (`Next` in Go), reusable as `start` (`Start`).
General nested child containers use the mixed container API below. This API is available
since 0.4.0.

### Nested mixed containers (since 0.4.0)

```go
textStyle := ofdrw.DefaultFlowTextStyle()
textStyle.FontSize, textStyle.LineHeight = 2, 3
container := ofdrw.FlowContainer{
    Layout: ofdrw.LayoutBox{Size: [2]float64{40, 0}, Padding: [4]float64{1, 1, 1, 1}},
    Gap: 1,
    Children: []ofdrw.FlowContainerChild{
        {Type: "paragraph", Spans: []ofdrw.FlowSpan{{Text: "Heading"}}, Style: &textStyle},
        {Type: "container", Container: &ofdrw.FlowContainer{
            Layout: ofdrw.LayoutBox{Size: [2]float64{30, 0}, Border: [4]float64{0.2, 0.2, 0.2, 0.2}},
            Children: []ofdrw.FlowContainerChild{
                {Type: "paragraph", Spans: []ofdrw.FlowSpan{{Text: "Nested text"}}, Style: &textStyle},
            },
        }},
    },
}
result, err := doc.AddFlowContainers(0, []ofdrw.FlowContainer{container}, ofdrw.DefaultBoxLayoutStyle())
if err != nil {
    return err
}
_ = result
```

Image children use `Data []byte`. Table children use `Table *FlowTable` so
zero padding remains explicit; `TableStyle *FlowContainerTableStyle` supplies
font size, line height and font ID. Supplied paragraph style pointers should
start from `DefaultFlowTextStyle`; a nil pointer selects native defaults.

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

```go
background := ofdrw.LayoutBox{
    Size: [2]float64{40, 10},
    BackgroundColor: &[3]uint8{240, 240, 240},
    Layer: "background",
}
_, err = document.AddBoxLayout(0, []ofdrw.BoxLayoutItem{
    {Type: "box", LayoutBox: &background},
}, ofdrw.DefaultBoxLayoutStyle())
if err != nil { log.Fatal(err) }
```

### Fixed page containers (since 0.4.0)

Available since 0.4.0; use bindings with their matching native library.

```go
if err := doc.AddBlankPage(80, 60); err != nil { return err }
placements, err := doc.AddFixedContainers(0, []ofdrw.FixedContainer{{
    Position: [2]float64{10, 20},
    Container: ofdrw.FlowContainer{
        Layout: ofdrw.LayoutBox{Size: [2]float64{40, 0}},
        Children: []ofdrw.FlowContainerChild{{
            Type: "paragraph", Spans: []ofdrw.FlowSpan{{Text: "Hello"}},
        }},
    },
}})
if err != nil { return err }
_ = placements // []ofdrw.BoxPlacement
```

Positions are absolute OFD millimeters on an existing global page. Zero height
measures content; positive heights control allocation and decoration while children
may overflow. No pages are added. Nested text/images/tables, offsets, layers and
placeholders follow flow-container rules. Page-area fillers are ignored. Invalid
later content rolls back the whole call. Images accept `Data []byte`.

### Inline placeholders (since 0.4.0)

`NewPlaceholderSpan(width, height)` creates a `FlowSpan` with empty text and an
explicit `PlaceholderWidth *float64`, preserving width zero in JSON. Geometry
participates in wrapping and line height without painting objects or allocating
font resources.

```go
metrics, err := doc.MeasureFlowedSpans(
    0,
    []ofdrw.FlowSpan{{Text: "Label:"}, ofdrw.NewPlaceholderSpan(8, 4), {Text: "value"}},
    50,
    ofdrw.DefaultFlowTextStyle(),
)
if err != nil { return err }
_ = metrics // line text is "Label:value"
```

The same spans work in styled flow, text boxes, flow blocks and fixed/flow
containers. A leading placeholder absorbs a configured first-line indent once.
For character-count reservations use `count * (fontSize + letterSpacing)` as
width. Available since 0.4.0; use matching bindings and native library.

### Protected content ranges (unreleased source API)

Use matching source bindings and native library; this addition is outside 0.5.0.
`BoxContentRangesFromText` builds indivisible intervals from measured lines:

```go
policy := ofdrw.BoxContentRangesFromText(0, metrics)
result, err := doc.AddBoxLayoutWithContentRanges(
    0,
    []ofdrw.BoxLayoutItem{{Type: "box", LayoutBox: &ofdrw.LayoutBox{Size: [2]float64{8, metrics.Height}}}},
    []ofdrw.BoxContentRanges{policy},
    ofdrw.BoxLayoutStyle{PageSize: [2]float64{20, 14}, Margins: [4]float64{2, 2, 2, 2}},
)
if err != nil { return err }
_ = result
```

Intervals use original content coordinates and must be ordered, positive,
disjoint and inside the box. Nil/empty policies retain geometric cuts. The
result uses ordinary box-layout geometry and continuation; callers draw child
content separately. Invalid policies roll back the entire call.

## Device hairlines (0.5.0 candidate)

For path writers, use `ofdrw.PathStyle{Stroke: true, Hairline: true}` to
preserve a zero line width. This applies to page paths, composite paths,
single/multiple/mixed path annotations, and appended annotation paths.
Without `Hairline`, the existing zero-value default remains 0.4 mm. A
positive `LineWidth` is always honored, even when `Hairline` is true.
`PathStrokeStyle{LineWidth: 0}` also updates existing paths to device hairlines.
PNG uses one device pixel at any DPI; PDF exports a native zero-width stroke.
Use a matching native library built from the same unreleased source.

## Vector canvas batches (0.5.0 candidate)

Use a matching native library built from the same unreleased source.

```go
package main

import (
    "log"
    "github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw"
)

func main() {
    document, err := ofdrw.New()
    if err != nil { log.Fatal(err) }
    defer document.Close()
    if err := document.AddBlankPage(40, 30); err != nil { log.Fatal(err) }
    ids, err := document.DrawCanvas(0, [4]float64{0, 0, 40, 30}, []ofdrw.CanvasCommand{
        {Type: "set_stroke_color", Color: &[3]uint8{0, 80, 200}},
        {Type: "rect", Rect: &[4]float64{2, 3, 12, 8}},
        {Type: "stroke"},
    })
    if err != nil { log.Fatal(err) }
    log.Println(ids)
    if err := document.Save("canvas.ofd"); err != nil { log.Fatal(err) }
}
```

Commands use typed pointer fields to retain explicit zeroes; for example,
`zero := 0.0` and `CanvasCommand{Type: "set_line_width", Width: &zero}`.
Populate only fields belonging to the command type. The batch commits
atomically and returns string object IDs. Nil commands form an empty batch,
and nil Alpha clears global opacity. Coordinates are millimeters and angles
are degrees. See the [command reference](../../docs/FEATURES.md#有状态向量画布)
for vector operations and remaining text limitations.

### Canvas images (0.5.0 candidate)

`CanvasCommand.Data` accepts encoded image bytes and serializes them as base64.
Use `Format`, `Rect` and, for `draw_image_region`, `Source`. Source coordinates
are millimeters converted to integer pixels by truncation; portions outside the
source are transparent, matching Java. `draw_image_at` uses `Point` and natural
pixel dimensions divided by `PixelsPerMM` (nil defaults to 3.78). Negative
target dimensions mirror images; zero dimensions collapse them.

```go
package main

import (
    "log"
    "os"
    "github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw"
)

func main() {
    image, err := os.ReadFile("picture.png")
    if err != nil { log.Fatal(err) }
    document, err := ofdrw.New()
    if err != nil { log.Fatal(err) }
    defer document.Close()
    if err := document.AddBlankPage(40, 30); err != nil { log.Fatal(err) }
    ppm := 2.0
    _, err = document.DrawCanvas(0, [4]float64{0, 0, 40, 30}, []ofdrw.CanvasCommand{
        {Type: "draw_image", Data: image, Format: "png", Rect: &[4]float64{2, 2, 12, 8}},
        {Type: "draw_image_region", Data: image, Format: "png",
            Source: &[4]float64{0, 0, 1, 1}, Rect: &[4]float64{20, 2, 8, 8}, PixelsPerMM: &ppm},
    })
    if err != nil { log.Fatal(err) }
    if err := document.Save("canvas-images.ofd"); err != nil { log.Fatal(err) }
}
```

### Canvas text (0.5.0 candidate)

CanvasFont configures size, tracking, weight, italic, alignment and quarter-turn
reading/character directions. Nil numeric fields use native defaults: SimSun,
1 mm, weight 400, zero tracking/directions and start alignment. Explicit zero
size is rejected. CanvasCommand.Text is a string pointer so empty text remains
an explicit no-op. MeasureCanvasText returns logical metrics without changing
the document; metrics exclude kerning and shaping. Embed a font for reproducibility.

```go
package main

import (
    "log"
    "github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw"
)

func main() {
    document, err := ofdrw.New()
    if err != nil { log.Fatal(err) }
    defer document.Close()
    if err := document.AddBlankPage(40, 30); err != nil { log.Fatal(err) }
    size := 4.0
    text := "Canvas"
    font := ofdrw.CanvasFont{Size: &size, Align: "center"}
    metrics, err := document.MeasureCanvasText(0, text, font)
    if err != nil { log.Fatal(err) }
    _, err = document.DrawCanvas(0, [4]float64{0, 0, 40, 30}, []ofdrw.CanvasCommand{
        {Type: "set_font", Font: &font},
        {Type: "fill_text", Text: &text, Point: &[2]float64{20, 10}},
    })
    if err != nil { log.Fatal(err) }
    if err := document.Save("canvas-text.ofd"); err != nil { log.Fatal(err) }
    log.Println(metrics.Width)
}
```
