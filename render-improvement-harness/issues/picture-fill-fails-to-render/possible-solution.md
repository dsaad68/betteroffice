# Possible solution: picture-fill-fails-to-render

## Approach

Two independent changes. Part A is small and unlocks two findings' shapes; Part B is the
real work and unlocks all nine metafile findings.

### Part A — parse the OLE fallback picture (medium)

`p:graphicFrame` with `graphicData uri=".../presentationml/2006/ole"` wraps an
`mc:AlternateContent` whose `mc:Fallback/p:oleObj` contains a complete `p:pic`. That
picture is exactly what PowerPoint and LibreOffice paint for a non-activated OLE object.

- In `parse_graphic_frame` (`crates/pptx-parse/src/drawing.rs:192`), before the
  `Unknown` arm, look for a `pic` descendant under `graphicData` and, when found, run the
  existing `parse_picture` on it. Store it as a new `GraphicFrameData::Ole { picture:
  Box<Picture> }` in `crates/pptx-parse/src/model.rs:243`.
- In `render_graphic_frame` (`crates/pptx-render/src/layout.rs:593`), add an arm that
  pushes the same `Primitive::Image` the `ShapeNode::Picture` arm builds
  (`crates/pptx-render/src/layout.rs:534`), using the frame's own rect rather than the
  nested `p:pic`'s `a:xfrm` — the graphic frame is authoritative for placement.
- Give `parse_shape_children` (`crates/pptx-parse/src/drawing.rs:101`) an
  `"AlternateContent"` arm that recurses into `mc:Fallback` (then `mc:Choice`), so
  fallback-wrapped shapes are not dropped outside graphic frames either.
- On its own this changes nothing visually for project17 — those fallbacks are EMFs — but
  it removes the spurious dashed placeholder and makes those slides depend only on Part B.

### Part B — a metafile interpreter that emits display-list primitives (hard)

Decoding must not live in `pptx-raster`: that crate never compiles for wasm
(`crates/pptx-raster/README.md:38`-`42`) and the browser path goes through a host
`CanvasImageResolver` (`packages/pptx/src/render/canvas.ts:206`) that cannot handle
metafile bytes either. Put the interpreter in a new wasm-safe crate (say
`ooxml-metafile`) that turns EMF/WMF bytes into the primitives the contract already has,
and call it from the layout pass so both backends get the artwork for free.

`Primitive::Shape` is a good target: its `path` is `Vec<GeometryPathCommand>` in
0..1 coordinates relative to the shape frame (`crates/pptx-raster/src/lib.rs:575`-`600`),
which is exactly what an EMF's window/viewport mapping normalises to.

- Detect metafiles by media content type, which `MediaPart` already carries
  (`crates/pptx-parse/src/package.rs:167`), rather than by sniffing bytes in the backend.
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

## Sketch

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

## Risks

- **Contract growth.** The display list has no clip primitive, so `INTERSECTCLIPRECT`,
  `SELECTCLIPPATH` and `EXTSELECTCLIPRGN` (23 records in this corpus) either get dropped —
  risking ink escaping the picture frame — or force a new field on `Primitive::Shape` and a
  matching change in `packages/pptx/src/types.ts` and
  `packages/pptx/src/render/canvas.ts`. Bumping `CONTRACT_VERSION` touches every consumer.
- **Primitive count.** rollout-plan `image7.emf` alone contains 116 fill/stroke groups; a
  slide with several EMFs could add thousands of primitives. Budget it the way charts are
  budgeted (`chart_budget`, `crates/pptx-render/src/layout.rs:620`), and treat a metafile
  that blows the budget as today's skip.
- **Fuzz surface.** A record interpreter reading offsets and counts out of untrusted bytes
  needs the same defensive posture as the existing decode budgets in
  `crates/pptx-raster/src/lib.rs:733`-`765`, plus a fuzz target.
- **`skipped_images` semantics.** Once metafiles route through layout, they stop passing
  through `ImageCache::decode`, so `skipped_images` no longer counts them; the golden
  assertion at `crates/pptx-raster/tests/golden.rs:31` and
  `crates/pptx-raster/README.md:62`-`64` need updating.
- Tests to add: unit tests per record group in the new crate against small hand-built
  EMFs; a `pptx-raster` golden that paints one small committed EMF end to end; a parse test
  for the OLE fallback shape asserting the frame yields a picture, not an `Unknown`.

## Effort

**hard.** Part A is an afternoon, but Part B is a new crate implementing a GDI record
interpreter with an object table, DC stack and path bracket, plus a probable display-list
contract change for clipping — and the artwork is genuine vector content with no embedded
bitmap to shortcut to.
