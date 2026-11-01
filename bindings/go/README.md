# ofdrw Go bindings

Go 1.22+ wrapper for the Rust OFD core. It loads the C ABI with purego and does
not require cgo. Use the module and native assets from the same release;
check GitHub Releases for version availability. Install the module:

```sh
go get github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@v0.3.0
```

Download native ZIPs from [GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases):

```sh
curl -fL -o native.zip https://github.com/lihongjie0209/ofdrw-rs/releases/download/v0.3.0/ofdrw-native-linux-x64-v0.3.0.zip
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
The release workflow publishes the Go module tag `bindings/go/v0.3.0`.

## 混合流布局（源码开发功能，尚未发布）

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

## 盒模型与浮动行（源码开发功能，尚未发布）

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
自动测量文本盒与嵌套混合容器见下文的源码开发接口。
详见[盒模型说明](../../docs/FEATURES.md#盒模型与浮动分段)。

## 文本自动测量（源码开发功能，尚未发布）

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

### Automatic text boxes (unreleased source API)

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
from the development source and is not included in published version 0.3.0.

### Nested mixed containers (unreleased source API)

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


### Dashed box borders (unreleased source API)

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
