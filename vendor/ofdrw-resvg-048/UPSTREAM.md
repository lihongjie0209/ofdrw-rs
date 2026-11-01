# Upstream and local changes

Source: https://github.com/linebender/resvg/tree/68b14c4c3bccdb60344c777406486b54c36ec1a4/crates/resvg

The upstream Apache-2.0 and MIT licenses are preserved alongside the source.
This fork translates the render budget for nested offscreen layers and masks,
and derives pattern tile budgets from the tile surface. It preserves all clip
paths, including fractional viewport edge coverage. The library API is unchanged;
the package name is `ofdrw-resvg`.
