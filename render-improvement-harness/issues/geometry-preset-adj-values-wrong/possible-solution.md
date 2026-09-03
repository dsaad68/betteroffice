# Possible solution: geometry-preset-adj-values-wrong

## Approach

One file: `crates/ooxml-drawingml/src/geometry.rs`. Nothing above it changes - both `pptx-render`
`geometry_path` helpers (`crates/pptx-render/src/layout.rs:1946`,
`crates/pptx-render/src/lib.rs:237`) and `docx-parse`
(`crates/docx-parse/src/drawingml.rs:273`) already hand `preset_geometry_to_path` the aspect ratio,
and every consumer below it draws the normalised command list as-is.

Three edits:

1. **Add an `ss`-relative helper** beside `clamp_fraction`
   (`crates/ooxml-drawingml/src/geometry.rs:328`) that converts an `adj` fraction into the
   width-fraction and height-fraction units the command list actually speaks. With
   `aspect_ratio = w / h`, `ss / w = 1 / max(aspect_ratio, 1)` and `ss / h = min(aspect_ratio, 1)`.
   Guard a non-finite or non-positive ratio the way `rounded_rect`
   (`crates/ooxml-drawingml/src/geometry.rs:231-236`) already does.

2. **Rewrite the `chevron` arm** (`crates/ooxml-drawingml/src/geometry.rs:199-209`) to run its
   `adj` through that helper, and fix its default to `0.5`
   (`crates/ooxml-drawingml/src/geometry.rs:19`). Keep the existing point order - the topology
   already matches the ECMA path.

3. **Give `homePlate` a real arm** (`crates/ooxml-drawingml/src/geometry.rs:210`) reading `adj`
   through the same helper, plus a `("adj", 0.5)` entry in
   `preset_geometry_default_adjustments` (`crates/ooxml-drawingml/src/geometry.rs:8-30`) so the
   editor exposes and round-trips the right value.

Replace `.min(0.5)` with the spec's `pin 0 adj maxAdj`, expressed in the same units, once the
`maxAdj` numerator has been read off `presetShapeDefinitions.xml`. If that read is deferred, clamp
the resulting *width fraction* to `0.5` for `chevron` and `1.0` for `homePlate` rather than
clamping the raw `adj` - the current clamp truncates legal wide-shape values.

Optional, same pass: `parallelogram` (`crates/ooxml-drawingml/src/geometry.rs:104`), `trapezoid`
(`:108`), `hexagon` (`:113`) and `octagon` (`:125`) share the basis error and can reuse the helper.
Their spec defaults should be re-read at the same time - the report flags them as unverified.

## Sketch

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

## Risks

- **`docx-parse` loses the `ss` basis.** `parse_preset_geometry_path`
  (`crates/docx-parse/src/drawingml.rs:255-274`) evaluates guides over a fixed 100000 x 100000 box
  (`crates/docx-parse/src/drawingml.rs:260`), so any DOCX shape whose `adj` is written as a formula
  over `ss` already resolves against a square. Literal `val` guides - the common case, and the only
  case in this corpus - are unaffected because they carry no extent. Adding a DOCX chevron/homePlate
  case to `crates/docx-parse` tests is cheap insurance.
- **`clamp_fraction`'s dual scale** (`crates/ooxml-drawingml/src/geometry.rs:328-334`) rescales any
  value above `1.0` by `/ 100_000`, which is what lets `docx-parse` pass raw 0..100000 units. That
  makes a legitimate `adj` of, say, `120000` (`1.2` after `pptx-parse` normalisation) collapse to
  `1.2e-5`. Out of scope here, but the same clamp rewrite is the place to fix it if the `maxAdj` pin
  is done properly.
- **Editor round-trip.** Changing the `chevron` default and adding a `homePlate` default changes
  what `crates/pptx-edit/src/deck.rs:126-130` seeds into document state and therefore what
  `crates/pptx-parse/src/write.rs:1225` can write back. It only makes newly authored XML correct
  (`val 50000` rather than `val 35000`), and existing shapes still diff clean against their own
  baseline (`crates/pptx-edit/src/save.rs:214`), but `crates/pptx-edit` snapshot tests that pin
  `adjustValuesJson` will need updating.
- **No existing test pins chevron or homePlate output**
  (`crates/ooxml-drawingml/src/geometry.rs:497-561`), so nothing guards a regression today. Add, in
  that module: a square chevron whose notch is unchanged at `adj = 0.5`; a 3:1 chevron whose notch
  sits at `1/6` of the width, not `0.5`; a wide `homePlate` honouring an explicit `adj`; and a
  `preset_geometry_default_adjustments("chevron") == 0.5` assertion.

## Effort

**easy** - roughly twenty lines in one file plus four unit tests, with no model, display-list or
schema change and no call-site churn, since the aspect ratio the fix needs is already a parameter of
the function being changed.
