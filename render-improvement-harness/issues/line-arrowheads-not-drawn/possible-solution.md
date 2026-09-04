# Possible solution: line-arrowheads-not-drawn

## Approach

Carry the two ends onto the display-list stroke and draw them in the backends.

1. Add an optional end description to `Stroke` in `crates/pptx-render/src/display_list.rs`,
   defaulted and skipped when absent so the contract stays additive and existing output is
   unchanged.
2. Populate it in `stroke()` from `ShapeOutline::head_end` and `tail_end`, ignoring
   `type="none"` so the common case adds nothing.
3. In `crates/pptx-raster/src/lib.rs`, after stroking the path, build the arrow as a filled
   triangle at the first and last point of the path, oriented along that segment, scaled from
   the stroke width by the `width` and `length` attributes.
4. Mirror the field in `packages/pptx/src/types.ts` and draw the same triangle in
   `packages/pptx/src/render/canvas.ts`.

## Sketch

```rust
// display_list.rs
pub struct Stroke {
    pub color: String,
    pub width: f32,
    pub dashed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_end: Option<LineEndMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_end: Option<LineEndMark>,
}
```

## Risks

- Only `triangle`, `arrow`, `oval` and `stealth` appear in the sample decks. Anything else should
  fall through to no mark rather than guess.
- The mark must be drawn in the shape's own space, so it inherits the transform. A rotated or
  flipped connector otherwise points the wrong way.
- Nothing renders until `line-zero-extent-skipped` merges, since every observable case is a
  connector. Do not read a flat diff as failure.

## Effort

medium - a display-list contract addition plus geometry in two backends, and the geometry has to
be right under rotation.
