# v0.1.1

Changes since v0.1.0:

- PDF annotation normal appearances import as native OFD composite appearances,
  including state selection, transformed BBox/Rect placement, rotation, CropBox,
  UserUnit, inherited resources, embedded text/images and supported transparency.
  Links keep their actions and quadrilateral hit regions; FreeText/Stamp/Widget/
  Caret/Redact/Watermark with AP are also accepted. Independent original-PDF
  rasters, exported-PDF raster checks, negative cases and three-language tests
  cover this path. PDF export retains independent normal Form appearances:
  Links keep actions/quads and other custom appearances become movable/resizable
  Stamp annotations. Page content contains no duplicate appearance. Mixed
  annotation order, unique names, Unicode metadata, popups, screen/print flags
  and NoView survive the tested cycles. Linux CI compares both view and print
  rasters. Interactive forms, field values, alternate mouse states and
  applying redactions remain outside this conversion.

- Rust, C, Python, Go and Node.js expose per-page-image interpolation setters
  with a reset-to-default option. The setter changes only the chosen placement,
  preserves shared media, rejects signed pages and leaves failed edits intact.
  Integration tests exercise all language surfaces, null/closed handles,
  invalid arguments, round trips and TypeScript types. README examples are
  explicitly marked as unreleased main APIs.

- Explicit native image Interpolate flags now control SVG/PNG and PDF sampling
  per placement, including shared media and matching alpha-image flags. PDF
  import keeps booleans and the standard false default. PDF normalization no
  longer overwrites explicit nearest sampling. JPEG compressed bytes remain
  unchanged. Seven integration tests cover mixed placements, alpha, source
  defaults/indirect values/errors, round trips and optional Ghostscript pixels.
  Native objects without a flag keep smooth sampling; separately filtered PDF
  color/mask display remains unrepresented after RGBA merging.

- Native PDF import consumes marked-content ActualText with inline, named and
  indirect properties. Complete glyph mappings retain original outlines,
  positions, advance and clipping; multi-run strings share intact graphemes.
  Non-text paint scopes receive nonpainting text. The bundled sRGB ICC profile
  is accepted for transparency groups, and zero-opacity groups retain selectable
  text without painting. Missing-glyph cases now survive native PDF/OFD/PDF
  cycles; a generated CFF fixture also exercises Python, Go and Node bindings.
  Empty/too-short replacements, changed unembedded-font text, scopes crossing
  composites and structure-tree ActualText still require further work.

- PDF export preserves per-occurrence source Unicode for missing glyph 0 with
  ActualText, avoiding the tested “中文” → “文文” extraction collision. The
  focused ofdrw-svg2pdf fork retains upstream licensing. Seven font-isolated
  cases cover TrueType/CFF, BMP, supplementary, repeated and mixed text; Poppler checks the
  extracted text and optional Ghostscript compares unchanged painted pixels.
  Missing fonts remain visibly missing; readers ignoring ActualText still rely
  on the ambiguous shared ToUnicode entry. Native import supports the mapped
  marked-content cases described above.

- OFD-A preparation removes Audio/Video declarations from document, public,
  page and referenced-template resource scopes. Existing action normalization
  removes playback actions. Obsolete payloads are pruned only when no XML value
  still references them; shared images and the implicit OFD.xml entry point are
  retained. Namespace lookalikes and foreign-wrapped image/media declarations
  are left intact. Core and Python/Go/Node tests cover resource scopes, source
  preservation, round trips and path/reference edge cases. Other unresolved
  OFD-A rules still require inspection after conversion.

- PDF export decomposes matching glyph-mapped RTL text into hidden positioned
  graphemes, fixing the tested Lam-Alef extraction reversals while retaining
  original outlines, explicit spaces and OFD/public-SVG logical text. Eleven
  integration cases and six units cover extraction, round trips, scaling,
  rotation, fallback and optional export-raster comparison. Linux CI includes
  nine Poppler extraction checks. Pages with combining marks keep the original
  semantic path to avoid collisions in shared font maps. Unmatched glyph aliases and
  reader base-direction/numeric ordering remain limitations; positions inside
  ligatures are apportioned equally rather than exact typographic caret data.

- PDF export normalizes font ToUnicode maps to type 2 and removes optional
  CMapVersion/WMode metadata that prevents some readers from decoding them.
  Character mappings and font payloads are retained. Seventeen text-export
  cases verify masked text, nested groups, ligatures, Chinese, supplementary
  Unicode, variation selectors and rotation. Linux CI also runs Poppler against
  the exported PDFs. Native/Poppler differences for unmapped variation-cluster
  glyphs remain explicit, and full RTL extraction order remains unresolved.

- Graphics-state PDF soft masks support Alpha and DeviceRGB Luminosity groups,
  BC backdrops and the four scalar transfer-function families. Activation CTM
  and resource scope, mask replacement/reset, nesting, image-mask precedence,
  group opacity, clipping, pattern cells and shading-pattern ExtGState masks are
  retained. A bounded RGBA fallback rasterizes masked paints, including text;
  direct masked text and text inside nested isolated groups retain a native
  nonpainting semantic layer for extraction, search and positioned region queries after OFD save/reopen. Group placement
  is flattened with fresh object IDs, retaining font and glyph data without
  modifying shared composite resources. PDF export retains the tested semantic
  text cases through embedded fonts and ToUnicode maps. Transparent knockout groups, other luminosity blending spaces and AIS shape masks remain unsupported.
  Forty-four original-source raster scenes, analytic pixels, negative/unit
  coverage and Python/Go/Node integration cover the feature. Inline-image
  fractional alpha uses a Poppler reference; the other references use
  Ghostscript. Optional export-raster checks compare the resulting PDFs too.
  Form byte streams now share the bounded filter decoder. Unmasked content
  continues through the existing native path.

- Isolated PDF Form transparency groups (`I=true`, `K=false`) import as native
  vector composite graphic units. Group opacity/blending/clipping applies once
  after member composition; member alpha and blend mode start at defaults.
  Nested groups, text, images, gradients and Forms inside pattern cells retain
  resources and text extraction. Composite SVG rendering now enforces isolation.
  Twenty-eight source/reference scenes, analytic opacity/backdrop checks,
  negative/unit coverage and Python/Go/Node integration cover the feature.
  One reference uses original-source Poppler because Ghostscript loses Form
  isolation inside a pattern; the others use original-source Ghostscript.
  Optional Ghostscript export checks cover every scene. Transparent non-isolated
  and knockout Form paints remain unsupported; supported soft masks are added
  above.

- Generated PDF raster images and alpha masks set `Interpolate=true`, matching
  SVG/PNG preview sampling. Their original image samples are preserved. PDF
  import still does not preserve enlarged nearest-neighbor image display when
  the source uses `Interpolate=false`; that separate limitation is documented.

- General PDF radial shading supports nonnested/equal-radius circles, separate
  endpoint extension, point circles and the tangent linear limit. A stable
  quadratic selects the greatest valid parameter with a nonnegative radius.
  Native nested gradients remain vector paints; general fields use bounded
  RGBA sampling (16 pixels/mm, four million pixels per paint), accounting for
  outer pattern magnification. Coincident nonzero circles remain unsupported.
  Thirty-three original-source Ghostscript raster/round-trip scenes, numeric
  boundary tests and Python/Go/Node integration cover the feature.

- Direct PDF axial/radial shading preserves full affine geometry under
  nonuniform scale and shear instead of projecting only endpoints/radii.
  Current rendering intent, alpha, blend mode, clipping and Form/page matrices
  are retained; direct-shading backgrounds stay ignored. Hidden/nonpainting
  validation leaves no resources or reserved IDs. Twenty-four original-PDF
  raster references, four new boundary tests and Python/Go/Node integration
  cover the change.

- PDF shading parameters resolve indirect `ShadingType`, bounds, coordinates,
  extension flags and domains, including referenced array components. Direct
  shading operators and pattern paints share the same array reader and numeric
  limits. Thirteen new integration tests cover all seven shading families,
  direct/reference SVG/PNG equality, independent rasters, round trips and
  malformed/cyclic values. Authored reference-parameter sources and equivalent
  direct-value PDFs are committed, with Python/Go/Node integration coverage.

- PDF shading patterns (`PatternType=2`) import through the seven existing
  shading families as reusable native paints for paths, strokes, text and
  stencils. Affine source-space mapping preserves radial/axial geometry;
  background colors, BBox clipping, initial parent state, rendering intent and
  source/outer alpha are retained. Twenty-eight independent raster/round-trip
  scenes and explicit opacity checks cover the implementation; three alpha
  references include equivalent Form-group PDFs because common renderers
  ignore the pattern ExtGState alpha. Pattern blending that depends on the
  backdrop remains unsupported. Explicit supported soft masks are added above.

- PDF colored and uncolored tiling patterns import as native OFD pattern
  paints with reusable vector cell resources. Paths, strokes, text, stencils,
  supported cell images/forms, nested patterns, matrices, gaps/overlaps,
  negative steps, alpha, page rotation and UserUnit have coverage in 26
  independent raster/round-trip scenes. Python, Go and Node.js exercise the
  same feature through the existing PDF import API. Cycles, malformed
  dictionaries, uncolored color-setting content and excessive overlap fail
  explicitly. Backdrop-dependent cell blending remains unsupported. Independent
  composite resources also reserve the in-memory identifier range before
  subsequent pages are added.

- Inline PDF image parsing resolves named color spaces in the current page
  or Form resources for both raw and filtered pixels. Candidate EI markers
  are checked against decoded sample length, including EI bytes inside pixels.
  Eight independent ICC raster/round-trip cases and Python/Go/Node calls cover
  intent inheritance, scoped resources, compressed data and image masks.

- PDF raster color conversion inherits the current `ri` or ExtGState `RI`
  when an image has no explicit Intent. Image Intent still overrides that
  state. Color-key, explicit and soft masks preserve the inherited setting;
  q/Q restores it. A synthetic ICC profile with distinct per-intent LUTs
  makes the regression checks sensitive to the previous fixed-default behavior.

- Explicit DeviceN JPEG2000 images now convert up to 32 source colorants
  directly, avoiding display-decoder RGB/alpha inference on raw codestreams.
  Source precision is retained for color-key masks, and external soft masks
  and existing embedded-alpha conversion remain supported. Ten independent
  raster cases cover 2/4/5/32 channels, container color overrides, embedded
  alpha, omitted bit depth and 8/16-bit masks.
  Component-buffer limits are checked before decoding; malformed, mismatched
  and oversized declarations have regression coverage through the core.

- PDF DeviceN composite RGB conversion supports up to 32 named colorants,
  multi-input sampled/calculator transforms, single-input exponential/stitching
  transforms, ICC intent conversion and Indexed bases. All-None spaces suppress
  painting; mixed None values remain inputs to the alternate transform.
  Raw image conversion now safely handles more than four source channels.
  Eighteen independent raster/round-trip cases cover vector/text painting,
  images up to 32 channels, masks, NChannel fallback and shading families.
  Physical plate blending, MixingHints and overprint remain outside this preview.

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
