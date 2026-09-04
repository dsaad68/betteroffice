# pptx: Connector shapes (p:cxnSp) never parsed, so no connector is ever drawn

**Describe the bug**

Every `p:cxnSp` in the corpus is missing from the BetterOffice render. On the `project20` template
that means the thick white rule above each title, the rule under the sidebar heading, and the
full-height navy divider between sidebar and content all vanish (evidence-1.png). On
`green-solutions/01` the sixteen diagonal lines that turn eight icon badges into a network are all
gone (evidence-2.png), and on `project17/04` the three grey leader rules above the text columns are
gone (evidence-3.png). The cluster id says "zero extent", but that is not the trigger: the
`green-solutions` connectors have fully non-degenerate extents (e.g. `cx="1987489" cy="3248709"`)
and are dropped just the same, while renaming the element from `p:cxnSp` to `p:sp` makes every one
of them - zero-height, zero-width and diagonal alike - render correctly (evidence-4.png).

Seen on 9 slides across 3 decks while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact medium, estimated effort easy, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: high.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

**1. project20/07** left third of the slide: the zero-height white title rule, the zero-height "Comprehensive Across" underline and the zero-width full-height navy divider are all present in the reference and absent in the candidate

![evidence-1](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-zero-extent-skipped/evidence-1.png)

**2. green-solutions/01** the 16 `p:cxnSp` connectors joining the icon badges, each with a non-degenerate `cx x cy`, all missing - so a degenerate bounding box is not what triggers the drop

![evidence-2](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-zero-extent-skipped/evidence-2.png)

**3. project17/04** the three horizontal `prst="line"` leader rules above the text columns, present in the reference, absent in the candidate

![evidence-3](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-zero-extent-skipped/evidence-3.png)

**4. project20/07** BetterOffice today vs BetterOffice on the same slide with `p:cxnSp` textually renamed to `p:sp`: all three lines appear, with correct width, colour and position

![evidence-4](https://raw.githubusercontent.com/dsaad68/betteroffice/harness/pptx-render-improvement/render-improvement-harness/issues/line-zero-extent-skipped/evidence-4.png)

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

- `Unique Way To Showcase Your Green Solutions in Microsoft PowerPoint (PPT).pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 1
- `project17.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slide 4
- `project20.pptx` ([source](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip)), slides 3, 4, 5, 7, 9, 12, 13

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

`p:cxnSp` has no branch in the shape-tree dispatch, so connectors never enter the model and there is
nothing downstream to skip.

1. `parse_shape_children` ([`crates/pptx-parse/src/drawing.rs:101`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L101)) matches only four element names
   ([`crates/pptx-parse/src/drawing.rs:109-128`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L109-L128)):

   ```rust
   let shape = match child.local_name() {
       "sp" => Some(ShapeNode::Shape(parse_shape(child, part, budget)?)),
       "pic" => ...,
       "graphicFrame" => ...,
       "grpSp" => ...,
       _ => None,
   };
   ```

   `_ => None` silently discards `cxnSp`. `ShapeNode` ([`crates/pptx-parse/src/model.rs:142-147`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/model.rs#L142-L147))
   likewise has no connector variant, and `cxnSp` appears nowhere in `pptx-parse`, `pptx-render`,
   `pptx-raster` or `ooxml-drawingml` - grep for it in `crates/` only hits `docx-parse` and the
   `pptx-edit` write-fidelity tests.

2. Nothing else in the pipeline is at fault, and in particular there is no zero-extent cull:
   - `Space::map_transform` ([`crates/pptx-render/src/layout.rs:1613`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1613)) maps a zero `cx`/`cy` to a
     zero `w`/`h` without dropping anything; the only `width > 0 && height > 0` test nearby is
     `resolved_transform_value` ([`crates/pptx-render/src/layout.rs:1866`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1866)), which merely decides
     whether to fall back to an inherited transform and then keeps the shape's own values.
   - `"line" | "straightConnector1"` is a supported preset and produces the correct corner-to-corner
     path ([`crates/ooxml-drawingml/src/geometry.rs:93-95`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/geometry.rs#L93-L95)).
   - `geometry_path` in the rasterizer ([`crates/pptx-raster/src/lib.rs:575`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L575)) builds a two-verb path;
     `tiny_skia::Rect::from_ltrb` accepts a degenerate box (`left <= right`), so `finish()` returns
     a real path for a zero-height or zero-width line. `stroke_paint`
     ([`crates/pptx-raster/src/lib.rs:698`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-raster/src/lib.rs#L698)) only refuses a non-finite or non-positive stroke width.

3. Verified against the running renderer at HEAD (c77daaa) through the Python binding. Taking
   `decks/project20/source.pptx`, textually renaming `<p:cxnSp>`/`<p:nvCxnSpPr>`/`<p:cNvCxnSpPr>` to
   `<p:sp>`/`<p:nvSpPr>`/`<p:cNvSpPr>` and changing nothing else:
   - slide 3 (one connector, `ext cx="2797810" cy="0"`): the render changes only in the box
     `(30, 23)-(324, 29)` - exactly the missing 4.5pt white rule;
   - slide 7 (three connectors: `cy="0"`, `cy="0"`, `cx="0"`): the render changes in
     `(15, 23)-(324, 720)`, and all three lines appear at the right width and colour
     (evidence-4.png).

   Nothing about the geometry changed, only the element name - the drop is the parse dispatch.

4. What is **not** confirmed: the `project20/09/2` claim that plain `p:sp` shapes are affected too.
   `Line 12`/`13`/`14` (ids 33/34/35) are `p:sp`, are parsed, and do reach the renderer, but their
   extent is `cx="0" cy="0"` - a point, not a line - with `cap="flat"`, so no renderer can stroke
   anything from them. The retail bag handle the finding attributes to them is actually
   `Freeform 10` (id 31, `custGeom`, `ext 201613 x 209550`) inside the same `Group 29`; the whole
   icon is missing from the candidate, which belongs to the `fill-grpfill-not-resolved` and
   `custGeom` clusters, not this one. Those three shapes should be dropped from this cluster.

5. Scope limit for the fix. All 41 connectors in the three decks of this cluster carry an explicit
   `<a:ln>` with a `solidFill`, so parsing `cxnSp` is enough to resolve every finding here. Across
   the whole corpus, though, 190 of 309 `p:cxnSp` elements have no `<a:ln>` at all and take their
   stroke from `<p:style><a:lnRef>` in the theme (nearly all of them on `cisco-cloud-security`
   slides 04 and 19). `lnRef`/`fillRef`/`effectRef` are parsed nowhere in `pptx-parse`, so those
   connectors will parse but still stroke nothing. That is a separate gap, not a blocker here.
   Similarly, `headEnd`/`tailEnd` are parsed into `ShapeOutline`
   ([`crates/pptx-parse/src/drawing.rs:641-642`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L641-L642), [`crates/ooxml-drawingml/src/shape.rs:55-57`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/ooxml-drawingml/src/shape.rs#L55-L57)) but
   never reach the display-list `Stroke` ([`crates/pptx-render/src/layout.rs:1930-1944`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L1930-L1944)), so the 11
   arrowheaded connectors in the corpus - including `Straight Arrow Connector 87` on `project20/04`
   - will draw as plain lines.

**Suggested fix**

Treat `p:cxnSp` as a `ShapeNode::Shape`. A connector is structurally an `p:sp` minus the `p:txBody`:
same `p:spPr` with `a:xfrm`, `a:prstGeom`, `a:ln`, only the non-visual wrapper differs
(`p:nvCxnSpPr` / `p:cNvCxnSpPr` instead of `p:nvSpPr` / `p:cNvSpPr`). Mapping it onto the existing
variant means nothing downstream has to change: `seed_shape`
([`crates/pptx-edit/src/deck.rs:123-138`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L123-L138)), `ShapeSnapshot`, `render_snapshot_shape`
([`crates/pptx-render/src/layout.rs:340`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L340)), `render_parsed_shape`
([`crates/pptx-render/src/layout.rs:480`](https://github.com/dsaad68/betteroffice/blob/df1a57dae7a091ea9ca8176ca013274cced71fdd/crates/pptx-render/src/layout.rs#L480)), the rasterizer and `packages/pptx/src/render/canvas.ts`
all already handle a `prstGeom` shape with an outline and no text.

Two edits:

1. `parse_shape_children` ([`crates/pptx-parse/src/drawing.rs:109`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L109)) gains a `"cxnSp"` arm, and
   `parse_shape` ([`crates/pptx-parse/src/drawing.rs:138-155`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/drawing.rs#L138-L155)) looks for the non-visual wrapper under
   either name. `p:cxnSp` has no `p:txBody`, so `element.child("txBody")` already yields `None` and
   the text branch is a no-op.

2. `is_shape_element` ([`crates/pptx-parse/src/write.rs:682-684`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L682-L684)) must gain `"cxnSp"` in the same
   change. This is the load-bearing part: shape ids embed the parsed-shape ordinal
   (`seed_shape` builds `"{slide_id}:shape:{path}"`, [`crates/pptx-edit/src/deck.rs:95`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/deck.rs#L95)), `save.rs`
   recovers that ordinal (`source_index`, [`crates/pptx-edit/src/save.rs:161-168`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L161-L168)), and `write.rs`
   resolves it against the XML children filtered by `is_shape_element`
   ([`crates/pptx-parse/src/write.rs:765-774`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-parse/src/write.rs#L765-L774)). Adding `cxnSp` to the parser but not to the writer
   desynchronises the two lists at the first connector on a slide, so every subsequent `Keep` or
   `Patch` would be applied to the wrong element - silent deck corruption on save, not a render bug.

```rust
// crates/pptx-parse/src/drawing.rs
let shape = match child.local_name() {
    "sp" | "cxnSp" => Some(ShapeNode::Shape(parse_shape(child, part, budget)?)),
    "pic" => ...,
};

fn parse_shape(element: &XmlElement, part: &str, budget: &mut ParseBudget<'_>) -> Result<Shape, PptxError> {
    budget.charge_shape(part)?;
    let properties = element.child("spPr");
    let transform = properties.and_then(|value| value.child("xfrm"));
    let non_visual = element.child("nvSpPr").or_else(|| element.child("nvCxnSpPr"));
    Ok(Shape {
        base: parse_base(non_visual, transform),
        // unchanged: geometry, adjust_values, fill, outline, text
    })
}
```

```rust
// crates/pptx-parse/src/write.rs
fn is_shape_element(local: &str) -> bool {
    matches!(local, "sp" | "cxnSp" | "pic" | "graphicFrame" | "grpSp")
}
```

Risks and tests to add:

- **Save-path index alignment** (above). The two `matches!` lists are an undeclared invariant; the
  fix should land both sides together and ideally leave a note tying them. `cargo test -p pptx-edit
  --test write_fidelity` covers decks containing `p:cxnSp`
  ([`crates/pptx-edit/tests/write_fidelity.rs:45`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/tests/write_fidelity.rs#L45), `:70`, `:257`, `:615`) and is the guard here; add
  a case where a connector sits *between* two `p:sp` and a patch is applied to the later one, since
  a same-order deck would not catch a one-off shift.
- **New shape ids.** Any slide with a connector now exposes more shapes and renumbers the ones after
  it, so persisted shape ids from an earlier session no longer point at the same shape. Same class
  of change as any parser addition, but worth calling out to whoever owns the editing surface.
- **`shape_add` on a connector.** [`crates/pptx-edit/src/save.rs:412-418`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/src/save.rs#L412-L418) refuses anything that is
  not `ShapeKind::Shape`; a connector now *is* one, so a copy-then-add of a connector would be
  written back as a `p:sp`. Acceptable (it renders identically), but it silently changes the element
  kind on round trip. If that matters, carry a flag on `Shape` rather than widening the arm.
- **Incomplete win on other decks.** As noted in the report, 190 of 309 corpus connectors get their
  stroke from `<p:style><a:lnRef>` and will still draw nothing; arrowheads (`headEnd`/`tailEnd`) are
  parsed but never reach the display list. Neither affects this cluster's 9 findings, but a reviewer
  looking at `cisco-cloud-security` after the fix should not expect its connectors to appear.
- **Tests to add:** a `pptx-parse` unit test that a `p:cxnSp` in a `spTree` parses to a shape with
  geometry `line`, the right `a:xfrm`, and its `a:ln`; and a `pptx-raster` golden covering a
  zero-height and a zero-width `prst="line"` stroke, which nothing exercises today
  (`crates/pptx-raster/tests/golden/` has no line case).

**How to verify**

Re-render `green-solutions/01`, `project17/04` and `project20` slides 03, 04, 05, 07, 09, 12, 13
with `.venv/bin/python render-improvement-harness/scripts/render_bo.py <deck>` then `diff.py <deck>`.

- `green-solutions/01` should gain all 16 connector lines; the residual diff there stays dominated
  by the missing background photo and the `spc` tracking issue, which are other clusters.
- `project20/03`, `05`, `12`, `13` are single-connector slides: the diff band at
  y in [0.033, 0.040] over x in [0.02, 0.26] should clear completely.
- `project20/07` and `09` should additionally gain the full-height divider at x = 0.108 and the
  sidebar underline.
- `project20/04` will gain the white rule and the four dotted leaders but keep a small residual at
  the green `straightConnector1`, whose `tailEnd type="triangle"` is still not drawn.
- The parse side has no coverage to extend: the unit tests in `crates/pptx-parse/src/drawing.rs`
  only exercise `p:sp`, so a new one is needed asserting that a `p:cxnSp` in a `spTree` yields a
  shape with geometry `line` and its `a:ln`.
- The round trip must be re-checked, not just the render.
  `cargo test -p pptx-edit --test write_fidelity` already contains decks with `p:cxnSp`
  ([`crates/pptx-edit/tests/write_fidelity.rs:45`](https://github.com/openooxml/betteroffice/blob/187cebc9ef5d414e4e65ccd96fe68b8f46c7f528/crates/pptx-edit/tests/write_fidelity.rs#L45), `:70`, `:257`, `:615`) and asserts the connector
  survives a save; those are the tests that catch the index-alignment hazard described in
  `possible-solution.md`.

**Additional context**

none.

Related issues found in the same run: `fill-grpfill-not-resolved`

Files most likely involved: `crates/pptx-parse/src/drawing.rs`, `crates/pptx-parse/src/write.rs`

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: https://github.com/dsaad68/betteroffice/blob/harness/pptx-render-improvement/render-improvement-harness/issues/line-zero-extent-skipped/report.md. Methodology: https://gist.github.com/dsaad68/038b63c2977aeca16fc873c2df1152d0. Line numbers link to the exact commit they were checked against.
