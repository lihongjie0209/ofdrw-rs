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
