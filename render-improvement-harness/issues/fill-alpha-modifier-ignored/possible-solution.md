# Possible solution: fill-alpha-modifier-ignored

## Approach

Switch the two pptx shape-paint resolvers from the opaque hex resolver to the RGBA one that
already exists and is already tested. `resolve_color_value_to_rgba_hex`
(`crates/ooxml-drawingml/src/color.rs:91`) wraps `resolve_color_value_to_hex_with_theme` and
appends the alpha byte, so hue/lumMod/satMod handling is unchanged and colors without
`a:alpha` come back as `#RRGGBBFF`.

Three call sites in `crates/pptx-render/src/layout.rs`:

1. `paint()` solid branch, `layout.rs:1926` - the shape and slide-background fill.
2. `paint()` gradient stops, `layout.rs:1914` - stop-level `a:alpha` is the same mechanism
   and the same one-line change.
3. `stroke()`, `layout.rs:1931` - `a:ln` alpha (needed by the connector lines on
   `green-solutions/01`, which are also blocked by a separate connector bug).

No display-list, raster or canvas change is needed: `Paint::Solid.color` is already an
untyped string (`crates/pptx-render/src/display_list.rs:25`), `parse_hex_color` already
accepts 8 hex digits (`crates/pptx-raster/src/lib.rs:782-799`), and `ctx.fillStyle` accepts
`#RRGGBBAA` (`packages/pptx/src/render/canvas.ts:183`).

Optional tidy-up: emit `#RRGGBB` when the alpha byte is `FF`, so the overwhelming majority
of decks produce byte-identical display lists to today and only genuinely translucent fills
change. That keeps display-list snapshots and any JS colour-string comparisons stable.

## Sketch

```rust
// crates/pptx-render/src/layout.rs
-use ooxml_drawingml::{..., resolve_color_value_to_hex_with_theme, ...};
+use ooxml_drawingml::{..., resolve_color_value_to_hex_with_theme, resolve_color_value_to_rgba_hex, ...};

 fn paint(fill: &ShapeFill, theme: &Theme) -> Option<Paint> {
     ...
                 Some(GradientStop {
                     position: (stop.position as f32 / 100_000.0).clamp(0.0, 1.0),
-                    color: resolve_color_value_to_hex_with_theme(Some(&stop.color), Some(theme))?,
+                    color: paint_color(Some(&stop.color), theme)?,
                 })
     ...
-    resolve_color_value_to_hex_with_theme(fill.color.as_ref(), Some(theme))
+    paint_color(fill.color.as_ref(), theme)
         .map(|color| Paint::Solid { color })
 }

 fn stroke(outline: &ShapeOutline, theme: &Theme) -> Option<Stroke> {
-    let color = resolve_color_value_to_hex_with_theme(outline.color.as_ref(), Some(theme))?;
+    let color = paint_color(outline.color.as_ref(), theme)?;

+/// Opaque colours stay `#RRGGBB` so only translucent fills change shape.
+fn paint_color(color: Option<&ColorValue>, theme: &Theme) -> Option<String> {
+    let rgba = resolve_color_value_to_rgba_hex(color, Some(theme))?;
+    Some(match rgba.strip_suffix("FF") {
+        Some(rgb) if rgb.len() == 7 => rgb.to_owned(),
+        _ => rgba,
+    })
+}
```

## Risks

- **`alpha="0"`.** A fully transparent fill becomes `#RRGGBB00` rather than `None`. The
  raster paints nothing visible, so this is correct, but a shape that previously painted an
  opaque block will now vanish - which is the intended behaviour and may move goldens.
- **Text colours are untouched.** `valid_color` (`crates/pptx-render/src/layout.rs:1982`)
  requires exactly 6 hex digits, so if the same change is later extended to run properties
  (`layout.rs:1049`, `:1854`) that predicate must be widened to accept 8 as well. Doing
  fills only avoids that edit entirely.
- **`pptx-edit` and `pptx-parse/src/write.rs` keep the opaque resolver.** `write.rs:1398`
  compares two resolved hexes to decide whether a fill is unchanged; leaving it opaque means
  two fills differing only in alpha compare equal there. Out of scope for the render fix,
  but worth a follow-up note.
- **Snapshot churn.** `crates/pptx-raster/tests/golden/*.png` should be unaffected (opaque
  fixtures, and the `FF` short-circuit keeps their strings identical); any that move
  indicate a fixture with an alpha that was silently ignored.

Tests to add: a unit test on `paint()` asserting a `solidFill` with `alpha="33000"` yields
`Paint::Solid { color: "#00000054" }` (0.33 * 255 = 84 = `0x54`) and that an alpha-free
fill still yields `#RRGGBB`; plus one raster golden with a translucent rectangle over an
image so the compositing itself is pinned.

## Effort

`easy` - the resolver, its alpha field and the 8-digit hex parsing on both backends already
exist and are tested; the change is three call sites in one file plus a small helper, and
the only judgement call is whether to keep opaque colours as 6-digit hex.
