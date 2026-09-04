# pptx: Run-level gradFill (white-to-white) not resolved

**Describe the bug**

Text runs whose `a:rPr` carries an `a:gradFill` instead of an `a:solidFill` render in the theme's
default body-text color rather than the gradient's color. Every occurrence in these two decks is
the same authoring pattern: a degenerate two-stop gradient with both stops `FFFFFF`, i.e. solid
white, used for label text sitting on a saturated fill. Because the fallback color resolves to
`dk1` (`#505050` in both decks' themes), the text turns dark-gray-on-red, dark-gray-on-blue and
dark-gray-on-purple - barely legible (evidence-1.png, evidence-2.png, evidence-3.png,
evidence-4.png).

Seen on 9 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/01** the red "Please work on noted slides only." callout: white in the reference, `#505050` gray in the candidate

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-gradfill-not-resolved/evidence-1.png)

**2. project20/05** the "Solutions" heading on its `accent1` blue band, gray instead of white

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-gradfill-not-resolved/evidence-2.png)

**3. project20/07** the full-height red instructional callout, every line gray instead of white

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-gradfill-not-resolved/evidence-3.png)

**4. rollout-plan/08** the three column headers "Business Lead" / "Business Contact" / "Team Member" on purple, green and blue bands, all gray instead of white

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-gradfill-not-resolved/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 5, 7, 9, 11
- `rollout-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 4, 5, 8

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(1, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Run-level fill is parsed as *solid fill only*. `parse_run_properties` reads exactly one fill
element:

- [`crates/pptx-parse/src/drawing.rs:917`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L917) - `color: element.child("solidFill").and_then(parse_color_container)`

There is no `gradFill` branch anywhere on the run path, so for these runs `RunProperties.color`
([`crates/pptx-parse/src/model.rs:344`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L344)) stays `None`. The gradient itself is never read: the
`gradFill` handling that does exist ([`crates/pptx-parse/src/drawing.rs:576`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L576), and
`parse_gradient_fill` at [`crates/pptx-parse/src/drawing.rs:585`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L585)) sits inside `parse_fill`
([`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565)), which is only reached for shape `spPr` and background
fills. Its `ShapeFill` result would have nowhere to live on a run anyway - `RunProperties` carries
a bare `Option<ColorValue>`, not a fill.

Downstream, the `None` propagates unchanged. `style_from_run_properties`
([`crates/pptx-edit/src/story.rs:643`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L643)) maps `properties.color` straight onto the snapshot's
`TextStyle.color`, and `resolve_style` ([`crates/pptx-render/src/layout.rs:1042`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1042)) then falls back to
the inherited `defRPr` color before its final `"#000000"` default. For `project20` slide 1 that
inherited value is the master's `otherStyle` `lvl1pPr/defRPr`
`<a:solidFill><a:schemeClr val="tx1"/></a:solidFill>`, which the deck's `clrMap` sends to `dk1` =
`505050` - exactly the gray sampled in every slide report of this cluster. Confirmed against the
extracted XML: `decks/project20/xml/01/master.xml` carries that `defRPr`, and
`decks/project20/xml/01/theme.xml` has `<a:dk1><a:srgbClr val="505050"/></a:dk1>`.

The property the findings cite is reachable on that exact path - it is a direct child of `a:rPr`
(`decks/project20/xml/01/slide.xml`):

```xml
<a:rPr lang="en-US" sz="2400">
  <a:gradFill><a:gsLst>
    <a:gs pos="0"><a:srgbClr val="FFFFFF"/></a:gs>
    <a:gs pos="100000"><a:srgbClr val="FFFFFF"/></a:gs>
  </a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill>
  <a:ea typeface="Segoe UI"/><a:cs typeface="Segoe UI"/>
</a:rPr>
```

All nine findings share that shape: two stops, both `FFFFFF`, `lin ang="5400000" scaled="0"`.

Two secondary observations, both flagged as such:

- The display list cannot express gradient-filled text at all: `TextRun.color`
  ([`crates/pptx-render/src/display_list.rs:204`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L204)) and `PositionedTextRun.color`
  ([`crates/pptx-render/src/display_list.rs:242`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L242)) are plain hex `String`s. A genuinely
  non-degenerate run gradient would still need a flattening approximation after this fix. Every
  occurrence in this cluster is degenerate, so flattening is exact here.
- **Not confirmed / separate gap:** the failing shapes also declare
  `<p:style><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>`, which would
  independently yield white. `fontRef`, `fillRef` and `lnRef` appear nowhere in `pptx-parse`,
  `pptx-render` or `ooxml-drawingml` (grep returns nothing), so the style-matrix fallback is a
  second, unrelated hole. Fixing `gradFill` is sufficient for these nine findings; `fontRef` is
  not part of this issue.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Teach the run-property parser about the fill choices it currently ignores, and flatten a gradient
to the one solid color the rest of the pipeline can carry.

1. In `parse_run_properties` ([`crates/pptx-parse/src/drawing.rs:900`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L900)), when `a:rPr` has no
   `a:solidFill`, look for `a:gradFill` and take a representative stop. Reuse the existing
   `parse_gradient_fill` ([`crates/pptx-parse/src/drawing.rs:585`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L585)) so stop parsing, `pos` clamping
   and color-modifier handling stay in one place, then pick the stop with the lowest `pos`. For the
   degenerate all-stops-equal case that every finding in this cluster uses, that is exact; for a
   real gradient it is the same approximation LibreOffice's text rendering settles on and is
   strictly better than dropping the fill.
2. Keep the flattening lossless on save. `apply_run_properties`
   ([`crates/pptx-parse/src/write.rs:1520`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1520)) currently treats `color.is_some()` as "the model owns the
   fill choice" and rewrites it as `solidFill`. Add a marker so the writer can tell an authored
   `solidFill` from a flattened `gradFill` - the smallest version is a
   `#[serde(skip_serializing_if = ...)] pub color_is_gradient: bool` (or an
   `Option<GradientFill>` field, if the round-trip is expected to keep the stops) on `RunProperties`
   ([`crates/pptx-parse/src/model.rs:338`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L338)), checked before the `FILL_ELEMENTS` strip at
   [`crates/pptx-parse/src/write.rs:1543`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1543): when the color came from a gradient and was not edited,
   leave the `gradFill` element untouched.

Nothing changes in `pptx-edit` or `pptx-render`: `style_from_run_properties`
([`crates/pptx-edit/src/story.rs:643`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/story.rs#L643)) and `resolve_style`
([`crates/pptx-render/src/layout.rs:1042`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1042)) already do the right thing once `color` is populated.

```rust
// crates/pptx-parse/src/drawing.rs, in parse_run_properties
color: element
    .child("solidFill")
    .and_then(parse_color_container)
    .or_else(|| run_gradient_color(element)),

/// A run `gradFill` flattened to its first stop; the display list carries one
/// colour per run, and these gradients are degenerate in practice.
fn run_gradient_color(element: &XmlElement) -> Option<ColorValue> {
    let fill = parse_gradient_fill(element.child("gradFill")?);
    let mut stops = fill.gradient?.stops;
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));
    stops.into_iter().next().map(|stop| stop.color)
}
```

```rust
// crates/pptx-parse/src/write.rs, in apply_run_properties
let removed_fills: &[&str] = match (&properties.color, properties.color_is_gradient) {
    (Some(_), false) => &FILL_ELEMENTS,
    (Some(_), true) => &[],      // flattened gradFill: leave the authored element alone
    (None, _) => &["solidFill"],
};
```

Risks and tests to add:

- **Round-trip regression** is the real hazard: without the writer guard, every `gradFill` on a run
  in a saved deck degrades to a `solidFill`. Add a save-and-reload test asserting the `a:gradFill`
  element and its `gsLst` survive a no-op edit, alongside one asserting an explicit colour edit
  still replaces it.
- **Over-eager flattening.** A run with a genuine two-colour gradient now paints in its first stop
  instead of the theme default. That is a visible change on decks not in this harness, but it is
  the closer approximation in every case, and none of the twelve harness decks contain a
  non-degenerate run gradient.
- Runs carrying `a:noFill` on `rPr` stay unhandled by this change (they would still fall back to
  the inherited colour instead of painting nothing). Out of scope here; worth a follow-up note.
- Tests to extend: `parses_text_formatting_and_nested_shape_types`
  ([`crates/pptx-parse/src/drawing.rs:957`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L957)) for the parse side, and the `pptx-parse` write tests
  around [`crates/pptx-parse/src/write.rs:1543`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1543) for the round-trip.

**How to verify**

Re-render `project20` slides 01, 05, 07, 09, 11 and `rollout-plan` slides 02, 04, 05, 08, then check
that the affected runs sample as `(255, 255, 255)` instead of `(80, 80, 80)`. `project20/09`
(diff 5.25%) and `project20/11` (4.5%) are the cleanest signals because their other findings are
small; `rollout-plan/08` (11.8%) should also drop visibly. `project20/01` (46.33%) will barely move,
since that slide is dominated by the separate `fill-alpha-modifier-ignored` bug.

Unit coverage lives beside the parser: `parses_text_formatting_and_nested_shape_types`
([`crates/pptx-parse/src/drawing.rs:957`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L957)) already asserts on `runs[0].properties`, so extend it - or
add a sibling - with an `rPr` carrying a `gradFill` and assert the resolved `color`. There is no
`crates/pptx-render/tests/` directory; raster coverage would go through
`crates/pptx-raster/tests/golden.rs`.

Round-trip is the one thing that must not regress. `apply_run_properties`
([`crates/pptx-parse/src/write.rs:1520`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1520)) strips every fill element and writes a `solidFill` whenever
`properties.color.is_some()`; the comment at [`crates/pptx-parse/src/write.rs:1542`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1542) states that
clearing the color is what currently lets an unmodeled `gradFill` survive a save. Populating
`color` from a `gradFill` without guarding that branch would silently rewrite authored gradients as
solid fills. A save-and-reload test over a `gradFill` run is required as part of the fix.

**Additional context**

none.

Related issues found in the same run: `fill-alpha-modifier-ignored`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/write.rs`, `crates/pptx-edit/src/story.rs`, `crates/pptx-render/src/layout.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/text-run-props-gradfill-not-resolved/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
