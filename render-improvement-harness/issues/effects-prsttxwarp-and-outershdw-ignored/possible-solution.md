# Possible solution: effects-prsttxwarp-and-outershdw-ignored

Two independent changes. Land the shadow first: it is smaller, it touches six decks, and it
does not disturb the text pipeline. Warp second.

## Approach

### A. `a:outerShdw`

1. **Shared model.** Add `OuterShadow` and `ShapeEffects` next to `ShapeFill` / `ShapeOutline`
   in `crates/ooxml-drawingml/src/shape.rs`, holding raw OOXML units plus a `ColorValue`.
2. **Parse.** A `parse_effects(properties)` in `crates/pptx-parse/src/drawing.rs` reading
   `p:spPr/a:effectLst/a:outerShdw`, wired into `parse_shape`
   (`crates/pptx-parse/src/drawing.rs:138-156`) and `parse_picture`
   (`crates/pptx-parse/src/drawing.rs:159-190`). Add `effects: Option<ShapeEffects>` with
   `#[serde(default)]` to `Shape` and `Picture` in `crates/pptx-parse/src/model.rs`, and carry
   it onto `ShapeSnapshot` (`crates/pptx-edit/src/model.rs:99`, built at
   `crates/pptx-edit/src/deck.rs:804`) so the editor render path sees it too.
3. **Display list.** A `Shadow` struct and an optional `shadow` field on `Primitive::Shape` and
   `Primitive::Image` in `crates/pptx-render/src/display_list.rs`. Both are additive and
   `skip_serializing_if`, so `CONTRACT_VERSION` stays at 1 and older readers ignore them.
4. **Layout.** At both emission sites (`crates/pptx-render/src/layout.rs:401` for the edit
   snapshot path, `crates/pptx-render/src/layout.rs:507` for the parse path) convert EMU to px
   with the same factor the rect uses, turn `dir` (60000ths of a degree) plus `dist` into
   `dx`/`dy`, and resolve the colour with `resolve_color_value_to_rgba_hex`
   (`crates/ooxml-drawingml/src/color.rs:91`) -- **not** the opaque resolver, or every shadow
   paints solid.
5. **Raster.** In `paint_shape` (`crates/pptx-raster/src/lib.rs:364`), before the fill: offset
   the path, and either fill it straight when the blur rounds to nothing, or render it into a
   scratch `Pixmap` and box-blur that. `tiny-skia` 0.12 ships no blur, so three passes of a
   separable box blur (the standard Gaussian approximation) go in a new private
   `crates/pptx-raster/src/blur.rs`.
6. **Canvas.** `paintShape` in `packages/pptx/src/render/canvas.ts:117` sets
   `ctx.shadowColor` / `shadowBlur` / `shadowOffsetX` / `shadowOffsetY` around the fill only,
   then zeroes them so the stroke does not get a second shadow. Mirror `Shadow` in
   `packages/pptx/src/types.ts:236`.

Deliberately out of scope for a first cut: `innerShdw`, `glow`, `reflection`, `softEdge`,
`effectDag`, shadows on text runs and on chart primitives, and `rotWithShape="1"` (all
occurrences in the corpus are `"0"`).

### B. `a:prstTxWarp`

Keep the warp inside the layout pass and express the result in the display list by giving each
glyph its own rotation, rather than inventing a text-on-path primitive that both backends and
the caret code would have to learn.

1. **Parse.** `warp: Option<String>` on `TextBody` (`crates/pptx-parse/src/model.rs:269`), read
   in `parse_text_body` (`crates/pptx-parse/src/drawing.rs:764`) from
   `a:bodyPr/a:prstTxWarp/@prst`. `textNoShape` is the identity warp -- map it to `None`; it is
   158 of the 181 warps in the corpus, so treating it as a warp would regress far more than it
   fixes. Cascade it through `BodyCascade` (`crates/pptx-render/src/layout.rs:761`) the way
   `anchor` and `vert` already are.
2. **Display list.** `rotation_deg: f32` on `PositionedGlyph`
   (`crates/pptx-render/src/display_list.rs:248`), `#[serde(default, skip_serializing_if =
   "is_zero")]`.
3. **Layout.** After `layout_content` has produced the straight lines and after autofit has
   settled, map each glyph onto the arc. Support `textArchUp` and `textArchDown` only; leave
   every other preset unwarped and unbroken.
4. **Raster.** `paint_glyph` (`crates/pptx-raster/src/font.rs:187`) already builds a per-glyph
   `Transform`; insert a rotation about the glyph origin.
5. **Canvas.** `paintTextRun` (`packages/pptx/src/render/canvas.ts:228`) wraps each rotated
   glyph in `save()`/`translate()`/`rotate()`/`restore()`.
6. **Interaction.** `packages/pptx-react/src/interactions.ts:136` finds a caret by nearest
   straight line. Rather than invert the arc, skip caret placement when a box is warped -- a
   WordArt label is not a text-editing target.

## Sketch

```rust
// crates/pptx-render/src/display_list.rs
pub struct Shadow {
    pub color: String, // #RRGGBBAA
    pub blur: f32,     // px
    pub dx: f32,
    pub dy: f32,
}

// crates/pptx-render/src/layout.rs, at both Primitive::Shape sites
fn shadow(effects: Option<&ShapeEffects>, theme: &Theme, scale: f32) -> Option<Shadow> {
    let outer = effects?.shadow.as_ref()?;
    let dir = (outer.direction_60k as f64 / 60_000.0).to_radians();
    let dist = emu_to_px(outer.distance_emu) * scale;
    Some(Shadow {
        color: resolve_color_value_to_rgba_hex(Some(&outer.color), theme)?,
        blur: emu_to_px(outer.blur_emu) * scale,
        dx: (dist as f64 * dir.cos()) as f32,
        dy: (dist as f64 * dir.sin()) as f32,
    })
}

// crates/pptx-raster/src/lib.rs, in paint_shape before the fill
if let Some(shadow) = shadow {
    let offset = Transform::from_translate(shadow.dx, shadow.dy).post_concat(transform);
    if shadow.blur < 0.5 {
        self.pixmap.fill_path(&path, &solid(&shadow.color)?, FillRule::Winding, offset, clip);
    } else {
        // scratch pixmap sized to path bounds + 3*blur, filled, box-blurred 3x, then
        // draw_pixmap'd at its origin under the shape
        self.paint_blurred(&path, shadow, offset, clip)?;
    }
}

// crates/pptx-render/src/layout.rs, warp mapping (textArchUp; Down mirrors in y)
// r is the arc radius the box implies; s is the glyph's pen offset from the line centre.
let r = box_w / 2.0;
let (cx, cy) = (box_x + box_w / 2.0, box_y + r);
let theta = (glyph.x + glyph.advance / 2.0 - line_centre_x) / r; // radians
glyph.x = cx + r * theta.sin();
glyph.y_offset = cy - r * theta.cos();
glyph.rotation_deg = theta.to_degrees();

// crates/pptx-raster/src/font.rs, in paint_glyph
let glyph_transform = Transform::from_translate(x, y)
    .pre_concat(Transform::from_rotate(rotation_deg))
    .pre_concat(Transform::from_row(scale, 0.0, 0.0, -scale, 0.0, 0.0));
```

## Risks

**Shadow.**

- Blur calibration. `blurRad` is an EMU extent; canvas `shadowBlur` is roughly `2 * sigma` and
  a 3-pass box blur of radius `k` approximates `sigma ~ k * 0.9`. The two backends will not
  agree unless the mapping is chosen once and asserted in a test. Pick it by eye against
  project20/04 and cisco-cloud-security, then pin it.
- Opaque shadows are worse than no shadow. If `resolve_color_value_to_rgba_hex` is not used the
  diamonds gain a solid gray blob. Add the 8-digit-hex assertion to the layout test before
  touching the raster.
- Blurring is per-shape allocation. project17/11 carries 413 `outerShdw` elements on one slide;
  size the scratch pixmap to the path bounds rather than the surface, and skip the blur path
  entirely when the resolved colour is fully transparent.
- Z-order. The shadow belongs under its own shape but over everything painted earlier, which
  the current in-order `paint` loop gives for free -- as long as it is drawn inside
  `paint_shape` and not hoisted into a separate pass.

**Warp.**

- `spAutoFit` on warped boxes. PowerPoint sizes the box to the *arc's* bounding box, so the
  straight text the layout measures is wider than the box. Warp after autofit and do not
  re-measure, or the two will fight.
- The arc geometry above is the default 180-degree sweep. `a:avLst` can override it via `adj1`
  (start angle) and `adj2` (sweep); the corpus only has empty `avLst`, so honouring the
  defaults is enough here -- but read the adjust values into the model anyway so the next deck
  does not need a re-parse.
- Every other preset must stay a no-op. There are ~40 `prstTxWarp` presets; shipping a partial
  table that silently mis-warps `textPlain` or `textCanUp` would be a regression on decks that
  render acceptably today.
- Display-list snapshot tests. Adding `rotation_deg` to `PositionedGlyph` will churn any test
  that compares serialized glyph JSON, even though the field is skipped when zero.

## Effort

`hard` combined. The shadow half alone is `medium`: parse, model, one display-list field and
two backends, with the hand-rolled box blur the only non-obvious piece. The warp half alone is
`hard`: it changes the per-glyph contract, needs arc geometry that has no precedent in the
codebase, must not disturb the 158 identity warps, and drags the caret/hit-test code with it --
all to fix three labels in a single deck.
