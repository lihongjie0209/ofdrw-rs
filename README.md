# ofdrw-rs

[![CI](https://github.com/lihongjie0209/ofdrw-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lihongjie0209/ofdrw-rs/actions/workflows/ci.yml)
[![Releases](https://img.shields.io/github/v/release/lihongjie0209/ofdrw-rs?include_prereleases)](https://github.com/lihongjie0209/ofdrw-rs/releases)

使用 Rust 实现的 OFD 文档库，提供 **Rust、Python、Go、Node.js 和 C ABI**。
以 [Java OFDRW](https://github.com/ofdrw/ofdrw) 为兼容性参考，优先实现包读写、页面对象、资源、布局、渲染和文档转换。

当前版本是 **0.1.0 预发布版**，已经覆盖大量核心功能，但尚未完整替代 Java OFDRW。
完整布局规则、PDF 转换、证书信任验证和 OFD-A 转换仍有未实现部分。
支持范围及具体限制见 [功能和 API 说明](docs/FEATURES.md)。

## GitHub Packages

库发布到 [GitHub Packages](https://github.com/lihongjie0209?tab=packages&repo_name=ofdrw-rs)：

| 语言 / 内容 | Package | 安装方式 |
| --- | --- | --- |
| Node.js / TypeScript | `@lihongjie0209/ofdrw-rs` | GitHub npm registry |
| Python | `ghcr.io/lihongjie0209/ofdrw-rs-python:0.1.0` | ORAS 拉取 wheel，再用 pip 安装 |
| Go 源码 | `ghcr.io/lihongjie0209/ofdrw-rs-go:0.1.0` | ORAS 拉取；也支持 Git 标签 `go get` |
| Rust 核心 | `ghcr.io/lihongjie0209/ofdrw-rs-rust:0.1.0` | ORAS 拉取 `.crate` 源码包 |
| C ABI / Go 原生库 | `ghcr.io/lihongjie0209/ofdrw-rs-native:0.1.0` | ORAS 拉取动态库、静态库和 C 头文件 |

GitHub Packages 原生支持 npm，但没有 PyPI、Cargo 或 Go 模块 registry。
因此 Python、Rust、Go 和 native 文件使用 GitHub Container Registry 的 OCI 包分发。
使用 [ORAS CLI](https://oras.land/docs/installation/) 拉取文件，不需要运行容器。
每个 OCI 包包含对应文件、许可证及 `SHA256SUMS`。
同一批文件也保留在 [GitHub Releases](https://github.com/lihongjie0209/ofdrw-rs/releases)，供浏览器下载。

`<platform>` 为 `linux-x64`、`linux-arm64`、`darwin-x64`、`darwin-arm64`、`win32-x64` 或 `win32-arm64`。
Linux 预编译库在 Ubuntu 24.04 构建；较旧的 glibc 系统请从源码编译。
macOS 在 macOS 15 构建。Python wheels 的平台名遵循 Python 的 wheel 命名规则。

## Python

需要 Python 3.10+。从 GitHub Packages 拉取 wheels，选择适合操作系统和 CPU 的文件安装：

```sh
oras pull ghcr.io/lihongjie0209/ofdrw-rs-python:0.1.0 -o packages/python
python -m pip install ./packages/python/ofdrw_rs-0.1.0-py3-none-linux_x86_64.whl
```

wheel 已带 Rust 共享库，不需要安装 Rust。它会自动安装 CFFI 依赖。

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

需要 Node.js 20+。配置 GitHub npm registry 后安装：

```sh
npm config set @lihongjie0209:registry https://npm.pkg.github.com
npm login --scope=@lihongjie0209 --auth-type=legacy --registry=https://npm.pkg.github.com
npm install @lihongjie0209/ofdrw-rs@0.1.0
```

GitHub npm Packages 安装需要登录；使用带 `read:packages` 权限的 GitHub personal access token (classic) 作为密码。

```js
const { Document } = require('@lihongjie0209/ofdrw-rs');

const document = Document.create();
try {
  document.addTextPage('你好，OFD', 210, 297);
  document.save('hello.ofd');
  console.log(document.pageText(0));
} finally {
  document.close();
}
```

TypeScript 可使用 `import { Document } from '@lihongjie0209/ofdrw-rs'`，类型声明随包提供。
更多内容见 [Node.js 说明](bindings/node/README.md)。

## Go

Go 包通过 purego 动态加载 C ABI，**不需要 cgo**。需要 Go 1.22+。
先从 GitHub Packages 拉取 native ZIP 并解压对应平台文件，设置动态库的绝对路径：

```sh
oras pull ghcr.io/lihongjie0209/ofdrw-rs-native:0.1.0 -o packages/native
unzip packages/native/ofdrw-native-linux-x64-v0.1.0.zip -d native
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

从 GitHub Packages 拉取并解压 Rust 核心源码：

```sh
oras pull ghcr.io/lihongjie0209/ofdrw-rs-rust:0.1.0 -o vendor
tar -xzf vendor/ofdrw-0.1.0.crate -C vendor
```

在 `Cargo.toml` 中引用解压后的包：

```toml
[dependencies]
ofdrw = { path = "vendor/ofdrw-0.1.0" }
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

GitHub Packages 的 native ZIP 中提供 `include/ofdrw.h`、动态库和静态库。
也可从源码构建：

```sh
git clone https://github.com/lihongjie0209/ofdrw-rs.git
cd ofdrw-rs
cargo build --release --locked -p ofdrw-ffi
```

产物位于 `target/release/`。Python、Go、Node.js 均支持通过 `OFDRW_LIB_PATH`
指定自定义共享库；路径应为绝对路径。
页面索引从 0 开始，页面尺寸和坐标使用毫米。

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
工作流等待同一提交的 CI 全部成功，再发布 GitHub npm / OCI Packages、Release 资产、校验值及 Go 子模块标签。
发布流程授予 `packages: write`，使用 GitHub 自带的 `GITHUB_TOKEN`，不需要包索引账号密钥。

## 许可证

[Apache-2.0](LICENSE)。来源和测试材料说明见 [NOTICE](NOTICE) 及
[fixtures README](crates/ofdrw/tests/fixtures/README.md)。
