# Layout accuracy plan

## Goal

Word fidelity becomes a number this repository can compute on demand, regress against, and improve deliberately. Today it is an anecdote: the 61.9% exact page-count match quoted in `packages/fonts/README.md` was measured over 813 documents by a harness that was never committed, so no one can reproduce it, attribute it, or notice when it drops.

## Where accuracy already stands

The obvious line-metric defects are closed, and closed against measurement rather than inference.

| Behaviour | State |
| --- | --- |
| `exact` line spacing | Baseline at `EXACT_BASELINE_RATIO = 0.8` of the box, font-independent. Measured against Word 16.112. |
| `atLeast` line spacing | Floor-active lines put all slack above the baseline and preserve descent; content-winning lines pass through untouched. |
| Metric quantization | Off by default. Word output was observed not to quantize, so GDI-style rounding survives only as an opt-in `CompatFlags` experiment. |
| Font provider | `@betteroffice/fonts` supplies metric-compatible faces; published engines pull no fonts unless a consumer opts in. |

The consequence is that we are no longer working through a backlog of known defects. We are flying blind, which is what makes measurement the next unit of work rather than one more fix.

## Phases

1. **Committed corpus and scorer.** Land a parametric fixture generator (font family × size × spacing rule × indent × alignment) plus a scorer that compares our layout to a golden per line: line count, line break positions, baseline y, and line box height. Goldens are committed JSON so CI scores without Word installed.
2. **Word capture path.** A separate, explicitly manual tool that drives Word over AppleScript to regenerate goldens on a machine that has it, writing the same JSON shape the scorer consumes. Regeneration is a deliberate act with a reviewable diff, never a CI step.
3. **Real-document scoring.** Extend the scorer to score whole documents by page count against `docProps/app.xml <Pages>`, which is what the fonts README figure measured. Reproduce that figure, commit the harness that produces it, and make the README cite a command instead of a memory.
4. **Attribute the residual error.** With per-line scoring over the corpus, rank what actually contributes: shaping and kerning, justification distribution, break opportunities, indent and tab resolution, table and float interaction. Rank before fixing; the ranking is the deliverable of this phase.
5. **Fix in ranked order.** Each fix carries a corpus delta in its PR body — the score before and after — so improvement is claimed with a number rather than a screenshot.

## Phase gate

A phase is done when the scorer runs in CI on the committed corpus, the score is printed, and a regression in it fails the build. Fixture generation must be deterministic: same inputs, byte-identical fixtures, or the goldens churn and the signal is lost.

## Deliberately out of scope

Pixel diffing. It answers "did rendering change" and not "does this match Word", it is noisy across platforms and font versions, and the geometry comparison above is both stricter and easier to attribute. Revisit only once the line-level score plateaus.

## Adjacent, not part of this plan

`open_docx(bytes, true)` now seeds linearly, so a first open is fast. The complementary win is skipping seeding entirely on repeat opens by persisting the encoded CRDT state — `encode_state()` and `load()` already exist and the demo room already loads a prebuilt seed. It needs a persistence location, an invalidation rule when the source file changes, and a size budget (the demo's state is 58 KB; a 16 000-paragraph synthetic is 7.7 MB). Tracked here so it is not lost, but it is a performance item, not an accuracy one.
