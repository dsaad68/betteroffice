# betteroffice-pptx-raster

The tiny-skia backend that paints a PPTX slide display list to PNG. Server-side
twin of the browser's canvas replayer, and the CPU reference the native viewer
diffs its GPU output against.

```rust
use pptx_raster::{AssetMap, RenderOptions, RenderResources, render_slide};

let images: AssetMap<'_> = presentation
    .media()
    .iter()
    .map(|part| (part.part_path.as_str(), part.bytes.as_slice()))
    .collect();
let resources = RenderResources::new(renderer.fonts(), &images);
let png = render_slide(&display_list, &resources, &RenderOptions::default())?;
```

Most callers want the facade instead — `betteroffice-pptx` with the `raster`
feature gives you `Presentation::render_png`, which resolves media out of the
package for you.

PNG encoding uses fixed settings, so identical inputs produce byte-identical
output. `tests/golden.rs` byte-compares every scenario against a committed PNG;
regenerate deliberately with:

```bash
GOLDEN_UPDATE=1 cargo test -p betteroffice-pptx-raster
```

## Fonts

Nothing is embedded. Text is painted from the `PositionedGlyph` runs the layout
pass already placed, so this crate shapes nothing — it resolves each run's
`font_id` against the `FontStore` you hand it and fills the outline. Register
faces on the `SlideRenderer` (or the `Presentation`) before laying the slide out.

## Never in the wasm build

Decoding pictures needs the `image` crate, so `src/lib.rs` refuses to compile for
`wasm32`. The browser gets its PNG from `slideToPng` in `@betteroffice/pptx`,
which drives the canvas replayer and `canvas.toBlob()` instead.

## SVG pictures

A picture whose bytes are SVG is rasterized natively with resvg/usvg instead of
the `image` crate. It draws shapes, paths, fills, strokes, linear and radial
gradients, clip paths, `<use>` and `<symbol>` instances, and stylesheets of
plain type, class and id rules. It does not draw text (`usvg`'s text feature is
off), embedded raster images (both href resolvers return `None`), markers,
filters, masks or patterns. A hyperlink or an embedded image leaves the rest of
the picture drawn; any other element outside that set declines the document.

The sandbox bounds what conversion and rendering cost by construction:

- **Before `usvg`.** No DTD, no reference outside the document, no reference
  cycle. The tree every `<use>`, clip path and paint reference expands into is
  sized first: `MAX_SVG_EXPANDED_NODES` elements, `MAX_SVG_EXPANDED_BYTES` of
  markup, `MAX_SVG_DEPTH` levels, `MAX_SVG_GRADIENT_STOPS` stops per gradient,
  and `MAX_SVG_STYLE_WORK` of selector matching over at most
  `MAX_SVG_STYLE_RULES` rules.
- **Before `resvg`.** The converted tree is priced: group layers stack at most
  `MAX_SVG_LAYER_DEPTH` deep and are charged to the slide's image budget with
  the output raster, painted area stays within `MAX_SVG_OVERDRAW` times that
  raster, and painting work within `MAX_SVG_RENDER_WORK`. The raster renders at
  up to four times the document's intrinsic size and steps down when it would
  not fit.
- **During both.** A panic in either library becomes a refusal.

A refused document is a skipped image like any other and carries nothing from
the document. A tiled SVG repeats at its intrinsic size, as a raster repeats at
its pixel size.

## What the display list does not carry

These are gaps upstream of this crate, in the contract `pptx-render` emits, so
the PNG can only be as faithful as what the canvas backend already draws:

- **Picture crops.** `PictureCrop` (`srcRect`) is parsed but dropped by the
  layout pass, so a cropped picture paints stretched to its frame.
- **Tables.** They arrive as dashed `Placeholder` boxes labelled `"Table"`; the
  parsed cell content is never laid out.
- **Effects and alpha.** There is no shadow, glow, reflection, soft edge, or
  opacity in the contract, and colors resolve to `#rrggbb` with no alpha.
- **Pattern and picture fills.** `Paint` is only `Solid` or `Gradient`.
- **Dash patterns.** A stroke's dash collapses to one boolean, so the specific
  OOXML pattern is lost; this crate synthesizes the same dashes the canvas
  backend does, keeping the two in agreement.

Unlike `docx-raster`, which errors on anything it cannot reproduce faithfully,
these are absences in the input rather than fields being refused, so this crate
paints what is there. Only images degrade at render time: one that is missing,
undecodable, or over budget is skipped and counted in
`RenderedSlide::skipped_images`.
