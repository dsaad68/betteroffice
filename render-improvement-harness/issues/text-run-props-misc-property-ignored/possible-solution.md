# Possible solution: text-run-props-misc-property-ignored

## Approach

Two halves, one of which is somebody else's change.

**Italic — do nothing here.** Take `text-run-props-bold-ignored`'s style-aware fallback chain in
`resolve_face` (`crates/pptx-render/src/layout.rs:245`). Its step "the same four style keys against
`fallback_family`" resolves `(fallback family, false, true)` and hands these runs Liberation Sans
Italic. Adding a second, italic-only fix would collide with it.

**Superscript — thread one `Option<f64>` from the XML to the glyph `y`.** The value is a
percentage of the font size, positive for superscript and negative for subscript, so one field
covers both. It changes two things at layout time: the run shapes at a reduced size, and its
glyphs are offset off the line's baseline.

1. **Parse.** Add `baseline_pct: Option<f64>` to `RunProperties`
   (`crates/pptx-parse/src/model.rs:338`) and read `baseline` in `parse_run_properties`
   (`crates/pptx-parse/src/drawing.rs:900`), dividing by 1000 like `sz` divides by 100, and
   rejecting non-finite values. `parse_run_properties` also serves `defRPr`
   (`crates/pptx-parse/src/drawing.rs:879`), so `lstStyle`/layout/master defaults come along free.
2. **Keep the write path honest.** Add the arms to `apply_run_properties`
   (`crates/pptx-parse/src/write.rs:1520`) and `run_properties_element`
   (`crates/pptx-parse/src/write.rs:1667`). `apply_run_properties` deletes every *modeled*
   attribute whose field is `None`, so this is not optional: `baseline` survives a round-trip
   today only because it is unmodeled. Its docstring at
   `crates/pptx-parse/src/write.rs:1518` lists what the model does not carry and needs updating.
3. **Carry it through the snapshot.** Add the field to `TextStyle`
   (`crates/pptx-edit/src/model.rs:36`) and `TextStylePatch`
   (`crates/pptx-edit/src/model.rs:47`), and populate it in `style_from_run_properties`
   (`crates/pptx-edit/src/story.rs:643`), `style_from_attrs`
   (`crates/pptx-edit/src/story.rs:654`, a Yjs `Any::Number`), `run_write`
   (`crates/pptx-edit/src/save.rs:384`), `style_from_properties`
   (`crates/pptx-render/src/layout.rs:1849`) and `merge_run_properties`
   (`crates/pptx-render/src/layout.rs:1825`).
4. **Resolve into pixels.** Add `baseline_shift_px` (and an effective size) to `ResolvedStyle`
   (`crates/pptx-render/src/layout.rs:924`), computed in `resolve_style`
   (`crates/pptx-render/src/layout.rs:1010`) direct-then-fallback. Shrink the size there, once, so
   every consumer of `style.font_size_pt` — shaping, line box, the display list — agrees without
   knowing why the run is small.
5. **Offset the glyphs.** `positioned_runs` (`crates/pptx-render/src/layout.rs:1472`) already
   writes `y_offset: baseline + glyph.y_offset` at `:1516`; subtract the run's shift there. Two
   companion edits: the coalescing test at `crates/pptx-render/src/layout.rs:1484` must include the
   shift in its key, or the marker keeps merging into the sentence run (it does today); and
   `clusters_line_box` (`crates/pptx-render/src/layout.rs:1525`) must fold the shift into ascent
   and descent the way `crates/ooxml-text/src/measure/line_filler.rs:547` does, so a tall
   superscript grows the line instead of clipping.
6. **Tell the canvas.** `crates/pptx-raster/src/font.rs` needs no change — it paints the absolute
   `glyph.y_offset` it is handed. The browser backend does: it draws a whole run with one
   `fillText` at `line.baseline` (`packages/pptx/src/render/canvas.ts:237`), so
   `PositionedTextRun` (`crates/pptx-render/src/display_list.rs:230`) needs a
   `baselineOffsetPx` that `paintTextRun` subtracts. Without it raster and canvas diverge on
   exactly these runs. `underline` is drawn from `line.baseline` in both backends
   (`crates/pptx-raster/src/font.rs:110`, `packages/pptx/src/render/canvas.ts:239`) and should use
   the shifted baseline too.

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs, in parse_run_properties
baseline_pct: element
    .attribute("baseline")
    .and_then(|value| value.parse::<f64>().ok())
    .filter(|value| value.is_finite() && value.abs() <= 100_000.0)
    .map(|value| value / 1000.0),

// crates/pptx-render/src/layout.rs, in resolve_style
let baseline_pct = direct
    .baseline_pct
    .or_else(|| fallback.and_then(|value| value.baseline_pct))
    .filter(|value| value.is_finite())
    .unwrap_or(0.0) as f32;
let scripted = baseline_pct != 0.0;
let font_size_pt = if scripted { font_size_pt * SCRIPT_SIZE_RATIO } else { font_size_pt };
let baseline_shift_px = points_to_px(font_size_pt) * baseline_pct / 100.0;

// crates/pptx-render/src/layout.rs, in positioned_runs
let append = output.last().is_some_and(|run| {
    run.end == cluster.start
        && run.font_id == cluster.style.face.id.to_u32()
        && run.baseline_offset_px == cluster.style.baseline_shift_px   // new
});
// ...
y_offset: baseline - cluster.style.baseline_shift_px + glyph.y_offset,
```

`SCRIPT_SIZE_RATIO` is the one judgement call. The reference's marker measures at roughly half the
base em (report, "Root cause"); LibreOffice's own default is 0.58; the repo's docx path uses 0.75
(`crates/ooxml-text/src/measure/prepare.rs:466`). 0.58 matches the evidence in hand, but the
number that matters is PowerPoint's, which this investigation did not establish — pick it by
rendering a PowerPoint-authored superscript, not by reading.

Whether the shift is a percentage of the *original* or the *reduced* size is a second small
choice; the sketch uses the reduced size, and at 30000 the two differ by ~2px at 12pt.

## Risks

- **Silent `baseline` loss on save.** `apply_run_properties` removes any modeled attribute left
  `None`. `baseline` round-trips safely today precisely because it is unmodeled; modeling it
  without step 2 deletes superscripts from user files. An edit-and-save round-trip test over
  cisco-cloud-security slide 5 is the guard.
- **Wrap points move.** A shrunk run advances less, so a line holding a superscript can gain
  characters. Only runs carrying a non-zero `baseline` are affected, and `baseline="0"` on
  `defRPr` must stay a strict no-op — assert that, or every deck with a default `lstStyle` shifts.
- **Line growth.** Folding the shift into ascent (step 5) makes a superscript-bearing line taller
  than its neighbours, which is correct but moves everything below it in a `spAutoFit` box. If
  ascent is left alone instead, a large `baseline` clips at the top of the text box.
- **Subscript is untested.** The same field carries `baseline="-25000"`, and neither deck in this
  cluster has one. Either implement both and test the negative case synthetically, or reject
  negatives explicitly rather than shipping an untried path.
- **Golden churn.** Existing pptx goldens have no `baseline`, so they must not move; that is the
  regression check. A superscript fixture added to `crates/pptx-raster/tests/golden.rs` pins the
  new behaviour.
- Tests to add: `baseline` parse and round-trip in `crates/pptx-parse` (extend the `rPr` fixture at
  `crates/pptx-parse/src/drawing.rs:956`); run-size, glyph-`y` and non-coalescing assertions in the
  `crates/pptx-render/src/layout.rs:2008` test module; a `baseline="0"` no-op case.

## Effort

Medium. The behaviour is three lines of arithmetic, but it is an `Option<f64>` threaded through
five crates and the TypeScript backend, and it crosses the edit and save path where getting it
wrong deletes the attribute from user files. The italic half is free: it needs no code of its own,
only `text-run-props-bold-ignored`.
