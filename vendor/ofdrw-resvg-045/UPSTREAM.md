# Upstream and local changes

Source: https://github.com/linebender/resvg/tree/1b6c2fddbcbeffa8135df4323b02aaae84890907/crates/resvg

The upstream Apache-2.0 and MIT licenses are preserved alongside the source.
This fork translates the render budget for nested offscreen layers and masks,
and derives pattern tile budgets from the tile surface. It preserves all clip
paths, including fractional viewport edge coverage. The library API is unchanged;
the package name is `ofdrw-resvg`.

Compiler compatibility: use explicit shader lifetimes and the existing bytemuck
pixel conversion API instead of deprecated rgb slice conversion.

The next, unpublished `ofdrw.2` revision recognizes OFDRW-generated SVG paths
whose reserved IDs begin with `ofd-device-hairline-`. These paths use the
native tiny-skia zero-width stroke, retaining one device pixel after transforms.
Other paths keep upstream physical stroke-width behavior. The generated SVG
also declares `vector-effect="non-scaling-stroke"` for external SVG consumers.
