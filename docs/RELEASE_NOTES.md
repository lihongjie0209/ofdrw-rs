Unreleased source changes after v0.1.0:

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
