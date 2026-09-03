---
id: text-run-props-spc-ignored
title: Parse and apply the run-level spc (character tracking) attribute
category: text-run-props
impact: medium
effort: medium
confidence: high
status: open
occurrences: 5
decks: [flat-chart, green-solutions, project20, stacked-bar]
findings: [flat-chart/01/1, flat-chart/02/1, green-solutions/01/3, project20/11/3, stacked-bar/04/6]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/model.rs, crates/pptx-parse/src/write.rs, crates/pptx-edit/src/model.rs, crates/pptx-edit/src/story.rs, crates/pptx-edit/src/save.rs, crates/pptx-render/src/layout.rs]
---

## Symptom

Runs that carry `spc` on `a:rPr` are drawn with the font's natural advances and no tracking at
all. Positive values are the common case in these decks: `spc="300"` (+3pt) and `spc="600"`
(+6pt) produce the wide, evenly tracked caps the reference shows, and the candidate collapses
them to ordinary kerning (evidence-1.png, evidence-2.png).

Because tracking changes measured text width, the damage is not only decorative. In
flat-chart/01 the title measures narrow enough to stay on one line where the reference wraps it
to `ENTER TITLE` / `HERE`, and the two body paragraphs come out at 4 and 3 lines against the
reference's 5 and 5 (evidence-1.png). Every line break below a tracked run is therefore wrong,
not just the letter gaps.

One finding is on the chart text path rather than the shape text path: the `INDUSTRY TRENDS`
chart title in stacked-bar/04 carries `spc="300"` and renders untracked (evidence-4.png). That
path never sees run properties at all, so it needs separate work.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | flat-chart/01 | `spc="300"` on the title, the `CREATIVE VENUS` subtitle and the body copy: the reference wraps the title to two lines and each paragraph to 5 lines; the candidate fits the title on one line and packs the paragraphs into 4 and 3 |
| 2 | green-solutions/01 | `sz="3200" spc="600"` on `OUR GREEN INITIATIVES`: 685px of ink in the reference against 505px in the candidate, both centred on the same axis |
| 3 | project20/11 | `sz="3600" b="1" spc="-100"` on the header. The candidate is missing both the weight and the tracking; the weight (text-run-props-bold-ignored) dominates, so the negative tracking is **not** independently visible here |
| 4 | stacked-bar/04 | `spc="300"` on the chart title run: tracked caps in the reference, plain in the candidate |

## Root cause (confirmed)

`spc` is never parsed. `parse_run_properties` (`crates/pptx-parse/src/drawing.rs:900`) reads
`sz`, `b`, `i`, `u`, `a:latin/@typeface`, `a:solidFill`, `lang` and `a:hlinkClick` and nothing
else, and `RunProperties` (`crates/pptx-parse/src/model.rs:338`) has no field it could land in.
Grepping the whole pptx pipeline — `crates/pptx-parse`, `crates/pptx-render`,
`crates/pptx-raster`, `crates/pptx-edit`, `crates/pptx-wasm` — for `spc` returns only
`lnSpcReduction` (`crates/pptx-parse/src/drawing.rs:802`). The attribute is not parsed and
dropped; it is never read.

The value would have to survive four more hops to reach the shaper, and none of them has a slot
for it either:

- `RunProperties` to `TextStyle` in `style_from_properties`
  (`crates/pptx-render/src/layout.rs:1849`) and `style_from_run_properties`
  (`crates/pptx-edit/src/story.rs:643`). `TextStyle` (`crates/pptx-edit/src/model.rs:36`) carries
  six fields, none of them spacing. Both render entry points funnel through it: the snapshot path
  via `content_from_story` (`crates/pptx-render/src/layout.rs:862`) and the inherited
  layout/master path via `content_from_body` (`crates/pptx-render/src/layout.rs:884`).
- placeholder and `lstStyle` inheritance merges through `merge_run_properties`
  (`crates/pptx-render/src/layout.rs:1825`), which copies field by field.
- `resolve_style` (`crates/pptx-render/src/layout.rs:1010`) produces `ResolvedStyle`
  (`crates/pptx-render/src/layout.rs:924`) — family, size, bold, italic, underline, colour.

Shaping then asks the font for advances and adds nothing:

```rust
let shaped = shape(fonts, run.style.face.id, text, size_px, &[]);  // layout.rs:1383
// ...
    glyph_x += glyph.x_advance;                                    // layout.rs:1416
// ...
    width: glyph_x.max(0.0),                                       // layout.rs:1423
```

`ShapedCluster::width` (`crates/pptx-render/src/layout.rs:1266`) is the single quantity every
downstream decision reads, which is why the bug is both a glyph-gap bug and a wrap bug:

- wrapping — `wrap_clusters` compares `line_width + cluster.width` against the box
  (`crates/pptx-render/src/layout.rs:1444`);
- line width and alignment — summed at `crates/pptx-render/src/layout.rs:1224`;
- caret stops — `crates/pptx-render/src/layout.rs:1237`;
- glyph placement — `crates/pptx-render/src/layout.rs:1519-1520`.

The raster side needs no change: `crates/pptx-raster/src/font.rs:45` paints the positions layout
already computed ("Nothing is shaped here"), so correcting the cluster advance moves the pixels.

Measured, one deck: in green-solutions/01 the tracked reference title and the untracked candidate
title are both centred at x=639.5 in a 1280px slide, on a full-width `algn="ctr"` shape. If
LibreOffice counted a trailing gap after the last character it would sit ~4px left of the
candidate at `spc="600"`. It does not, so the line width is `n-1` gaps — the same convention
`ooxml-text` already uses in `span_width` (`crates/ooxml-text/src/measure/line_filler.rs:877`).
That crate's measure API has `letter_spacing` (`crates/ooxml-text/src/measure/input.rs:226`) and
docx uses it (`crates/docx-layout/src/measure_blocks.rs:507`), but pptx-render calls the
low-level `shape` directly and so has to apply tracking itself.

Two things the fix will run into, both confirmed by reading:

- **Chart text (stacked-bar/04/6) is a different path.** `chart_text_primitive`
  (`crates/pptx-render/src/layout.rs:1077`) shapes from `ChartText`
  (`crates/pptx-render/src/chart.rs:21`), whose only styling is `PlotFont`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:65`: weight, size, family, italic). No run
  property of any kind reaches chart titles, axis labels or legends. Threading `spc` into the
  shape text path leaves this finding unfixed.
- **The write path currently relies on `spc` being unmodeled.** `apply_run_properties`
  (`crates/pptx-parse/src/write.rs:1520`) is documented as leaving "what the model does not carry
  (hyperlinks, strike, spacing, language) in place" (`crates/pptx-parse/src/write.rs:1518`), and
  every attribute it does model is removed when the field is `None`. `run_write`
  (`crates/pptx-edit/src/save.rs:384`) rebuilds `RunProperties` from `TextStyle`, so a spacing
  field added to `RunProperties` but not to `TextStyle` would silently strip `spc` from every
  saved deck.

Not confirmed:

- Whether tracking should be scaled by the `normAutofit` factor that `layout.rs` already applies
  to font size (`points_to_px(run.style.font_size_pt * scale)`). `spc` is an absolute point
  measure, so scaling it with the text is the plausible reading, but no deck here combines
  `normAutofit` with `spc`, so nothing in this evidence settles it.
- project20/11/3. `spc="-100"` is present on that run, and the code above proves it is ignored,
  but the same run also loses its `b="1"` (text-run-props-bold-ignored), which changes the string
  width far more than 1pt of tracking does. The negative-tracking direction is asserted from the
  XML and the code, not from evidence-3.png.

## Verification

Re-render and re-diff the four decks:

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py flat-chart
.venv/bin/python render-improvement-harness/scripts/diff.py flat-chart
```

flat-chart is the cleanest signal: slides 01 (`fine_pct` 3.51) and 02 (3.29) are almost entirely
this defect, and both title and body line counts must match the reference afterwards — a fix that
spaces the glyphs but not the wrap will leave the paragraph diff untouched. green-solutions/01
(74.52) is dominated by the missing background photo and connectors, so expect only the title band
to change; the check there is the measurement in evidence-2.png — the candidate's ink should grow
from 505px to roughly the reference's 685px and stay centred at 639.5. project20/11 (4.50) should
improve slightly. stacked-bar/04 (14.46) will not move unless the chart text path is done too.

No existing test covers this. `crates/pptx-render/src/layout.rs:2008` is the place for unit
assertions — that a cluster's width grows by the tracking, that a line that fits untracked wraps
when tracked, and that a centred tracked line keeps the untracked line's centre. The raster
goldens (`crates/pptx-raster/tests/golden/text.png`) would lock the painted result down.
`crates/pptx-parse/src/drawing.rs:961` already has an rPr round-trip fixture to extend with `spc`,
and the write path needs an edit-and-save test asserting `spc` survives (see the risk above).
