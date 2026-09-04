# pptx: a:grpFill (fill inherited from parent group) not resolved

**Describe the bug**

A shape whose `spPr` defers its fill to `<a:grpFill/>` -- "use the fill my enclosing group
declares" -- renders with no fill at all. The shape becomes invisible, and because these
shapes are almost always the coloured backdrop for white text, the text goes with it:
evidence-1.png and evidence-2.png show four numbered callout bars losing their blue and
dark-grey backgrounds while their white body text turns nearly unreadable against the pale
card, and evidence-3.png shows the same for four orange bars. evidence-4.png shows the
other failure shape: sidebar icons built from `custGeom` freeforms with `<a:grpFill/>` and
`<a:ln><a:noFill/></a:ln>` disappear entirely, since with no fill and no stroke there is
nothing left to paint.

Seen on 16 slides across 2 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. cisco-cloud-security/17** Four blue `roundRect` callout bars (ids 60, 63, 66, 69) lose the `13A7E0` fill their parent groups declare; the white bullet text and divider all but vanish.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-grpfill-not-resolved/evidence-1.png)

**2. cisco-cloud-security/18** Same pattern with the dark-grey (`676767`) list bars (ids 51, 54, 57, 60).

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-grpfill-not-resolved/evidence-2.png)

**3. cisco-cloud-security/12** Same pattern with the orange (`F9771D`) list bars (ids 53, 56, 59, 62).

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-grpfill-not-resolved/evidence-3.png)

**4. project20/09** Three sidebar icon groups (`accent1` on the group, `custGeom` + `grpFill` + `noFill` line on the children) render as blank space.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/fill-grpfill-not-resolved/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 5, 7, 8, 9, 10, 12, 15, 16, 17, 18, 23
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 6, 7, 8, 9

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(4, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

Confirmed. `<a:grpFill/>` is never parsed, and a group's own fill is never parsed either, so
there is nothing to inherit and nothing to inherit from.

- `parse_fill` handles `noFill`, `solidFill`, `gradFill` and `blipFill` and falls through to
  `None` for anything else -- [`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565), with the fall-through at
  [`crates/pptx-parse/src/drawing.rs:582`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L582). `grpFill` (and `pattFill`) land there.
- That `None` becomes `Shape.fill` / `Picture.fill` at
  [`crates/pptx-parse/src/drawing.rs:150`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L150) and [`crates/pptx-parse/src/drawing.rs:187`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L187) -- the
  same value the shape would carry if the XML had specified no fill element at all, so the
  information that a fill was requested is lost at parse time.
- `parse_group` reads only `nvGrpSpPr` and `grpSpPr/xfrm`; the `grpSpPr` fill is dropped --
  [`crates/pptx-parse/src/drawing.rs:252`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L252)-`266`. `GroupShape` correspondingly has no `fill`
  field ([`crates/pptx-parse/src/model.rs:261`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L261)), and `node_fill` returns `None` for every
  group ([`crates/pptx-render/src/layout.rs:1760`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1760)).
- Both render paths therefore have nothing to fall back on. `render_parsed_shape` recurses
  through a group carrying only the transform ([`crates/pptx-render/src/layout.rs:490`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L490)) and
  paints the child from `value.fill` alone ([`crates/pptx-render/src/layout.rs:526`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L526)); the
  snapshot path does the same at [`crates/pptx-render/src/layout.rs:362`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L362) and `:384`.
- `pptx-raster` fills only when a paint is present -- [`crates/pptx-raster/src/lib.rs:379`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L379) --
  so `fill: None` plus the usual `<a:ln><a:noFill/></a:ln>` produces literally no pixels.
- The edit layer has the same hole: `seed_shape` stores `fillJson` for shapes and pictures
  ([`crates/pptx-edit/src/deck.rs:131`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L131), `:142`) but the group arm stores nothing
  ([`crates/pptx-edit/src/deck.rs:162`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L162)), so a snapshot-driven render cannot recover the group
  fill either.
- The write side already knows the element exists: `grpFill` is in `FILL_ELEMENTS`
  ([`crates/pptx-parse/src/write.rs:995`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L995)) so a fill edit replaces it correctly. Only the read
  side is missing.

What PowerPoint does: `a:grpFill` means "inherit the fill of the group I belong to", and the
lookup walks up the group chain -- if the immediate parent's `grpSpPr` also says `grpFill`
(or declares no fill), the search continues to its parent.

Scale, measured over the two decks' slide XML: 299 `a:grpFill` elements -- 287 in `spPr`
(245 `p:sp`, 36 `p:cxnSp`, 6 `p:pic`) and 12 in `grpSpPr`. Every one of the 251 `sp`/`pic`
cases resolves to an ancestor group's `solidFill`; none resolves to a gradient, pattern or
picture fill. 224 resolve at the immediate parent, 25 need two levels and 2 need three, so
the upward walk is required, not optional. A concrete multi-level case:
`cisco-cloud-security/05` `Freeform 133` (id 58) sits inside `Group 51` (id 52, `grpFill`)
inside `Group 50` (id 51, `grpFill`) inside `Group 45` (id 46, `solidFill`).

Two adjacent gaps found while confirming this, both out of scope here:

- `p:cxnSp` is not a `ShapeNode` at all -- `parse_shape_children` matches only `sp`, `pic`,
  `graphicFrame` and `grpSp` ([`crates/pptx-parse/src/drawing.rs:110`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L110)-`128`). The 36
  connector `grpFill`s stay invisible after this fix because the connectors themselves are
  never parsed.
- `p:style/a:fillRef` is not parsed anywhere in `pptx-parse` or `pptx-render` (no hit for
  `fillRef` in either crate). It does not affect this issue -- `grpFill` overrides the style
  reference -- but it is the other reason a `spPr` with no explicit fill renders empty.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Resolve the inheritance in `pptx-parse`, where both halves of the information are already in
hand, so nothing downstream (render, raster, edit, react) has to learn about groups.

1. Give `parse_fill` a `grpFill` arm that returns a sentinel `ShapeFill::named("group")`
   ([`crates/pptx-parse/src/drawing.rs:565`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L565)). This distinguishes "defer to my group" from
   "no fill element at all", which today both produce `None`.
2. In `parse_group` ([`crates/pptx-parse/src/drawing.rs:252`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L252)), after the children are parsed,
   read the group's own fill from `grpSpPr` with the same `parse_fill`. If it is a concrete
   fill (not the sentinel), walk the whole child subtree and replace every remaining sentinel
   with a clone of it.

Because `parse_group` recurses bottom-up, this handles nesting for free: an inner group with
its own `solidFill` resolves its children first, so the outer pass finds no sentinels left
there; an inner group that itself carries `grpFill` (12 such groups in the harness decks)
leaves its children's sentinels in place for the outer pass to fill in. That covers the 25
two-level and 2 three-level cases measured in the decks.

3. In `common_slide_data` ([`crates/pptx-parse/src/drawing.rs:55`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L55)-`59`), resolve one last time
   against the `spTree`'s own `grpSpPr` fill, then clear any sentinel that is still
   unresolved back to `None`, so the sentinel never escapes the crate. Without that sweep a
   `"group"` fill type could reach `paint` ([`crates/pptx-render/src/layout.rs:1897`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1897), harmless
   -- it resolves to `None`) and `fill_element` ([`crates/pptx-parse/src/write.rs:1042`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1042), which
   would raise `unsupported fill type`).

Round-tripping is safe: `save.rs` only emits a fill patch when `shape.fill != base.fill`
([`crates/pptx-edit/src/save.rs:204`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L204)), and the baseline is parsed through the same resolution,
so an untouched `grpFill` shape produces no patch and its `<a:grpFill/>` stays in the XML.

The alternative -- add `fill: Option<ShapeFill>` to `GroupShape`, seed it in
[`crates/pptx-edit/src/deck.rs:162`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L162), and thread an inherited fill down both group recursions
([`crates/pptx-render/src/layout.rs:490`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L490) and `:362`) -- keeps the model faithful to the XML
but touches four crates and the snapshot schema for the same pixels. Prefer it only if the
editor later needs to show or edit a group's fill.

```rust
// drawing.rs
const GROUP_FILL: &str = "group";

fn parse_fill(element: &XmlElement) -> Option<ShapeFill> {
    // ... existing noFill / solidFill / gradFill / blipFill arms ...
    if element.child("grpFill").is_some() {
        return Some(ShapeFill::named(GROUP_FILL));
    }
    None
}

fn resolve_group_fill(nodes: &mut [ShapeNode], fill: &ShapeFill) {
    for node in nodes {
        match node {
            ShapeNode::Shape(shape) => replace_sentinel(&mut shape.fill, fill),
            ShapeNode::Picture(picture) => replace_sentinel(&mut picture.fill, fill),
            // descend: an inner group that had no concrete fill left its children deferring
            ShapeNode::Group(group) => resolve_group_fill(&mut group.children, fill),
            ShapeNode::GraphicFrame(_) => {}
        }
    }
}

fn replace_sentinel(slot: &mut Option<ShapeFill>, fill: &ShapeFill) {
    if slot.as_ref().is_some_and(|f| f.fill_type == GROUP_FILL) {
        *slot = Some(fill.clone());
    }
}

// in parse_group, after children are parsed
let mut children = parse_shape_children(element, relationships, part, budget)?;
if let Some(fill) = element.child("grpSpPr").and_then(parse_fill)
    && fill.fill_type != GROUP_FILL
{
    resolve_group_fill(&mut children, &fill);
}

// in common_slide_data, after the top-level tree is parsed: resolve against the
// spTree's own grpSpPr fill, then clear leftovers so the sentinel never escapes.
```

Risks and tests to add:

- **Custom geometry interaction.** Six of the sixteen findings are `custGeom` icons that are
  invisible today only because they have no fill. `geometry_path` falls back to the `rect`
  preset for `"custom"` ([`crates/pptx-render/src/layout.rs:1955`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1955)-`1956`), so resolving their
  fill turns each icon into a solid coloured rectangle -- visually a regression on
  `cisco-cloud-security/05` and `project20/02, 06, 07, 08, 09` until
  `geometry-custom-collapses-to-bbox` lands. Land the two together, or land this one and
  accept the temporary diff on those slides.
- **Scheme colours on the group.** Several groups inherit via `<a:schemeClr val="bg1"/>` /
  `accent1` rather than an sRGB literal. The cloned `ColorValue` is resolved by the same
  `paint`/theme path the child would have used, so `theme-color-scheme-color-resolution-broken`
  applies here too -- do not read a wrong colour after this fix as a grpFill bug.
- **Non-solid group fills.** Cloning a gradient or picture fill onto every child is not what
  PowerPoint does (the group's fill is painted once across the group's bounds and children
  window into it). No such case exists in the harness decks -- all 251 resolve to `solidFill`
  -- but restricting the clone to `solid` (and leaving the sentinel otherwise) would keep the
  approximation honest.
- **Connectors stay broken.** 36 of the 299 `grpFill`s sit on `p:cxnSp`, which
  `parse_shape_children` ([`crates/pptx-parse/src/drawing.rs:110`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L110)) never turns into a
  `ShapeNode`. Do not expect those to appear.

Tests to add: a `parse_fill`/`parse_group` unit test in
`crates/pptx-parse/src/drawing.rs` (module at `:951`) covering a two-level chain -- outer
group `solidFill`, inner group `grpFill`, leaf `grpFill` -- asserting the leaf resolves to
the outer colour; and a `pptx-render` layout test asserting the emitted `Primitive::Shape`
carries the group's `Paint::Solid`, alongside the existing fill assertions near
[`crates/pptx-render/src/layout.rs:2334`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L2334).

**How to verify**

Re-render `cisco-cloud-security` slides 05, 07, 08, 09, 10, 12, 15, 16, 17, 18, 23 and
`project20` slides 02, 06, 07, 08, 09. The bar-and-text slides (12, 17, 18) should go from
~21% fine diff to a small residual: they are near-identical apart from the missing fills, so
the hot cells covering the callout column should collapse. Slides whose `grpFill` shapes are
`custGeom` icons (`cisco-cloud-security/05`, `project20/02`, `06`, `07`, `08`, `09`) will get
*worse-looking* before they get better -- see the risk note in `possible-solution.md` --
because custom geometry currently falls back to the `rect` preset
([`crates/pptx-render/src/layout.rs:1955`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1955)-`1956`), so a newly-resolved fill paints a solid
rectangle where an invisible shape used to be. Judge those slides only after
`geometry-custom-collapses-to-bbox` is fixed, or gate the check on the bar slides.

No existing test covers `grpFill`: `grep -rn grpFill crates/` hits only
[`crates/pptx-parse/src/write.rs:1001`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1001). The nearest coverage is the fill assertions in
`crates/pptx-render/src/layout.rs` tests (around `:2334` and `:2403`) and the
`crates/pptx-raster/tests/golden/shapes.png` golden; neither exercises group inheritance, so
the fix needs new tests rather than updated ones.

**Additional context**

none.

Related issues found in the same run: `geometry-custom-collapses-to-bbox`, `theme-color-scheme-color-resolution-broken`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-parse/src/write.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/fill-grpfill-not-resolved/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
