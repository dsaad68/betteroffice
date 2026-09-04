# pptx: Chart data labels drawn even though show flags are all disabled

**Describe the bug**

Every slide of the `stacked-bar` deck holds the same horizontal stacked bar chart, and every one
of them carries a plot-group `c:dLbls` whose six `show*` switches are all `0`. LibreOffice draws
no labels at all. BetterOffice draws a small numeric value at the end of every bar
(evidence-1.png, evidence-2.png) — the visible digits are the Series 3 values `2 2 3 5`, and the
last one is clipped by the chart frame's right edge.

The labels are not limited to Series 3. `emit_bar` emits one per series per category, so twelve
labels are drawn; the Series 1 and Series 2 labels land on their own segment boundaries with the
baseline at the bar's bottom edge, where the bar itself covers all but the top pixel row of each
digit (evidence-3.png). The defect recurs unchanged on all four slides, which differ only in
accent colour (evidence-4.png).

Seen on 4 slides across 1 deck while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. stacked-bar/01** reference and candidate over the same bar-end column: no labels in LibreOffice, a value past every bar end in BetterOffice

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-dlbls-shown-when-disabled/evidence-1.png)

**2. stacked-bar/01** 4x zoom on the candidate's four visible labels — `2`, `2`, `3`, `5`, the Series 3 values; the bottom one is cut by the frame edge

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-dlbls-shown-when-disabled/evidence-2.png)

**3. stacked-bar/01** 6x zoom on the Category 1 bar: the Series 1 and Series 2 labels are drawn too, at the segment boundaries, all but hidden under the bar

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-dlbls-shown-when-disabled/evidence-3.png)

**4. stacked-bar/04** the same labels on the recoloured copy of the chart, confirming the cluster is one defect four times

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/chart-dlbls-shown-when-disabled/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `Stacked Bar Graph That Will Impress Your Clients  Microsoft PowerPoint (PPT) Tutorial.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 2, 3, 4

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(0, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

The shared chart geometry handles this correctly. `plot_labels_from_model`
([`crates/ooxml-drawingml/src/chart/geometry.rs:594`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L594)) resolves the four-level cascade and, for
this chart, returns `Some(PlotDataLabels)` with every switch `false`; `point_label`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1780`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1780)) then bails out on
`if !spec.shows_anything() { return None; }` and no text op is emitted. Run against the parsed
model alone, the chart would draw no labels.

`pptx-render` overwrites that result afterwards. `plot_model`
([`crates/pptx-render/src/chart.rs:68`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L68)) exists to give a part that carries only the legacy
`show_data_labels` flag the switches Excel would default to, but its guard does not cover a
`c:dLbls` that declares its switches and turns them all off:

```rust
// [`crates/pptx-render/src/chart.rs:70-86`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L70-L86)
for (group, plotted) in space.plot_groups.iter().zip(chart.plot_groups.iter_mut()) {
    if !group.show_data_labels {
        continue;
    }
    for (model, series) in group.series.iter().zip(plotted.series.iter_mut()) {
        let declared = model.data_labels.is_some() || group.data_labels.is_some();
        if declared && series.labels.is_none() {
            continue;
        }
        if series.labels.is_none_or(|labels| !labels.shows_anything()) {
            series.labels = Some(PlotDataLabels {
                show_value: true,
                ..PlotDataLabels::default()
            });
        }
    }
}
```

Traced with this deck's XML — one plot-group `c:dLbls`, no series-level `c:dLbls`, no `c:delete`
(verified on all four chart parts: `decks/stacked-bar/xml/0[1-4]/chart-chart[1-4].xml`):

1. `group.show_data_labels` is `true`. `shows_data_labels`
   ([`crates/ooxml-drawingml/src/chart/parse.rs:660`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L660)) delegates to `data_labels_visible`
   ([`crates/ooxml-drawingml/src/chart/parse.rs:667`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L667)), which is
   `labels.is_some_and(|labels| val_attr(child(labels, "delete")) != Some("1"))` — the mere
   presence of a non-deleted `c:dLbls` counts as "labels shown". The `show*` flags are not
   consulted. So the loop is not skipped.
2. `declared` is `true`, because `group.data_labels` is `Some` — `parse_data_labels`
   ([`crates/ooxml-drawingml/src/chart/parse.rs:426`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L426)) parsed the block and `label_switches`
   ([`crates/ooxml-drawingml/src/chart/parse.rs:451`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L451)) recorded `show_value: Some(false)` and the
   five siblings.
3. `series.labels` is `Some`, not `None`, so the `continue` on line 77 does not fire. That guard
   was written for the deleted case, where the cascade returns `None`.
4. `labels.shows_anything()` ([`crates/ooxml-drawingml/src/chart/geometry.rs:370`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L370)) is `false`, so
   the negation on line 79 is `true` and the resolved all-off spec is replaced with
   `show_value: true`.

`emit_bar` ([`crates/ooxml-drawingml/src/chart/geometry.rs:1992`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1992)) then calls `push_point_label`
([`crates/ooxml-drawingml/src/chart/geometry.rs:1856`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1856)) once per series per category — the
`for (ser_idx, series) in family.series.iter().enumerate()` loop at
[`crates/ooxml-drawingml/src/chart/geometry.rs:2033`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2033) — with the y coordinate `y + bands.bar`
([`crates/ooxml-drawingml/src/chart/geometry.rs:2054`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L2054)), the bar's bottom edge. That is why the
stacked series' labels sit under their own bar and only the outermost series' label is legible.

Both output backends inherit the defect, because it happens before the display list is built:
`chart_primitive` ([`crates/pptx-render/src/chart.rs:31`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L31)) calls `plot_model` and pushes the
resulting text ops as primitives, so the raster and the browser canvas both paint them. Nothing
in `packages/pptx` reads `c:dLbls` on its own.

Two things worth flagging rather than fixing blind:

- **`shows_data_labels` is loose in the same way, and no test pins it down.** It returns `true`
  for any non-deleted `c:dLbls`, including one that shows nothing.
  `data_labels_count_from_either_placement_and_deletion_switches_them_off`
  ([`crates/ooxml-drawingml/src/chart/parse.rs:1038`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L1038)) only exercises `showVal="1"` and
  `c:delete="1"`, never the all-zero block this deck uses. Tightening it would also fix the
  symptom, but the flag is a coarse "did the part mention labels" signal and only `plot_model`
  reads it, so the narrower change belongs in `plot_model`.
- **A second, adjacent hole is _not_ exercised by this deck and so is not confirmed visually.**
  `declared` is computed per series but `show_data_labels` is per group, so a group where series
  A declares `c:dLbls` and series B declares none leaves B with `declared == false` and
  `series.labels == None` — and B gets synthesised value labels PowerPoint would not draw. That
  follows from the same code, but no slide here reproduces it.

**Suggested fix**

Narrow `plot_model` ([`crates/pptx-render/src/chart.rs:68`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L68)) so it only fills in Excel's defaults
when the part declared no `c:dLbls` at all. Once a `c:dLbls` exists anywhere on the cascade, the
shared geometry has already resolved it — including the case where it resolves to "show
nothing" — and `pptx-render` must not second-guess the result.

The current guard tests `series.labels.is_none()`, which is only true for the deleted case. The
condition that actually separates "the part told us what to show" from "the part only set the
legacy flag" is `declared` on its own:

1. If the group or the series carries a `c:dLbls`, `continue`. The cascade result stands, whether
   it shows everything, one field, or nothing.
2. Otherwise `series.labels` is necessarily `None` (`plot_labels_from_model`
   ([`crates/ooxml-drawingml/src/chart/geometry.rs:594`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L594)) returns `None` when every level is
   `None`), so synthesise the `show_value: true` default unconditionally.

That removes `shows_anything()` from `plot_model` entirely; the only place it should be consulted
is `point_label` ([`crates/ooxml-drawingml/src/chart/geometry.rs:1780`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/geometry.rs#L1780)), which already does the
right thing.

Optional hardening, in the same change or a follow-up: make `data_labels_visible`
([`crates/ooxml-drawingml/src/chart/parse.rs:667`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L667)) require a switch that is actually on, not just
a non-deleted element. That makes `show_data_labels` mean what its name says and makes the fix
defence-in-depth; the existing assertions in
`data_labels_count_from_either_placement_and_deletion_switches_them_off`
([`crates/ooxml-drawingml/src/chart/parse.rs:1038`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L1038)) all use `showVal="1"` or `c:delete="1"`, so
they keep passing.

```rust
// crates/pptx-render/src/chart.rs, in plot_model
for (model, series) in group.series.iter().zip(plotted.series.iter_mut()) {
    // A declared c:dLbls, anywhere on the cascade, is authoritative — including
    // one that switches every field off.
    if model.data_labels.is_some() || group.data_labels.is_some() {
        continue;
    }
    if series.labels.is_none() {
        series.labels = Some(PlotDataLabels {
            show_value: true,
            ..PlotDataLabels::default()
        });
    }
}
```

```rust
// optional, crates/ooxml-drawingml/src/chart/parse.rs
fn data_labels_visible<E: ChartXml>(labels: Option<&E>) -> bool {
    labels.is_some_and(|labels| {
        val_attr(child(labels, "delete")) != Some("1")
            && ["showVal", "showCatName", "showSerName", "showPercent", "showBubbleSize"]
                .iter()
                .any(|name| flag(labels, *name) != Some(false))
    })
}
```

Risks and tests to add:

- **The four existing `plot_model` tests are the contract; check each by hand.**
  `per_point_colours_markers_and_data_labels_reach_the_primitives`
  ([`crates/pptx-render/src/chart.rs:574`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L574)) and `data_labels_keep_the_colour_the_chart_part_gave_each_point`
  ([`crates/pptx-render/src/chart.rs:669`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L669)) set `show_data_labels` with no `c:dLbls` — `declared`
  is false, so they still get the synthesised default.
  `label_switches_compose_the_text_the_part_asks_for` ([`crates/pptx-render/src/chart.rs:654`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L654))
  declares switches, so it now takes the `continue` and reads them from the cascade, which is
  where they already came from. In `a_series_that_switches_its_own_labels_off_draws_none`
  ([`crates/pptx-render/src/chart.rs:637`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L637)) all three arms keep their current label counts:
  `delete: Some(true)` resolves to `None` and draws none, `delete: Some(false)` with
  `show_value: Some(true)` draws them from the cascade, and the `None` arm falls to the
  synthesised default — so the `assert_eq!(labelled(None), labelled(Some(false)))` still holds.
- **A `c:dLbls` carrying only `c:numFmt`, `c:dLblPos`, `c:spPr` or `c:txPr` and no `show*`
  element** will now draw nothing where it previously drew values. That is the correct reading —
  a label with no fields switched on has no text to compose — but it is a behaviour change beyond
  this deck, and no deck in the harness exercises it. Worth a deliberate decision rather than a
  silent one.
- **The per-series `declared`/per-group `show_data_labels` mismatch described in the report is
  left alone.** Fixing it as well (computing `declared` per series only, and dropping
  `show_data_labels` from the loop) would change the mixed-declaration case; it has no repro here
  and should not ride along unmeasured.
- Tests to add, next to [`crates/pptx-render/src/chart.rs:637`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L637): a group-level `ChartDataLabels`
  with `show_value`, `show_category_name`, `show_series_name`, `show_percent`,
  `show_legend_key` and `show_bubble_size` all `Some(false)`, `group.show_data_labels = true`,
  and no series-level labels — the plotted chart must emit no value text. A sibling case with the
  same block on the series rather than the group covers the other cascade level. If
  `data_labels_visible` is tightened too, extend
  [`crates/ooxml-drawingml/src/chart/parse.rs:1038`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/chart/parse.rs#L1038) with an all-zero `c:dLbls` asserting
  `!show_data_labels`.

**How to verify**

Re-render and re-diff the deck:

```
.venv/bin/python render-improvement-harness/scripts/render_bo.py stacked-bar
.venv/bin/python render-improvement-harness/scripts/diff.py stacked-bar
```

All four slides currently sit at `fine_pct` 14.46. The labels are a handful of small glyphs, so
expect a small drop, not a collapse — the same slides also carry the axis-placement, category
order and legend defects (`stacked-bar/0N/2`, `/3`, `/4`), which own most of that number. The
check that matters is binary and local: no text op may be emitted inside the plot area of these
charts. Cropping the candidate panel at the bar ends, as evidence-2.png does, must show clean
whitespace.

Tests covering the area, all of which must keep passing:
`a_series_that_switches_its_own_labels_off_draws_none`
([`crates/pptx-render/src/chart.rs:637`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L637)), `label_switches_compose_the_text_the_part_asks_for`
([`crates/pptx-render/src/chart.rs:654`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L654)),
`per_point_colours_markers_and_data_labels_reach_the_primitives`
([`crates/pptx-render/src/chart.rs:574`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L574)) and
`data_labels_keep_the_colour_the_chart_part_gave_each_point`
([`crates/pptx-render/src/chart.rs:669`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L669)) — the last two are the legitimate use of the synthesised
default, a group with `show_data_labels` set and no `c:dLbls` anywhere. The missing case is a
group-level `c:dLbls` with every switch `0`; it belongs next to
[`crates/pptx-render/src/chart.rs:637`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L637).

**Additional context**

none.

Related issues found in the same run: none.

Files most likely involved: `crates/pptx-render/src/chart.rs`, `crates/ooxml-drawingml/src/chart/parse.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/chart-dlbls-shown-when-disabled/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
