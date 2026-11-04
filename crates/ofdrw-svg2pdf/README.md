# OFDRW SVG to PDF converter

A focused fork of [typst/svg2pdf](https://github.com/typst/svg2pdf), based on
svg2pdf 0.13.0 (upstream commit `ccab6d7a3081f512d412dfef456caed2caf3a698`). The upstream MIT OR Apache-2.0 licenses and authors are retained.

The OFDRW change adds per-glyph PDF ActualText when a glyph is missing. It preserves the
original font, outlines, advance, text matrix, and painting operations.
A missing glyph remains visibly missing; its source text can still be extracted.

Local changes: missing-glyph ActualText, two explicit elided lifetimes for
current Rust lint compatibility, formatting, self-contained runnable documentation examples, package metadata, and disabling
raster filters in default features (still available through `filters`).

The fork also exposes its exact built-in sRGB profile for faithful native
transparency-group reimport.

The converter has an independent version. After changes, bump its version and
the core's dependency together. Once main CI succeeds for the exact commit,
run the `Publish SVG PDF converter` workflow with that commit. Publish this
dependency before packaging the next Rust core release. Previously released
converter versions must not be replaced or republished.

Raster conversion honors SVG image-rendering per placement. Nearest sampling
sets PDF Interpolate=false; smooth sampling sets true on both color images and
associated alpha images. JPEG samples retain their original compressed bytes.

The raster backend uses `ofdrw-resvg` 0.48.1-ofdrw.2 to preserve nested
layer, mask and pattern tile coordinates. Publish the renderer dependency with
`Publish SVG renderers` before publishing this converter version.

Converter revision `0.13.0-ofdrw.3` preserves
OFD path hairlines: reserved `ofd-device-hairline-` SVG path IDs select the
native PDF zero-width stroke operator. The renderer dependencies above must
be published first. This change does not add zero-width text strokes.

Converter revision `0.13.0-ofdrw.4` upgrades the SVG
model to usvg 0.48.1 and preserves the SVG radial-gradient focal radius (`fr`)
in native PDF radial shading coordinates. Color and opacity shadings share the
same two circles, including gradients inside pattern cells. Text adapts the
new model's wider glyph IDs through checked OpenType glyph-ID conversions.
Regression tests inspect PDF shading dictionaries and compare Poppler renders
against the OFD PNG output at two resolutions. This fix is absent from v0.5.0.

The next, unpublished revision `0.13.0-ofdrw.5` also retains SVG `repeat` and `reflect` in native
axial/radial PDF shadings and their opacity masks. It expands the shading to
cover the painted bounds, including negative axial phases and the radius-zero
limit of nonzero focal radii. A shared binary tree of stitching functions
preserves period boundaries with logarithmic space. Coincident color stops
remain hard transitions, without invalid zero-width stitching intervals.
Nonfinite geometry and spans exceeding 2^24 periods return `InvalidGradient`,
rather than losing integer period precision in PDF coordinates.
