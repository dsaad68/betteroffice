---
id: chart-dlbls-shown-when-disabled
title: Stop synthesising data labels over a c:dLbls that switches every field off
category: chart
impact: medium
effort: easy
confidence: high
status: open
occurrences: 4
decks: [stacked-bar]
findings: [stacked-bar/01/1, stacked-bar/02/1, stacked-bar/03/1, stacked-bar/04/7]
files: [crates/pptx-render/src/chart.rs, crates/ooxml-drawingml/src/chart/parse.rs]
---

## Symptom

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

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | stacked-bar/01 | reference and candidate over the same bar-end column: no labels in LibreOffice, a value past every bar end in BetterOffice |
| 2 | stacked-bar/01 | 4x zoom on the candidate's four visible labels — `2`, `2`, `3`, `5`, the Series 3 values; the bottom one is cut by the frame edge |
| 3 | stacked-bar/01 | 6x zoom on the Category 1 bar: the Series 1 and Series 2 labels are drawn too, at the segment boundaries, all but hidden under the bar |
| 4 | stacked-bar/04 | the same labels on the recoloured copy of the chart, confirming the cluster is one defect four times |

## Root cause (confirmed)

The shared chart geometry handles this correctly. `plot_labels_from_model`
(`crates/ooxml-drawingml/src/chart/geometry.rs:594`) resolves the four-level cascade and, for
this chart, returns `Some(PlotDataLabels)` with every switch `false`; `point_label`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1780`) then bails out on
`if !spec.shows_anything() { return None; }` and no text op is emitted. Run against the parsed
model alone, the chart would draw no labels.

`pptx-render` overwrites that result afterwards. `plot_model`
(`crates/pptx-render/src/chart.rs:68`) exists to give a part that carries only the legacy
`show_data_labels` flag the switches Excel would default to, but its guard does not cover a
`c:dLbls` that declares its switches and turns them all off:

```rust
// crates/pptx-render/src/chart.rs:70-86
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
   (`crates/ooxml-drawingml/src/chart/parse.rs:660`) delegates to `data_labels_visible`
   (`crates/ooxml-drawingml/src/chart/parse.rs:667`), which is
   `labels.is_some_and(|labels| val_attr(child(labels, "delete")) != Some("1"))` — the mere
   presence of a non-deleted `c:dLbls` counts as "labels shown". The `show*` flags are not
   consulted. So the loop is not skipped.
2. `declared` is `true`, because `group.data_labels` is `Some` — `parse_data_labels`
   (`crates/ooxml-drawingml/src/chart/parse.rs:426`) parsed the block and `label_switches`
   (`crates/ooxml-drawingml/src/chart/parse.rs:451`) recorded `show_value: Some(false)` and the
   five siblings.
3. `series.labels` is `Some`, not `None`, so the `continue` on line 77 does not fire. That guard
   was written for the deleted case, where the cascade returns `None`.
4. `labels.shows_anything()` (`crates/ooxml-drawingml/src/chart/geometry.rs:370`) is `false`, so
   the negation on line 79 is `true` and the resolved all-off spec is replaced with
   `show_value: true`.

`emit_bar` (`crates/ooxml-drawingml/src/chart/geometry.rs:1992`) then calls `push_point_label`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1856`) once per series per category — the
`for (ser_idx, series) in family.series.iter().enumerate()` loop at
`crates/ooxml-drawingml/src/chart/geometry.rs:2033` — with the y coordinate `y + bands.bar`
(`crates/ooxml-drawingml/src/chart/geometry.rs:2054`), the bar's bottom edge. That is why the
stacked series' labels sit under their own bar and only the outermost series' label is legible.

Both output backends inherit the defect, because it happens before the display list is built:
`chart_primitive` (`crates/pptx-render/src/chart.rs:31`) calls `plot_model` and pushes the
resulting text ops as primitives, so the raster and the browser canvas both paint them. Nothing
in `packages/pptx` reads `c:dLbls` on its own.

Two things worth flagging rather than fixing blind:

- **`shows_data_labels` is loose in the same way, and no test pins it down.** It returns `true`
  for any non-deleted `c:dLbls`, including one that shows nothing.
  `data_labels_count_from_either_placement_and_deletion_switches_them_off`
  (`crates/ooxml-drawingml/src/chart/parse.rs:1038`) only exercises `showVal="1"` and
  `c:delete="1"`, never the all-zero block this deck uses. Tightening it would also fix the
  symptom, but the flag is a coarse "did the part mention labels" signal and only `plot_model`
  reads it, so the narrower change belongs in `plot_model`.
- **A second, adjacent hole is _not_ exercised by this deck and so is not confirmed visually.**
  `declared` is computed per series but `show_data_labels` is per group, so a group where series
  A declares `c:dLbls` and series B declares none leaves B with `declared == false` and
  `series.labels == None` — and B gets synthesised value labels PowerPoint would not draw. That
  follows from the same code, but no slide here reproduces it.

## Verification

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
(`crates/pptx-render/src/chart.rs:637`), `label_switches_compose_the_text_the_part_asks_for`
(`crates/pptx-render/src/chart.rs:654`),
`per_point_colours_markers_and_data_labels_reach_the_primitives`
(`crates/pptx-render/src/chart.rs:574`) and
`data_labels_keep_the_colour_the_chart_part_gave_each_point`
(`crates/pptx-render/src/chart.rs:669`) — the last two are the legitimate use of the synthesised
default, a group with `show_data_labels` set and no `c:dLbls` anywhere. The missing case is a
group-level `c:dLbls` with every switch `0`; it belongs next to
`crates/pptx-render/src/chart.rs:637`.
