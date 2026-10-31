# 原生仓库发布维护

Python 发布到 PyPI（`ofdrw-rs`），Node.js 发布到 npm（`ofdrw-rs`），
Rust 核心发布到 crates.io（`ofdrw`），Go 使用 GitHub 子模块标签。
C ABI 的六个平台原生 ZIP 和所有语言的备份文件放在 GitHub Releases。

## 凭据

| 服务 | passman 引用 | GitHub Actions Secret |
| --- | --- | --- |
| PyPI | `registry/pypi#token` | `PYPI_API_TOKEN` |
| crates.io | `registry/crates-io#token` | `CARGO_REGISTRY_TOKEN` |
| npm | `registry/npm#token` | `NPM_TOKEN` |

Go 标签和 GitHub Release 使用内置 `GITHUB_TOKEN`，不需要额外凭据。
不要把 token 写入源码、命令参数或聊天记录。

首次发布新项目时，在各服务生成凭据：

- [PyPI API token](https://pypi.org/manage/account/token/)：项目不存在时选择 Entire account；
  项目建立后可改用项目范围的 token 或 Trusted Publishing。
- [crates.io token](https://crates.io/settings/tokens)：先在 [账号设置](https://crates.io/settings/profile) 验证邮箱，
  再生成允许发布新 crate 和更新 `ofdrw` 的 token。
  发布流程不需要修改 owner 或 yank 版本的权限。
- [npm Granular token](https://www.npmjs.com/settings/~/tokens)：选择包的
  Read and write (publish and stage)，CI 无交互发布需允许 Bypass 2FA。
  尚不存在的包首次发布需授予创建该包的权限。

在本机终端交互保存：

```sh
passman entry set registry/pypi#token
passman entry set registry/crates-io#token
passman entry set registry/npm#token
```

通过标准输入同步到仓库 Secrets，避免显示密钥：

```sh
passman run --stdin registry/pypi#token -- gh secret set PYPI_API_TOKEN --repo lihongjie0209/ofdrw-rs
passman run --stdin registry/crates-io#token -- gh secret set CARGO_REGISTRY_TOKEN --repo lihongjie0209/ofdrw-rs
passman run --stdin registry/npm#token -- gh secret set NPM_TOKEN --repo lihongjie0209/ofdrw-rs
```

长期维护建议使用各仓库的 Trusted Publishing。npm 已宣布在 2027 年 1 月
取消绕过 2FA 的 token 直接发布能力，需在此前迁移到可信发布或采用 staged publish 的人工确认流程。
配置可信发布时，仓库为 `lihongjie0209/ofdrw-rs`，工作流文件为 `release.yml`；
配置完成后还需相应更新本仓库的认证步骤。
参考 [npm token 规则](https://docs.npmjs.com/about-access-tokens/)、
[PyPI Trusted Publishing](https://docs.pypi.org/trusted-publishers/)。

## 发布步骤

转换器 [ofdrw-svg2pdf 0.13.0-ofdrw.1](https://crates.io/crates/ofdrw-svg2pdf/0.13.0-ofdrw.1)
已发布，源提交为 `f919fd7c7ff717643c459ca005082b554000b142`。
[发布工作流](https://github.com/lihongjie0209/ofdrw-rs/actions/runs/41136637318)
已成功，公开包的校验和、源码提交和核心所需的 `srgb_profile()` 接口均已核验。

转换器依赖 `ofdrw-svg2pdf` 使用独立版本。首次发布或修改该分支时，
先更新其 Cargo 版本及核心依赖，将提交推送到 main 并等待完整 CI 成功。
然后运行 `Publish SVG PDF converter`（`publish-svg2pdf.yml`），输入该提交 SHA；
工作流验证同一提交的 CI、打包并使用现有 `CARGO_REGISTRY_TOKEN` 发布到 crates.io。
确认该依赖版本可见后，再打包和发布依赖它的新核心版本。已发布的转换器版本
保持不可变，后续修改需要递增版本。来源、许可证和改动见
[转换器说明](../crates/ofdrw-svg2pdf/README.md)。


1. 更新 Rust 核心、C ABI、Python 和 Node.js 的版本，运行 `scripts/check_release_version.py vX.Y.Z`。
2. 将提交推送到 `main`，等待该提交的完整 CI 成功。
3. 创建并推送相同提交的 `vX.Y.Z` 标签。
4. Release 工作流使用该提交的已验证产物，检查全部凭据，上传原生仓库并校验公开资产摘要。
5. 工作流从 PyPI 和 npm 安装并验证库，创建 `bindings/go/vX.Y.Z` 标签，再发布 GitHub Release。

Linux wheel 经过 auditwheel 修复，CI 会安装修复后的 manylinux wheel。
原始 `linux_*` 标签的 wheel 不上传 PyPI。

同一版本重跑会验证已发布文件的摘要，仅补传缺失的 Python wheels。
若已有版本的文件字节不同，工作流失败，避免覆盖或误认不可变版本。
发布中断时可用 Release 工作流的 Run workflow 按钮，输入原有标签继续；
不要移动已对外发布的版本标签。

若 GitHub Release 已存在，也可单独运行 `Publish existing Python release`
工作流（`publish-python.yml`），输入原有版本标签。它验证该源提交已有成功 CI，
检查 Release 中六个平台 wheel 的版本及 `SHA256SUMS`，再仅补传 PyPI 缺失文件。
此流程不依赖其他语言仓库的发布状态，完成后会在六个平台从公开 PyPI 安装并验证库。

Rust 可通过 `Publish existing Rust release` 工作流（`publish-rust.yml`）单独重试。
它从原标签打包，先与 Release 的 `.crate` 校验和及内含源码提交比对，再发布到
crates.io，最后从原生仓库安装并运行 Rust、Go 的 README 示例。

注册表可能在接受上传后继续处理数分钟。可见性校验超时只表示验证尚未完成，
请先检查原版本状态，避免把上传成功后的等待误判为上传失败。
npm 的版本元数据和安装索引可能先后可见；安装验证强制在线查询，使用独立缓存，
并最多重试 12 次。上传已成功而安装验证失败时，可等待索引同步后重跑原版本。
发布工作流显式设置 `RUSTUP_TOOLCHAIN=1.96.0`，避免打包时自动安装开发用的
rustfmt/clippy 组件；它仍从原标签打包，并核验已上传文件的摘要。

macOS wheel 的架构标记取自内含 Rust 动态库的 Mach-O 头；
每个平台单独构建的动态库不得标为 `universal2`。发布工具也会校验或修复历史产物的标记。
