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

## Mixed flow (since 0.4.0)

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

### Inline placeholders (since 0.4.0)

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

This input is available since 0.4.0. C/Python/Go/Node.js
accept `placeholder_width`, empty `text` and a `font_size` height in span inputs. See the feature reference for validation and
leading-indent rules.

## Stateful vector canvas (unreleased; after 0.4.0)

```rust
use ofdrw::{CanvasCommand, OfdDocument};

let mut doc = OfdDocument::new();
doc.add_blank_page(40., 30.)?;
let objects = doc.draw_canvas(0, [4., 3., 24., 20.], &[
    CanvasCommand::SetStrokeColor { color: [0, 80, 200] },
    CanvasCommand::SetLineWidth { width: 0.5 },
    CanvasCommand::Rect { rect: [1., 2., 12., 8.] },
    CanvasCommand::Stroke,
])?;
```

The batch commits atomically and returns painted object IDs. Paths, drawing
parameters, transforms, opacity and clipping participate in save/restore.
Path coordinates are transformed when painted; rectangle helpers preserve the
current path. See the feature reference for the Java clip fixture normalization
and remaining canvas text/image work. C, Python, Go and Node.js expose the
same batch through `ofd_draw_canvas_json`, `draw_canvas`, `DrawCanvas` and
`drawCanvas`, respectively. See each binding README for examples.
