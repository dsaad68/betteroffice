# pptx: Run-level character spacing (spc) ignored

**Describe the bug**

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

Seen on 5 slides across 4 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. flat-chart/01** `spc="300"` on the title, the `CREATIVE VENUS` subtitle and the body copy: the reference wraps the title to two lines and each paragraph to 5 lines; the candidate fits the title on one line and packs the paragraphs into 4 and 3

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-spc-ignored/evidence-1.png)

**2. green-solutions/01** `sz="3200" spc="600"` on `OUR GREEN INITIATIVES`: 685px of ink in the reference against 505px in the candidate, both centred on the same axis

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-spc-ignored/evidence-2.png)

**3. project20/11** `sz="3600" b="1" spc="-100"` on the header. The candidate is missing both the weight and the tracking; the weight (text-run-props-bold-ignored) dominates, so the negative tracking is **not** independently visible here

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-spc-ignored/evidence-3.png)

**4. stacked-bar/04** `spc="300"` on the chart title run: tracked caps in the reference, plain in the candidate

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-spc-ignored/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `The Secret of Creating Custom Chart Design in Microsoft PowerPoint PPT  Flat Chart Design.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 2
- `Unique Way To Showcase Your Green Solutions in Microsoft PowerPoint (PPT).pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 11
- `Stacked Bar Graph That Will Impress Your Clients  Microsoft PowerPoint (PPT) Tutorial.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 4

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(10, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

`spc` is never parsed. `parse_run_properties` ([`crates/pptx-parse/src/drawing.rs:900`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L900)) reads
`sz`, `b`, `i`, `u`, `a:latin/@typeface`, `a:solidFill`, `lang` and `a:hlinkClick` and nothing
else, and `RunProperties` ([`crates/pptx-parse/src/model.rs:338`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L338)) has no field it could land in.
Grepping the whole pptx pipeline — `crates/pptx-parse`, `crates/pptx-render`,
`crates/pptx-raster`, `crates/pptx-edit`, `crates/pptx-wasm` — for `spc` returns only
`lnSpcReduction` ([`crates/pptx-parse/src/drawing.rs:802`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L802)). The attribute is not parsed and
dropped; it is never read.

The value would have to survive four more hops to reach the shaper, and none of them has a slot
for it either:

- `RunProperties` to `TextStyle` in `style_from_properties`
  ([`crates/pptx-render/src/layout.rs:1849`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1849)) and `style_from_run_properties`
  ([`crates/pptx-edit/src/story.rs:643`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L643)). `TextStyle` ([`crates/pptx-edit/src/model.rs:36`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L36)) carries
  six fields, none of them spacing. Both render entry points funnel through it: the snapshot path
  via `content_from_story` ([`crates/pptx-render/src/layout.rs:862`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L862)) and the inherited
  layout/master path via `content_from_body` ([`crates/pptx-render/src/layout.rs:884`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L884)).
- placeholder and `lstStyle` inheritance merges through `merge_run_properties`
  ([`crates/pptx-render/src/layout.rs:1825`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1825)), which copies field by field.
- `resolve_style` ([`crates/pptx-render/src/layout.rs:1010`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1010)) produces `ResolvedStyle`
  ([`crates/pptx-render/src/layout.rs:924`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L924)) — family, size, bold, italic, underline, colour.

Shaping then asks the font for advances and adds nothing:

```rust
let shaped = shape(fonts, run.style.face.id, text, size_px, &[]);  // layout.rs:1383
// ...
    glyph_x += glyph.x_advance;                                    // layout.rs:1416
// ...
    width: glyph_x.max(0.0),                                       // layout.rs:1423
```

`ShapedCluster::width` ([`crates/pptx-render/src/layout.rs:1266`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1266)) is the single quantity every
downstream decision reads, which is why the bug is both a glyph-gap bug and a wrap bug:

- wrapping — `wrap_clusters` compares `line_width + cluster.width` against the box
  ([`crates/pptx-render/src/layout.rs:1444`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1444));
- line width and alignment — summed at [`crates/pptx-render/src/layout.rs:1224`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1224);
- caret stops — [`crates/pptx-render/src/layout.rs:1237`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1237);
- glyph placement — [`crates/pptx-render/src/layout.rs:1519-1520`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1519-L1520).

The raster side needs no change: [`crates/pptx-raster/src/font.rs:45`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/font.rs#L45) paints the positions layout
already computed ("Nothing is shaped here"), so correcting the cluster advance moves the pixels.

Measured, one deck: in green-solutions/01 the tracked reference title and the untracked candidate
title are both centred at x=639.5 in a 1280px slide, on a full-width `algn="ctr"` shape. If
LibreOffice counted a trailing gap after the last character it would sit ~4px left of the
candidate at `spc="600"`. It does not, so the line width is `n-1` gaps — the same convention
`ooxml-text` already uses in `span_width` ([`crates/ooxml-text/src/measure/line_filler.rs:877`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/measure/line_filler.rs#L877)).
That crate's measure API has `letter_spacing` ([`crates/ooxml-text/src/measure/input.rs:226`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-text/src/measure/input.rs#L226)) and
docx uses it ([`crates/docx-layout/src/measure_blocks.rs:507`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-layout/src/measure_blocks.rs#L507)), but pptx-render calls the
low-level `shape` directly and so has to apply tracking itself.

Two things the fix will run into, both confirmed by reading:

- **Chart text (stacked-bar/04/6) is a different path.** `chart_text_primitive`
  ([`crates/pptx-render/src/layout.rs:1077`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1077)) shapes from `ChartText`
  ([`crates/pptx-render/src/chart.rs:21`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L21)), whose only styling is `PlotFont`
  ([`crates/ooxml-drawingml/src/chart/geometry.rs:65`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L65): weight, size, family, italic). No run
  property of any kind reaches chart titles, axis labels or legends. Threading `spc` into the
  shape text path leaves this finding unfixed.
- **The write path currently relies on `spc` being unmodeled.** `apply_run_properties`
  ([`crates/pptx-parse/src/write.rs:1520`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1520)) is documented as leaving "what the model does not carry
  (hyperlinks, strike, spacing, language) in place" ([`crates/pptx-parse/src/write.rs:1518`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1518)), and
  every attribute it does model is removed when the field is `None`. `run_write`
  ([`crates/pptx-edit/src/save.rs:384`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L384)) rebuilds `RunProperties` from `TextStyle`, so a spacing
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

**Suggested fix**

Thread one optional value, tracking in points, from the XML to the cluster advance. Every
consumer downstream of `ShapedCluster::width` — wrap, alignment, caret stops, glyph x — already
reads that one number, so nothing else in layout has to learn about tracking.

1. **Parse.** Add `spacing_pt: Option<f64>` to `RunProperties`
   ([`crates/pptx-parse/src/model.rs:338`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L338)) and read `spc` in `parse_run_properties`
   ([`crates/pptx-parse/src/drawing.rs:900`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L900)), dividing by 100 like `sz` does and rejecting
   non-finite or absurd values. Because `parse_run_properties` also serves `defRPr`
   ([`crates/pptx-parse/src/drawing.rs:877`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L877)), `spc` on `lstStyle` / layout / master defaults comes
   along for free.
2. **Keep the write path honest.** Add the matching arm to `apply_run_properties`
   ([`crates/pptx-parse/src/write.rs:1520`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1520)) and `run_properties_element`
   ([`crates/pptx-parse/src/write.rs:1667`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1667)), and update the docstring at
   [`crates/pptx-parse/src/write.rs:1518`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1518) that currently promises spacing is left alone. This is
   not optional: `apply_run_properties` removes any modeled attribute whose field is `None`.
3. **Carry it through the snapshot.** Add the field to `TextStyle`
   ([`crates/pptx-edit/src/model.rs:36`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L36)) and `TextStylePatch`, populate it in
   `style_from_run_properties` ([`crates/pptx-edit/src/story.rs:643`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L643)), `style_from_attrs`
   ([`crates/pptx-edit/src/story.rs:654`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L654), a Yjs `Any::Number` attr) and `run_write`
   ([`crates/pptx-edit/src/save.rs:384`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L384)), and in `style_from_properties`
   ([`crates/pptx-render/src/layout.rs:1849`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1849)). Add the merge arm in `merge_run_properties`
   ([`crates/pptx-render/src/layout.rs:1825`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1825)).
4. **Resolve and apply.** Add `tracking_px` to `ResolvedStyle`
   ([`crates/pptx-render/src/layout.rs:924`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L924)), resolved direct-then-fallback in `resolve_style`
   ([`crates/pptx-render/src/layout.rs:1010`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1010)) and converted with the same `scale` the font size
   gets. In `add_shaped_segment` ([`crates/pptx-render/src/layout.rs:1365`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1365)) add it to each
   cluster's width, and keep it on the cluster so the trailing gap can be removed when a line's
   width is summed ([`crates/pptx-render/src/layout.rs:1224`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1224)) — the green-solutions measurement in
   the report says the reference counts `n-1` gaps, not `n`.
5. **Do not paint the gap.** `positioned_runs` advances the pen by `cluster.width`
   ([`crates/pptx-render/src/layout.rs:1520`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1520)) while glyph offsets inside a cluster stay relative,
   so the extra space lands after the cluster, which is what tracking means. No raster change:
   `crates/pptx-raster/src/font.rs` paints supplied positions.

Two follow-ups that this change does not cover and should be scoped separately:

- stacked-bar/04/6 needs `PlotFont` ([`crates/ooxml-drawingml/src/chart/geometry.rs:65`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L65)) or
  `ChartText` ([`crates/pptx-render/src/chart.rs:21`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L21)) to carry run properties before
  `chart_text_primitive` ([`crates/pptx-render/src/layout.rs:1077`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1077)) can track chart titles.
- the browser backend paints a run as one `fillText` call
  ([`packages/pptx/src/render/canvas.ts:237`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L237)), so it will keep drawing untracked text unless
  `PositionedTextRun` ([`crates/pptx-render/src/display_list.rs:230`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L230)) gains a `letterSpacingPx`
  the canvas sets on `ctx.letterSpacing`. Until then raster and canvas diverge on these slides.

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

Risks and tests to add:

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
  [`crates/pptx-parse/src/drawing.rs:961`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L961)); cluster width, wrap-point and centred-line assertions
  in the [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008) test module.

**How to verify**

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

No existing test covers this. [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2008) is the place for unit
assertions — that a cluster's width grows by the tracking, that a line that fits untracked wraps
when tracked, and that a centred tracked line keeps the untracked line's centre. The raster
goldens (`crates/pptx-raster/tests/golden/text.png`) would lock the painted result down.
[`crates/pptx-parse/src/drawing.rs:961`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L961) already has an rPr round-trip fixture to extend with `spc`,
and the write path needs an edit-and-save test asserting `spc` survives (see the risk above).

**Additional context**

none.

Related issues found in the same run: `text-run-props-bold-ignored`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/write.rs`, `crates/pptx-edit/src/model.rs`, `crates/pptx-edit/src/story.rs`, `crates/pptx-edit/src/save.rs`, `crates/pptx-render/src/layout.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-spc-ignored/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
