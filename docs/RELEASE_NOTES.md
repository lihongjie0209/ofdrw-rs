Initial public preview of ofdrw-rs.

- Rust OFD package/document core with shared C ABI.
- Python, Go and Node.js bindings, including TypeScript declarations.
- Supported page editing, layout, SVG/PNG/PDF export and PDF import.
- Supported annotations, attachments, signatures and encryption.
- Six native targets: Linux, macOS and Windows on x64 and ARM64.

Node.js is published as `@lihongjie0209/ofdrw-rs` in GitHub npm Packages.
Python, Go, Rust and native distributions are also published as GHCR OCI packages.
Install instructions are in the repository README. Wheels and the npm tarball
bundle their native library. Go consumers download the matching native ZIP
and set OFDRW_LIB_PATH. Rust consumers can use the Git tag or the .crate asset.

This is a preview with incomplete Java OFDRW/standards coverage. See
docs/FEATURES.md for supported behavior and remaining limitations.
