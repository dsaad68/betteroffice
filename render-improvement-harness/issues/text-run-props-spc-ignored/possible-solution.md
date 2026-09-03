# Possible solution: text-run-props-spc-ignored

## Approach

Thread one optional value, tracking in points, from the XML to the cluster advance. Every
consumer downstream of `ShapedCluster::width` — wrap, alignment, caret stops, glyph x — already
reads that one number, so nothing else in layout has to learn about tracking.

1. **Parse.** Add `spacing_pt: Option<f64>` to `RunProperties`
   (`crates/pptx-parse/src/model.rs:338`) and read `spc` in `parse_run_properties`
   (`crates/pptx-parse/src/drawing.rs:900`), dividing by 100 like `sz` does and rejecting
   non-finite or absurd values. Because `parse_run_properties` also serves `defRPr`
   (`crates/pptx-parse/src/drawing.rs:877`), `spc` on `lstStyle` / layout / master defaults comes
   along for free.
2. **Keep the write path honest.** Add the matching arm to `apply_run_properties`
   (`crates/pptx-parse/src/write.rs:1520`) and `run_properties_element`
   (`crates/pptx-parse/src/write.rs:1667`), and update the docstring at
   `crates/pptx-parse/src/write.rs:1518` that currently promises spacing is left alone. This is
   not optional: `apply_run_properties` removes any modeled attribute whose field is `None`.
3. **Carry it through the snapshot.** Add the field to `TextStyle`
   (`crates/pptx-edit/src/model.rs:36`) and `TextStylePatch`, populate it in
   `style_from_run_properties` (`crates/pptx-edit/src/story.rs:643`), `style_from_attrs`
   (`crates/pptx-edit/src/story.rs:654`, a Yjs `Any::Number` attr) and `run_write`
   (`crates/pptx-edit/src/save.rs:384`), and in `style_from_properties`
   (`crates/pptx-render/src/layout.rs:1849`). Add the merge arm in `merge_run_properties`
   (`crates/pptx-render/src/layout.rs:1825`).
4. **Resolve and apply.** Add `tracking_px` to `ResolvedStyle`
   (`crates/pptx-render/src/layout.rs:924`), resolved direct-then-fallback in `resolve_style`
   (`crates/pptx-render/src/layout.rs:1010`) and converted with the same `scale` the font size
   gets. In `add_shaped_segment` (`crates/pptx-render/src/layout.rs:1365`) add it to each
   cluster's width, and keep it on the cluster so the trailing gap can be removed when a line's
   width is summed (`crates/pptx-render/src/layout.rs:1224`) — the green-solutions measurement in
   the report says the reference counts `n-1` gaps, not `n`.
5. **Do not paint the gap.** `positioned_runs` advances the pen by `cluster.width`
   (`crates/pptx-render/src/layout.rs:1520`) while glyph offsets inside a cluster stay relative,
   so the extra space lands after the cluster, which is what tracking means. No raster change:
   `crates/pptx-raster/src/font.rs` paints supplied positions.

Two follow-ups that this change does not cover and should be scoped separately:

- stacked-bar/04/6 needs `PlotFont` (`crates/ooxml-drawingml/src/chart/geometry.rs:65`) or
  `ChartText` (`crates/pptx-render/src/chart.rs:21`) to carry run properties before
  `chart_text_primitive` (`crates/pptx-render/src/layout.rs:1077`) can track chart titles.
- the browser backend paints a run as one `fillText` call
  (`packages/pptx/src/render/canvas.ts:237`), so it will keep drawing untracked text unless
  `PositionedTextRun` (`crates/pptx-render/src/display_list.rs:230`) gains a `letterSpacingPx`
  the canvas sets on `ctx.letterSpacing`. Until then raster and canvas diverge on these slides.

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs, in parse_run_properties
spacing_pt: element
    .attribute("spc")
    .and_then(|value| value.parse::<f64>().ok())
    .filter(|value| value.is_finite() && value.abs() <= 400_000.0)
    .map(|value| value / 100.0),

// crates/pptx-render/src/layout.rs, in resolve_style
let tracking_pt = direct
    .spacing_pt
    .or_else(|| fallback.and_then(|value| value.spacing_pt))
    .filter(|value| value.is_finite())
    .unwrap_or(0.0) as f32;

// crates/pptx-render/src/layout.rs, in add_shaped_segment
let tracking = points_to_px(run.style.tracking_pt * scale);
// ...
output.push(ShapedCluster {
    width: (glyph_x + tracking).max(0.0),
    tracking,
    // ...
});

// crates/pptx-render/src/layout.rs, where a line's width is summed (~1224)
let line_width = slice.iter().map(|cluster| cluster.width).sum::<f32>()
    - slice.last().map_or(0.0, |cluster| cluster.tracking);
```

## Risks

- **Silent `spc` loss on save.** Modeling the attribute without populating it everywhere makes
  `apply_run_properties` delete it. An edit-and-save round-trip test over a deck with `spc` is
  the guard.
- **Negative tracking.** `spc="-100"` must not drive a cluster width below zero; the
  `.max(0.0)` above clamps the cluster but a large negative value on a narrow glyph will still
  compress the line. project20/11 is the repro, though its bold defect masks the effect today.
- **Trailing-gap convention.** Counting `n` gaps instead of `n-1` shifts every centred and
  right-aligned tracked line by half a gap (4px at `spc="600"`). The green-solutions title is a
  direct check: reference and candidate ink must both centre at x=639.5.
- **Autofit interaction.** Scaling tracking by the `normAutofit` factor is a judgement call the
  evidence does not settle; if it turns out wrong, the fix is one multiplication.
- **Wrap changes are the point, and they move every downstream line.** Slides whose text
  currently happens to fit will start wrapping. Expect golden churn in
  `crates/pptx-raster/tests/golden` if a tracked run is added to a fixture; the existing goldens
  have no `spc` and must not move.
- Tests to add: `spc` parse and round-trip in `crates/pptx-parse` (extend the rPr fixture at
  `crates/pptx-parse/src/drawing.rs:961`); cluster width, wrap-point and centred-line assertions
  in the `crates/pptx-render/src/layout.rs:2008` test module.

## Effort

Medium. The shape-text half is a mechanical `Option<f64>` threaded through five crates with one
real behaviour change in `add_shaped_segment`, but it crosses the edit and save paths (where
getting it wrong deletes `spc` from user files) and it leaves the chart-title finding for a
separate change to the chart text path.
