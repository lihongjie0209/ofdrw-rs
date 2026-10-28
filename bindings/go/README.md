# ofdrw Go bindings

Go 1.22+ wrapper for the Rust OFD core. It loads the C ABI with purego and does
not require cgo. Install the module:

```sh
go get github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@v0.1.0
```

Pull native ZIPs from GitHub Packages with [ORAS](https://oras.land/docs/installation/):

```sh
oras pull ghcr.io/lihongjie0209/ofdrw-rs-native:0.1.0 -o packages/native
unzip packages/native/ofdrw-native-linux-x64-v0.1.0.zip -d native
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
The Go module version tag is `bindings/go/v0.1.0` in the repository.
