---
id: fill-grpfill-not-resolved
title: Resolve a:grpFill against the enclosing group's fill
category: fill
impact: high
effort: easy
confidence: high
status: open
occurrences: 16
decks: [cisco-cloud-security, project20]
findings: [cisco-cloud-security/05/1, cisco-cloud-security/07/2, cisco-cloud-security/08/1, cisco-cloud-security/09/5, cisco-cloud-security/10/2, cisco-cloud-security/12/2, cisco-cloud-security/15/2, cisco-cloud-security/16/3, cisco-cloud-security/17/1, cisco-cloud-security/18/1, cisco-cloud-security/23/2, project20/02/2, project20/06/2, project20/07/2, project20/08/1, project20/09/1]
files: [crates/pptx-parse/src/drawing.rs, crates/pptx-render/src/layout.rs, crates/pptx-parse/src/write.rs]
---

## Symptom

A shape whose `spPr` defers its fill to `<a:grpFill/>` -- "use the fill my enclosing group
declares" -- renders with no fill at all. The shape becomes invisible, and because these
shapes are almost always the coloured backdrop for white text, the text goes with it:
evidence-1.png and evidence-2.png show four numbered callout bars losing their blue and
dark-grey backgrounds while their white body text turns nearly unreadable against the pale
card, and evidence-3.png shows the same for four orange bars. evidence-4.png shows the
other failure shape: sidebar icons built from `custGeom` freeforms with `<a:grpFill/>` and
`<a:ln><a:noFill/></a:ln>` disappear entirely, since with no fill and no stroke there is
nothing left to paint.

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/17 | Four blue `roundRect` callout bars (ids 60, 63, 66, 69) lose the `13A7E0` fill their parent groups declare; the white bullet text and divider all but vanish. |
| 2 | cisco-cloud-security/18 | Same pattern with the dark-grey (`676767`) list bars (ids 51, 54, 57, 60). |
| 3 | cisco-cloud-security/12 | Same pattern with the orange (`F9771D`) list bars (ids 53, 56, 59, 62). |
| 4 | project20/09 | Three sidebar icon groups (`accent1` on the group, `custGeom` + `grpFill` + `noFill` line on the children) render as blank space. |

## Root cause (hypothesis)

Confirmed. `<a:grpFill/>` is never parsed, and a group's own fill is never parsed either, so
there is nothing to inherit and nothing to inherit from.

- `parse_fill` handles `noFill`, `solidFill`, `gradFill` and `blipFill` and falls through to
  `None` for anything else -- `crates/pptx-parse/src/drawing.rs:565`, with the fall-through at
  `crates/pptx-parse/src/drawing.rs:582`. `grpFill` (and `pattFill`) land there.
- That `None` becomes `Shape.fill` / `Picture.fill` at
  `crates/pptx-parse/src/drawing.rs:150` and `crates/pptx-parse/src/drawing.rs:187` -- the
  same value the shape would carry if the XML had specified no fill element at all, so the
  information that a fill was requested is lost at parse time.
- `parse_group` reads only `nvGrpSpPr` and `grpSpPr/xfrm`; the `grpSpPr` fill is dropped --
  `crates/pptx-parse/src/drawing.rs:252`-`266`. `GroupShape` correspondingly has no `fill`
  field (`crates/pptx-parse/src/model.rs:261`), and `node_fill` returns `None` for every
  group (`crates/pptx-render/src/layout.rs:1760`).
- Both render paths therefore have nothing to fall back on. `render_parsed_shape` recurses
  through a group carrying only the transform (`crates/pptx-render/src/layout.rs:490`) and
  paints the child from `value.fill` alone (`crates/pptx-render/src/layout.rs:526`); the
  snapshot path does the same at `crates/pptx-render/src/layout.rs:362` and `:384`.
- `pptx-raster` fills only when a paint is present -- `crates/pptx-raster/src/lib.rs:379` --
  so `fill: None` plus the usual `<a:ln><a:noFill/></a:ln>` produces literally no pixels.
- The edit layer has the same hole: `seed_shape` stores `fillJson` for shapes and pictures
  (`crates/pptx-edit/src/deck.rs:131`, `:142`) but the group arm stores nothing
  (`crates/pptx-edit/src/deck.rs:162`), so a snapshot-driven render cannot recover the group
  fill either.
- The write side already knows the element exists: `grpFill` is in `FILL_ELEMENTS`
  (`crates/pptx-parse/src/write.rs:995`) so a fill edit replaces it correctly. Only the read
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
  `graphicFrame` and `grpSp` (`crates/pptx-parse/src/drawing.rs:110`-`128`). The 36
  connector `grpFill`s stay invisible after this fix because the connectors themselves are
  never parsed.
- `p:style/a:fillRef` is not parsed anywhere in `pptx-parse` or `pptx-render` (no hit for
  `fillRef` in either crate). It does not affect this issue -- `grpFill` overrides the style
  reference -- but it is the other reason a `spPr` with no explicit fill renders empty.

## Verification

Re-render `cisco-cloud-security` slides 05, 07, 08, 09, 10, 12, 15, 16, 17, 18, 23 and
`project20` slides 02, 06, 07, 08, 09. The bar-and-text slides (12, 17, 18) should go from
~21% fine diff to a small residual: they are near-identical apart from the missing fills, so
the hot cells covering the callout column should collapse. Slides whose `grpFill` shapes are
`custGeom` icons (`cisco-cloud-security/05`, `project20/02`, `06`, `07`, `08`, `09`) will get
*worse-looking* before they get better -- see the risk note in `possible-solution.md` --
because custom geometry currently falls back to the `rect` preset
(`crates/pptx-render/src/layout.rs:1955`-`1956`), so a newly-resolved fill paints a solid
rectangle where an invisible shape used to be. Judge those slides only after
`geometry-custom-collapses-to-bbox` is fixed, or gate the check on the bar slides.

No existing test covers `grpFill`: `grep -rn grpFill crates/` hits only
`crates/pptx-parse/src/write.rs:1001`. The nearest coverage is the fill assertions in
`crates/pptx-render/src/layout.rs` tests (around `:2334` and `:2403`) and the
`crates/pptx-raster/tests/golden/shapes.png` golden; neither exercises group inheritance, so
the fix needs new tests rather than updated ones.
