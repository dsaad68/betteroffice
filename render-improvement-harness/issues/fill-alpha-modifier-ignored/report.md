---
id: fill-alpha-modifier-ignored
title: Carry a:alpha through to the display list so translucent fills blend
category: fill
impact: high
effort: easy
confidence: high
status: open
occurrences: 6
decks: [green-solutions, ocp-psp-plan, project20]
findings: [green-solutions/01/1, ocp-psp-plan/03/2, project20/01/1, project20/06/3, project20/10/2, project20/15/2]
files: [crates/pptx-render/src/layout.rs, crates/ooxml-drawingml/src/color.rs, crates/pptx-parse/src/drawing.rs, crates/pptx-raster/src/lib.rs, crates/pptx-render/src/display_list.rs, packages/pptx/src/render/canvas.ts]
---

## Symptom

An `<a:alpha>` child on a `srgbClr` or `schemeClr` inside a `solidFill` is dropped between
parsing and painting, so every translucent shape paints at 100% opacity. Where the
translucent shape is a full-slide scrim the effect is catastrophic: `green-solutions/01`
renders as a solid black slide with the city photo completely hidden behind a 33%-black
rectangle (`evidence-1.png`, 74.5% diff), and `project20/01` renders as three flat
gray/near-white panels instead of a tinted mountain banner (`evidence-2.png`, 46.3% diff).
Where the alpha is mild the effect is a flat, textureless fill - the 97%-opaque navy square
on `project20/10` and `/15` loses the bi-level texture bleeding through it
(`evidence-3.png`).

Pixel sampling confirms the fill is unblended rather than mis-blended. On `project20/10`
BetterOffice reports exactly `(36, 38, 93)` = `#24265D` at (60,150), (200,300) and
(120,550); LibreOffice reports `(42, 44, 97)` at all three, which is
`0x24 * 0.97 + 255 * 0.03 = 42.6` - the declared `alpha="97000"` composited over white. On
`green-solutions/01` BetterOffice reports `(0, 0, 0)` across the whole photo area and
`(242, 242, 242)` inside the icon circles (`bg1` at `lumMod 95000`, i.e. the raw resolved
color) where LibreOffice reports photo pixels such as `(5, 48, 79)` and `(165, 148, 136)`.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | green-solutions/01 | Full slide. `Rectangle 109`, a slide-sized `tx1` scrim at `alpha="33000"`, paints solid black over the full-bleed JPEG; the `bg1`/`lumMod 95000`/`alpha 50000` icon circles paint solid white instead of showing the photo through. |
| 2 | project20/01 | Full slide. Three full-height layout rectangles (`353535` at `alpha="30196"` twice, `bg1`+`lumMod 95000` at `alpha="80000"`) tile the slide width and paint opaque, hiding the layout's mountain banner entirely. |
| 3 | project20/10 | Crop of the navy square. `24265D` at `alpha="97000"` over a bi-level texture picture: the reference shows the mountain texture faintly through it, the candidate is flat navy. Same shape and cause on `project20/15` and `project20/06`. |
| 4 | ocp-psp-plan/03 | Crop of the left panel. A 60%-alpha `tx2` shadow layer and two 50%-alpha `bg1` fade layers paint opaque and black out the photo collage. This slide *also* suffers `geometry-custom-collapses-to-bbox`, which is why the black area is a rectangle rather than a crescent. |

## Root cause (hypothesis)

**Confirmed.** `a:alpha` is parsed, stored on the model, and then discarded by the only
resolver the pptx render path calls.

- Parse: `crates/pptx-parse/src/drawing.rs:691` reads `<a:alpha>` into `ColorValue::alpha`
  as a fraction, alongside `lumMod`/`lumOff`/`satMod`
  (`crates/pptx-parse/src/drawing.rs:685-691`). The field is declared at
  `crates/ooxml-drawingml/src/color.rs:30` and its doc comment already states the problem:
  "Opaque hex output drops it; use [`resolve_color_value_to_rgba_hex`]."
- Model: the value survives into `ShapeFill::color`
  (`crates/ooxml-drawingml/src/shape.rs:11`) and `ShapeOutline::color`
  (`crates/ooxml-drawingml/src/shape.rs:47`) as a full `ColorValue`.
- Drop: `paint()` at `crates/pptx-render/src/layout.rs:1897` is the single funnel for every
  shape fill and for the slide background (`layout.rs:183`, `:388`, `:526`). Its last
  statement, `crates/pptx-render/src/layout.rs:1926`, calls
  `resolve_color_value_to_hex_with_theme`, which returns `#RRGGBB`
  (`crates/ooxml-drawingml/src/color.rs:61-85`) and never reads `color.alpha`. `stroke()`
  at `crates/pptx-render/src/layout.rs:1931` does the same for `a:ln`. Gradient stops go
  through the same opaque resolver at `crates/pptx-render/src/layout.rs:1914`.
- The correct resolver already exists and is unit-tested -
  `resolve_color_value_to_rgba_hex` at `crates/ooxml-drawingml/src/color.rs:91`, test
  `alpha_reaches_the_rgba_resolver_and_never_the_opaque_one` at
  `crates/ooxml-drawingml/src/color.rs:236` - but a repo-wide grep for it returns only its
  own definition and that test. It has **no** production caller in any crate.

Both consumers of the display list already accept the 8-digit form, so nothing downstream
needs to change:

- Raster: `parse_color` (`crates/pptx-raster/src/lib.rs:770`) delegates to
  `parse_hex_color` (`crates/pptx-raster/src/lib.rs:780`), whose length match accepts
  `6 | 8` and reads the alpha byte at `crates/pptx-raster/src/lib.rs:798`. tiny-skia's
  `fill_path` (`crates/pptx-raster/src/lib.rs:382`) composites source-over by default.
- Canvas: `Paint::Solid.color` is a plain string
  (`crates/pptx-render/src/display_list.rs:24-27`) that `paintStyle` hands straight to
  `ctx.fillStyle` (`packages/pptx/src/render/canvas.ts:183`), and Canvas2D accepts
  `#RRGGBBAA`.

**Reattribution.** `green-solutions/01/1` was originally filed as a picture failure ("the
full-bleed JPEG is not drawn"). It is not: `decks/green-solutions/bo-log.json` records
`"01".skipped_images = 0`, so the JPEG was decoded and drawn, and the
`picture-fill-fails-to-render` investigation confirmed empirically that deleting
`Rectangle 109` from `slide1.xml` makes the photo appear. The finding has been moved into
this cluster and out of that one.

**Not confirmed / out of scope.** Two adjacent alpha paths are left alone deliberately and
are *not* claimed to be fixed by this issue:

- Run-level text alpha also goes through the opaque resolver
  (`crates/pptx-render/src/layout.rs:1049`, `:1854`), and `valid_color`
  (`crates/pptx-render/src/layout.rs:1982`) hard-requires a 6-digit hex, so widening text
  color to RGBA is a larger change than the fill fix.
- Picture transparency (`a:alphaModFix` on a `blipFill`) is a separate mechanism and is not
  covered here.

## Verification

- Re-render `green-solutions` 01, `project20` 01/06/10/15 and `ocp-psp-plan` 03.
  `green-solutions/01` (`fine_pct` 74.52) and `project20/01` (46.33) should fall
  dramatically - those two are dominated by this single defect. `project20/15` (1.81) and
  `project20/10` (5.32) should drop to near the texture's own contribution.
- `ocp-psp-plan/03` (33.01) will improve but **not** resolve: the same shapes are drawn as
  bounding boxes rather than Bezier crescents (`geometry-custom-collapses-to-bbox`), so
  expect a translucent rectangle instead of an opaque one.
- Pixel check without a diff run: sample `project20/10` at (60,150) - the candidate must
  move from `(36, 38, 93)` toward LibreOffice's `(42, 44, 97)`.
- Existing coverage: `crates/ooxml-drawingml/src/color.rs:236` already pins the resolver's
  behaviour; the missing test is one asserting `paint()` emits `#RRGGBBAA`. The raster
  goldens under `crates/pptx-raster/tests/golden/` use opaque fixture fills, so they should
  not move; if any does, that fixture has an alpha nobody noticed.
