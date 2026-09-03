# Possible solution: chart-dlbls-shown-when-disabled

## Approach

Narrow `plot_model` (`crates/pptx-render/src/chart.rs:68`) so it only fills in Excel's defaults
when the part declared no `c:dLbls` at all. Once a `c:dLbls` exists anywhere on the cascade, the
shared geometry has already resolved it — including the case where it resolves to "show
nothing" — and `pptx-render` must not second-guess the result.

The current guard tests `series.labels.is_none()`, which is only true for the deleted case. The
condition that actually separates "the part told us what to show" from "the part only set the
legacy flag" is `declared` on its own:

1. If the group or the series carries a `c:dLbls`, `continue`. The cascade result stands, whether
   it shows everything, one field, or nothing.
2. Otherwise `series.labels` is necessarily `None` (`plot_labels_from_model`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:594`) returns `None` when every level is
   `None`), so synthesise the `show_value: true` default unconditionally.

That removes `shows_anything()` from `plot_model` entirely; the only place it should be consulted
is `point_label` (`crates/ooxml-drawingml/src/chart/geometry.rs:1780`), which already does the
right thing.

Optional hardening, in the same change or a follow-up: make `data_labels_visible`
(`crates/ooxml-drawingml/src/chart/parse.rs:667`) require a switch that is actually on, not just
a non-deleted element. That makes `show_data_labels` mean what its name says and makes the fix
defence-in-depth; the existing assertions in
`data_labels_count_from_either_placement_and_deletion_switches_them_off`
(`crates/ooxml-drawingml/src/chart/parse.rs:1038`) all use `showVal="1"` or `c:delete="1"`, so
they keep passing.

## Sketch

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

## Risks

- **The four existing `plot_model` tests are the contract; check each by hand.**
  `per_point_colours_markers_and_data_labels_reach_the_primitives`
  (`crates/pptx-render/src/chart.rs:574`) and `data_labels_keep_the_colour_the_chart_part_gave_each_point`
  (`crates/pptx-render/src/chart.rs:669`) set `show_data_labels` with no `c:dLbls` — `declared`
  is false, so they still get the synthesised default.
  `label_switches_compose_the_text_the_part_asks_for` (`crates/pptx-render/src/chart.rs:654`)
  declares switches, so it now takes the `continue` and reads them from the cascade, which is
  where they already came from. In `a_series_that_switches_its_own_labels_off_draws_none`
  (`crates/pptx-render/src/chart.rs:637`) all three arms keep their current label counts:
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
- Tests to add, next to `crates/pptx-render/src/chart.rs:637`: a group-level `ChartDataLabels`
  with `show_value`, `show_category_name`, `show_series_name`, `show_percent`,
  `show_legend_key` and `show_bubble_size` all `Some(false)`, `group.show_data_labels = true`,
  and no series-level labels — the plotted chart must emit no value text. A sibling case with the
  same block on the series rather than the group covers the other cascade level. If
  `data_labels_visible` is tightened too, extend
  `crates/ooxml-drawingml/src/chart/parse.rs:1038` with an all-zero `c:dLbls` asserting
  `!show_data_labels`.

## Effort

Easy. One guard in one function, four lines shorter than what it replaces, with the correct
behaviour already implemented in the shared geometry; the work is in the regression tests and in
deciding how far to take the optional `parse.rs` tightening.
