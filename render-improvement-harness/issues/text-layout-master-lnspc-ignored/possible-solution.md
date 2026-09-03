# Possible solution: text-layout-master-lnspc-ignored

## Approach

Four changes, in dependency order. The first three are the `lnSpc` plumb; the fourth is the
anchor clamp, which has to land with them or the two-line titles still overflow.

**Model the spacing.** Add `line_spacing: Option<LineSpacing>` to `ParagraphProperties`
(`crates/pptx-parse/src/model.rs:303-312`) with
`enum LineSpacing { Percent(f64), Points(f64) }`, mirroring how `TextAutofit`
(`crates/pptx-parse/src/model.rs:286-292`) is shaped. `ParagraphProperties` is
`Serialize`/`Deserialize` and only re-exported by `crates/betteroffice-pptx/src/types.rs:9`, so
the field is additive — nothing in `pptx-edit` or `pptx-parse/src/write.rs` reads it.

**Parse it.** In `parse_paragraph_properties`
(`crates/pptx-parse/src/drawing.rs:849-881`), read `lnSpc`'s single child: `spcPct val` through
the existing `percentage_attribute` helper (`crates/pptx-parse/src/drawing.rs:806-813`), `spcPts
val` as hundredths of a point. This is the only edit needed on the parse side — `titleStyle`,
`bodyStyle`, `otherStyle`, layout `lstStyle` and slide `pPr` all funnel through this one
function via `parse_style_levels` (`crates/pptx-parse/src/drawing.rs:78`). Add `space_before` /
`space_after` here too if the adjacent gap is in scope; the cascade below carries them for free.

**Cascade and apply it.** Extend `merge_paragraph_properties`
(`crates/pptx-render/src/layout.rs:1804-1822`) with the same `is_some()` override the other five
fields use, add `line_spacing: Option<LineSpacing>` to `ResolvedParagraph`
(`crates/pptx-render/src/layout.rs:910-915`), populate it at
`crates/pptx-render/src/layout.rs:994-1004` next to `margin_left_px`, and consume it in
`layout_paragraph`: run every `clusters_line_box` / `style_line_box` result
(`crates/pptx-render/src/layout.rs:1525-1562`) through a `spaced_line_box` helper before the
line's `height`, `baseline` and the `line_y +=` advance at
`crates/pptx-render/src/layout.rs:1261` are taken from it. The helper does what the measurements
say LibreOffice does:

- `Percent(p)`: target height = `p * 1.2 * font_size_px` — the 1.2 constant is PowerPoint's, not
  the font's 1.1499 em, and is what reproduces the reference's 41 px at 32 pt / 80 %.
- `Points(pt)`: target height = `points_to_px(pt)`.
- Then redistribute: if `target < ascent + descent`, scale both by `target / (ascent + descent)`
  and zero the leading; otherwise keep ascent and descent and put the slack in `leading`. That is
  literally the `Auto` arm of `apply_spacing_rule`
  (`crates/ooxml-text/src/word_metrics.rs:260-281`), so express it as
  `apply_spacing_rule(content, &LineSpacingRule::Auto { line_240ths: (240.0 * target / content.height()).round() as u32 })`
  rather than reimplementing the redistribution, or add a `Scaled { target_px }` arm to
  `LineSpacingRule` if the 240ths round-trip loses too much precision at small sizes.

Everything downstream already carries per-line geometry: `PositionedTextLine`
(`crates/pptx-render/src/display_list.rs:209-219`) has explicit `height` and `baseline`, and
`packages/pptx/src/render/canvas.ts:224` paints from `line.baseline`, so the display-list
contract does not change.

**Unclamp the anchor.** Drop the `.max(0.0)` from the `Center` and `Bottom` arms at
`crates/pptx-render/src/layout.rs:710-714` so an overflowing block centres about the box instead
of pinning to the top inset. `overflow` (`crates/pptx-render/src/layout.rs:739`) still reports
the spill, and `shift_line` (`crates/pptx-render/src/layout.rs:1565`) already handles a negative
`y`.

## Sketch

```rust
// pptx-parse/src/drawing.rs, in parse_paragraph_properties
line_spacing: element.child("lnSpc").and_then(|spacing| {
    if let Some(pct) = spacing.child("spcPct") {
        percentage_attribute(pct, "val").map(LineSpacing::Percent)
    } else {
        spacing
            .child("spcPts")
            .and_then(|pts| numeric_attribute(Some(pts), "val"))
            .map(|value| LineSpacing::Points(value as f64 / 100.0))
    }
}),

// pptx-render/src/layout.rs, in layout_paragraph, replacing the bare line box
let content = clusters_line_box(fonts, slice, scale)?;
let line_box = spaced_line_box(content, paragraph.line_spacing, primary_size_px(slice, scale));
// ... height: line_box.height(), baseline: line_y + line_box.ascent, line_y += line_box.height()

fn spaced_line_box(content: LineBox, spacing: Option<LineSpacing>, size_px: f32) -> LineBox {
    let target = match spacing {
        Some(LineSpacing::Percent(p)) => p as f32 * PPT_PERCENT_LINE_EM * size_px, // 1.2
        Some(LineSpacing::Points(pt)) => points_to_px(pt as f32),
        None => return content,
    };
    if content.height() <= 0.0 { return content; }
    apply_spacing_rule(content, &LineSpacingRule::Auto {
        line_240ths: ((240.0 * target / content.height()).round() as u32).max(1),
    })
}

// pptx-render/src/layout.rs:710
TextAnchor::Center => (content_rect.h - laid_out.total_height) / 2.0,
TextAnchor::Bottom => content_rect.h - laid_out.total_height,
```

## Risks

- **This moves text on every deck.** All twelve harness decks carry a sub-100 % `lnSpc`
  somewhere (90 % almost everywhere, plus 80 % and 105 % in `project17`), and the anchor unclamp
  touches every overflowing `anchor="ctr"`/`"b"` shape whether or not it has an `lnSpc`. Re-run
  the whole harness, not just `cisco-cloud-security`, and treat unrelated `diff_pct` regressions
  as blocking.
- **The 1.2 constant is calibrated, not sourced.** It reproduces the reference to within a pixel
  on seven slides at one size (32 pt) in one font, and the alternative reading (percentage of the
  font's own line height) is off by 1.8 px there. Add a fixture at a second size and a second
  font before trusting it; if the two readings disagree on those, the constant is wrong.
- **`normAutofit lnSpcReduction` becomes reachable.** It is already parsed into
  `TextAutofit::Normal` (`crates/pptx-parse/src/model.rs:286-292`,
  `crates/pptx-parse/src/drawing.rs:802`) and, like `fontScale`
  (`crates/pptx-render/src/layout.rs:681-685`), ignored. Once a line-height knob exists it is a
  two-line follow-up, but it also interacts with the autofit shrink loop at
  `crates/pptx-render/src/layout.rs:687-698`, which re-lays out at a smaller `scale` — the
  spacing must scale with the font size, not be recomputed from an unscaled one. Feed
  `style.font_size_pt * scale` into `spaced_line_box`, the same value `style_line_box`
  (`crates/pptx-render/src/layout.rs:1558-1561`) already uses.
- **Mixed-size lines** need a rule for which run's size drives `Percent`. PowerPoint uses the
  largest run on the line; `clusters_line_box` (`crates/pptx-render/src/layout.rs:1525-1548`)
  already takes the max ascent/descent/leading, so take the max font size over the same
  deduplicated run set.
- **Hit testing and caret geometry** read `line.y` / `line.height` / `caret_stops`
  (`crates/pptx-render/src/layout.rs:296`), so tighter lines change which line a click lands on
  in `packages/pptx-react`. Nothing needs a code change, but
  `packages/pptx-react/src/interactions.test.ts` hard-codes baselines (`:63`, `:77`) and will
  need its fixtures refreshed if they come from a real deck.
- Tests to add in `crates/pptx-render/src/layout.rs`'s module (`:2008`): a title placeholder
  inheriting `titleStyle` `lnSpc 80%` produces a two-line block whose line pitch is
  `0.8 * 1.2 * font_size_px`; a slide-level `pPr/lnSpc` overrides the master's; `spcPts` gives a
  fixed pitch independent of font size; an `anchor="ctr"` box whose text overflows places the
  block symmetrically (negative shift) while a fitting one is unchanged.

## Effort

Medium — the model/parse/cascade half is mechanical and mirrors `marL` exactly, but the line-box
redistribution needs the percentage base pinned by fixtures, and the anchor unclamp shifts text
in every deck, so the change is small and the verification is not.
