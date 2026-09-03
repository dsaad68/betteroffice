# Possible solution: text-overflow-autofit-not-handled

## Approach

Three changes, independent enough to land separately.

**1. Never shrink for `spAutoFit`** — `render_text_box` in `crates/pptx-render/src/layout.rs:687`.
Drop `TextAutofit::Shape` from the `matches!` that guards the shrink loop, so only
`normAutofit` scales the font. `spAutoFit` then behaves like no autofit: the text is laid out at
its authored size and overflows. That is what LibreOffice draws on rollout-plan/06 — the green
placeholder bar keeps its stored height and the second line spills below it — so there is no need
to grow the shape rect as well. Growing the box would also change the shape's fill and outline,
which the reference does not do.

**2. Let text paint outside its box** — `crates/pptx-raster/src/lib.rs:282-297`. Stop narrowing
the clip to the text box; pass the inherited `clip` (the group or chart clip, or `None`) straight
to `font::paint_lines`. Keep the off-surface cull, but compute it from the laid-out line extents
rather than the box, so a wholly off-slide text box still costs nothing. The `overflow` flag on
the primitive (`crates/pptx-render/src/display_list.rs:130`) needs no new plumbing under this
approach; it stays a hint for the editor UI.

Mirror it in `packages/pptx/src/render/canvas.ts:217-222`: drop the `beginPath`/`rect`/`clip`
from `paintTextBox`. Note `paintPrimitive` wraps each primitive in `ctx.save()`/`ctx.restore()`,
so removing the clip there does not leak state.

**3. Let a centred or bottom-anchored overflow spill symmetrically** —
`crates/pptx-render/src/layout.rs:710-714`. Remove the `.max(0.0)` from the centre and bottom
shifts so an overflowing box centres its text on the box instead of pinning it to the top.

## Sketch

```rust
// layout.rs — 1. only normAutofit shrinks
if matches!(autofit, Some(TextAutofit::Normal { .. })) {
    while laid_out.total_height > content_rect.h && scale > MIN_AUTOFIT_SCALE { /* unchanged */ }
}

// layout.rs — 3. overflow spills both ways
let vertical_shift = match anchor {
    TextAnchor::Top => 0.0,
    TextAnchor::Center => (content_rect.h - laid_out.total_height) / 2.0,
    TextAnchor::Bottom => content_rect.h - laid_out.total_height,
};
```

```rust
// pptx-raster/src/lib.rs — 2. text is not clipped to its own box
Primitive::TextBox { lines, .. } => {
    if lines.is_empty() || !self.lines_on_surface(lines, transform) {
        return Ok(());
    }
    font::paint_lines(self.pixmap, self.resources, self.glyphs, lines, transform, clip)
}
```

## Risks

- Overflowing text now paints over whatever sits below the box. That is correct, and it is what
  the reference does, but it will move pixels on slides the harness currently calls `match`, so
  every deck needs a re-render pass, not just the thirteen findings' slides.
- Change 1 makes `spAutoFit` boxes render larger text than before. On decks where the stored
  `cy` is stale relative to the text, more overflow appears — again matching the reference.
- Change 3 shifts every centred text box whose text overflows, including ones no finding names.
- `crates/pptx-raster/tests/golden.rs` `text.png` will need regenerating if its fixture overflows;
  `packages/pptx/src/render/canvas.test.ts:113,205` assert that `clip` is called and must be
  updated to assert it is *not* called for a text box (the chart clip assertion stays).
- Add: a layout unit test next to
  `normal_autofit_scales_text_until_the_shape_height_is_respected`
  (`crates/pptx-render/src/layout.rs:2239`) that sets `TextAutofit::Shape` and asserts the font
  size is unchanged and `overflow` is true; a second asserting a centred overflowing box places
  its first line above `rect.y`; and a raster golden with a text box taller than its shape.

## Effort

Medium — the three edits are small and local, but they are cross-crate (render, raster, web
canvas) and they move pixels on effectively every deck, so the cost is in re-baselining goldens
and re-running the harness rather than in the change itself.
