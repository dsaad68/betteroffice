---
id: line-zero-extent-skipped
title: Parse p:cxnSp so connector shapes reach the renderer
category: unsupported-element
impact: medium
effort: easy
confidence: high
status: open
occurrences: 9
decks: [green-solutions, project17, project20]
findings: [green-solutions/01/2, project17/04/4, project20/03/3, project20/04/1, project20/05/4, project20/07/1, project20/09/2, project20/12/3, project20/13/3]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-parse/src/write.rs]
---

## Symptom

Every `p:cxnSp` in the corpus is missing from the BetterOffice render. On the `project20` template
that means the thick white rule above each title, the rule under the sidebar heading, and the
full-height navy divider between sidebar and content all vanish (evidence-1.png). On
`green-solutions/01` the sixteen diagonal lines that turn eight icon badges into a network are all
gone (evidence-2.png), and on `project17/04` the three grey leader rules above the text columns are
gone (evidence-3.png). The cluster id says "zero extent", but that is not the trigger: the
`green-solutions` connectors have fully non-degenerate extents (e.g. `cx="1987489" cy="3248709"`)
and are dropped just the same, while renaming the element from `p:cxnSp` to `p:sp` makes every one
of them - zero-height, zero-width and diagonal alike - render correctly (evidence-4.png).

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | project20/07 | left third of the slide: the zero-height white title rule, the zero-height "Comprehensive Across" underline and the zero-width full-height navy divider are all present in the reference and absent in the candidate |
| 2 | green-solutions/01 | the 16 `p:cxnSp` connectors joining the icon badges, each with a non-degenerate `cx x cy`, all missing - so a degenerate bounding box is not what triggers the drop |
| 3 | project17/04 | the three horizontal `prst="line"` leader rules above the text columns, present in the reference, absent in the candidate |
| 4 | project20/07 | BetterOffice today vs BetterOffice on the same slide with `p:cxnSp` textually renamed to `p:sp`: all three lines appear, with correct width, colour and position |

## Root cause (confirmed)

`p:cxnSp` has no branch in the shape-tree dispatch, so connectors never enter the model and there is
nothing downstream to skip.

1. `parse_shape_children` (`crates/pptx-parse/src/drawing.rs:101`) matches only four element names
   (`crates/pptx-parse/src/drawing.rs:109-128`):

   ```rust
   let shape = match child.local_name() {
       "sp" => Some(ShapeNode::Shape(parse_shape(child, part, budget)?)),
       "pic" => ...,
       "graphicFrame" => ...,
       "grpSp" => ...,
       _ => None,
   };
   ```

   `_ => None` silently discards `cxnSp`. `ShapeNode` (`crates/pptx-parse/src/model.rs:142-147`)
   likewise has no connector variant, and `cxnSp` appears nowhere in `pptx-parse`, `pptx-render`,
   `pptx-raster` or `ooxml-drawingml` - grep for it in `crates/` only hits `docx-parse` and the
   `pptx-edit` write-fidelity tests.

2. Nothing else in the pipeline is at fault, and in particular there is no zero-extent cull:
   - `Space::map_transform` (`crates/pptx-render/src/layout.rs:1613`) maps a zero `cx`/`cy` to a
     zero `w`/`h` without dropping anything; the only `width > 0 && height > 0` test nearby is
     `resolved_transform_value` (`crates/pptx-render/src/layout.rs:1866`), which merely decides
     whether to fall back to an inherited transform and then keeps the shape's own values.
   - `"line" | "straightConnector1"` is a supported preset and produces the correct corner-to-corner
     path (`crates/ooxml-drawingml/src/geometry.rs:93-95`).
   - `geometry_path` in the rasterizer (`crates/pptx-raster/src/lib.rs:575`) builds a two-verb path;
     `tiny_skia::Rect::from_ltrb` accepts a degenerate box (`left <= right`), so `finish()` returns
     a real path for a zero-height or zero-width line. `stroke_paint`
     (`crates/pptx-raster/src/lib.rs:698`) only refuses a non-finite or non-positive stroke width.

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
   (`crates/pptx-parse/src/drawing.rs:641-642`, `crates/ooxml-drawingml/src/shape.rs:55-57`) but
   never reach the display-list `Stroke` (`crates/pptx-render/src/layout.rs:1930-1944`), so the 11
   arrowheaded connectors in the corpus - including `Straight Arrow Connector 87` on `project20/04`
   - will draw as plain lines.

## Verification

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
  (`crates/pptx-edit/tests/write_fidelity.rs:45`, `:70`, `:257`, `:615`) and asserts the connector
  survives a save; those are the tests that catch the index-alignment hazard described in
  `possible-solution.md`.
