---
id: picture-fill-fails-to-render
title: Draw EMF/WMF pictures and OLE fallback pictures instead of skipping them
category: picture
impact: high
effort: hard
confidence: high
status: open
occurrences: 9
decks: [cisco-cloud-security, project17, rollout-plan]
findings: [cisco-cloud-security/01/2, cisco-cloud-security/16/4, cisco-cloud-security/22/1, project17/09/1, project17/11/4, rollout-plan/01/3, rollout-plan/06/1, rollout-plan/07/2, rollout-plan/08/3]
files: [crates/pptx-raster/Cargo.toml, crates/pptx-raster/src/lib.rs, crates/pptx-raster/README.md, crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-render/src/layout.rs, crates/pptx-render/src/lib.rs, crates/pptx-render/src/display_list.rs, packages/pptx/src/types.ts, packages/pptx/src/render/canvas.ts]
---

## Symptom

Logos, clip-art illustrations and OLE chart fallbacks vanish from the candidate render,
leaving bare background where the reference draws artwork: the Cisco wordmark on the
title slide (`evidence-1.png`), the business-team clip art beside a table
(`evidence-2.png`), and a think-cell pie chart that becomes an empty dashed frame with
its labels floating in mid-air (`evidence-3.png`).

The cluster holds **two distinct causes**, not one:

1. **Nine findings — the media is an EMF or WMF metafile.** The picture reaches the
   raster backend, the decoder rejects the bytes, and the image is silently dropped and
   counted in `skipped_images`.
2. **Two findings (project17/09/1, project17/11/4) — the picture is inside an OLE
   `p:graphicFrame`.** It never becomes a picture at all: the OLE fallback `p:pic` is
   never parsed, so the frame degrades to a dashed `Placeholder`. Those two also point at
   EMF media, so both causes must be fixed for those slides to render.

A tenth finding, `green-solutions/01/1`, **was not a picture failure** and has since been
moved to `fill-alpha-modifier-ignored` — see "Not confirmed" below.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/01 | Layout picture "Picture 8" -> `ppt/media/image3.emf`: the Cisco wordmark is drawn by the reference and absent from the candidate. `bo-log.json` `"01".skipped_images = 1`. |
| 2 | rollout-plan/07 | "Picture 4" (id 5) -> `ppt/media/image10.emf`: the whole silhouette illustration is missing; `"07".skipped_images = 1`. |
| 3 | project17/11 | "Object 426" (id 457), an OLE `p:graphicFrame` whose `mc:Fallback` `p:pic` points at `ppt/media/image18.emf`: no pie, only the dashed placeholder frame and orphaned labels. `"11".skipped_images = 0` — the image was never even attempted. |
| 4 | green-solutions/01 | Full slide. The candidate is solid black. This is **not** the JPEG failing; see below. |

## Root cause (hypothesis)

**Confirmed, cause 1 (EMF/WMF).** The picture is parsed and laid out correctly; only the
final decode fails.

- `crates/pptx-parse/src/drawing.rs:159` parses `<p:pic>`, resolving `r:embed` to a media
  part path at `crates/pptx-parse/src/drawing.rs:176`.
- Both layout paths emit a `Primitive::Image` carrying that path as `asset_id` —
  `crates/pptx-render/src/layout.rs:425` (slide snapshot shapes) and
  `crates/pptx-render/src/layout.rs:534` (layout/master shapes).
- `crates/pptx-raster/src/lib.rs:404` looks the asset up and calls `ImageCache::decode` at
  `crates/pptx-raster/src/lib.rs:733`, which hands the bytes to
  `image::ImageReader::with_guessed_format()` / `into_decoder()`
  (`crates/pptx-raster/src/lib.rs:736`-`740`). The `image` crate has no EMF or WMF support
  at any feature level, and the enabled features are only `bmp, gif, jpeg, png, tiff,
  webp` (`crates/pptx-raster/Cargo.toml:18`-`25`), so `with_guessed_format` cannot
  identify the stream and the `?` returns `None`.
- `crates/pptx-raster/src/lib.rs:426` turns that `None` into `skipped_images += 1` and
  paints nothing. This is deliberate degradation, documented at
  `crates/pptx-raster/README.md:62`-`64`.
- The browser backend is in the same position: `packages/pptx/src/render/canvas.ts:206`
  hands `assetId` to a host-supplied `CanvasImageResolver`, and no browser
  `CanvasImageSource` can be built from EMF/WMF bytes either. A fix that lives only in
  `pptx-raster` leaves the canvas renderer blank.

Media confirmed as metafiles by reading the parts out of `source.pptx`: cisco
`image3.emf` and `image6.emf` (both `" EMF"` signature at offset 0x28), cisco
`image62.wmf` (placeable header `d7cdc69a`), rollout-plan `image7.emf`-`image11.emf`,
project17 `image14/15/18.emf`. Across the whole harness `skipped_images` totals exactly 9,
matching the nine metafile findings — every skipped image in the corpus is a metafile, and
no non-metafile picture is skipped anywhere.

**Confirmed, cause 2 (OLE fallback).** `p:oleObj` is not handled anywhere in the codebase
(`grep -ri 'oleObj|AlternateContent'` over `crates/pptx-parse/src` and
`crates/pptx-render/src` returns nothing).

- `parse_graphic_frame` at `crates/pptx-parse/src/drawing.rs:192` recognises exactly three
  payloads — `a:tbl`, `c:chart` and `dgm:relIds`. Anything else, including
  `<a:graphicData uri=".../presentationml/2006/ole">`, falls into
  `GraphicFrameData::Unknown` at `crates/pptx-parse/src/drawing.rs:240`, keeping only the
  URI string.
- `crates/pptx-render/src/layout.rs:593` (`render_graphic_frame`) only special-cases chart
  spaces; everything else pushes a `Primitive::Placeholder` at
  `crates/pptx-render/src/layout.rs:625` with `label: None` (`graphic_label` at
  `crates/pptx-render/src/layout.rs:1960` returns `None` for `Unknown`), which is the
  dashed empty box in `evidence-3.png`.
- Separately, `parse_shape_children` (`crates/pptx-parse/src/drawing.rs:101`) dispatches on
  `local_name()` and has no `mc:AlternateContent` arm, so any picture wrapped in
  `mc:Choice`/`mc:Fallback` is dropped even outside a graphic frame. Not exercised by a
  finding in this cluster, but the same gap.

**Not confirmed — `green-solutions/01/1` did not belong in this cluster and has been moved
to `fill-alpha-modifier-ignored`.** The finding
claims the full-bleed `image1.jpeg` is not drawn; the black slide has a different cause.
`bo-log.json` reports `"01".skipped_images = 0`, so nothing was skipped. Re-rendering the
deck with the single shape `Rectangle 109` deleted from `slide1.xml` — a full-slide
`<a:solidFill><a:schemeClr val="tx1"><a:alpha val="33000"/>` scrim — makes the photo
appear correctly. `a:alpha` is dropped on the way to the display list: every pptx call site
uses `resolve_color_value_to_hex_with_theme` (`crates/pptx-render/src/layout.rs:1049`,
`:1854`, `:1914`, `:1926`, `:1931`), and `resolve_color_value_to_rgba_hex`
(`crates/ooxml-drawingml/src/color.rs:91`) has no non-test caller in the repo, so the
33%-black scrim paints fully opaque over the photo. This belongs with
`fill-alpha-modifier-ignored`; the cluster's symptom line ("One full-bleed JPEG background
also fails to draw") should be dropped.

**No shortcut: these metafiles carry no embedded bitmap.** Walking the EMR record stream of
all ten EMFs in the affected decks gives 35 distinct record types, dominated by path
construction and filling — `SETPOLYFILLMODE` 436, `POLYBEZIERTO16` 346, `MOVETOEX` 315,
`CLOSEFIGURE` 309, `BEGINPATH`/`ENDPATH` 248 each, `FILLPATH` 242, `SELECTOBJECT` 218,
`POLYPOLYGON16` 194, `CREATEBRUSHINDIRECT` 178, `POLYLINETO16` 141, plus
`SAVEDC`/`RESTOREDC`, `INTERSECTCLIPRECT`, `SELECTCLIPPATH`, `EXTSELECTCLIPRGN`, `PIE`,
`EXTCREATEPEN`/`CREATEPEN` and the window/viewport mapping records. There is **no**
`STRETCHDIBITS`, `SETDIBITSTODEVICE`, `BITBLT` or `STRETCHBLT` record anywhere, so there is
no raster payload to lift out; and there is no `EXTTEXTOUTW`/`EXTTEXTOUTA`, so even the
"TOMORROW starts here." tagline in cisco `image6.emf` is outlined paths rather than text. A
real record interpreter is required, but for this corpus it needs no font handling and no
DIB blitting.

## Verification

- Re-render the ten affected slides: `cisco-cloud-security` 01, 16, 22; `project17` 09, 11;
  `rollout-plan` 01, 06, 07, 08. All `skipped_images` counters in `decks/*/bo-log.json`
  must reach 0 — harness-wide the total is currently 9, spread over cisco (3), rollout-plan
  (5) and `ocp-psp-plan` slide 01 (1, not itself a finding in this cluster but the same
  defect).
- `rollout-plan/06` and `rollout-plan/07` are the cleanest signal: the missing artwork sits
  on plain background, so `diff-summary.json` `fine_pct` should drop by roughly the shape's
  area fraction with no confounding text differences.
- `project17/09` and `project17/11` additionally require cause 2; until the OLE fallback is
  parsed their `skipped_images` stay at 0 while the pie charts stay blank, so use the
  dashed placeholder disappearing as the pass condition rather than the counter.
- `green-solutions/01` will not improve from this issue; it improves when
  `fill-alpha-modifier-ignored` lands.
- Existing coverage to extend: `crates/pptx-raster/tests/golden.rs:31` already asserts
  `skipped_images == 0` for every golden scenario, so a golden that paints a small EMF
  becomes a regression test for free. `a_missing_asset_is_skipped_and_counted`
  (`crates/pptx-raster/src/lib.rs:854`) is the matching negative case and must keep passing
  for genuinely undecodable bytes.
