# Possible solution: picture-srcrect-crop-ignored

## Approach

Carry the crop through the four layers that currently drop it, then mask.

1. **Contract.** Add an optional `crop` to `Primitive::Image`
   (`crates/pptx-render/src/display_list.rs:99`) as four `f32` fractions in `0..1`, and the
   matching optional `geometry` + `path` so a picture can be clipped to a preset shape.
   Skip-serialize both so a rect picture with no crop keeps today's JSON and
   `CONTRACT_VERSION` (`crates/pptx-render/src/display_list.rs:5`) does not have to move;
   confirm against the contract fixtures before deciding to leave it at 1. Mirror the
   fields on `ImagePrimitive` (`packages/pptx/src/types.ts:246`).

2. **Parse.** `parse_picture` (`crates/pptx-parse/src/drawing.rs:158`) already stores the
   crop; add `geometry` and `adjust_values` read from `spPr/prstGeom`, reusing whatever
   `parse_shape` does for the same element so preset names and `<a:avLst>` stay consistent.

3. **Snapshot.** Replace the hardcoded `"rect"` at `crates/pptx-edit/src/deck.rs:141` with
   the parsed geometry, seed `adjustValuesJson` the way the `Shape` arm does, and add
   `cropJson`. Add `crop: PictureCrop` to `ShapeSnapshot`
   (`crates/pptx-edit/src/model.rs:99`) and read it back at
   `crates/pptx-edit/src/deck.rs:823`. Round-trip is safe: `save.rs` only serializes newly
   added shapes and rejects non-`Shape` kinds (`crates/pptx-edit/src/save.rs:412`), so no
   writer needs to learn `srcRect`.

4. **Layout.** Both picture arms (`crates/pptx-render/src/layout.rs:425` and
   `crates/pptx-render/src/layout.rs:534`) convert the thousandths-of-a-percent crop to
   fractions and, when the geometry is not `rect`, attach the same
   `geometry_path(...)` (`crates/pptx-render/src/layout.rs:1946`) the shape arm already
   builds. Do the same in the host-composed path
   (`crates/pptx-render/src/lib.rs:48`, `crates/pptx-render/src/lib.rs:200`).

5. **Raster.** In `paint_image` (`crates/pptx-raster/src/lib.rs:391`), scale the fit
   transform by the kept fraction and translate by the discarded left/top, then draw
   through a mask: intersect the existing `clip` with the frame rect (or with the
   geometry path when present) using the same machinery as `clipped`
   (`crates/pptx-raster/src/lib.rs:325`), which is what stops the outsized source spilling
   past the frame.

6. **Canvas.** Switch `packages/pptx/src/render/canvas.ts:208` to the 9-argument
   `drawImage`, guarded by a `ctx.save()`/`clip()`/`restore()` when the primitive carries a
   path. `packages/docx/src/layout/render/canvasBackend.ts:1340` (`drawCroppedImage`) is
   the reference implementation, including its negative-crop clamping.

## Sketch

```rust
// crates/pptx-raster/src/lib.rs, in paint_image
let (kx, ky) = (1.0 - crop.left - crop.right, 1.0 - crop.top - crop.bottom);
if kx <= 0.0 || ky <= 0.0 { return Ok(()); }        // fully cropped away
let (sw, sh) = (source.width() as f32, source.height() as f32);
let fit = Transform::from_row(
    frame.width() / (sw * kx), 0.0,
    0.0, frame.height() / (sh * ky),
    frame.x() - crop.left * sw * frame.width() / (sw * kx),
    frame.y() - crop.top  * sh * frame.height() / (sh * ky),
);
// the source now overhangs the frame, so the blit must be masked
let mask = self.clipped_to(clip, frame, path.as_deref(), transform)?;
self.pixmap.draw_pixmap(0, 0, source.as_ref(), &paint,
                        transform.pre_concat(fit), mask.as_ref());
```

```rust
// crates/pptx-parse/src/drawing.rs, parse_picture
geometry: properties
    .and_then(|value| value.child("prstGeom"))
    .and_then(|value| value.attribute("prst"))
    .unwrap_or("rect")
    .to_owned(),
```

## Risks

- **Masking is now mandatory for pictures.** Today an image is blitted unmasked because it
  exactly fills its frame; after the fix the source overhangs, so a missing mask turns a
  crop into a bleed over neighbouring shapes. Every path that paints an image needs the
  mask, including the rotated case — `Mask` is built in device space, so the frame path
  must be transformed first, exactly as `crates/pptx-raster/src/lib.rs:337` does.
- **Negative `srcRect`** (outset) parses fine into `i32`
  (`crates/pptx-parse/src/drawing.rs:942`) but would make the kept fraction exceed 1 and
  place the source inside the frame with a gap. Clamp, and add a parse test.
- **Divide by zero / degenerate crops** when the four sides sum to 100%; guard before
  computing the fit, as the sketch does.
- **Pictures gaining a real geometry** changes `ShapeSnapshot::geometry` for every picture
  from the constant `"rect"`. Anything asserting that constant, and any host reading the
  snapshot, sees a new value — grep the edit and wasm tests before changing it.
- **Contract churn.** If the fixtures pin exact JSON, the new optional fields need the
  fixtures regenerated even though they are skip-serialized when absent.
- Tests to add: a `pptx-parse` unit test for `PictureCrop` and for a picture's
  `prstGeom`; a cropped and an ellipse-masked golden beside `golden_image`
  (`crates/pptx-raster/tests/golden.rs:283`); a layout assertion that
  `Primitive::Image` carries the crop on both the snapshot and the parsed-shape path
  (master pictures only reach the parsed path).

## Effort

Medium. Each individual step is small, but the crop has to be threaded through five
layers (parse, edit snapshot, display-list contract, raster, TS canvas) plus the
host-composed path, the geometry half is not parsed at all today, and the raster change
introduces a mask where none existed — enough surface that "easy" understates it.
