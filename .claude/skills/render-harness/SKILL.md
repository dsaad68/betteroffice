---
name: render-harness
description: Drive the pptx render improvement harness end to end - register decks, render with LibreOffice and BetterOffice, diff, fan out slide comparisons, cluster findings, and investigate issues. Use for "run the harness", "compare deck X", "cluster findings", "investigate issue Y".
---

# Render improvement harness

Everything lives under `render-improvement-harness/`. Deterministic stages are scripts; judgement stages are subagents. Run scripts with `.venv/bin/python` from the repo root.

## Stages

1. **Register and render** (script, per deck):
   `.venv/bin/python render-improvement-harness/scripts/pipeline.py <deck.pptx> --id <deck-id> --source-url <url>`
   Produces `decks/<deck-id>/{meta.json,lo-img,bo-img,xml,diff-img,diff-summary.json,bo-log.json}`. Pass a deck id instead of a path to re-run; `--skip-lo` reuses the LibreOffice renders. LibreOffice takes about a minute on first use and a few seconds per deck afterwards.
   Commit `meta.json`, `diff-summary.json` and `bo-log.json` with `scripts/commit.sh "harness(<deck-id>): register"`.

2. **Compare** (subagent `slide-comparator`, one per slide, run in parallel batches of up to 8):
   Prompt: `Compare deck <deck-id> slide <NN>.` Skip slides whose `diff-summary.json` verdict is `match` unless asked otherwise; still compare every `major`, `minor` and `bo-render-failed` slide. Each agent commits its own report.

3. **Cluster** (subagent `failure-taxonomist`, one run per batch of decks):
   Prompt: `Cluster the current findings.` Produces `clusters.json`, `issues/INDEX.md`, taxonomy updates.

4. **Investigate** (subagent `issue-investigator`, one per cluster id, parallel batches of up to 4):
   Prompt: `Investigate issue <issue-id>.` Produces `issues/<issue-id>/{report.md,possible-solution.md,evidence-*.png}`.

5. **Report back**: read `issues/INDEX.md` and summarise the top issues to the user: id, impact, effort, files.

## Conventions

- Deck ids are kebab-case slugs. Slide numbers are two-digit and 1-based.
- Source decks and raw renders are gitignored; reports, summaries, clusters and issue folders are committed.
- Every commit uses `scripts/commit.sh` so concurrent agents do not clobber each other's staging. Message format: `harness(<scope>): <summary>`.
- This branch (`harness/pptx-render-improvement`) never merges to `main`. Fixes go on their own branches from `main`; the harness is re-run there to verify.
- Categories come from `taxonomy.md`. Additions go in the table, not in prose.

## Re-running after a fix

Check out the fix branch, rebuild the binding (`cd bindings/python-pptx && ../../.venv/bin/maturin develop`), then `pipeline.py <deck-id> --skip-lo` for every deck the issue lists, and compare the new `diff-summary.json` against the committed one.
