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
