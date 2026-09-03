# Possible solution: transform-text-orientation-wrong-under-rotation

## Approach

Give the text box its own transform instead of reusing the shape's, and derive it inside
`render_text_box` (`crates/pptx-render/src/layout.rs:657`) so both call sites — the snapshot path
at `crates/pptx-render/src/layout.rs:460` and the parsed path at
`crates/pptx-render/src/layout.rs:565` — are fixed at once. Nothing downstream needs to change:
`Primitive::TextBox` already carries a per-primitive `transform`, and `pptx-raster`
(`crates/pptx-raster/src/lib.rs:242`), the canvas backend
(`packages/pptx/src/render/canvas.ts:69`) and hit testing all read it from the primitive.

Two pieces:

1. **Un-mirror.** A shape transform with an odd number of flips is a reflection; text must not be
   reflected. Cancelling it with a local horizontal flip collapses to a pure rotation:
   `text_rotation = rot + if flip_v { 180 } else { 0 }`, with `flip_h`/`flip_v` cleared. Applies to
   all four flip combinations (see the table in `report.md`).

2. **`bodyPr/@vert`.** Add a `vertical()` accessor to `BodyCascade`
   (`crates/pptx-render/src/layout.rs:769`) alongside `anchor()`, reading
   `TextBody::vertical` (`crates/pptx-parse/src/model.rs:271`, already parsed at
   `crates/pptx-parse/src/drawing.rs:779`). For `vert` add +90° and for `vert270` add −90° to the
   text rotation, and lay the text out in the shape box with width and height swapped about the
   shape centre — the box the glyphs occupy before that extra quarter turn. Treat `horz` and the
   unhandled East-Asian values (`eaVert`, `mongolianVert`, `wordArtVert*`) as horizontal.

`HitRegion` (`crates/pptx-render/src/layout.rs:1641`) needs the text transform and the swapped
text rect stored next to the shape's, so `local_point` (`crates/pptx-render/src/layout.rs:1651`)
maps caret hits through the transform the glyphs were actually painted with rather than the
shape's.

## Sketch

```rust
// layout.rs — inside render_text_box, replacing the passed-through `transform`

#[derive(Clone, Copy)]
enum TextFlow { Horizontal, Vert, Vert270 }

fn text_flow(vertical: Option<&str>) -> TextFlow {
    match vertical {
        Some("vert") => TextFlow::Vert,
        Some("vert270") => TextFlow::Vert270,
        _ => TextFlow::Horizontal,
    }
}

let flow = text_flow(cascade.vertical());
// glyphs are rotated with the shape but never mirrored: fold the reflection away
let mut text_rotation = transform.rotation_deg + if transform.flip_v { 180.0 } else { 0.0 };
text_rotation += match flow {
    TextFlow::Vert => 90.0,
    TextFlow::Vert270 => -90.0,
    TextFlow::Horizontal => 0.0,
};
let text_transform = Transform {
    rotation_deg: text_rotation.rem_euclid(360.0),
    flip_h: false,
    flip_v: false,
};

// vertical flow lays out in the shape box turned on its side, about the same centre
let text_rect = match flow {
    TextFlow::Horizontal => rect,
    _ => PxRect {
        x: rect.x + (rect.w - rect.h) / 2.0,
        y: rect.y + (rect.h - rect.w) / 2.0,
        w: rect.h,
        h: rect.w,
    },
};
// ... content_rect, layout_content, anchor shift and Primitive::TextBox all use
// `text_rect` and `text_transform` from here on; TextHit carries them out for HitRegion.
```

## Risks

- `hit_testing_flipped_text_reads_the_mirrored_caret`
  (`crates/pptx-render/src/layout.rs:2070`) asserts the current, wrong behaviour and must be
  rewritten: a flipped shape no longer reverses its text's caret mapping. The test builds its
  `HitRegion` by hand, so it will keep compiling and silently keep asserting the bug unless it is
  updated deliberately.
- Shape hit testing must keep using the shape transform. Splitting the two transforms on
  `HitRegion` is the part most likely to introduce a regression; the rect-membership test
  `hit_testing_a_rotated_shape_follows_its_painted_frame`
  (`crates/pptx-render/src/layout.rs:2026`) guards it.
- The swapped text rect changes wrapping and autofit input for every `vert` shape, so
  `normAutofit`/`spAutoFit` shrink loops (`crates/pptx-render/src/layout.rs:687`) now run against
  the rotated extent. That is correct, but it moves text on any deck that previously overflowed.
- Insets (`lIns`/`tIns`/`rIns`/`bIns`) are expressed in the text body's own frame, so under `vert`
  they rotate with it. The sketch applies them after the swap, which is the intended reading;
  worth an explicit test since slide 21's bars use asymmetric insets
  (`lIns="91440" tIns="45720" rIns="91440" bIns="91440"`).
- Charts (`Primitive::Chart`) and placeholders keep the shape transform — only `TextBox` changes.

Tests to add: a `pptx-render` unit test pinning `text rotation = rot + 180·flip_v` across the four
flip combinations; a `pptx-raster` golden with a rotated-and-flipped text box next to
`golden_rotated` (`crates/pptx-raster/tests/golden.rs:306`); and a layout test that a `vert270`
shape with `cx << cy` lays its text out on one line.

## Effort

Medium. The un-mirror half is a handful of lines in one function and needs no new plumbing, but
the `vert` half adds a cascade accessor, a swapped layout box that feeds wrapping and autofit, and
a second transform on `HitRegion` that has to be threaded through hit testing and its tests.
