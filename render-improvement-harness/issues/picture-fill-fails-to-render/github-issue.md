# pptx: Picture fails to render at all (EMF/WMF, OLE fallback, one JPEG)

**Describe the bug**

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

Seen on 9 slides across 3 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort hard, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/01** Layout picture "Picture 8" -> `ppt/media/image3.emf`: the Cisco wordmark is drawn by the reference and absent from the candidate. `bo-log.json` `"01".skipped_images = 1`.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-fill-fails-to-render/evidence-1.png)

**2. rollout-plan/07** "Picture 4" (id 5) -> `ppt/media/image10.emf`: the whole silhouette illustration is missing; `"07".skipped_images = 1`.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-fill-fails-to-render/evidence-2.png)

**3. project17/11** "Object 426" (id 457), an OLE `p:graphicFrame` whose `mc:Fallback` `p:pic` points at `ppt/media/image18.emf`: no pie, only the dashed placeholder frame and orphaned labels. `"11".skipped_images = 0` — the image was never even attempted.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-fill-fails-to-render/evidence-3.png)

**4. green-solutions/01** Full slide. The candidate is solid black. This is **not** the JPEG failing; see below.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/picture-fill-fails-to-render/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 16, 22
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 9, 11
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 6, 7, 8

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(8, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed, cause 1 (EMF/WMF).** The picture is parsed and laid out correctly; only the
final decode fails.

- [`crates/pptx-parse/src/drawing.rs:159`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L159) parses `<p:pic>`, resolving `r:embed` to a media
  part path at [`crates/pptx-parse/src/drawing.rs:176`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L176).
- Both layout paths emit a `Primitive::Image` carrying that path as `asset_id` —
  [`crates/pptx-render/src/layout.rs:425`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L425) (slide snapshot shapes) and
  [`crates/pptx-render/src/layout.rs:534`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L534) (layout/master shapes).
- [`crates/pptx-raster/src/lib.rs:404`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L404) looks the asset up and calls `ImageCache::decode` at
  [`crates/pptx-raster/src/lib.rs:733`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L733), which hands the bytes to
  `image::ImageReader::with_guessed_format()` / `into_decoder()`
  ([`crates/pptx-raster/src/lib.rs:736`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L736)-`740`). The `image` crate has no EMF or WMF support
  at any feature level, and the enabled features are only `bmp, gif, jpeg, png, tiff,
  webp` ([`crates/pptx-raster/Cargo.toml:18`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/Cargo.toml#L18)-`25`), so `with_guessed_format` cannot
  identify the stream and the `?` returns `None`.
- [`crates/pptx-raster/src/lib.rs:426`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L426) turns that `None` into `skipped_images += 1` and
  paints nothing. This is deliberate degradation, documented at
  [`crates/pptx-raster/README.md:62`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/README.md#L62)-`64`.
- The browser backend is in the same position: [`packages/pptx/src/render/canvas.ts:206`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L206)
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

- `parse_graphic_frame` at [`crates/pptx-parse/src/drawing.rs:192`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L192) recognises exactly three
  payloads — `a:tbl`, `c:chart` and `dgm:relIds`. Anything else, including
  `<a:graphicData uri=".../presentationml/2006/ole">`, falls into
  `GraphicFrameData::Unknown` at [`crates/pptx-parse/src/drawing.rs:240`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L240), keeping only the
  URI string.
- [`crates/pptx-render/src/layout.rs:593`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L593) (`render_graphic_frame`) only special-cases chart
  spaces; everything else pushes a `Primitive::Placeholder` at
  [`crates/pptx-render/src/layout.rs:625`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L625) with `label: None` (`graphic_label` at
  [`crates/pptx-render/src/layout.rs:1960`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1960) returns `None` for `Unknown`), which is the
  dashed empty box in `evidence-3.png`.
- Separately, `parse_shape_children` ([`crates/pptx-parse/src/drawing.rs:101`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L101)) dispatches on
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
uses `resolve_color_value_to_hex_with_theme` ([`crates/pptx-render/src/layout.rs:1049`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1049),
`:1854`, `:1914`, `:1926`, `:1931`), and `resolve_color_value_to_rgba_hex`
([`crates/ooxml-drawingml/src/color.rs:91`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L91)) has no non-test caller in the repo, so the
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

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Two independent changes. Part A is small and unlocks two findings' shapes; Part B is the
real work and unlocks all nine metafile findings.

### Part A — parse the OLE fallback picture (medium)

`p:graphicFrame` with `graphicData uri=".../presentationml/2006/ole"` wraps an
`mc:AlternateContent` whose `mc:Fallback/p:oleObj` contains a complete `p:pic`. That
picture is exactly what PowerPoint and LibreOffice paint for a non-activated OLE object.

- In `parse_graphic_frame` ([`crates/pptx-parse/src/drawing.rs:192`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L192)), before the
  `Unknown` arm, look for a `pic` descendant under `graphicData` and, when found, run the
  existing `parse_picture` on it. Store it as a new `GraphicFrameData::Ole { picture:
  Box<Picture> }` in [`crates/pptx-parse/src/model.rs:243`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L243).
- In `render_graphic_frame` ([`crates/pptx-render/src/layout.rs:593`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L593)), add an arm that
  pushes the same `Primitive::Image` the `ShapeNode::Picture` arm builds
  ([`crates/pptx-render/src/layout.rs:534`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L534)), using the frame's own rect rather than the
  nested `p:pic`'s `a:xfrm` — the graphic frame is authoritative for placement.
- Give `parse_shape_children` ([`crates/pptx-parse/src/drawing.rs:101`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L101)) an
  `"AlternateContent"` arm that recurses into `mc:Fallback` (then `mc:Choice`), so
  fallback-wrapped shapes are not dropped outside graphic frames either.
- On its own this changes nothing visually for project17 — those fallbacks are EMFs — but
  it removes the spurious dashed placeholder and makes those slides depend only on Part B.

### Part B — a metafile interpreter that emits display-list primitives (hard)

Decoding must not live in `pptx-raster`: that crate never compiles for wasm
([`crates/pptx-raster/README.md:38`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/README.md#L38)-`42`) and the browser path goes through a host
`CanvasImageResolver` ([`packages/pptx/src/render/canvas.ts:206`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L206)) that cannot handle
metafile bytes either. Put the interpreter in a new wasm-safe crate (say
`ooxml-metafile`) that turns EMF/WMF bytes into the primitives the contract already has,
and call it from the layout pass so both backends get the artwork for free.

`Primitive::Shape` is a good target: its `path` is `Vec<GeometryPathCommand>` in
0..1 coordinates relative to the shape frame ([`crates/pptx-raster/src/lib.rs:575`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L575)-`600`),
which is exactly what an EMF's window/viewport mapping normalises to.

- Detect metafiles by media content type, which `MediaPart` already carries
  ([`crates/pptx-parse/src/package.rs:167`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/package.rs#L167)), rather than by sniffing bytes in the backend.
- Interpret a GDI subset: an object table for `CREATEPEN`/`EXTCREATEPEN`/
  `CREATEBRUSHINDIRECT`/`SELECTOBJECT`/`DELETEOBJECT`, a DC stack for `SAVEDC`/`RESTOREDC`,
  window/viewport mapping (`SETWINDOWORGEX`, `SETWINDOWEXTEX`, `SETVIEWPORTORGEX`,
  `SETVIEWPORTEXTEX`, `SETMAPMODE`), the path bracket (`BEGINPATH`/`ENDPATH`/`CLOSEFIGURE`
  then `FILLPATH`/`STROKEPATH`/`STROKEANDFILLPATH`), and the geometry records
  `MOVETOEX`, `POLYBEZIERTO16`, `POLYLINETO16`, `POLYPOLYGON16`, `POLYGON16`, `PIE`.
  That set plus `SETPOLYFILLMODE`, `SETBKMODE`, `SETROP2` and the clip records covers
  every record present in the ten EMFs in this corpus (35 distinct types total).
- Emit one `Primitive::Shape` per fill/stroke operation, with `fill: Paint::Solid` from
  the selected brush and `stroke` from the selected pen.
- WMF (one finding, cisco `image62.wmf`) is a different, smaller record format with a
  placeable header; add it as a second front end feeding the same interpreter, or defer it
  and accept eight of nine.

Do not reach for the `image` crate feature list — no feature there decodes metafiles — and
do not add a bitmap fast path: none of these files contain `STRETCHDIBITS`,
`SETDIBITSTODEVICE`, `BITBLT` or `STRETCHBLT`.

```rust
// crates/pptx-parse/src/drawing.rs, inside parse_graphic_frame
} else if let Some(pic) = data.and_then(|value| value.descendants_named("pic").first().copied()) {
    GraphicFrameData::Ole {
        picture: Box::new(parse_picture(pic, relationships, part, budget)?),
    }
} else {

// crates/ooxml-metafile/src/lib.rs — shape of the new crate
pub struct MetafileDrawing {
    pub commands: Vec<(Vec<GeometryPathCommand>, Option<Paint>, Option<Stroke>)>,
}
pub fn decode(bytes: &[u8], content_type: &str) -> Option<MetafileDrawing>;

// crates/pptx-render/src/layout.rs, ShapeKind::Picture / ShapeNode::Picture arms
match self.metafiles.get(media_part_path) {
    Some(drawing) => self.push_metafile(base, rect, transform, drawing), // Primitive::Shape per op
    None => self.primitives.push(Primitive::Image { .. }),               // unchanged
}
```

Risks and tests to add:

- **Contract growth.** The display list has no clip primitive, so `INTERSECTCLIPRECT`,
  `SELECTCLIPPATH` and `EXTSELECTCLIPRGN` (23 records in this corpus) either get dropped —
  risking ink escaping the picture frame — or force a new field on `Primitive::Shape` and a
  matching change in `packages/pptx/src/types.ts` and
  `packages/pptx/src/render/canvas.ts`. Bumping `CONTRACT_VERSION` touches every consumer.
- **Primitive count.** rollout-plan `image7.emf` alone contains 116 fill/stroke groups; a
  slide with several EMFs could add thousands of primitives. Budget it the way charts are
  budgeted (`chart_budget`, [`crates/pptx-render/src/layout.rs:620`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L620)), and treat a metafile
  that blows the budget as today's skip.
- **Fuzz surface.** A record interpreter reading offsets and counts out of untrusted bytes
  needs the same defensive posture as the existing decode budgets in
  [`crates/pptx-raster/src/lib.rs:733`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L733)-`765`, plus a fuzz target.
- **`skipped_images` semantics.** Once metafiles route through layout, they stop passing
  through `ImageCache::decode`, so `skipped_images` no longer counts them; the golden
  assertion at [`crates/pptx-raster/tests/golden.rs:31`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L31) and
  [`crates/pptx-raster/README.md:62`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/README.md#L62)-`64` need updating.
- Tests to add: unit tests per record group in the new crate against small hand-built
  EMFs; a `pptx-raster` golden that paints one small committed EMF end to end; a parse test
  for the OLE fallback shape asserting the frame yields a picture, not an `Unknown`.

**How to verify**

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
- Existing coverage to extend: [`crates/pptx-raster/tests/golden.rs:31`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/tests/golden.rs#L31) already asserts
  `skipped_images == 0` for every golden scenario, so a golden that paints a small EMF
  becomes a regression test for free. `a_missing_asset_is_skipped_and_counted`
  ([`crates/pptx-raster/src/lib.rs:854`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L854)) is the matching negative case and must keep passing
  for genuinely undecodable bytes.

**Additional context**

none.

Related issues found in the same run: `fill-alpha-modifier-ignored`

Files most likely involved: `crates/pptx-raster/Cargo.toml`, `crates/pptx-raster/src/lib.rs`, `crates/pptx-raster/README.md`, `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-render/src/display_list.rs`, `packages/pptx/src/types.ts`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/picture-fill-fails-to-render/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
