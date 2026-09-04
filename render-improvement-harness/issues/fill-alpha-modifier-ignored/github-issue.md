# pptx: Alpha/opacity modifier on solid fills ignored

**Describe the bug**

An `<a:alpha>` child on a `srgbClr` or `schemeClr` inside a `solidFill` is dropped between
parsing and painting, so every translucent shape paints at 100% opacity. Where the
translucent shape is a full-slide scrim the effect is catastrophic: `green-solutions/01`
renders as a solid black slide with the city photo completely hidden behind a 33%-black
rectangle (`evidence-1.png`, 74.5% diff), and `project20/01` renders as three flat
gray/near-white panels instead of a tinted mountain banner (`evidence-2.png`, 46.3% diff).
Where the alpha is mild the effect is a flat, textureless fill - the 97%-opaque navy square
on `project20/10` and `/15` loses the bi-level texture bleeding through it
(``).

Pixel sampling confirms the fill is unblended rather than mis-blended. On `project20/10`
BetterOffice reports exactly `(36, 38, 93)` = `#24265D` at (60,150), (200,300) and
(120,550); LibreOffice reports `(42, 44, 97)` at all three, which is
`0x24 * 0.97 + 255 * 0.03 = 42.6` - the declared `alpha="97000"` composited over white. On
`green-solutions/01` BetterOffice reports `(0, 0, 0)` across the whole photo area and
`(242, 242, 242)` inside the icon circles (`bg1` at `lumMod 95000`, i.e. the raw resolved
color) where LibreOffice reports photo pixels such as `(5, 48, 79)` and `(165, 148, 136)`.

Seen on 6 slides across 3 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. green-solutions/01** Full slide. `Rectangle 109`, a slide-sized `tx1` scrim at `alpha="33000"`, paints solid black over the full-bleed JPEG; the `bg1`/`lumMod 95000`/`alpha 50000` icon circles paint solid white instead of showing the photo through.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-alpha-modifier-ignored/evidence-1.png)

**2. project20/01** Full slide. Three full-height layout rectangles (`353535` at `alpha="30196"` twice, `bg1`+`lumMod 95000` at `alpha="80000"`) tile the slide width and paint opaque, hiding the layout's mountain banner entirely.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-alpha-modifier-ignored/evidence-2.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `Unique Way To Showcase Your Green Solutions in Microsoft PowerPoint (PPT).pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1
- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 3
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 6, 10, 15

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(2, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed.** `a:alpha` is parsed, stored on the model, and then discarded by the only
resolver the pptx render path calls.

- Parse: [`crates/pptx-parse/src/drawing.rs:691`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L691) reads `<a:alpha>` into `ColorValue::alpha`
 as a fraction, alongside `lumMod`/`lumOff`/`satMod`
 ([`crates/pptx-parse/src/drawing.rs:685-691`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L685-L691)). The field is declared at
 [`crates/ooxml-drawingml/src/color.rs:30`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L30) and its doc comment already states the problem:
 "Opaque hex output drops it; use [`resolve_color_value_to_rgba_hex`]."
- Model: the value survives into `ShapeFill::color`
 ([`crates/ooxml-drawingml/src/shape.rs:11`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L11)) and `ShapeOutline::color`
 ([`crates/ooxml-drawingml/src/shape.rs:47`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L47)) as a full `ColorValue`.
- Drop: `paint` at [`crates/pptx-render/src/layout.rs:1897`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1897) is the single funnel for every
 shape fill and for the slide background (`layout.rs:183`, `:388`, `:526`). Its last
 statement, [`crates/pptx-render/src/layout.rs:1926`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1926), calls
 `resolve_color_value_to_hex_with_theme`, which returns `#RRGGBB`
 ([`crates/ooxml-drawingml/src/color.rs:61-85`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L61-L85)) and never reads `color.alpha`. `stroke`
 at [`crates/pptx-render/src/layout.rs:1931`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1931) does the same for `a:ln`. Gradient stops go
 through the same opaque resolver at [`crates/pptx-render/src/layout.rs:1914`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1914).
- The correct resolver already exists and is unit-tested -
 `resolve_color_value_to_rgba_hex` at [`crates/ooxml-drawingml/src/color.rs:91`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L91), test
 `alpha_reaches_the_rgba_resolver_and_never_the_opaque_one` at
 [`crates/ooxml-drawingml/src/color.rs:236`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L236) - but a repo-wide grep for it returns only its
 own definition and that test. It has **no** production caller in any crate.

Both consumers of the display list already accept the 8-digit form, so nothing downstream
needs to change:

- Raster: `parse_color` ([`crates/pptx-raster/src/lib.rs:770`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-raster/src/lib.rs#L770)) delegates to
 `parse_hex_color` ([`crates/pptx-raster/src/lib.rs:780`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-raster/src/lib.rs#L780)), whose length match accepts
 `6 | 8` and reads the alpha byte at [`crates/pptx-raster/src/lib.rs:798`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-raster/src/lib.rs#L798). tiny-skia's
 `fill_path` ([`crates/pptx-raster/src/lib.rs:382`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-raster/src/lib.rs#L382)) composites source-over by default.
- Canvas: `Paint::Solid.color` is a plain string
 ([`crates/pptx-render/src/display_list.rs:24-27`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L24-L27)) that `paintStyle` hands straight to
 `ctx.fillStyle` ([`packages/pptx/src/render/canvas.ts:183`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L183)), and Canvas2D accepts
 `#RRGGBBAA`.

**Reattribution.** `green-solutions/01/1` was originally filed as a picture failure ("the
full-bleed JPEG is not drawn"). It is not: `decks/green-solutions/bo-log.json` records
`"01".skipped_images = 0`, so the JPEG was decoded and drawn, and the
`picture-fill-fails-to-render` investigation confirmed empirically that deleting
`Rectangle 109` from `slide1.xml` makes the photo appear. The finding has been moved into
this cluster and out of that one.

**Not confirmed / out of scope.** Two adjacent alpha paths are left alone deliberately and
are *not* claimed to be fixed by this issue:

- Run-level text alpha also goes through the opaque resolver
 ([`crates/pptx-render/src/layout.rs:1049`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1049), `:1854`), and `valid_color`
 ([`crates/pptx-render/src/layout.rs:1982`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1982)) hard-requires a 6-digit hex, so widening text
 color to RGBA is a larger change than the fill fix.
- Picture transparency (`a:alphaModFix` on a `blipFill`) is a separate mechanism and is not
 covered here.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Switch the two pptx shape-paint resolvers from the opaque hex resolver to the RGBA one that
already exists and is already tested. `resolve_color_value_to_rgba_hex`
([`crates/ooxml-drawingml/src/color.rs:91`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L91)) wraps `resolve_color_value_to_hex_with_theme` and
appends the alpha byte, so hue/lumMod/satMod handling is unchanged and colors without
`a:alpha` come back as `#RRGGBBFF`.

Three call sites in `crates/pptx-render/src/layout.rs`:

1. `paint` solid branch, `layout.rs:1926` - the shape and slide-background fill.
2. `paint` gradient stops, `layout.rs:1914` - stop-level `a:alpha` is the same mechanism
 and the same one-line change.
3. `stroke`, `layout.rs:1931` - `a:ln` alpha (needed by the connector lines on
 `green-solutions/01`, which are also blocked by a separate connector bug).

No display-list, raster or canvas change is needed: `Paint::Solid.color` is already an
untyped string ([`crates/pptx-render/src/display_list.rs:25`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L25)), `parse_hex_color` already
accepts 8 hex digits ([`crates/pptx-raster/src/lib.rs:782-799`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-raster/src/lib.rs#L782-L799)), and `ctx.fillStyle` accepts
`#RRGGBBAA` ([`packages/pptx/src/render/canvas.ts:183`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L183)).

Optional tidy-up: emit `#RRGGBB` when the alpha byte is `FF`, so the overwhelming majority
of decks produce byte-identical display lists to today and only genuinely translucent fills
change. That keeps display-list snapshots and any JS colour-string comparisons stable.

```rust
// crates/pptx-render/src/layout.rs
-use ooxml_drawingml::{..., resolve_color_value_to_hex_with_theme, ...};
+use ooxml_drawingml::{..., resolve_color_value_to_hex_with_theme, resolve_color_value_to_rgba_hex, ...};

 fn paint(fill: &ShapeFill, theme: &Theme) -> Option<Paint> {
 ...
 Some(GradientStop {
 position: (stop.position as f32 / 100_000.0).clamp(0.0, 1.0),
- color: resolve_color_value_to_hex_with_theme(Some(&stop.color), Some(theme))?,
+ color: paint_color(Some(&stop.color), theme)?,
 })
 ...
- resolve_color_value_to_hex_with_theme(fill.color.as_ref, Some(theme))
+ paint_color(fill.color.as_ref, theme)
 .map(|color| Paint::Solid { color })
 }

 fn stroke(outline: &ShapeOutline, theme: &Theme) -> Option<Stroke> {
- let color = resolve_color_value_to_hex_with_theme(outline.color.as_ref, Some(theme))?;
+ let color = paint_color(outline.color.as_ref, theme)?;

+/// Opaque colours stay `#RRGGBB` so only translucent fills change shape.
+fn paint_color(color: Option<&ColorValue>, theme: &Theme) -> Option<String> {
+ let rgba = resolve_color_value_to_rgba_hex(color, Some(theme))?;
+ Some(match rgba.strip_suffix("FF") {
+ Some(rgb) if rgb.len == 7 => rgb.to_owned,
+ _ => rgba,
+ })
+}
```

Risks and tests to add:

- **`alpha="0"`.** A fully transparent fill becomes `#RRGGBB00` rather than `None`. The
 raster paints nothing visible, so this is correct, but a shape that previously painted an
 opaque block will now vanish - which is the intended behaviour and may move goldens.
- **Text colours are untouched.** `valid_color` ([`crates/pptx-render/src/layout.rs:1982`](https://github.com/dsaad68/betteroffice/blob/a8cedf637f3a39ff23e8fc4aef588e5b26442e03/crates/pptx-render/src/layout.rs#L1982))
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

Tests to add: a unit test on `paint` asserting a `solidFill` with `alpha="33000"` yields
`Paint::Solid { color: "#00000054" }` (0.33 * 255 = 84 = `0x54`) and that an alpha-free
fill still yields `#RRGGBB`; plus one raster golden with a translucent rectangle over an
image so the compositing itself is pinned.

**How to verify**

- Re-render `green-solutions` 01, `project20` 01/06/10/15 and `ocp-psp-plan` 03.
 `green-solutions/01` (`fine_pct` 74.52) and `project20/01` (46.33) should fall
 dramatically - those two are dominated by this single defect. `project20/15` (1.81) and
 `project20/10` (5.32) should drop to near the texture's own contribution.
- `ocp-psp-plan/03` (33.01) will improve but **not** resolve: the same shapes are drawn as
 bounding boxes rather than Bezier crescents (`geometry-custom-collapses-to-bbox`), so
 expect a translucent rectangle instead of an opaque one.
- Pixel check without a diff run: sample `project20/10` at (60,150) - the candidate must
 move from `(36, 38, 93)` toward LibreOffice's `(42, 44, 97)`.
- Existing coverage: [`crates/ooxml-drawingml/src/color.rs:236`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/color.rs#L236) already pins the resolver's
 behaviour; the missing test is one asserting `paint` emits `#RRGGBBAA`. The raster
 goldens under `crates/pptx-raster/tests/golden/` use opaque fixture fills, so they should
 not move; if any does, that fixture has an alpha nobody noticed.

**Additional context**

none.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`, `picture-fill-fails-to-render`

Files most likely involved: `crates/pptx-render/src/layout.rs`, `crates/ooxml-drawingml/src/color.rs`, `crates/pptx-parse/src/drawing.rs`, `crates/pptx-raster/src/lib.rs`, `crates/pptx-render/src/display_list.rs`, `packages/pptx/src/render/canvas.ts`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/fill-alpha-modifier-ignored/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
