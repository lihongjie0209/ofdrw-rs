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

## Mixed flow (unreleased source feature)

```rust
use ofdrw::{FlowBlock, FlowDocumentStyle, OfdDocument};

fn main() -> Result<(), ofdrw::OfdError> {
    let mut document = OfdDocument::new();
    let mut style = FlowDocumentStyle::default();
    let result = document.add_flow_blocks(0, &[
        FlowBlock::paragraph("First paragraph"),
        FlowBlock::Spacer { height: 4. },
        FlowBlock::paragraph("Second paragraph"),
    ], style.clone())?;
    style.text.start = result.next;
    document.add_flow_blocks(0, &[FlowBlock::paragraph("Continued")], style)?;
    Ok(())
}
```

The same flow supports sized images, paginated grids, and explicit page breaks.
A failed later block rolls back the complete call. See the repository feature
reference for supported layout behavior and limits.
