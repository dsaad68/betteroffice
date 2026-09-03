# Possible solution: theme-color-scheme-color-resolution-broken

Two separable changes. (A) is small and clears 17 of the 20 findings; (B) is the medium half.
They can land in either order.

## Approach

### A. Carry `p:style/a:fontRef` as the shape's default text colour

1. `crates/pptx-parse/src/model.rs:198` - add an optional style to `Shape`:

   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub style: Option<ShapeStyle>,
   ```

   with `pub struct ShapeStyle { pub font_ref_color: Option<ColorValue>, pub font_ref: Option<String> }`
   (`font_ref` being `idx`, i.e. `minor`/`major`, so `+mn-lt`/`+mj-lt` can be fed to the existing
   `resolve_theme_font_ref`). `serde(default)` keeps packages serialized before this change loading.

2. `crates/pptx-parse/src/drawing.rs:138` - in `parse_shape`, read the sibling element. The colour
   is a direct child of `a:fontRef`, so `parse_color_container`
   (`crates/pptx-parse/src/drawing.rs:654`) already handles it as-is; no `formatScheme` is needed.

3. `crates/pptx-render/src/layout.rs:761` - add `font_ref_color: Option<&'a ColorValue>` to
   `BodyCascade`, filled at the two construction sites (`:451` and `:571`) from
   `original` / `layout_node` / `master_node` via a `node_font_ref_color` helper written like
   `node_fill` (`crates/pptx-render/src/layout.rs:1756`).

4. `crates/pptx-render/src/layout.rs:808` - in `paragraph_properties`, install it as the base
   colour *after* the master `txStyles` seed and *before* the master/layout/primary body merge, so
   the run's own `rPr` and any `lstStyle`/`defRPr` still win. Gate it on `self.placeholder.is_none()`:
   for a real placeholder the `titleStyle`/`bodyStyle` seed is the correct list-style chain and must
   keep priority, whereas for a plain autoshape that seed is `otherStyle`, which does not belong in
   the chain at all (see the report) - replacing it with the `fontRef` colour is exactly right.

### B. Parse and apply the colour map

5. `crates/ooxml-drawingml/src/theme.rs` - add

   ```rust
   pub struct ColorMap { /* 12 slot -> theme-slot entries */ }
   ```

   whose `Default` is the standard master map (`text1 -> dk1`, `background1 -> lt1`,
   `text2 -> dk2`, `background2 -> lt2`, accents/links identity), plus
   `fn resolve<'a>(&'a self, slot: &'a str) -> &'a str`.

   Keep `normalize_scheme_color` (`crates/pptx-parse/src/drawing.rs:695`) and
   `denormalize_scheme_color` (`crates/pptx-parse/src/write.rs:1137`) exactly as they are: the
   `text1`/`background1`/... names they produce become the *map keys* rather than hardcoded
   aliases, so no serialized model or round trip changes. Keep the existing alias arms in
   `ThemeColorScheme::get` (`crates/ooxml-drawingml/src/theme.rs:59`) as well - they then only
   serve the no-map path, which stays behaviour-identical.

6. `crates/ooxml-drawingml/src/color.rs:61` - add
   `resolve_color_value_to_hex_with_map(color, theme, map)` and make the existing
   `resolve_color_value_to_hex_with_theme` a wrapper that passes `&ColorMap::default()`. docx,
   xlsx and every current pptx caller keep compiling unchanged.

7. `crates/pptx-parse/src/model.rs` - `color_map: ColorMap` on `SlideMaster` (`:102`),
   `color_map_override: Option<ColorMap>` on `SlideLayout` (`:90`) and `Slide` (`:78`), all
   `#[serde(default)]`. Populate in `crates/pptx-parse/src/package.rs:75-128` and in the slide loop
   (`:60-68`): `p:clrMap` on the master, `p:clrMapOvr/a:overrideClrMapping` on layout and slide,
   with `<a:masterClrMapping/>` parsing to `None`.

8. `crates/pptx-render/src/layout.rs:317` - resolve the effective map once per slide
   (slide override, else layout override, else master map, else default) into a
   `color_map: ColorMap` field on `SlideRenderer`, and pass it at the six sites that reach a
   resolver: `paint` (`:1897`, used at `:183`, `:388`, `:526`), `stroke` (`:1930`, used at `:393`,
   `:530`, `:547`), `style_from_properties` (`:1849`) and `resolve_style` (`:1049`).

## Sketch

```rust
// crates/pptx-parse/src/drawing.rs, parse_shape
let style = element.child("style").map(|style| ShapeStyle {
    font_ref_color: style.child("fontRef").and_then(parse_color_container),
    font_ref: style
        .child("fontRef")
        .and_then(|value| value.attribute("idx"))
        .map(str::to_owned),
});

// crates/pptx-render/src/layout.rs, BodyCascade::paragraph_properties
let mut properties = self
    .master_slide
    .and_then(|master| master_style(master, self.placeholder, level))
    .cloned()
    .unwrap_or_default();
if self.placeholder.is_none()
    && let Some(color) = self.font_ref_color
{
    properties
        .default_run
        .get_or_insert_with(RunProperties::default)
        .color = Some(color.clone());
}
for body in [self.master, self.layout, self.primary].into_iter().flatten() { /* unchanged */ }

// crates/ooxml-drawingml/src/color.rs
pub fn resolve_color_value_to_hex_with_map(
    color: Option<&ColorValue>,
    theme: Option<&Theme>,
    map: &ColorMap,
) -> Option<String> {
    // ... same body, except:
    color.theme_color.as_deref().map(|slot| {
        let slot = map.resolve(slot);
        theme
            .and_then(|theme| theme.color_scheme.get(slot))
            .unwrap_or_else(|| default_theme_color(slot))
    })
}
```

## Risks

- **The `phClr` / style-matrix gap stays open.** `ColorMap::resolve` must pass unknown names
  through untouched, and `default_theme_color` (`crates/ooxml-drawingml/src/color.rs:183`) still
  turns anything it does not know into `000000`. Add a regression test that a `phClr` value is not
  made worse by the change.
- **Placeholders that also carry a `p:style`.** The `placeholder.is_none()` gate in step 4 is a
  deliberate simplification: PowerPoint applies `fontRef` under the list-style chain rather than
  skipping it. If a deck turns up where a placeholder's only colour source is its `fontRef`, the
  gate has to become "apply `fontRef` below the list style" instead, which needs the master
  `txStyles` seed and the shape's own `lstStyle` to be distinguishable - they are merged into one
  `ParagraphProperties` today.
- **A wider blast radius than the findings.** Step 8 changes shape fills, outlines, gradient stops
  and backgrounds as well as text, on every deck with a non-standard map. `rollout-plan` is the
  regression canary: its slide 1 background goes from grey to purple.
- **Charts.** `crates/pptx-render/src/chart.rs` and `crates/ooxml-drawingml/src/chart/` resolve
  their own colours; if they take the `..._with_theme` wrapper they silently keep the default map.
  That is correct only if no charted deck uses `clrMapOvr` - worth a grep before landing rather
  than an assumption.
- **Round trip.** Keeping `normalize_scheme_color`/`denormalize_scheme_color` untouched is what
  makes this safe; if a later cleanup removes the normalization, `ThemeColorScheme::get` must gain
  `tx1`/`bg1`/`tx2`/`bg2` arms in the same commit or every mapped colour goes black.
- **Tests to add** (none exist for either mechanism): a `pptx-parse` case asserting `p:style` and
  `p:clrMapOvr` reach the model; an `ooxml-drawingml` case that `tx1` resolves to `lt1` under an
  override and to `dk1` without one; a `pptx-render` case that an autoshape whose run has no
  `solidFill` takes its `fontRef` colour, and that a run-level `solidFill` still beats it.

## Effort

medium - (A) is four small edits and is easy on its own; (B) adds a type to `ooxml-drawingml`, two
model fields, a second resolver signature and six threaded call sites, and it changes colours
outside the text path, so it needs the four-deck re-render to be trusted.
