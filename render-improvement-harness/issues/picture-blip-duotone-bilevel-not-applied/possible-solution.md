# Possible solution: picture-blip-duotone-bilevel-not-applied

## Approach

Reuse the docx model rather than inventing a pptx one. `ImageEffect` already lives in the
shared crate (`crates/ooxml-drawingml/src/picture.rs:35`, re-exported at
`crates/ooxml-drawingml/src/lib.rs:13`) and `crates/docx-parse/src/image.rs:525` is a
working `<a:blip>`-child reader for it. The work is to point pptx at the same type, thread
it through the four layers that have no slot for it, and — unlike docx, whose raster
backend refuses images carrying effects at `crates/docx-raster/src/lib.rs:1133` — actually
apply it in `pptx-raster`.

1. **Parse.** Give `Picture` (`crates/pptx-parse/src/model.rs:211`) an
   `effects: Vec<ImageEffect>` and fill it in `parse_picture`
   (`crates/pptx-parse/src/drawing.rs:159`) from the `<a:blip>` children, in document
   order — order matters, `clrChange` then `duotone` is not `duotone` then `clrChange`.
   Lift `parse_blip_effects` into `ooxml-drawingml` so both formats share one reader, or
   mirror it if the two `XmlElement` types differ. `duotone` and `clrChange` need their
   colours resolved, which docx never did: `ImageEffect::colors` is the field for it, and
   `parse_color_container` (`crates/pptx-parse/src/drawing.rs:654`) already handles
   `srgbClr`/`schemeClr`/`sysClr`/`prstClr` with `shade`/`tint`/`satMod` — but it returns
   the *first* colour child, so `duotone`'s two children need iterating, not one call.
   Resolution to hex needs the theme, which the parser does not have; either store two
   `ColorValue`s and resolve in layout the way `crates/pptx-render/src/layout.rs:1926`
   does, or pass the theme down. Storing `ColorValue` is the smaller change.

2. **Snapshot.** Add `effectsJson` beside `fillJson` in the picture arm at
   `crates/pptx-edit/src/deck.rs:139`, a field on `ShapeSnapshot`
   (`crates/pptx-edit/src/model.rs:99`), and the read-back at
   `crates/pptx-edit/src/deck.rs:823`. Without this the slide path
   (`crates/pptx-render/src/layout.rs:425`) still sees nothing — master and layout
   pictures reach the other arm, so skipping this fixes only half the deck.

3. **Contract.** Add an optional `effects` to `Primitive::Image`
   (`crates/pptx-render/src/display_list.rs:99`), skip-serialized so a plain picture keeps
   today's JSON and `CONTRACT_VERSION` can stay at 1; mirror it on `ImagePrimitive`
   (`packages/pptx/src/types.ts:246`), reusing the `ImageEffect` shape already declared at
   `packages/docx/src/types/content/image.ts:108`. Populate it in all three producers:
   `crates/pptx-render/src/layout.rs:425`, `crates/pptx-render/src/layout.rs:534` and
   `crates/pptx-render/src/lib.rs:200`.

4. **Raster.** `ImageCache` (`crates/pptx-raster/src/lib.rs:723`) is a budget counter, not
   a map — `decode` re-decodes on every call — so the transform can mutate the returned
   `Pixmap` in place in `paint_image` (`crates/pptx-raster/src/lib.rs:391`) with no cache
   key to worry about. The one trap is that `decode` premultiplies before returning
   (`crates/pptx-raster/src/lib.rs:758`), so a colour transform must unpremultiply, apply,
   and premultiply again, or be applied inside `decode` before that loop. Applying it
   before the premultiply loop is cleaner and one pass cheaper.

5. **Canvas.** `packages/docx/src/layout/render/canvasBackend.ts:1406` is the reference:
   `biLevel` becomes `grayscale(1) contrast(N)`. Port that switch into `paintImage`
   (`packages/pptx/src/render/canvas.ts:208`) via `ctx.filter`. It approximates rather than
   thresholds, and it cannot express `duotone` or `clrChange` at all — an SVG
   `feComponentTransfer` + `feColorMatrix` filter, or an offscreen per-pixel pass, is the
   honest option there. Approximating in canvas while the raster path is exact means the
   two backends disagree on these pictures; decide that deliberately.

## Sketch

```rust
// crates/pptx-raster/src/lib.rs, applied to the RGBA buffer before premultiplying
fn apply_effects(pixels: &mut [[u8; 4]], effects: &[ResolvedEffect]) {
    for effect in effects {
        match effect {
            // luma < thresh -> black, else white; alpha untouched
            ResolvedEffect::BiLevel { threshold } => {
                for p in pixels.iter_mut() {
                    let luma = 0.299 * f32::from(p[0])
                        + 0.587 * f32::from(p[1])
                        + 0.114 * f32::from(p[2]);
                    let v = if luma < threshold * 255.0 { 0 } else { 255 };
                    (p[0], p[1], p[2]) = (v, v, v);
                }
            }
            // luma lerps between the shadow and highlight colours
            ResolvedEffect::Duotone { shadow, highlight } => {
                for p in pixels.iter_mut() {
                    let t = (0.299 * f32::from(p[0])
                        + 0.587 * f32::from(p[1])
                        + 0.114 * f32::from(p[2])) / 255.0;
                    for c in 0..3 {
                        p[c] = (f32::from(shadow[c]) * (1.0 - t)
                            + f32::from(highlight[c]) * t) as u8;
                    }
                }
            }
            ResolvedEffect::Grayscale => { /* ... */ }
        }
    }
}
```

```rust
// crates/pptx-parse/src/drawing.rs, in parse_picture
effects: blip_fill
    .and_then(|value| value.child("blip"))
    .map(parse_blip_effects)
    .unwrap_or_default(),
```

## Risks

- **`clrChange` can erase artwork.** It is the one effect in this family that changes
  alpha, and `clrFrom="FFFFFF"` → `clrTo` with `alpha="0"` is exactly the pattern in this
  deck (5 occurrences, `crates/pptx-parse` parses none). Ship it in the same pass as
  `duotone`/`biLevel` or the pictures that pair the two will get half a treatment and look
  worse than they do today. Match on RGB with a tolerance; an exact-equality match on
  antialiased edges leaves a fringe.
- **Premultiplied alpha.** Transforming premultiplied bytes silently darkens semi-
  transparent edges. Every one of these logos is transparent-background PNG, so the bug
  would show as a halo on exactly the pictures this issue is about.
- **Effect order.** `<a:blip>` children are an ordered sequence; applying them as an
  unordered set changes the result whenever `clrChange` and `duotone` co-occur, which is
  4 of the 5 `clrChange` sites here.
- **`duotone` colour resolution needs the theme.** `schemeClr val="bg2"` with `shade` and
  `satMod` is the common form; resolving it in the parser (which has no theme) rather than
  in layout would silently produce black.
- **Two backends diverging.** The canvas filter approximation and an exact raster
  transform will not match pixel for pixel; the golden tests only cover raster, so the gap
  will not be caught automatically.
- Tests to add: a `pptx-parse` unit test for a `<a:blip>` carrying `biLevel`, `duotone`
  and `clrChange` in order; a layout assertion at `crates/pptx-render/src/layout.rs:2008`
  that both picture arms emit the effects; `biLevel` and `duotone` goldens beside
  `golden_image` (`crates/pptx-raster/tests/golden.rs:283`); a premultiply round-trip
  assertion on a semi-transparent source.

## Effort

Medium. The parse step is close to a copy of `crates/docx-parse/src/image.rs:525` and the
raster transform is thirty lines, but the value has to cross five layers that all lack a
field for it (parse model, edit snapshot, display-list contract, raster, TS canvas),
`duotone` needs theme-aware colour resolution that docx never implemented, and `clrChange`
has to come along or the paired pictures regress.
