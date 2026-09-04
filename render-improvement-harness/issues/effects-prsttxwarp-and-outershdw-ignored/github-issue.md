# pptx: Text warp (prstTxWarp) and outer shadow effects not applied

**Describe the bug**

Two unrelated DrawingML properties share one root cause: neither is read out of the XML at all.

`a:bodyPr/a:prstTxWarp` with `prst="textArchUp"` / `"textArchDown"` is a WordArt-style
text-on-a-path transform. The candidate lays the run out as ordinary straight text inside the
shape rectangle and applies only the shape's own `rot`. On ocp-psp-plan/01 the un-warped
`INCENTIVES` label lands flat across the middle of the white centre circle instead of curving
along the bottom cyan band, and `COMMUNITIES` is cut to `COMMUNITI` (evidence-1.png). On
ocp-psp-plan/03 both ring labels become straight diagonals cutting across the panel
(evidence-2.png).

`a:effectLst/a:outerShdw` is a drop shadow. The candidate draws no shadow of any kind: the
project20/04 status diamonds render as flat, hard-edged shapes where the reference has a soft
offset halo (evidence-3.png).

Seen on 3 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact low, estimated effort hard, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. ocp-psp-plan/01** `textArchDown` / `textArchUp` on the three ring labels. Reference curves `COMMUNITIES`, `GROWTH` and `INCENTIVES` around the donut; candidate draws `INCENTIVES` flat inside the centre circle and clips `COMMUNITIES` to `COMMUNITI`

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/effects-prsttxwarp-and-outershdw-ignored/evidence-1.png)

**2. ocp-psp-plan/03** `Powering Partner GROWTH` (`textArchUp`, rot 72.6 deg) and `Deepening COMMUNITY Connection` (`textArchDown`, rot 300.1 deg). Reference arcs them along the navy ring; candidate draws two straight rotated lines

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/effects-prsttxwarp-and-outershdw-ignored/evidence-2.png)

**3. project20/04** the two status diamonds at 6x. Reference has the `outerShdw blurRad="25400" dist="25400" dir="2700000"` halo down-right of each; candidate has nothing. (The missing green dashed connector in the same crop is a separate issue, `unsupported-element`.)

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/effects-prsttxwarp-and-outershdw-ignored/evidence-3.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 3
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

Confirmed, not a hypothesis: both elements are never parsed, so nothing downstream can act on
them.

**`a:prstTxWarp`.** `parse_text_body` reads exactly five things off `a:bodyPr` -- `anchor`,
`vert`, the autofit child, and the four insets ([`crates/pptx-parse/src/drawing.rs:764-788`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L764-L788)).
`prstTxWarp` is not among them, and `TextBody` has no field that could hold it
([`crates/pptx-parse/src/model.rs:269-278`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L269-L278)). The display list has no representation for warped
text either: a `Primitive::TextBox` carries `PositionedTextLine`s whose runs hold
`PositionedGlyph { glyph_id, cluster, x, advance, x_offset, y_offset }`
([`crates/pptx-render/src/display_list.rs:248-255`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L248-L255)) -- a per-glyph pen position on a straight
baseline and no per-glyph rotation. The only rotation the contract can express is the whole-box
`Transform { rotation_deg, flip_h, flip_v }`
([`crates/pptx-render/src/display_list.rs:63-68`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L63-L68)), which is what both backends apply:
`pptx-raster` paints every glyph of a line with the same box transform
([`crates/pptx-raster/src/font.rs:46-67`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L46-L67) and [`crates/pptx-raster/src/font.rs:86-106`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L86-L106)), and the
layout pass emits the box with the shape's `rot` only
([`crates/pptx-render/src/layout.rs:742-755`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L742-L755)). That is exactly what evidence-2.png shows: the
shape rotation is there, the warp is not.

The `COMMUNITI` truncation in evidence-1.png is a compound effect, not warp code: the
un-warped run is wider than the box (the box was authored to fit the *arc*, which is shorter
across than along), and `pptx-raster` clips a `TextBox` to its own rect before painting
([`crates/pptx-raster/src/lib.rs:282-297`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L282-L297), via `clipped` at
[`crates/pptx-raster/src/lib.rs:325-355`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L325-L355)). The clip behaviour itself belongs to
`text-overflow-autofit-not-handled`; fixing the warp removes the overflow that triggers it.

**`a:outerShdw`.** `parse_shape` reads `a:xfrm`, `a:prstGeom`/`a:custGeom`, `a:avLst`, the fill
and the outline off `p:spPr` and stops ([`crates/pptx-parse/src/drawing.rs:138-156`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138-L156));
`parse_picture` does the same ([`crates/pptx-parse/src/drawing.rs:159-190`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L159-L190)). Neither looks at
`a:effectLst`, and neither `Shape` ([`crates/pptx-parse/src/model.rs:197-207`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L197-L207)) nor `Picture`
([`crates/pptx-parse/src/model.rs:211-220`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L211-L220)) nor `ShapeSnapshot` on the edit path
([`crates/pptx-edit/src/model.rs:99-121`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99-L121)) has a field for effects. `Primitive::Shape` and
`Primitive::Image` carry only `fill`, `stroke` and `transform`
([`crates/pptx-render/src/display_list.rs:78-111`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L78-L111)), and `paint_shape` fills the path then strokes
it, with no third pass ([`crates/pptx-raster/src/lib.rs:364-385`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L364-L385)). No code under `crates/`
matches `outerShdw` outside `docx-parse`; the only pptx mention is
[`crates/pptx-parse/src/write.rs:1003`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1003), where `effectLst` appears in `POST_FILL_ELEMENTS` purely
so the writer inserts a replacement fill *before* it. Saving therefore round-trips the element
untouched -- this is a render-side gap only, not data loss.

Two dependencies worth naming:

- The shadow colour on every occurrence here is `<a:schemeClr val="bg1"><a:lumMod val="50000"/>
  <a:alpha val="40000"/></a:schemeClr>`. `resolve_color_value_to_rgba_hex`
  ([`crates/ooxml-drawingml/src/color.rs:91-102`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L91-L102)) already emits `#RRGGBBAA` and `pptx-raster`'s
  `parse_color` already accepts 8-digit hex ([`crates/pptx-raster/src/lib.rs:780-799`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L780-L799)), so the
  alpha path exists -- but the layout pass must use the rgba resolver for shadows, or every
  shadow paints opaque. Same underlying defect as `fill-alpha-modifier-ignored`.
- `tiny-skia` 0.12 has no blur filter (no `blur` symbol anywhere in its sources), so `blurRad`
  has to be hand-rolled: render the shadow shape into an offscreen `Pixmap` and run a separable
  box blur over it before compositing. The browser backend gets this free from `ctx.shadowBlur`;
  [`packages/pptx/src/render/canvas.ts:117-199`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L117-L199) has no shadow handling today.

**Scope across the corpus** (grep over `decks/*/xml/*/slide.xml`): non-identity `prstTxWarp`
appears in exactly one deck -- ocp-psp-plan, 23 occurrences. `prst="textNoShape"` is the
identity warp and must stay a no-op; it appears 158 times across ocp-psp-plan and project20 and
must not be treated as a warp. `a:outerShdw` is far more widespread: project17 476,
cisco-cloud-security 19, project20 14, triangles-corporate 4, typography-trick 3,
green-solutions 1. Only project20/04 produced a finding, because the comparator judged the rest
below the reporting bar -- see `decks/project17/reports/02.md:95`, which explicitly parks the
purple circle's shadow as "present and comparable in both renders" (unverified: that shadow is
probably baked into the source bitmap). So the shadow half is low-severity but high-frequency,
and the warp half is the reverse.

**Recommendation:** split this cluster. The two halves share nothing but a taxonomy label -- no
file, no data structure, no test. `effects-outershdw-not-drawn` is a self-contained medium job
that touches 6 of the 12 decks; `effects-prsttxwarp-not-applied` is a hard job for one deck and
should be scheduled behind it. The combined `hard` in the front matter reflects doing both.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

### A. `a:outerShdw`

1. **Shared model.** Add `OuterShadow` and `ShapeEffects` next to `ShapeFill` / `ShapeOutline`
   in `crates/ooxml-drawingml/src/shape.rs`, holding raw OOXML units plus a `ColorValue`.
2. **Parse.** A `parse_effects(properties)` in `crates/pptx-parse/src/drawing.rs` reading
   `p:spPr/a:effectLst/a:outerShdw`, wired into `parse_shape`
   ([`crates/pptx-parse/src/drawing.rs:138-156`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138-L156)) and `parse_picture`
   ([`crates/pptx-parse/src/drawing.rs:159-190`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L159-L190)). Add `effects: Option<ShapeEffects>` with
   `#[serde(default)]` to `Shape` and `Picture` in `crates/pptx-parse/src/model.rs`, and carry
   it onto `ShapeSnapshot` ([`crates/pptx-edit/src/model.rs:99`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L99), built at
   [`crates/pptx-edit/src/deck.rs:804`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L804)) so the editor render path sees it too.
3. **Display list.** A `Shadow` struct and an optional `shadow` field on `Primitive::Shape` and
   `Primitive::Image` in `crates/pptx-render/src/display_list.rs`. Both are additive and
   `skip_serializing_if`, so `CONTRACT_VERSION` stays at 1 and older readers ignore them.
4. **Layout.** At both emission sites ([`crates/pptx-render/src/layout.rs:401`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L401) for the edit
   snapshot path, [`crates/pptx-render/src/layout.rs:507`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L507) for the parse path) convert EMU to px
   with the same factor the rect uses, turn `dir` (60000ths of a degree) plus `dist` into
   `dx`/`dy`, and resolve the colour with `resolve_color_value_to_rgba_hex`
   ([`crates/ooxml-drawingml/src/color.rs:91`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L91)) -- **not** the opaque resolver, or every shadow
   paints solid.
5. **Raster.** In `paint_shape` ([`crates/pptx-raster/src/lib.rs:364`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L364)), before the fill: offset
   the path, and either fill it straight when the blur rounds to nothing, or render it into a
   scratch `Pixmap` and box-blur that. `tiny-skia` 0.12 ships no blur, so three passes of a
   separable box blur (the standard Gaussian approximation) go in a new private
   `crates/pptx-raster/src/blur.rs`.
6. **Canvas.** `paintShape` in [`packages/pptx/src/render/canvas.ts:117`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L117) sets
   `ctx.shadowColor` / `shadowBlur` / `shadowOffsetX` / `shadowOffsetY` around the fill only,
   then zeroes them so the stroke does not get a second shadow. Mirror `Shadow` in
   [`packages/pptx/src/types.ts:236`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L236).

Deliberately out of scope for a first cut: `innerShdw`, `glow`, `reflection`, `softEdge`,
`effectDag`, shadows on text runs and on chart primitives, and `rotWithShape="1"` (all
occurrences in the corpus are `"0"`).

### B. `a:prstTxWarp`

Keep the warp inside the layout pass and express the result in the display list by giving each
glyph its own rotation, rather than inventing a text-on-path primitive that both backends and
the caret code would have to learn.

1. **Parse.** `warp: Option<String>` on `TextBody` ([`crates/pptx-parse/src/model.rs:269`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L269)), read
   in `parse_text_body` ([`crates/pptx-parse/src/drawing.rs:764`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L764)) from
   `a:bodyPr/a:prstTxWarp/@prst`. `textNoShape` is the identity warp -- map it to `None`; it is
   158 of the 181 warps in the corpus, so treating it as a warp would regress far more than it
   fixes. Cascade it through `BodyCascade` ([`crates/pptx-render/src/layout.rs:761`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L761)) the way
   `anchor` and `vert` already are.
2. **Display list.** `rotation_deg: f32` on `PositionedGlyph`
   ([`crates/pptx-render/src/display_list.rs:248`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L248)), `#[serde(default, skip_serializing_if =
   "is_zero")]`.
3. **Layout.** After `layout_content` has produced the straight lines and after autofit has
   settled, map each glyph onto the arc. Support `textArchUp` and `textArchDown` only; leave
   every other preset unwarped and unbroken.
4. **Raster.** `paint_glyph` ([`crates/pptx-raster/src/font.rs:187`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/font.rs#L187)) already builds a per-glyph
   `Transform`; insert a rotation about the glyph origin.
5. **Canvas.** `paintTextRun` ([`packages/pptx/src/render/canvas.ts:228`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L228)) wraps each rotated
   glyph in `save()`/`translate()`/`rotate()`/`restore()`.
6. **Interaction.** [`packages/pptx-react/src/interactions.ts:136`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx-react/src/interactions.ts#L136) finds a caret by nearest
   straight line. Rather than invert the arc, skip caret placement when a box is warped -- a
   WordArt label is not a text-editing target.

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

Risks and tests to add:

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

**How to verify**

Re-render `ocp-psp-plan` slides 01 and 03 and `project20` slide 04.

- ocp-psp-plan/03 (33.01% today) is dominated by `geometry-custom-collapses-to-bbox` and
  `fill-alpha-modifier-ignored`; the two warped labels are a few hundred pixels, so expect only
  a fraction of a point from this issue alone. Judge it on evidence-2.png's crop, not the
  slide-level number.
- ocp-psp-plan/01 (12.93%): the three ring labels should land on their arcs and `COMMUNITIES`
  should read in full. Same caveat -- the bounding-box wedges dominate the diff.
- project20/04 (8.89%): the ten diamonds should gain their halo. Sub-0.1% on the slide number;
  verify against evidence-3.png.

Because none of the three slides is diff-limited by this issue, gate the work on unit tests
rather than on the slide diff:

- [`crates/pptx-parse/src/drawing.rs:951`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L951) tests -- `prstTxWarp prst="textArchUp"` reaches the
  model, `textNoShape` does not, and `outerShdw`'s attributes and colour reach the model.
- `crates/pptx-parse/src/write.rs` -- a round-trip of a deck carrying `a:effectLst` still has it.
- [`crates/pptx-render/src/layout.rs:2008`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2008) tests -- the `Primitive::Shape` for a shadowed shape
  carries a shadow whose colour is 8-digit hex.
- [`crates/pptx-raster/src/lib.rs:803`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L803) tests -- a shadowed shape puts non-background pixels
  down-right of its geometry and an unshadowed one does not; glyph ink for an arched run leaves
  the straight baseline band.
- `packages/pptx/src/render/canvas.test.ts` -- the canvas backend sets `shadowColor` and
  `shadowBlur` for a shadowed shape.

**Additional context**

none.

Related issues found in the same run: `fill-alpha-modifier-ignored`, `geometry-custom-collapses-to-bbox`, `text-overflow-autofit-not-handled`

Files most likely involved: `crates/pptx-parse/src/model.rs`, `crates/pptx-parse/src/drawing.rs`, `crates/pptx-render/src/display_list.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-raster/src/lib.rs`, `crates/pptx-raster/src/font.rs`, `crates/pptx-edit/src/model.rs`, `crates/pptx-edit/src/deck.rs`, `crates/ooxml-drawingml/src/geometry.rs`, `packages/pptx/src/types.ts`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/effects-prsttxwarp-and-outershdw-ignored/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
