Unreleased source changes after v0.1.0:

- PDF patch meshes now skip padding after each patch. Shared-edge fixtures
  use nonzero padding, and symmetric raster comparisons also detect extra
  paint outside the reference shape.

- PDF Separation tint conversion covers alternate device/calibrated/ICC colors,
  function types 0/2/3/4, Indexed palettes, paths, text, images and shadings.
  All and None retain distinct painting semantics, including clipping and
  independently colored content. Tests include malformed inputs, four ICC
  intents and eighteen raster/round-trip fixtures. Composite RGB does not
  preserve physical spot-ink plates or overprint.

- PDF Type 1 function-based shading imports two-input sampled/calculator
  functions and component arrays with domain/matrix mapping, color conversion,
  clips, opacity and blend modes. Arbitrary functions become bounded RGB PNG
  resources at a target eight pixels/mm, capped at four million pixels.
  Twelve Ghostscript references cover discontinuities, packed/cubic samples,
  reversed mappings, CalGray/CMYK and transformed clips. Resource/work limits
  and all standard sample bit depths have unit coverage; three-sample cubic
  tables now also retain cubic interpolation in existing one-input importers.

- PDF Type 6/7 Coons and tensor-product patch meshes import through bounded
  geometry/color refinement. Byte-aligned patch records, three shared-edge modes,
  curved boundaries, internal control points and bilinear source colors are
  retained. Fourteen new Ghostscript references cover folded patches, functions,
  nonlinear colors, transforms, clipping, opacity, packed and wide coordinates.
  Rust and language bindings verify rendering and round trips; malformed
  streams and complexity
  limits have unit coverage.

- PDF Type 4/5 Gouraud meshes import with source topology, packed vertex
  records, transforms, clips, opacity and blend modes. Nonlinear color spaces
  and shading functions retain source interpolation through bounded mesh
  subdivision. Tests cover twelve independent Ghostscript references,
  all standard bit depths, ignored flag bits, invalid topology, limits,
  OFD/PDF round trips and Python/Go/Node calls.

- CFF outlines support full affine top-level and per-FD font matrices,
  including FDSelect formats 0 and 3. PDF advances and stroke widths retain
  source text-space metrics. Raw CFF resources with explicit glyph mappings
  render directly in existing OFD packages without changing their bytes.
  Imported text clips retain glyph maps and emit direct SVG clip shapes.
  Tests cover 20 CFF and one TrueType Ghostscript references, bounds,
  matrix inheritance, clipping, round trips and Python/Go/Node calls.

- CFF Encoding dictionaries without BaseEncoding inherit the embedded font's
  native encoding before applying Differences. Explicit bases retain priority;
  subset Zapf Dingbats fonts use their special glyph-name Unicode mapping.
  Internal glyph names share the existing 1,024-byte bound. Tests cover raw
  and OpenType programs, indirect metadata, retained/overridden codes,
  independent raster references and three language bindings.

- Embedded Type1 and CIDFontType0 CFF programs import from raw FontFile3
  streams or OpenType CFF tables. CID charset selectors stay independent of
  Unicode and TrueType CIDToGIDMap. Raw programs are retained unchanged in
  deterministic sfnt wrappers for mapped outline rendering. Internal CFF
  encodings also recover simple-font text when PDF Encoding/ToUnicode are
  absent. Tests cover nine independent Ghostscript rasters, CID/GID differences,
  ligatures, vertical text, bounded parsing, checksums and language bindings.
  Remaining CFF limitations are listed in FEATURES.md.

- All 189 current Adobe CMap resources are bundled, including 73 vertical
  maps. Tests parse every resource offline, verify original source digests
  and licenses, check vertical punctuation CID substitutions through embedded
  font glyph selection, and exercise named CJK mappings with and without
  ToUnicode through Rust and language bindings.

- Vertical Type0 text imports with Identity-V or embedded WMode 1 CMaps.
  DW2/W2 select two-dimensional glyph origins and vertical advances; TJ,
  spacing, text rise, scaling and transformed baselines retain PDF geometry.
  Tests cover inherited modes, indirect metrics, malformed/oversized tables,
  independent Ghostscript raster references and all three language bindings.

- Simple-font Differences without ToUnicode decode Adobe glyph-name sequences,
  including underscore components, multiple BMP groups and AGL combining
  sequences. Embedded source glyphs and advances remain intact; decoding
  shares the 200,000 UTF-16-unit run limit. Tests cover Adobe examples,
  original ligature rendering, package round trips and Python integration.

- Embedded TrueType/OpenType text keeps source glyph IDs independently of
  ToUnicode text, using OFD CGTransform when the mappings differ. Ligatures,
  multi-character aliases and registered Unicode variation sequences retain
  glyph advances and text semantics; CID fonts without a Unicode cmap also
  render from their outlines. Tests cover exact source glyph selection,
  UTF-16/escaped text spans, limits, malformed glyph maps, font/package reuse,
  independent Ghostscript raster comparison and three language bindings.
  Mapped glyph bounds also expand object boundaries to retain overhanging ink.

- Type0 fonts without ToUnicode recover text through five pinned official
  Adobe CID-to-Unicode resources when their descendant declares a supported
  collection. Named, embedded and Identity-H source encodings still select
  CID widths independently. Explicit ToUnicode retains priority and errors;
  unknown collections and unrepresentable sequences are rejected. Tests
  cover indirect metadata, CJK text, supplementary-plane characters, raster
  and package equivalence, source integrity and language bindings.

- The native library includes 189 pinned horizontal/vertical Adobe CMap resources for
  named CJK encodings. Named maps validate CIDFont collection metadata and
  retain independent Unicode/CID lookups. A bounded cache shares immutable
  mappings across text runs while preserving copy-on-write inheritance and
  parsing limits. Comment handling accepts official Adobe source headers and
  comments between section operands; nested operands are bounded. Tests parse
  every bundled resource and verify Chinese, Japanese, Korean, malformed
  metadata, package/render round trips and three language bindings.

- Type0 text accepts embedded horizontal Encoding CMaps with mixed one- to
  four-byte source codes, CID character/range mappings, notdef fallbacks and
  bounded UseCMap inheritance. Unicode lookup uses the source code while
  widths and embedded glyph validation use the mapped CID. Tests cover
  bytewise codespaces, word spacing, malformed data, limits, embedded font
  reuse, equivalent geometry/raster output and Python/Go/Node.js consumers.

- Custom and subset Type1 fonts import without ToUnicode when an explicit
  named encoding or named BaseEncoding/Differences provides the character
  mapping. MacExpertEncoding is accepted alongside the other predefined
  single-byte encodings. Unknown built-in encodings require explicit metadata;
  PDF source-code widths and ToUnicode precedence remain intact. Unit and
  Python/Go/Node.js integration tests cover decoding and package round trips.

- PDF text preserves standard-font and FontDescriptor weight/italic styles,
  including indirect descriptor fields and CID descendants. Safely reused
  embedded TrueType/OpenType programs supply their intrinsic style. Tests
  cover conflicting metadata, font reuse, invalid references and values,
  package/render round trips, and Python/Node.js consumers.

- Simple-font FirstChar/LastChar and width entries accept indirect objects,
  with bounded numeric validation before layout. BT now permits its default
  text origin and resets positioning for each text object while preserving
  font parameters. Tests compare explicit and implicit positions, Type1 and
  CID text, embedded TrueType resources, and all three language bindings.

- All fourteen standard Type1 faces have built-in AFM advances when Widths
  is omitted, including consecutive text runs, TJ adjustments, character and
  word spacing. Symbol and ZapfDingbats default encodings also import.
  Explicit widths retain priority and ToUnicode does not change source-code
  advances. Numerical regressions and Python/Go/Node.js integration cases
  cover these paths; distribution notices include the source data permissions.

- Simple-font Encoding dictionaries support named base encodings and
  Differences glyph-name overrides without ToUnicode. Tests cover Adobe
  glyph names, Unicode names, indirect entries, spacing, embedded TrueType
  reuse, ToUnicode precedence and three language bindings. An embedded-font
  fixture is also checked against an independent Ghostscript raster.

- Type0/CID text accepts indirect Identity-H encoding names and width-table
  metadata, including default widths and both W array forms. Chinese text,
  spacing and rendering have direct/indirect equivalence tests and a shared
  Python/Go/Node.js fixture. Width expansion counts overwritten declarations
  against a one-million-assignment limit.

- Color-space family names, ICC channel counts and Indexed palette bounds
  accept indirect objects. ICC profile compatibility and palette length/type
  checks remain enforced. Tests cover Gray/RGB/CMYK ICC image and vector
  conversion, Indexed palettes, invalid types and cyclic references.

- CalGray, CalRGB and Lab parameters accept indirect arrays and numeric
  entries, including scalar CalGray Gamma. Direct and indirect definitions
  retain equivalent vector, image and shading output. Invalid references and
  numeric metadata return errors rather than silently selecting defaults.

- PDF axial and radial shading accept arrays of independent component
  functions, mixing sampled, exponential, nested stitching and calculator
  functions. Values remain in component space through Lab/ICC conversion;
  knots, extrema and discontinuities are retained before bounded sampling.
  New numerical, independent-render and binding tests cover this form.
  Linear single-function gradients also retain nonlinear calibrated, Lab and
  ICC conversion curves; single and array forms have equivalence regressions.

- PDF Type 4 calculator functions support the 42 standard operators, bounded
  typed evaluation, and adaptive axial/radial gradient import. Conditional and
  integer-step jumps are retained as separate color stops; finite sampling
  does not exhaustively resolve oscillations between samples. Numerical, malformed
  input, resource-limit, independent-render and language-binding tests cover
  the supported behavior.

- PDF sampled, exponential and stitching functions resolve indirect parameters
  and numeric array members. Reference validation retains type, array-length,
  range and size checks, and rejects dangling or cyclic references. New axial
  and radial fixtures exercise the behavior in Rust, Python, Go and Node.js.

- PDF gradients support nested stitching functions with reversed and constant
  encodings, sequential component-range clipping, discontinuity preservation,
  and bounded recursion. New numerical and Ghostscript-rendered regressions
  cover nested axial and radial import through all three language bindings.

- PDF sampled shading functions now support cubic (Order 3) interpolation,
  including table edges, interior extrema, reversed mappings and output clipping.
  Axial and radial import fixtures exercise the C ABI through Python, Go and
  Node.js; the axial pixel regression uses an independent Ghostscript render.
- macOS Python wheel tags identify the bundled library architecture. The v0.1.0
  GitHub wheel assets have also been corrected without changing native code.

These source changes do not alter the immutable v0.1.0 npm package or Git tags.

Initial public preview of ofdrw-rs.

- Rust OFD package/document core with shared C ABI.
- Python, Go and Node.js bindings, including TypeScript declarations.
- Supported page editing, layout, SVG/PNG/PDF export and PDF import.
- Supported annotations, attachments, signatures and encryption.
- Six native targets: Linux, macOS and Windows on x64 and ARM64.

Python is published on PyPI as `ofdrw-rs`, Node.js on npm as `ofdrw-rs`,
and the Rust core on crates.io as `ofdrw`. Go uses the GitHub module tag.
Install instructions are in the repository README. Wheels and the npm tarball
bundle their native library. Go consumers download the matching native ZIP
and set OFDRW_LIB_PATH. Rust consumers can use the Git tag or the .crate asset.

This is a preview with incomplete Java OFDRW/standards coverage. See
docs/FEATURES.md for supported behavior and remaining limitations.
