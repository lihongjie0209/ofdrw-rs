# ofdrw-rs

[![CI](https://github.com/lihongjie0209/ofdrw-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lihongjie0209/ofdrw-rs/actions/workflows/ci.yml)
[![Releases](https://img.shields.io/github/v/release/lihongjie0209/ofdrw-rs?include_prereleases)](https://github.com/lihongjie0209/ofdrw-rs/releases)

使用 Rust 实现的 OFD 文档库，提供 **Rust、Python、Go、Node.js 和 C ABI**。
以 [Java OFDRW](https://github.com/ofdrw/ofdrw) 为兼容性参考，优先实现包读写、页面对象、资源、布局、渲染和文档转换。

当前版本是 **0.1.0 预发布版**，已经覆盖大量核心功能，但尚未完整替代 Java OFDRW。
完整布局规则、PDF 转换、证书信任验证和 OFD-A 转换仍有未实现部分。
支持范围及具体限制见 [功能和 API 说明](docs/FEATURES.md)。
`main` 分支包含后续开发功能，与已发布版本的差异见 [未发布变更](docs/RELEASE_NOTES.md)。

## 安装渠道

| 语言 | 原生仓库 | v0.1.0 状态 | 当前安装方式 |
| --- | --- | --- | --- |
| Python | [PyPI](https://pypi.org/project/ofdrw-rs/) | 已发布 | `pip install ofdrw-rs==0.1.0` |
| Node.js / TypeScript | [npm](https://www.npmjs.com/package/ofdrw-rs) | 已发布 | `npm install ofdrw-rs@0.1.0` |
| Rust | [crates.io](https://crates.io/crates/ofdrw) | 已发布 | `ofdrw = "0.1.0"` |
| Go | GitHub Go module | 已发布 | `go get github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@v0.1.0` |

各语言使用原生包管理工具，无需 ORAS 或 GitHub Packages 登录。
Go / C 所需动态库、静态库和 C 头文件保留在 [GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases)。
Release 还提供各语言源码包、Python wheels、npm tarball 和 `SHA256SUMS`。
支持 Linux、macOS、Windows 的 x64 / ARM64 六个平台。
Linux wheels 通过 auditwheel 检查并标记为 manylinux；预编译 native 库在 Ubuntu 24.04 构建。
较旧的 glibc 系统可从源码编译。macOS 产物在 macOS 15 构建。

## Python

需要 Python 3.10+，直接从 PyPI 安装：

```sh
python -m pip install ofdrw-rs==0.1.0
```

wheel 已带 Rust 共享库，不需要安装 Rust。它会自动安装 CFFI 依赖。
也可从 [v0.1.0 Release](https://github.com/lihongjie0209/ofdrw-rs/releases/tag/v0.1.0)
下载匹配系统和架构的 wheel 后安装。

```python
from ofdrw_rs import Document

with Document.new() as document:
    document.add_text_page("你好，OFD", 210, 297)
    document.save("hello.ofd")

with Document.open("hello.ofd") as document:
    print(document.page_text(0))
    with open("page.svg", "w", encoding="utf-8") as output:
        output.write(document.page_svg(0))
    with open("output.pdf", "wb") as output:
        output.write(document.to_pdf())
```

更多内容见 [Python 说明](bindings/python/README.md)。

## Node.js / TypeScript

需要 Node.js 20+。直接从 npm 安装：

```sh
npm install ofdrw-rs@0.1.0
```

```js
const { Document } = require('ofdrw-rs');

const document = Document.create();
try {
  document.addTextPage('你好，OFD', 210, 297);
  document.save('hello.ofd');
  console.log(document.pageText(0));
} finally {
  document.close();
}
```

TypeScript 可使用 `import { Document } from 'ofdrw-rs'`，类型声明随包提供。
更多内容见 [Node.js 说明](bindings/node/README.md)。

## Go

Go 包通过 purego 动态加载 C ABI，**不需要 cgo**。需要 Go 1.22+。
先从 GitHub Releases 下载对应平台的 native ZIP 并解压，设置动态库的绝对路径：

```sh
curl -fL -o native.zip https://github.com/lihongjie0209/ofdrw-rs/releases/download/v0.1.0/ofdrw-native-linux-x64-v0.1.0.zip
unzip native.zip -d native
export OFDRW_LIB_PATH="$PWD/native/ofdrw-native-linux-x64-v0.1.0/libofdrw_ffi.so"
go get github.com/lihongjie0209/ofdrw-rs/bindings/go/ofdrw@v0.1.0
```

macOS 使用 `libofdrw_ffi.dylib`，Windows 使用 `ofdrw_ffi.dll`。

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
    if err := document.AddTextPage("你好，OFD", 210, 297); err != nil { log.Fatal(err) }
    if err := document.Save("hello.ofd"); err != nil { log.Fatal(err) }
}
```

Release 会同时创建 Go 子模块标签 `bindings/go/v0.1.0`。
更多内容见 [Go 说明](bindings/go/README.md)。

## Rust

在 Cargo.toml 中添加 crates.io 依赖：

```toml
[dependencies]
ofdrw = "0.1.0"
```

```rust
use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = OfdDocument::new();
    document.add_text_page("你好，OFD", 210.0, 297.0)?;
    document.save("hello.ofd")?;
    println!("{:?}", document.page_text(0)?);
    Ok(())
}
```

## C / 自行编译共享库

GitHub Releases 的 native ZIP 中提供 `include/ofdrw.h`、动态库和静态库。
也可从源码构建：

```sh
git clone https://github.com/lihongjie0209/ofdrw-rs.git
cd ofdrw-rs
cargo build --release --locked -p ofdrw-ffi
```

产物位于 `target/release/`。Python、Go、Node.js 均支持通过 `OFDRW_LIB_PATH`
指定自定义共享库；路径应为绝对路径。
页面索引从 0 开始，页面尺寸和坐标使用毫米。

## main 开发版功能

以下功能尚未包含在 v0.1.0 安装包中，需使用 main 源码及对应共享库。

### 图像插值

传入图像对象 ID（非媒体资源 ID），即可设置采样方式：

| 语言 | 最近邻采样 | 清除设置，恢复平滑默认值 |
| --- | --- | --- |
| Rust | `doc.set_page_image_interpolate(0, &image_id, Some(false))?` | 最后一个参数改为 `None` |
| Python | `doc.set_page_image_interpolate(0, image_id, False)` | 最后一个参数改为 `None` |
| Node.js | `doc.setPageImageInterpolate(0, imageId, false)` | 最后一个参数改为 `null` 或省略 |
| Go | `nearest := false; doc.SetPageImageInterpolate(0, imageID, &nearest)` | 最后一个参数改为 `nil` |
| C | `ofd_set_page_image_interpolate(doc, 0, image_id, 0)` | 最后一个参数改为 `-1` |

启用平滑采样时，Rust 使用 `Some(true)`、Python/Node.js 使用 `True`/`true`、
Go 传入值为 `true` 的指针，C 使用 `1`。设置仅影响指定图像对象的放置位置；
已签名页面拒绝编辑。

### PDF 批注外观

PDF 导入支持批注的正常外观流（`AP/N`），包括 Widget、FreeText
和 Stamp 的已选外观。各语言继续使用现有 PDF 导入接口；交互表单行为和
导出限制见[功能说明](docs/FEATURES.md#pdf-导入与兼容范围)。

## 测试与发布

CI 在 Linux、macOS、Windows 的 x64/ARM64 上运行 Rust 和三个语言绑定的测试，
并验证实际打包产物。行覆盖率门槛：Rust 核心 90%、C ABI 93%、Python 90%、
Go 87%、Node.js 90%。Go 还执行 race、vet、lint 和依赖漏洞检查。

Java 参考样本固定为 OFDRW 提交 `84e9eb260ba6fd351ecccf08a01a40305d4af635`。
本地运行完整兼容性测试时，将本仓库放在该 Java 仓库的 `ofdrw-rs/` 子目录，
然后执行：

```sh
cargo test --workspace --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
bash scripts/check-rust-coverage.sh
```

更新各包版本并推送 `vX.Y.Z` 标签会触发 Release 工作流。
工作流等待同一提交的 CI 全部成功，再发布 PyPI、npm、crates.io、Release 资产、校验值及 Go 子模块标签。
原生仓库发布使用 `PYPI_API_TOKEN`、`NPM_TOKEN`、`CARGO_REGISTRY_TOKEN` 三个仓库 Secret。
凭据保管、初始化和发布步骤见 [发布维护说明](docs/PUBLISHING.md)。

## 许可证

[Apache-2.0](LICENSE)。来源和测试材料说明见 [NOTICE](NOTICE) 及
[fixtures README](crates/ofdrw/tests/fixtures/README.md)。
内含的 [SVG→PDF 转换器分支](crates/ofdrw-svg2pdf/README.md)
保留上游 MIT OR Apache-2.0 许可证。
