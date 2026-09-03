# Possible solution: chart-category-order-reversed

## Approach

One predicate, one XOR. `category_position`
(`crates/ooxml-drawingml/src/chart/geometry.rs:1946`) already knows how to count categories from
the far end; it just doesn't know that a horizontal bar family always needs that, because its
categories run down the screen while its axis runs up.

1. **Name the orientation on `PlotFamily`.** Add `transposed()` next to `stacking()`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:1039`). The sibling
   `chart-axis-position-swapped` issue proposes the identical helper for `emit_axes`; whoever
   lands first should add it, and the second should reuse it rather than duplicate the test.

   ```rust
   /// A horizontal bar family plots categories down the screen while its axis
   /// counts up, so its slots run from the far end.
   fn transposed(&self) -> bool {
       self.chart_type == "bar"
   }
   ```

   This is exactly `emit_bar`'s `horizontal` argument: `emit_family` sends `"bar"` to
   `emit_bar(.., true)` and everything else to `emit_bar(.., false)`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:1312-1313`).

2. **XOR it with `c:orientation`.** `reversed` means `maxMin`
   (`crates/ooxml-drawingml/src/chart/parse.rs:552`), so a transposed family with `maxMin` is
   *not* flipped - it is the one case that draws index 0 at the top, which is what the code does
   for every case today.

3. **Change nothing else.** The category label at
   `crates/ooxml-drawingml/src/chart/geometry.rs:2014-2021` and the bar rect at
   `crates/ooxml-drawingml/src/chart/geometry.rs:2046` both derive from the same `slot`
   (`crates/ooxml-drawingml/src/chart/geometry.rs:2011`), as does the data label at
   `crates/ooxml-drawingml/src/chart/geometry.rs:2047-2056`, so all three move together. The four
   other `category_position` callers (`line_x` `:2094`, `emit_radar` `:2489`, `emit_stock` `:2624`,
   `emit_surface` `:2758` and `:2783`) map the slot onto x and are never reached with
   `chart_type == "bar"`, so the guard leaves them untouched.

## Sketch

```rust
// crates/ooxml-drawingml/src/chart/geometry.rs:1946
/// Drawing position of the `index`th category. A reversed category axis counts
/// from the far end, and so does a transposed family, whose slots run down the
/// screen against an axis that runs up.
fn category_position(family: PlotFamily<'_>, index: usize, count: usize) -> usize {
    let reversed = family.category_axis.is_some_and(|axis| axis.reversed);
    if reversed != family.transposed() {
        count.saturating_sub(1).saturating_sub(index)
    } else {
        index
    }
}
```

## Risks

- **Shared geometry, three renderers.** `plot_chart_into` is reached from
  `crates/pptx-render/src/chart.rs:47`, `crates/xlsx-render/src/chart.rs:498` and
  `crates/docx-layout/src/display_list.rs:7975`. Every `barDir="bar"` chart in every format flips;
  nothing else may. `family.transposed()` is the gate, and
  `a_reversed_category_axis_draws_the_categories_from_the_far_end`
  (`crates/ooxml-drawingml/src/chart/geometry.rs:4831`) is the tripwire - it asserts a **column**
  chart's reversal, so it must keep passing byte for byte.
- **Double-flip on `maxMin`.** The XOR is the whole subtlety. A `barDir="bar"` chart that also
  says `c:orientation val="maxMin"` must keep today's top-down order; a test for that pair is
  cheap and is the one a careless `||` would break.
- **The fix is invisible with one category.** Any new test needs at least two categories with
  distinguishable values, or it asserts nothing.
- **Clustered bar lane order stays wrong.** Series lanes inside a slot still run downward
  (`crates/ooxml-drawingml/src/chart/geometry.rs:2041`). PowerPoint puts Series 1 at the bottom of
  a clustered bar group. Out of scope here - the harness deck is stacked, so it has no evidence -
  but a reviewer should know the transposition is only half-applied until that is settled.
- Tests to add, in the `crates/ooxml-drawingml/src/chart/geometry.rs` test module: a `"bar"` family
  with two categories asserting `bars[0].1 > bars[1].1` (the first category draws lower), the
  `maxMin` mirror asserting it draws higher, and the existing column assertion left alone as the
  no-regression half of the pair.

## Effort

Easy. One function, one XOR, one new predicate, and two new assertions - the guard is exactly the
flag `emit_bar` already receives, and no golden or existing test constrains the transposed
direction today.
