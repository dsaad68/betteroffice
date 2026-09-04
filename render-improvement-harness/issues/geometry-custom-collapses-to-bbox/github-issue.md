# pptx: custGeom shapes render as their bounding-box rectangle

**Describe the bug**

Every `<a:custGeom>` shape paints as a plain axis-aligned rectangle spanning its `a:off`/`a:ext`
box, in the shape's own fill and stroke, with none of the authored outline. A donut chart becomes
a pile of overlapping rectangles (`evidence-1.png`); a honeycomb of hexagons becomes a plus sign
of white boxes (`evidence-2.png`); 131 person silhouettes become 131 blue squares
(`evidence-3.png`); a Venn diagram of two traced circles plus a lens and a handshake glyph becomes
two grey slabs and a white square (`evidence-4.png`). The failure does not depend on how the path
is written — literal `lnTo` polygons, `cubicBezTo` wedges, and paths whose shape carries legacy
`T0..Tn` or `connsiteX/Y` guides all collapse the same way.

At 17 findings across 4 decks this is the single largest visual defect in the corpus; on
`cisco-cloud-security/04` and `/19` it drives every top hot cell.

Seen on 17 slides across 4 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact high, estimated effort medium, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. ocp-psp-plan/01** 21 `cubicBezTo` donut wedges (`Freeform 6..46`, ids 3-23) render as stacked rectangles; the ring, its label segments and the outer band all disappear.

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-custom-collapses-to-bbox/evidence-1.png)

**2. project17/08** Seven literal 6-point hexagons (`Freeform 5/6/7/8/9/11/18`, ids 10-20, path `w="856" h="584"`) render as white rectangles, leaving purple gaps where the corners were cut.

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-custom-collapses-to-bbox/evidence-2.png)

**3. cisco-cloud-security/04** The person-silhouette freeforms inside every node group (`Freeform 994` id 6 and 130 siblings) render as filled squares inside their correct circles.

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-custom-collapses-to-bbox/evidence-3.png)

**4. project17/04** `Freeform 58`/`59` (two ~150-point traced circles) become grey slabs, `Freeform 57` (the lens) is not visible at all, and `Freeform 16` (the bezier handshake) becomes a white box.

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-custom-collapses-to-bbox/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `cisco-cloud-security.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 2, 3, 4, 5, 6, 7, 11, 13, 16, 19, 20
- `ocp-psp-plan.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 1, 3
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 4, 8, 11
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 16

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png(1, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. PowerPoint and LibreOffice agree on this behaviour; the XML in the report shows the property that should be honoured.

**Root cause**

**Confirmed. The path data is never parsed; the renderer then substitutes a rectangle.**

1. `parse_geometry` ([`crates/pptx-parse/src/drawing.rs:335`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L335)) is the only code in `crates/pptx-parse`
   that touches `custGeom`. It checks for the element's presence and returns the string `"custom"`
   ([`crates/pptx-parse/src/drawing.rs:341-343`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L341-L343)); `a:pathLst` is never read. `Shape`
   ([`crates/pptx-parse/src/model.rs:198`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L198)) accordingly carries only `geometry: String`
   ([`crates/pptx-parse/src/model.rs:201`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L201)) and `adjust_values`
   ([`crates/pptx-parse/src/model.rs:203`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L203)), and `parse_adjust_values`
   ([`crates/pptx-parse/src/drawing.rs:355-359`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L355-L359)) reads guides from `prstGeom/avLst` only — a
   `custGeom`'s `gdLst` is not evaluated either.

2. Every layout path turns that string into a shape by asking the preset table for it.
   `geometry_path` ([`crates/pptx-render/src/layout.rs:1946`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1946)) calls
   `preset_geometry_to_path(geometry, ...)` and, when that returns `None`, falls back to
   `preset_geometry_to_path("rect", ...)` ([`crates/pptx-render/src/layout.rs:1955-1957`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L1955-L1957)).
   `preset_geometry_to_path` ([`crates/ooxml-drawingml/src/geometry.rs:38`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L38)) has no `"custom"` arm and
   ends in `_ => return None` ([`crates/ooxml-drawingml/src/geometry.rs:227`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L227)), so the fallback always
   fires. That is the rectangle on screen.

3. All three emit sites share the same helper, which is why no deck escapes it: slide shapes at
   [`crates/pptx-render/src/layout.rs:410`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L410) (snapshot path), master/layout shapes at
   [`crates/pptx-render/src/layout.rs:516`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L516) (parsed path), and the host-composed path at
   [`crates/pptx-render/src/lib.rs:170`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L170) via its own copy of the same fallback at
   [`crates/pptx-render/src/lib.rs:237-250`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L237-L250).

4. The edit snapshot cannot carry a path even if one existed. `seed_shape` writes only `geometry`
   and `adjustValuesJson` into the collaborative document ([`crates/pptx-edit/src/deck.rs:125-129`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L125-L129)),
   and `ShapeSnapshot` ([`crates/pptx-edit/src/model.rs:111`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L111)) has no path field, so the read-back at
   [`crates/pptx-edit/src/deck.rs:816`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L816) has nothing to return.

**Nothing downstream needs to change.** The display-list contract already carries arbitrary
geometry: `Primitive::Shape` holds `path: Vec<GeometryPathCommand>`
([`crates/pptx-render/src/display_list.rs:89`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L89)), the raster backend scales those unit-space commands
into the primitive box in `geometry_path` ([`crates/pptx-raster/src/lib.rs:575-600`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L575-L600), used at
[`crates/pptx-raster/src/lib.rs:376`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L376)), and the canvas backend does the same in `buildPath`
([`packages/pptx/src/render/canvas.ts:118`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L118), typed at [`packages/pptx/src/types.ts:240`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/types.ts#L240)). The chart
renderer already exercises exactly this: `PlotOp::Path` emits `geometry: "custom"` with real
commands at [`crates/pptx-render/src/chart.rs:155-179`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L155-L179), and both backends draw those pie wedges
correctly. So the gap is confined to the shape parse to layout hand-off.

**Working prior art exists in this repo.** `docx-parse` implements the whole feature:
`parse_custom_geometry_path` ([`crates/docx-parse/src/drawingml.rs:276`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L276)), the per-path walker
`parse_custom_path` ([`crates/docx-parse/src/drawingml.rs:297`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L297)) covering
`moveTo`/`lnTo`/`quadBezTo`/`cubicBezTo`/`arcTo`/`close`, `arc_to_cubics`
([`crates/docx-parse/src/drawingml.rs:395`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L395)), unit-space normalisation by the path's own `w`/`h` in
`normalize_raw_path` ([`crates/docx-parse/src/drawingml.rs:434`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L434)), guide seeding in
`build_custom_guides` ([`crates/docx-parse/src/drawingml.rs:529`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L529)) and the formula evaluator
`evaluate_guide` ([`crates/docx-parse/src/drawingml.rs:604`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L604)), bounded by
`MAX_CUSTOM_PATH_COMMANDS = 2_048` and `MAX_CUSTOM_GUIDES = 512`
([`crates/docx-parse/src/drawingml.rs:14-15`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L14-L15)). It is wired in at
[`crates/docx-parse/src/shape.rs:418-420`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L418-L420) onto `Shape::geometry_path`
([`crates/docx-parse/src/shape.rs:341`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/shape.rs#L341)). It lives in `docx-parse`, not in the shared
`ooxml-drawingml`, and is written against `docx_parse::xml::XmlElement`, which is a different type
from `pptx_parse::xml::XmlElement` ([`crates/pptx-parse/src/drawing.rs:8`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L8)) — so it is a port or a
generalisation, not a re-export. `pptx-parse` does already have its own guide-formula evaluator,
`evaluate_guide_formula` ([`crates/pptx-parse/src/drawing.rs:445`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L445)), covering `val`, `*/`, `+-`, `+/`,
`?:`, `abs`, `at2`, `cat2`, `cos`, `max`, `min`, `mod`, `pin`, `sat2`, `sin`, `sqrt` — it is only
ever fed `prstGeom/avLst` today.

### What the corpus actually needs (measured)

Scanning all 41 slide/layout/master XML parts under `render-improvement-harness/decks` that contain
`custGeom`:

- 959 `a:path` elements, **all** with both `w` and `h`, and **every** `pathLst` holds exactly one
  path. No multi-subpath shapes, and no `fill=`/`stroke=` path attributes to honour.
- 67,809 `a:pt` coordinates, **zero** of them non-numeric — no point in this corpus resolves through
  a guide name. The `T0..Tn` and `connsiteX/Y` guides that several findings cite are real, but they
  feed `a:cxnLst` (connection sites), not the drawn outline; the paths beside them use literal
  integers. Two findings state those guides "reach the drawn outline" — **that part is not
  confirmed, and the XML contradicts it.**
- Command mix: 58,552 `lnTo`, 2,673 `cubicBezTo`, 1,238 `moveTo`, 828 `close`. **Zero `arcTo` and
  zero `quadBezTo`.**

So a port that handles `moveTo`/`lnTo`/`cubicBezTo`/`close` with literal coordinates fixes all 17
findings; guides, `arcTo` and `quadBezTo` are correctness work for decks outside this corpus.

### Two findings in this cluster will not be fully fixed by this change

- `cisco-cloud-security/13/2` ("icon entirely missing"): both `custGeom` shapes on that slide use
  `<a:grpFill/>` (`Freeform 39` id 104, `Freeform 40` id 105). With no resolved fill and
  `<a:ln w="0"><a:noFill/>`, the raster paints nothing at
  [`crates/pptx-raster/src/lib.rs:376-386`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L376-L386) whatever the path is. Blocked behind
  `fill-grpfill-not-resolved`. `cisco-cloud-security/11` is the same story at scale: 42 `custGeom`
  shapes against 50 `grpFill` uses, which is why that finding reads "or not drawn at all".
- `project17/04/3`: `Freeform 57`, the purple lens, is `<a:solidFill><a:srgbClr val="48365A"/>` but
  is invisible in the candidate even as a rectangle. Document order inside `Group 46` is 57, 58, 59,
  16, so the two circles paint over it — yet LibreOffice shows the lens on top. **Not confirmed**;
  this looks like a separate z-order or alpha-compositing question and was not investigated here.

_(hypothesis, not yet confirmed by a fix)_

**Suggested fix**

Parse `a:custGeom/a:pathLst` in `pptx-parse` into the shared
`ooxml_drawingml::GeometryPathCommand` vocabulary, normalised to the same 0..1 unit space the
preset table already emits, hang it off `Shape`, and have the three layout emit sites prefer it over
`preset_geometry_to_path`. Nothing below the display list changes: `Primitive::Shape.path`
([`crates/pptx-render/src/display_list.rs:89`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/display_list.rs#L89)), [`crates/pptx-raster/src/lib.rs:575`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L575) and
[`packages/pptx/src/render/canvas.ts:118`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/packages/pptx/src/render/canvas.ts#L118) already draw arbitrary command lists, and
[`crates/pptx-render/src/chart.rs:155`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L155) already ships `geometry: "custom"` primitives through them.

Four edits, in dependency order:

1. **`crates/pptx-parse/src/drawing.rs`** — port `parse_custom_geometry_path` /
   `parse_custom_path` / `normalize_raw_path` from [`crates/docx-parse/src/drawingml.rs:276-527`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L276-L527),
   swapping `docx_parse::xml::XmlElement` calls (`child_by_full_name`, `children_by_local_name`,
   `local_child`) for the `pptx_parse::xml::XmlElement` equivalents already used in this file
   (`child`, `child_elements`, `local_name`). Resolve point coordinates through the existing
   `evaluate_guide_formula` ([`crates/pptx-parse/src/drawing.rs:445`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L445)) seeded with `gdLst` and the
   path's own `w`/`h`, falling back to a plain numeric parse — the corpus needs only the numeric
   branch, but the guide branch is what makes the port correct for other decks. Keep the
   `MAX_CUSTOM_PATH_COMMANDS` / `MAX_CUSTOM_GUIDES` caps; charge nothing extra against
   `ParseBudget`, which already bounds XML events upstream.

   Prefer moving the shared half (`normalize_raw_path`, `arc_to_cubics`, `standard_guide_values`,
   the command builder) into `crates/ooxml-drawingml` behind a tiny element-accessor trait, so
   `docx-parse` and `pptx-parse` stop carrying two copies. Duplicating into `pptx-parse` is the
   cheaper first cut; say which one the PR takes.

2. **`crates/pptx-parse/src/model.rs`** — add
   `pub geometry_path: Option<Vec<GeometryPathCommand>>` to `Shape` (beside `geometry` at
   [`crates/pptx-parse/src/model.rs:201`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L201)), `#[serde(default, skip_serializing_if = "Option::is_none")]`
   so existing serialised models still deserialise.

3. **`crates/pptx-render`** — thread it through the two `geometry_path(...)` calls at
   [`crates/pptx-render/src/layout.rs:410`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L410) and [`crates/pptx-render/src/layout.rs:516`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L516), and add an
   optional `path` to `ComposedShape::Shape` for [`crates/pptx-render/src/lib.rs:170`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/lib.rs#L170). Keep the
   rect fallback for a `custGeom` that parses to nothing.

4. **`crates/pptx-edit`** — store the commands as `geometryPathJson` next to `adjustValuesJson`
   ([`crates/pptx-edit/src/deck.rs:126-129`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L126-L129)), add the field to `ShapeSnapshot`
   ([`crates/pptx-edit/src/model.rs:111`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/model.rs#L111)) and read it back at [`crates/pptx-edit/src/deck.rs:816`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L816).
   That is a document-shape change, so bump `SCHEMA_VERSION` 2.0 -> 3.0
   ([`crates/pptx-edit/src/deck.rs:23`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L23)) and extend `MIGRATABLE_SCHEMA_VERSIONS`
   ([`crates/pptx-edit/src/deck.rs:25`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L25)); a v2 document simply has no path key and keeps rendering as
   it does today.

[`crates/pptx-parse/src/write.rs:1716`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L1716) needs no change — it only refuses to *author* a new `custom`
shape, and writing is part-preserving, so existing `custGeom` XML round-trips untouched.

```rust
// crates/pptx-parse/src/drawing.rs
fn parse_custom_geometry_path(geometry: Option<&XmlElement>) -> Option<Vec<GeometryPathCommand>> {
    let path = geometry?.child("pathLst")?.child("path")?; // corpus: always exactly one
    let width = numeric_attribute(Some(path), "w")? as f64;
    let height = numeric_attribute(Some(path), "h")? as f64;
    let guides = custom_guide_values(geometry, width, height); // gdLst via evaluate_guide_formula
    let mut out = Vec::new();
    for child in path.child_elements() {
        if out.len() >= MAX_CUSTOM_PATH_COMMANDS { break; }
        match child.local_name() {
            "moveTo" => push_point(&mut out, child, &guides, GeometryPathCommand::Move),
            "lnTo" => push_point(&mut out, child, &guides, GeometryPathCommand::Line),
            "cubicBezTo" => { /* 3 pts */ }
            "quadBezTo" => { /* 2 pts */ }
            "arcTo" => { /* port arc_to_cubics */ }
            "close" => out.push(GeometryPathCommand::Close),
            _ => {}
        }
    }
    // divide every coordinate by the path's own w/h -> the 0..1 space the presets emit
    (!out.is_empty()).then(|| normalize(out, width, height))
}

fn parse_geometry_path(properties: Option<&XmlElement>) -> Option<Vec<GeometryPathCommand>> {
    parse_custom_geometry_path(properties.and_then(|value| value.child("custGeom")))
}

// crates/pptx-render/src/layout.rs (both emit sites)
path: shape
    .geometry_path
    .clone()
    .unwrap_or_else(|| geometry_path(&shape.geometry, &shape.adjust_values, aspect_ratio)),
```

Risks and tests to add:

- **Display-list size.** `cisco-cloud-security/04` and `/19` emit 131 custom shapes / 18,569
  commands; `project17/11` emits 388 / 10,378. Roughly a megabyte of extra JSON per slide crosses
  the wasm boundary and, for the snapshot path, lands in the Yjs document. If that bites, store the
  commands once in the parsed model and let the snapshot reference them rather than copying, or add
  a decimation pass for near-collinear runs. Measure before optimising.
- **Winding rule.** [`crates/pptx-raster/src/lib.rs:376-386`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L376-L386) fills with `FillRule::Winding`. Donut
  and keyhole shapes with a reversed inner loop rely on that; a shape drawn with two same-direction
  loops will fill solid. Add a golden for one (`ocp-psp-plan/01`'s ring, or the padlock on
  `cisco-cloud-security/05`).
- **Unclosed paths.** Many corpus paths end without `<a:close/>`. Filling an open path is defined
  (implicit close) but stroking it is not — [`crates/pptx-raster/src/lib.rs:482`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L482) must not draw a
  closing segment where PowerPoint leaves the outline open. `project20/16/3` is exactly this case
  and is the cheapest slide to check it on.
- **`pptx-edit` schema bump.** Bumping `SCHEMA_VERSION` touches every persisted document; keep the
  v2 migration path and cover it with the existing migration test around
  [`crates/pptx-edit/src/deck.rs:673`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L673).
- **No regression on charts.** [`crates/pptx-render/src/chart.rs:485`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L485) and
  [`crates/pptx-render/src/chart.rs:594`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L594) assert on `geometry == "custom"` primitives produced by the
  chart path; the shape-side change must leave them alone.

**How to verify**

Re-render with `.venv/bin/python render-improvement-harness/scripts/render_bo.py <deck>` then
`diff.py <deck>` for `cisco-cloud-security` (02, 03, 04, 05, 06, 07, 11, 13, 16, 19, 20),
`ocp-psp-plan` (01, 03), `project17` (04, 08, 11) and `project20` (16).
`cisco-cloud-security/04` and `/19` carry the most pixels — their `diff_pct` (9.66 and 8.7) should
drop substantially, and the three top hot cells on slide 04 (r1c1 17.0%, r1c2 20.4%, r1c3 16.5%)
should collapse. `project17/08` (hexagons) and `ocp-psp-plan/01` (donut) are the cleanest
pass/fail reads because the shapes are large and unobstructed.

Watch the display-list size while doing it. Worst slides by emitted command count:
`cisco-cloud-security/04` and `/19` at 131 shapes / 18,569 commands each, `project17/11` at 388
shapes / 10,378 commands; worst single shape 461 commands (`cisco-cloud-security/07`). That is
roughly a megabyte of extra JSON per slide on the snapshot and composed paths.

Existing coverage to extend:

- `crates/pptx-parse` has no `custGeom` test at all — the only occurrences of the string under
  `crates/` are [`crates/docx-parse/src/drawingml.rs:754`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L754), [`crates/docx-parse/src/drawingml.rs:765`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/docx-parse/src/drawingml.rs#L765)
  and [`crates/pptx-parse/src/drawing.rs:342`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L342). A parse test beside
  `seeds_standard_geometry_guides_from_extent` ([`crates/pptx-parse/src/drawing.rs:1009`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L1009)) is new
  ground.
- [`crates/pptx-render/src/layout.rs:2152-2154`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-render/src/layout.rs#L2152-L2154) already asserts some emitted `Primitive::Shape` has a
  non-empty path; tighten it to assert a `custGeom` shape's path is not the four-command rect.
- [`crates/pptx-raster/src/lib.rs:953`](https://github.com/dsaad68/betteroffice/blob/a47dbde7498c781ab81b141e834da1950dcf4175/crates/pptx-raster/src/lib.rs#L953) (`geometry_commands_scale_by_the_primitive_box`) already pins
  the unit-space to pixel contract the parsed path must satisfy; a golden beside
  `crates/pptx-raster/tests/golden.rs` for a non-convex freeform would catch winding-rule
  regressions.
- [`crates/pptx-render/src/chart.rs:485`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L485) and [`crates/pptx-render/src/chart.rs:594`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-render/src/chart.rs#L594) assert on
  `geometry == "custom"` primitives from the chart path; make sure a shape-side change does not
  disturb those.

**Additional context**

none.

Related issues found in the same run: `fill-grpfill-not-resolved`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/model.rs`, `crates/pptx-render/src/layout.rs`, `crates/pptx-render/src/lib.rs`, `crates/pptx-edit/src/deck.rs`, `crates/pptx-edit/src/model.rs`, `crates/docx-parse/src/drawingml.rs`

**How this was found**

A comparison harness renders each deck twice, once with LibreOffice and once with BetterOffice,
pixel-diffs the two images slide by slide, and traces every visible difference back to the OOXML
and to the code path responsible. Reference renders come from LibreOffice through
[pptx-pdf](https://github.com/dsaad68/pptx-pdf), a single binary with LibreOffice embedded, at 96 dpi. Both engines
are given the same Liberation, Carlito and Caladea faces under the family names the decks ask for,
so a difference in text metrics is a real difference and not font substitution.

- Harness, with the per-slide reports and all 35 issues this run produced: https://github.com/dsaad68/betteroffice/tree/harness/pptx-render-improvement/render-improvement-harness
- Full report behind this issue, with every finding, the evidence table and the proposed fix: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/geometry-custom-collapses-to-bbox/report.md
- How the harness works and why it is built this way: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0

Line numbers link to the exact commit they were checked against.
