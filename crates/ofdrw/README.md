# ofdrw

Rust OFD document core: package reading/writing, text and graphics, layout,
SVG/PNG/PDF output, supported PDF import, annotations, SM2 signatures and encryption.
This is an early subset of Java OFDRW; full standards compatibility is still in progress.

```rust
use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = OfdDocument::new();
    document.add_text_page("Hello, OFD", 210.0, 297.0)?;
    document.save("hello.ofd")?;
    Ok(())
}
```

See the [repository](https://github.com/lihongjie0209/ofdrw-rs) for language
bindings, release assets, supported operations, testing and limitations.
