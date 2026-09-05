---
"@betteroffice/pptx": minor
"@betteroffice/python-pptx": minor
"@betteroffice/rust-crates": minor
---

Rasterize a PPTX slide to PNG.

PPTX was the one format that could not produce pixels: `pptx-render` stopped at
a display list, and only the browser canvas replayer and the native viewer's
GPU path ever painted it. The new `betteroffice-pptx-raster` crate paints that
display list with tiny-skia, the way `docx-raster` and `xlsx-raster` already do
for their formats — vector geometry, gradients, images, and the pre-shaped
glyph runs the layout pass placed, with chart and text-box clipping and
per-primitive rotation and flips. PNG encoding uses fixed settings, so identical
inputs produce byte-identical output, and a golden suite byte-compares every
scenario.

`betteroffice-pptx` gains a `raster` feature and `Presentation::render_png`,
which resolves pictures out of the package so only fonts need registering, and
caches glyph outlines across a deck's slides. `betteroffice_pptx.Presentation`
gains the same as `render_png(slide, *, scale, background) -> Png`. Both take a
scale for hidpi output and a background of `slide`, `transparent`, or a
`#rrggbb` color.

In the browser, `slideToPng` in `@betteroffice/pptx` drives the existing canvas
replayer through `canvas.toBlob()`, and the React editor gains an Export PNG
button beside Save. Nothing new crosses the wasm boundary: decoding pictures
needs the `image` crate, so the raster crate refuses to compile for `wasm32`,
exactly as `docx-raster` does.

The native viewer's GPU-versus-CPU diff now covers PPTX too, where it previously
printed "not produced (PPTX has no raster backend)".

Minor rather than patch: `betteroffice_pptx::Error` gains a `Raster` variant, so
anything matching it exhaustively must be updated.
