# Possible solution: line-stroke-color-resolution-broken

Two separable changes. (A) parses the theme's line-style matrix and the shape's `p:style/a:lnRef`,
and clears findings 04/2, 16/6 and 19/2. (B) widens the stroke to carry a gradient, and clears 09/4.
They can land in either order, but **(A) only becomes visible once `line-zero-extent-skipped` has
landed** - until `p:cxnSp` is parsed, slides 04 and 19 have no connector shapes to stroke.

`p:style` also carries `a:fontRef`, which `theme-color-scheme-color-resolution-broken` needs. Step 2
below is the same edit that report's step 1-2 describe; whichever lands first should add the whole
`ShapeStyle` struct and let the other fill in its field.

## Approach

### A. Parse the line-style matrix and apply `a:lnRef`

1. `crates/ooxml-drawingml/src/theme.rs:134` - add `format_scheme: ThemeFormatScheme` to `Theme`,
   `#[serde(default)]` so serialized decks keep loading. Only the line list is needed now:

   ```rust
   pub struct ThemeFormatScheme { pub line_styles: Vec<ShapeOutline> }
   ```

   `ShapeOutline` already models `w`/`cap`/`prstDash`/`join`, and the `a:ln` inside `lnStyleLst` is
   the same element `parse_outline` reads, so the type fits without inventing a new one. Its `color`
   holds the `phClr` placeholder verbatim.

2. `crates/pptx-parse/src/theme.rs:5` - parse `a:fmtScheme/a:lnStyleLst` into that vector, reusing
   `parse_outline`'s body (move it to a `pub(crate) fn parse_line(&XmlElement) -> ShapeOutline` in
   `drawing.rs` and have both callers use it).

3. `crates/pptx-parse/src/model.rs:198` - add `style: Option<ShapeStyle>` to `Shape`
   (`#[serde(default, skip_serializing_if = "Option::is_none")]`), with

   ```rust
   pub struct ShapeStyle {
       pub line_ref: Option<StyleRef>,      // idx + colour
       pub font_ref_color: Option<ColorValue>,
       pub font_ref: Option<String>,
   }
   pub struct StyleRef { pub index: u32, pub color: Option<ColorValue> }
   ```

   and fill it in `parse_shape` (`crates/pptx-parse/src/drawing.rs:138`) from the sibling `p:style`.
   `parse_color_container` (`:654`) already reads the colour, `a:shade`/`a:satMod` included.

   Keep the reference *unresolved* in the model. Do **not** bake a synthesized outline into
   `Shape.outline`: `patch_shape` (`crates/pptx-parse/src/write.rs:866`) and the editor's stroke path
   (`crates/pptx-edit/src/deck.rs:489-508`) round-trip `ShapeOutline` verbatim, so a synthesized one
   would materialize an explicit `<a:ln>` into a file that never had one the first time anything
   touches that shape's stroke.

4. `crates/pptx-render/src/layout.rs:1930` - resolve at render time. `stroke` grows the shape's
   style and merges: start from `lnStyleLst[idx - 1]` (`idx = 0` means "no line", return `None`),
   substitute the `lnRef`'s own colour for every `phClr` in it, then overlay whatever the explicit
   `a:ln` states. That is what makes 16/6 (colour from `a:ln`, width from the matrix) and 04/2
   (everything from the matrix) fall out of one code path.

5. `crates/ooxml-drawingml/src/color.rs:61` - `phClr` needs a substitution hook, since
   `ThemeColorScheme::get` (`crates/ooxml-drawingml/src/theme.rs:59`) has no arm for it and
   `default_theme_color` (`:183`) would turn it black. Cheapest form: resolve the `phClr` slot into
   the reference's colour *before* calling the resolver, keeping the matrix entry's own
   `shade`/`satMod` modifiers.

   `crates/docx-parse/src/shape.rs:698-712` is the prior art, and also the cheap fallback if the
   `fmtScheme` work has to be deferred: it ignores `lnStyleLst` entirely and synthesizes
   `width: 9525` plus the `lnRef`'s colour. That alone fixes 04/2 and 19/2, but not 16/6.

### B. Let a stroke carry a gradient

6. `crates/ooxml-drawingml/src/shape.rs:43` - add `gradient: Option<GradientFill>` to
   `ShapeOutline`, mirroring `ShapeFill` (`:9-14`), and read `a:gradFill` in `parse_outline`
   (`crates/pptx-parse/src/drawing.rs:632`) via the existing `parse_gradient_fill` (`:585`).

7. `crates/pptx-render/src/display_list.rs:54` - add
   `#[serde(default, skip_serializing_if = "Option::is_none")] pub paint: Option<Paint>` to
   `Stroke`, keeping `color` as the flattened first-stop fallback. Additive and optional, so
   `CONTRACT_VERSION` stays 1 and any consumer that only reads `color` still draws a plausible line.
   Mirror the field in `packages/pptx/src/types.ts:214`.

8. `crates/pptx-raster/src/lib.rs:697` - `stroke_paint` takes the shape box and calls the existing
   `gradient_paint` (`:627`) when `stroke.paint` is a gradient. `paint_shape` (`:364`) and
   `paint_image` (`:390`) already have `x, y, w, h` in scope; only `stroke_path` (`:482`) needs them
   threaded through.

## Sketch

```rust
// crates/pptx-render/src/layout.rs
fn stroke(outline: Option<&ShapeOutline>, style: Option<&ShapeStyle>, theme: &Theme) -> Option<Stroke> {
    let reference = style.and_then(|style| style.line_ref.as_ref());
    let base = match reference {
        Some(reference) if reference.index == 0 => return None,
        Some(reference) => theme
            .format_scheme
            .line_styles
            .get(reference.index as usize - 1)
            .map(|line| substitute_placeholder(line, reference.color.as_ref())),
        None => None,
    };
    let merged = merge(base.as_ref(), outline)?; // explicit a:ln wins property by property
    let paint = merged
        .gradient
        .as_ref()
        .and_then(|gradient| gradient_paint_from(gradient, theme));
    let color = resolve_color_value_to_hex_with_theme(merged.color.as_ref(), Some(theme))
        .or_else(|| first_stop_hex(paint.as_ref()))?;
    Some(Stroke { color, paint, width: /* unchanged */, dashed: /* unchanged */ })
}
```

```rust
// crates/pptx-parse/src/drawing.rs, parse_shape
style: element.child("style").map(|style| ShapeStyle {
    line_ref: style.child("lnRef").map(|reference| StyleRef {
        index: reference.attribute("idx").and_then(|v| v.parse().ok()).unwrap_or(0),
        color: parse_color_container(reference),
    }),
    font_ref_color: style.child("fontRef").and_then(parse_color_container),
    font_ref: style.child("fontRef").and_then(|v| v.attribute("idx")).map(str::to_owned),
}),
```

## Risks

- **The width change reaches 83 shapes the findings never mention** - every `a:ln` with a visible
  fill but no `w` under a `lnRef idx >= 1`: 66 on `cisco-cloud-security`, 16 on `project20`, 1 on
  `ocp-psp-plan`. They all get thicker. That is correct per spec and matches LibreOffice on 16/6,
  but it is the one part of this change that can move slides nobody looked at. `project20` is the
  canary because it is otherwise close to the reference.
- **The colour change reaches nothing else in this corpus.** Zero of 2546 `p:sp` need `lnRef` for
  their stroke colour (they all carry an explicit `a:ln`, usually `<a:ln><a:noFill/></a:ln>`), so
  step 4's blast radius outside connectors is the width only. The `idx="0"` -> no-line rule must be
  honoured or that flips: `lnRef idx="0"` is common on shapes whose `a:ln` is `noFill`.
- **`phClr` half-done is worse than not done.** If the matrix entry is applied but the placeholder
  is not substituted, `default_theme_color` (`crates/ooxml-drawingml/src/color.rs:183`) paints every
  themed line `000000`. Add a regression test that a `phClr` line style resolves to the `lnRef`'s
  colour and never to black.
- **Round trip.** Keeping the reference in `Shape.style` and resolving in `pptx-render` is what makes
  this safe. `cargo test -p pptx-edit --test write_fidelity` must stay green, and a case where a
  stroke edit lands on a shape whose only outline came from `lnRef` is worth adding - the editor
  (`crates/pptx-edit/src/deck.rs:489`) starts from `outlineJson`, which will still be `None` there.
- **Gradient direction on a degenerate box.** The radar spokes are `ext cx="0" cy="830086"`, a
  zero-width box, with `<a:lin ang="5400000"/>`. `gradient_paint`
  (`crates/pptx-raster/src/lib.rs:627`) sizes its shader off `w.hypot(h)`, which survives a zero
  side, but the stroke is drawn *outside* that box by half the line width - check the ends do not
  land on `SpreadMode::Pad` fringe.
- **The canvas backend must match.** `packages/pptx` renders the same display list; a `Stroke.paint`
  the web backend ignores means raster and browser disagree on seven visible lines. The
  fallback `color` keeps that a colour difference rather than a missing line.
- **Tests to add** (none exist for any of the three mechanisms): a `pptx-parse` case that a `p:style`
  with `lnRef` and an `a:ln` with `gradFill` both reach the model; a `pptx-render` case that a shape
  with no `a:ln` and `lnRef idx="1"` gets the theme width and the reference colour, that an explicit
  `a:ln` colour beats the matrix while the matrix width still applies, and that `idx="0"` strokes
  nothing; a `pptx-raster` golden with a gradient-stroked line.

## Effort

medium - (A) adds one type to `ooxml-drawingml`, a theme parse, a model field and a merge in
`stroke`, and the `phClr` hook is the only genuinely new mechanism; (B) is four small edits because
`Paint::Gradient` and `gradient_paint` already exist and the display-list field is additive. What
makes it medium rather than easy is that it spans four crates plus the TS contract, and that the
width half changes 83 shapes the findings never looked at.
