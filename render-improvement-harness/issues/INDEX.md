# Issues

Generated from `clusters.json` (2026-09-03). Ordered by impact, then effort.

| # | issue | category | impact | effort | occurrences | decks | status |
|---|---|---|---|---|---|---|---|
| 1 | [fill-grpfill-not-resolved](fill-grpfill-not-resolved/report.md) | fill | high | easy | 16 | 2 | open |
| 2 | [text-run-props-bold-ignored](text-run-props-bold-ignored/report.md) | text-run-props | high | easy | 15 | 4 | open |
| 3 | [text-inheritance-layout-lststyle-ignored](text-inheritance-layout-lststyle-ignored/report.md) | text-inheritance | high | easy | 14 | 4 | open |
| 4 | [hidden-shape-drawn-anyway](hidden-shape-drawn-anyway/report.md) | hidden | high | easy | 11 | 2 | open |
| 5 | [text-run-props-gradfill-not-resolved](text-run-props-gradfill-not-resolved/report.md) | text-run-props | high | easy | 9 | 2 | open |
| 6 | [fill-alpha-modifier-ignored](fill-alpha-modifier-ignored/report.md) | fill | high | easy | 6 | 3 | open |
| 7 | [theme-color-scheme-color-resolution-broken](theme-color-scheme-color-resolution-broken/report.md) | theme-color | high | medium | 20 | 4 | open |
| 8 | [geometry-custom-collapses-to-bbox](geometry-custom-collapses-to-bbox/report.md) | geometry-custom | high | medium | 17 | 4 | open |
| 9 | [picture-srcrect-crop-ignored](picture-srcrect-crop-ignored/report.md) | picture | high | medium | 14 | 2 | open |
| 10 | [text-overflow-autofit-not-handled](text-overflow-autofit-not-handled/report.md) | text-autofit | high | medium | 13 | 5 | open |
| 11 | [text-bullets-char-indent-dropped](text-bullets-char-indent-dropped/report.md) | text-bullets | high | medium | 9 | 4 | open |
| 12 | [unsupported-table-not-rendered](unsupported-table-not-rendered/report.md) | table | high | hard | 19 | 3 | open |
| 13 | [picture-fill-fails-to-render](picture-fill-fails-to-render/report.md) | picture | high | hard | 9 | 3 | open |
| 14 | [text-slidenum-field-not-evaluated](text-slidenum-field-not-evaluated/report.md) | field-eval | medium | easy | 13 | 1 | open |
| 15 | [line-zero-extent-skipped](line-zero-extent-skipped/report.md) | unsupported-element | medium | easy | 9 | 3 | open |
| 16 | [text-run-props-solidfill-scope-bug](text-run-props-solidfill-scope-bug/report.md) | text-run-props | medium | easy | 6 | 2 | open |
| 17 | [chart-dlbls-shown-when-disabled](chart-dlbls-shown-when-disabled/report.md) | chart | medium | easy | 4 | 1 | open |
| 18 | [text-layout-master-lnspc-ignored](text-layout-master-lnspc-ignored/report.md) | text-layout | medium | medium | 7 | 1 | open |
| 19 | `transform-text-orientation-wrong-under-rotation` | transform | medium | medium | 6 | 1 | triaged |
| 20 | `fill-nonsolid-fill-types-not-resolved` | fill | medium | medium | 6 | 3 | triaged |
| 21 | [text-run-props-spc-ignored](text-run-props-spc-ignored/report.md) | text-run-props | medium | medium | 5 | 4 | open |
| 22 | [text-bullets-autonum-not-drawn](text-bullets-autonum-not-drawn/report.md) | text-bullets | medium | medium | 5 | 1 | open |
| 23 | `line-stroke-color-resolution-broken` | connector | medium | medium | 4 | 1 | triaged |
| 24 | `chart-axis-position-swapped` | chart | medium | medium | 4 | 1 | triaged |
| 25 | `chart-category-order-reversed` | chart | medium | medium | 3 | 1 | triaged |
| 26 | `text-font-substitution-issues` | text-font | medium | hard | 8 | 1 | triaged |
| 27 | `chart-legend-and-title-position-wrong` | chart | low | easy | 5 | 1 | triaged |
| 28 | `text-run-props-misc-property-ignored` | text-run-props | low | easy | 2 | 2 | triaged |
| 29 | `geometry-preset-adj-values-wrong` | geometry-preset | low | easy | 1 | 1 | triaged |
| 30 | `chart-axis-autoscale-not-rounded` | chart | low | medium | 4 | 1 | triaged |
| 31 | `effects-prsttxwarp-and-outershdw-ignored` | effects | low | medium | 3 | 2 | triaged |
| 32 | `picture-blip-duotone-bilevel-not-applied` | picture | low | medium | 3 | 1 | triaged |
| 33 | `unsupported-custgeom-picturefill-wordmark-not-drawn` | unsupported-element | low | medium | 3 | 1 | triaged |
| 34 | `chart-minimal-chart-series-axis-broken` | chart | low | medium | 3 | 1 | triaged |
| 35 | `transform-group-child-rotation-scale-wrong` | transform | low | hard | 1 | 1 | triaged |

## Deferred

- `cisco-cloud-security/01/3`: category lo-suspect: comparator judged the reference wrong (LO draws layout placeholder prompt text for empty placeholders; BetterOffice correctly renders nothing).
- `cisco-cloud-security/04/5`: self-identified lo-suspect in the summary (wrap='none' + spAutoFit label): the reference wraps a label the XML says must stay on one line; BetterOffice's unwrapped render matches the XML.
- `cisco-cloud-security/04/6`: confidence low: wrap-point difference from font-metric differences on a default font (Arial) with no explicit override; not clearly a BetterOffice defect.
- `cisco-cloud-security/10/4`: font-metric wrap-point difference on a default font with no explicit override; likely a rendering-engine metric difference rather than a missed property, low actionability.
- `cisco-cloud-security/13/4`: font-metric wrap-point difference (off-slide banner text) with no explicit override; low actionability, same family as cisco-cloud-security/04/6.
- `cisco-cloud-security/14/2`: font-metric wrap-point difference (numbered callout boxes) with no explicit override; low actionability, same family as cisco-cloud-security/04/6.
- `cisco-cloud-security/15/5`: font-metric wrap-point difference (off-slide production note) with no explicit override; low actionability, same family as cisco-cloud-security/04/6.
- `cisco-cloud-security/16/7`: font-metric wrap-point difference (off-slide note) with no explicit override; low actionability, same family as cisco-cloud-security/04/6.
- `cisco-cloud-security/19/5`: category lo-suspect: reference drops a trailing character the source text has; BetterOffice's fuller render matches the XML.
- `ocp-psp-plan/01/4`: confidence low: wrap-count difference consistent with a different (but plausible) fallback substitute for the unavailable 'Segoe UI Semilight' face; not clearly a BetterOffice defect.
- `ocp-psp-plan/03/4`: category lo-suspect: LO mid-word-wraps a wrap='none' label; BetterOffice's unwrapped single line matches the XML.
- `project17/03/2`: category lo-suspect: neither renderer has the specified font; BetterOffice's substitute is judged closer to the source design than LO's.
- `project17/05/5`: category lo-suspect per comparator, but the symptom (text mirrored under xfrm flipH) matches the transform-text-orientation-wrong-under-rotation cluster (rot=180/flipV cases) — worth the investigator double-checking this deck too instead of writing it off.
- `project17/09/3`: corrupted/unusual field XML (fld type='datetime' containing a scrambled, repeated string) drives a dropped second line and a tofu glyph; looks like a source-document oddity rather than a generalizable BetterOffice defect.
- `project17/11/6`: ambiguous: wrap='none' labels wrap anyway, but several sibling lo-suspect findings in other decks show LO itself violating wrap='none' — unclear which renderer is at fault here without visual inspection.
- `project20/05/5`: category lo-suspect: LO appears to have wrapped/clipped a noAutofit single-line footer run; BetterOffice's unwrapped render matches the XML.
- `project20/07/5`: category lo-suspect: BetterOffice honors the run's explicit direct-formatting solidFill; LO overrides it with the auto hyperlink color, which is the LO-specific quirk.
- `project20/09/5`: category lo-suspect: same hyperlink-color override behavior as project20/07/5 — BetterOffice matches the explicit direct formatting, LO substitutes its theme hlink color.
- `project20/13/4`: confidence medium, severity low: a single paragraph wraps to one line instead of two, shifting a centered block ~7% of slide height; plausible font-metric difference rather than a missed property.
- `project20/16/4`: confidence medium, severity low: trailing empty paragraphs given less line height than expected, shifting a centered block; needs code-level investigation of empty-paragraph line-height handling before it can be scoped to a specific cause.
- `rollout-plan/03/3`: category lo-suspect: LO mid-word-wraps a wrap='none' label; BetterOffice's unwrapped single line matches the XML.
- `rollout-plan/06/3`: confidence medium: header placeholders sit ~46px/15px off from default insets with no explicit override in slide or layout; unclear root cause without more examples.
- `swot-analysis/01/2`: category lo-suspect: LO splits/drops a letter from a word that fits the box; BetterOffice's whole-word single line matches the XML.
- `swot-analysis/01/3`: category lo-suspect: LO wraps a title that fits on one line per the box width; BetterOffice's single-line render matches the XML.
