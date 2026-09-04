# pptx: Preset geometry adjust values (chevron/homePlate notch) wrong

**Describe the bug**

On `project20/04` the "Week Ending" ribbon is a row of seven `chevron`/`homePlate` banners whose
bounding boxes deliberately overlap, so each shape's point tucks into the next shape's notch and the
row reads as one continuous blue band. BetterOffice draws every notch roughly twice as deep as it
should be, so each of the six joints opens into a large white bowtie of slide background
(`evidence-1.png`, and at 4x in `evidence-2.png`). The same defect hits the dark-navy phase row
below, where the `homePlate` arrows are wide and short: their points are drawn three to five times
too long, turning stubby banners into long spikes (`evidence-3.png`, `evidence-4.png`).

The error grows with the shape's aspect ratio, which is why the failure is invisible on square-ish
presets and severe on these banner shapes.

Seen on 1 slide across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/04** The whole week-ending chevron ribbon. Reference: hairline seams. Candidate: a wide white gap at every one of the six joints.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-preset-adj-values-wrong/evidence-1.png)

**2. project20/04** 4x on the `April 27`/`May 4` joint (`Arrow: Pentagon 60` id 61 into `Arrow: Chevron 47` id 48) - the clearest single view of the over-deep chevron notch.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-preset-adj-values-wrong/evidence-2.png)

**3. project20/04** The dark-navy phase row (`homePlate`, default `adj`); candidate points are visibly several times longer than the reference's.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-preset-adj-values-wrong/evidence-3.png)

**4. project20/04** 4x on `Arrow: Pentagon 46` (id 47, `Content Review`): reference point 19 px, candidate point 67 px.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-preset-adj-values-wrong/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 4

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(3, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed. `preset_geometry_to_path` treats an `adj` guide as a fraction of the shape's *width*;
ECMA-376 defines it as a fraction of the shape's *shortest side* (`ss`).**

1. Parsing is correct and is not the problem. `parse_adjust_values`
   ([`crates/pptx-parse/src/drawing.rs:355`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L355)) evaluates the `prstGeom/avLst` guides and divides a
   plain `val` by `ADJUSTMENT_SCALE`, so `<a:gd name="adj" fmla="val 47414"/>` reaches the model as
   `0.47414` ([`crates/pptx-parse/src/drawing.rs:377-386`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L377-L386)). `standard_guide_values`
   ([`crates/pptx-parse/src/drawing.rs:393`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L393)) even seeds a correct `ss = min(w, h)`
   ([`crates/pptx-parse/src/drawing.rs:394`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L394), [`crates/pptx-parse/src/drawing.rs:398`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L398)) for guides that
   reference it. Nothing is dropped.

2. The value is then consumed as a width fraction. The chevron arm
   ([`crates/ooxml-drawingml/src/geometry.rs:199-209`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L199-L209)) builds
   `polygon(&[(0,0), (1-adj,0), (1,0.5), (1-adj,1), (0,1), (adj,0.5)])`. The polygon *topology* is
   exactly the ECMA `chevron` path (`moveTo 0,0 -> x2,0 -> r,vc -> x2,b -> 0,b -> x1,vc`), but the
   x coordinates emitted here are fractions of the shape's width - [`crates/pptx-raster/src/lib.rs:575`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L575)
   and its `px()` helper multiply x by `w`. The spec's `x1` is `*/ ss a 100000`, i.e. `adj` times the
   *shortest side*. For `Arrow: Chevron 47` (`w/h = 3.08`) that alone is a 3.08x overshoot, before
   the default-value error below.

3. `homePlate` ignores `adj` entirely. [`crates/ooxml-drawingml/src/geometry.rs:210`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L210) is a constant
   `polygon(&[(0,0), (0.75,0), (1,0.5), (0.75,1), (0,1)])`, and
   `preset_geometry_default_adjustments` ([`crates/ooxml-drawingml/src/geometry.rs:8-30`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L8-L30)) has no
   `homePlate` arm at all, so the explicit `<a:gd name="adj" fmla="val 47414"/>` on
   `Arrow: Pentagon 60` (id 61) is parsed, carried through the model, and then discarded at the
   geometry table. Every `homePlate` on the slide gets the same hardcoded `w * 0.25` point.

4. The `chevron` default is also wrong. [`crates/ooxml-drawingml/src/geometry.rs:19`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L19) returns
   `("adj", 0.35)`; the ECMA-376 `chevron` preset defines `<gd name="adj" fmla="val 50000"/>`, i.e.
   `0.5`. This is a second, independent error stacked on top of (2) - on `Arrow: Chevron 47` the two
   partly cancel (`0.35 * w` = 60.0 px against the correct `0.5 * ss` = 27.8 px, a 2.16x overshoot
   rather than 3.08x), which is why the gaps are large but not catastrophic.

5. `.min(0.5)` at [`crates/ooxml-drawingml/src/geometry.rs:200`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L200) is the wrong clamp for the same
   reason: ECMA pins `a` to `[0, maxAdj]` where `maxAdj` is itself `ss`-relative, so a legitimate
   `adj` above `0.5` on a wide shape is silently truncated today. **Not confirmed**: no deck in the
   corpus carries a `chevron` or `homePlate` with `adj > 50000`, and I have no local copy of
   `presetShapeDefinitions.xml`, so the exact `maxAdj` numerator (I believe `*/ 100000 w ss`, giving
   `dx <= w`, for both presets) must be read off the spec before the clamp is rewritten.

6. One fix covers every surface. Both `pptx-render` call sites already pass the aspect ratio
   ([`crates/pptx-render/src/layout.rs:410-414`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L410-L414), [`crates/pptx-render/src/layout.rs:516-520`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L516-L520), and the
   composed path at [`crates/pptx-render/src/lib.rs:170`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L170), all funnelling into the two `geometry_path`
   helpers at [`crates/pptx-render/src/layout.rs:1946`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1946) and [`crates/pptx-render/src/lib.rs:237`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L237)), so
   `preset_geometry_to_path` already has the `w/h` it needs and nothing above it has to change.
   There is no second preset table under `packages/` - the web canvas consumes the same command
   list - so raster and canvas are fixed together.

Secondary, outside this cluster's evidence but caused by the same table: `pptx-edit` seeds a new
preset shape's adjust values from `preset_geometry_default_adjustments`
([`crates/pptx-edit/src/deck.rs:412`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L412)) and merges them into every parsed shape's document state
([`crates/pptx-edit/src/deck.rs:126-130`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L126-L130)), and `set_adjust_values`
([`crates/pptx-parse/src/write.rs:1225`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1225)) writes them back as `val <value * 100000>`. A chevron
authored in the editor therefore lands in the file as `val 35000` instead of `val 50000`. Existing
shapes are safe - [`crates/pptx-edit/src/save.rs:214`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L214) diffs two snapshots that both carry the
injected default, so no spurious patch is produced - but the wrong default does escape into newly
authored XML.

Same class, sharing the width-vs-`ss` basis error and worth checking in the same pass (**not
confirmed - no finding in the corpus backs them**): `parallelogram`
([`crates/ooxml-drawingml/src/geometry.rs:104`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L104)), `trapezoid`
([`crates/ooxml-drawingml/src/geometry.rs:108`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L108)), `hexagon`
([`crates/ooxml-drawingml/src/geometry.rs:113`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L113)) and `octagon`
([`crates/ooxml-drawingml/src/geometry.rs:125`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L125)) all use `adj` as a plain width (and, for `octagon`,
height) fraction, and all four are `ss`-relative in ECMA-376.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

One file: `crates/ooxml-drawingml/src/geometry.rs`. Nothing above it changes - both `pptx-render`
`geometry_path` helpers ([`crates/pptx-render/src/layout.rs:1946`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1946),
[`crates/pptx-render/src/lib.rs:237`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L237)) and `docx-parse`
([`crates/docx-parse/src/drawingml.rs:273`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L273)) already hand `preset_geometry_to_path` the aspect ratio,
and every consumer below it draws the normalised command list as-is.

Three edits:

1. **Add an `ss`-relative helper** beside `clamp_fraction`
   ([`crates/ooxml-drawingml/src/geometry.rs:328`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L328)) that converts an `adj` fraction into the
   width-fraction and height-fraction units the command list actually speaks. With
   `aspect_ratio = w / h`, `ss / w = 1 / max(aspect_ratio, 1)` and `ss / h = min(aspect_ratio, 1)`.
   Guard a non-finite or non-positive ratio the way `rounded_rect`
   ([`crates/ooxml-drawingml/src/geometry.rs:231-236`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L231-L236)) already does.

2. **Rewrite the `chevron` arm** ([`crates/ooxml-drawingml/src/geometry.rs:199-209`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L199-L209)) to run its
   `adj` through that helper, and fix its default to `0.5`
   ([`crates/ooxml-drawingml/src/geometry.rs:19`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L19)). Keep the existing point order - the topology
   already matches the ECMA path.

3. **Give `homePlate` a real arm** ([`crates/ooxml-drawingml/src/geometry.rs:210`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L210)) reading `adj`
   through the same helper, plus a `("adj", 0.5)` entry in
   `preset_geometry_default_adjustments` ([`crates/ooxml-drawingml/src/geometry.rs:8-30`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L8-L30)) so the
   editor exposes and round-trips the right value.

Replace `.min(0.5)` with the spec's `pin 0 adj maxAdj`, expressed in the same units, once the
`maxAdj` numerator has been read off `presetShapeDefinitions.xml`. If that read is deferred, clamp
the resulting *width fraction* to `0.5` for `chevron` and `1.0` for `homePlate` rather than
clamping the raw `adj` - the current clamp truncates legal wide-shape values.

Optional, same pass: `parallelogram` ([`crates/ooxml-drawingml/src/geometry.rs:104`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L104)), `trapezoid`
(`:108`), `hexagon` (`:113`) and `octagon` (`:125`) share the basis error and can reuse the helper.
Their spec defaults should be re-read at the same time - the report flags them as unverified.

```rust
/// `adj` is a fraction of the shortest side; the path speaks fractions of w and h.
fn short_side_fractions(adjustment: f64, aspect_ratio: f64) -> (f64, f64) {
    let ratio = if aspect_ratio.is_finite() && aspect_ratio > 0.0 { aspect_ratio } else { 1.0 };
    (adjustment / ratio.max(1.0), adjustment * ratio.min(1.0))
}

// defaults
"chevron" | "homePlate" => vec![("adj", 0.5)],

"chevron" => {
    let adj = clamp_fraction(adjustments.get("adj").copied(), 0.5);
    let (dx, _) = short_side_fractions(adj, aspect_ratio);
    let dx = dx.min(0.5);
    polygon(&[(0.0, 0.0), (1.0 - dx, 0.0), (1.0, 0.5), (1.0 - dx, 1.0), (0.0, 1.0), (dx, 0.5)])
}
"homePlate" => {
    let adj = clamp_fraction(adjustments.get("adj").copied(), 0.5);
    let (dx, _) = short_side_fractions(adj, aspect_ratio);
    let dx = dx.min(1.0);
    polygon(&[(0.0, 0.0), (1.0 - dx, 0.0), (1.0, 0.5), (1.0 - dx, 1.0), (0.0, 1.0)])
}
```

Risks and tests to add:

- **`docx-parse` loses the `ss` basis.** `parse_preset_geometry_path`
  ([`crates/docx-parse/src/drawingml.rs:255-274`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L255-L274)) evaluates guides over a fixed 100000 x 100000 box
  ([`crates/docx-parse/src/drawingml.rs:260`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L260)), so any DOCX shape whose `adj` is written as a formula
  over `ss` already resolves against a square. Literal `val` guides - the common case, and the only
  case in this corpus - are unaffected because they carry no extent. Adding a DOCX chevron/homePlate
  case to `crates/docx-parse` tests is cheap insurance.
- **`clamp_fraction`'s dual scale** ([`crates/ooxml-drawingml/src/geometry.rs:328-334`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L328-L334)) rescales any
  value above `1.0` by `/ 100_000`, which is what lets `docx-parse` pass raw 0..100000 units. That
  makes a legitimate `adj` of, say, `120000` (`1.2` after `pptx-parse` normalisation) collapse to
  `1.2e-5`. Out of scope here, but the same clamp rewrite is the place to fix it if the `maxAdj` pin
  is done properly.
- **Editor round-trip.** Changing the `chevron` default and adding a `homePlate` default changes
  what [`crates/pptx-edit/src/deck.rs:126-130`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L126-L130) seeds into document state and therefore what
  [`crates/pptx-parse/src/write.rs:1225`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1225) can write back. It only makes newly authored XML correct
  (`val 50000` rather than `val 35000`), and existing shapes still diff clean against their own
  baseline ([`crates/pptx-edit/src/save.rs:214`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L214)), but `crates/pptx-edit` snapshot tests that pin
  `adjustValuesJson` will need updating.
- **No existing test pins chevron or homePlate output**
  ([`crates/ooxml-drawingml/src/geometry.rs:497-561`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L497-L561)), so nothing guards a regression today. Add, in
  that module: a square chevron whose notch is unchanged at `adj = 0.5`; a 3:1 chevron whose notch
  sits at `1/6` of the width, not `0.5`; a wide `homePlate` honouring an explicit `adj`; and a
  `preset_geometry_default_adjustments("chevron") == 0.5` assertion.

**How to verify**

Re-render `project20/04` (`.venv/bin/python render-improvement-harness/scripts/pipeline.py`, or
`render_bo.py` for that deck alone) and compare against `decks/project20/lo-img/04.png`. The
week-ending ribbon must close: the white run at each of the six seams, sampled along the row's
vertical centre (`y = 214` at 1280x720), must fall from ~34 px to the reference's ~3 px, and the
navy phase row's `homePlate` points must shorten from ~67 px to ~19 px on `Arrow: Pentagon 46`. The
slide's `diff_pct` is 8.89 today; this row is one of six defects on the slide, so expect a partial
drop, not a clean pass.

Unit coverage in `crates/ooxml-drawingml/src/geometry.rs` currently stops at `roundRect`,
`parallelogram` defaults and `flowChartTerminator`
([`crates/ooxml-drawingml/src/geometry.rs:497-561`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L497-L561)); there is no `chevron` or `homePlate` test at
all. Add aspect-ratio assertions in that module (a 3.08:1 chevron with the default `adj` must put
its notch at `0.5 / 3.08 = 0.1624` of the width, not `0.35`) - that is the cheapest regression
guard, and it pins the `ss` basis rather than the pixel output.

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/ooxml-drawingml/src/geometry.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-edit/src/deck.rs`, `crates/docx-parse/src/drawingml.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-preset-adj-values-wrong/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
