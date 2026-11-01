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

### Inline placeholders (development)

`FlowSpan::placeholder(width, height)` reserves an indivisible inline rectangle
without painting text, glyphs or decorations. Width zero is allowed. Height
participates in the maximum line height, and following text starts after the
reserved width. `FlowSpan::placeholder_characters(count, font_size, letter_spacing)`
uses `count * (font_size + letter_spacing)` millimeters. Both constructors work
with measurement, styled flow, text boxes and mixed containers. Placeholder-only
shaped content does not require an embedded font.

```rust
use ofdrw::{FlowSpan, FlowTextStyle, OfdDocument};

fn main() -> Result<(), ofdrw::OfdError> {
    let document = OfdDocument::new();
    let metrics = document.measure_flowed_spans(0, &[
        FlowSpan::new("Label:"),
        FlowSpan::placeholder(8., 4.),
        FlowSpan::new("value"),
    ], 50., &FlowTextStyle::default())?;
    assert_eq!(metrics.lines[0].text, "Label:value");
    Ok(())
}
```

This input is new development functionality after 0.3.0. Its C/Python/Go/Node.js
span fields follow separately. See the feature reference for validation and
leading-indent rules.
