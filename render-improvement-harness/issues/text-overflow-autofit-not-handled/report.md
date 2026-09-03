---
id: text-overflow-autofit-not-handled
title: Stop clipping text at the shape box and stop shrinking spAutoFit text
category: text-autofit
impact: high
effort: medium
confidence: high
status: open
occurrences: 13
decks: [cisco-cloud-security, ocp-psp-plan, project17, project20, rollout-plan]
findings: [cisco-cloud-security/01/1, cisco-cloud-security/02/2, cisco-cloud-security/09/1, cisco-cloud-security/12/1, cisco-cloud-security/13/1, cisco-cloud-security/19/3, cisco-cloud-security/20/2, ocp-psp-plan/01/3, project17/03/1, project17/08/2, project17/11/3, project20/01/3, rollout-plan/06/2]
files: [crates/pptx-raster/src/lib.rs, crates/pptx-render/src/layout.rs, packages/pptx/src/render/canvas.ts]
---

## Symptom

A text box whose text is taller than its box is hard-clipped at the box's bottom edge, so the
overflowing line is sliced through the middle of the glyphs (evidence-1.png) or lost entirely
(evidence-4.png). PowerPoint and LibreOffice let that text spill outside the box. Where the
inheritance chain does carry `<a:spAutoFit/>`, the opposite happens: BetterOffice shrinks the
font in 10% steps until the text fits, so one placeholder renders at 59% of its sibling's size
(evidence-2.png) and a 48pt title collapses to ~31pt (evidence-3.png) — `spAutoFit` never
changes the font size.

Twelve of the thirteen findings are these two behaviours. The thirteenth,
`cisco-cloud-security/01/1`, is **not** an autofit failure — see "Root cause".

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | cisco-cloud-security/09 | Title `<a:bodyPr/>` with no autofit anywhere in the chain: reference spills "business" below the 0.8in box, candidate slices it at the box edge and also starts the first line ~14px lower. |
| 2 | rollout-plan/06 | Two sibling `idx=10`/`idx=11` placeholders, both `<a:spAutoFit/>`, both `sz="3529"`. The one whose text fits renders correctly; the one that overflows is shrunk to 0.9^5 = 59%. Reference draws both at the same size and lets the second spill below the green bar. |
| 3 | project17/03 | `<a:bodyPr ...><a:spAutoFit/></a:bodyPr>` with an explicit `sz="4800"` run. Reference keeps 48pt and lets the text grow past the stored box; candidate rewraps it at ~31pt. (The face difference is a separate font-substitution issue.) |
| 4 | project20/01 | `<a:bodyPr lIns="91440" anchor="t"><a:noAutofit/></a:bodyPr>`: "Strategy, and Execution Plan" and "May 8, 2018" are clipped away entirely. |

## Root cause (hypothesis)

Three separate defects sit behind this cluster. All three are confirmed against the decks' XML.

**1. The rasteriser clips every text box to its own rect — confirmed.**
`crates/pptx-raster/src/lib.rs:282` intersects the current clip with the primitive's `x/y/w/h`
(`crates/pptx-raster/src/lib.rs:288`, helper at `crates/pptx-raster/src/lib.rs:325`) and hands
that mask to `font::paint_lines`. There is no condition on it. DrawingML has no clip on autoshape
text: text that does not fit is drawn outside the shape. The layout stage already knows this —
it computes `overflow` at `crates/pptx-render/src/layout.rs:739` and ships it on the primitive
(`crates/pptx-render/src/display_list.rs:130`) — but nothing in the repo reads that field. The
web canvas renderer clips the same way at `packages/pptx/src/render/canvas.ts:218-220`.

This explains cisco-cloud-security 02/2, 09/1, 12/1, 13/1, 19/3, 20/2 (master title `bodyPr` with
no autofit child, `cy="731837"`), ocp-psp-plan/01/3, project17/08/2 and 11/3 (layout title has
`<a:noAutofit/>`, `cy="369332"`), and project20/01/3 (`<a:noAutofit/>` on the slide).

**2. `spAutoFit` is treated as shrink-to-fit — confirmed.**
`crates/pptx-render/src/layout.rs:687-689` enters the shrink loop for
`Some(TextAutofit::Normal { .. } | TextAutofit::Shape)`, and the loop at
`crates/pptx-render/src/layout.rs:691-697` multiplies the scale by 0.9 until the text fits or the
scale hits `MIN_AUTOFIT_SCALE = 0.5` (`crates/pptx-render/src/layout.rs:28`). `spAutoFit` means
the *shape* resizes to the text; the font size is untouched. `TextAutofit::Shape` is parsed
correctly at `crates/pptx-parse/src/drawing.rs:795-797` into the variant at
`crates/pptx-parse/src/model.rs:286-293`, so the value reaches layout intact — it is used wrongly,
not lost.

This explains rollout-plan/06/2 (0.9^5 = 0.590, the reported 59% exactly) and project17/03/1
(0.9^4 = 0.656, 48pt -> 31.5pt, the reported ~31pt).

**3. The anchor shift is clamped at zero — confirmed, contributing.**
`crates/pptx-render/src/layout.rs:712-713` clamps the centre/bottom shift with `.max(0.0)`, so
overflowing text in an `anchor="ctr"` box starts at the box top instead of spilling equally above
and below. That is the ~14px downward shift of the first line reported in cisco-cloud-security/19/3
and visible in evidence-1.png. It only matters once the clip is lifted, but it must be fixed with
it or centred titles will land in the wrong place.

**Not this cluster: `cisco-cloud-security/01/1`.** The finding claims a 60% shrink on the `ctrTitle`.
Checked in `decks/cisco-cloud-security/xml/01`: neither the slide's `<a:bodyPr/>`, the layout's
`<a:bodyPr anchor="b"/>` on the `ctrTitle` shape, nor the master's `title` placeholder `bodyPr`
carries any autofit child, so `BodyCascade::autofit` (`crates/pptx-render/src/layout.rs:777-782`)
returns `None`, the scale stays 1.0 and the shrink loop never runs. The size drop is
5200 -> 3200 = 61.5%: the layout shape's `<a:lstStyle>` `defRPr sz="5200"` is ignored and the
master's `titleStyle` `sz="3200"` wins. That is `text-inheritance-layout-lststyle-ignored`, and
this issue's fix will not move that slide.

**Not investigated:** `lnSpcReduction` is parsed (`crates/pptx-parse/src/drawing.rs:802`) and
stored (`crates/pptx-parse/src/model.rs:291`) but never read by layout. No finding in this cluster
depends on it; it is a latent gap in the same code path, not a claim about these slides.

## Verification

Re-render the thirteen findings' slides and compare `diff-summary.json`:

- Clip removal: `cisco-cloud-security` 02, 09, 12, 13, 19, 20; `project17` 08, 11; `project20` 01;
  `ocp-psp-plan` 01. The overflowing line must appear whole, below the box. Titles anchored `ctr`
  must also move up by roughly half the overflow.
- `spAutoFit`: `rollout-plan` 06 — both header placeholders must render at the same size, with
  "Monthly run exec meeting" wrapping to two lines that spill past the green bar.
  `project17` 03 — the title must stay at 48pt.
- `cisco-cloud-security` 01 must be unchanged by this fix.

Existing coverage: `normal_autofit_scales_text_until_the_shape_height_is_respected`
(`crates/pptx-render/src/layout.rs:2239`) is the only autofit test and must keep passing, since
`normAutofit` still shrinks. `crates/pptx-raster/tests/golden.rs` holds `text.png`; the
`a_clipped_primitive_off_the_surface_draws_nothing` test at `crates/pptx-raster/src/lib.rs:899`
uses a `Chart` primitive, so lifting the text-box clip does not weaken it. Canvas coverage that
asserts the clip call is at `packages/pptx/src/render/canvas.test.ts:113,205`.
