# pptx render improvement harness

Finds rendering inconsistencies in `crates/pptx-*` by comparing BetterOffice's raster output against LibreOffice on real decks, and turns them into ranked, evidence-backed issues.

```
decks/<deck-id>/
  meta.json            id, source URL, sha256, slide count, failed slides   (tracked)
  diff-summary.json    per-slide pixel diff and verdict                     (tracked)
  bo-log.json          per-slide render timings and errors                  (tracked)
  reports/NN.md        per-slide findings, written by slide-comparator      (tracked)
  source.pptx          the deck                                             (ignored)
  lo-img/NN.png        LibreOffice reference, 96 dpi                        (ignored)
  bo-img/NN.png        BetterOffice candidate, scale 1                      (ignored)
  diff-img/NN.png      heatmap; NN-sbs.png side by side                     (ignored)
  xml/NN/              slide, layout, master, theme, charts, summary.json   (ignored)
clusters.json          distinct failures, ranked, written by failure-taxonomist
issues/INDEX.md        rendered ranking table
issues/<issue-id>/     report.md, possible-solution.md, evidence-N.png, by issue-investigator
taxonomy.md            category vocabulary for findings
templates/             report skeletons the agents fill in
scripts/               deterministic stages; see pipeline.py
```

Prerequisites: `.venv` with `betteroffice_pptx` built from `bindings/python-pptx` (`maturin develop`), `pillow`, `numpy`, `pyyaml`; the `pptx-pdf` binary at `~/GitHub/pptx-pdf/target/release/pptx-pdf`. Both renderers use the Liberation, Carlito and Caladea faces from `packages/fonts/assets`, so text metrics differences are real, not font substitution.

The orchestration is described in `.claude/skills/render-harness/SKILL.md`; the three agents are in `.claude/agents/`.
