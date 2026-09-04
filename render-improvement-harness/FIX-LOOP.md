# Fix loop

How one issue goes from the ranked list to a merged pull request, and which steps a person still
owns. `FIX-PLAN.md` says what to do next; this says how to do it.

Only two pieces here are new. Everything else is a command that already exists.

## The loop

### 1. Select

Take the next issue from `FIX-PLAN.md` whose predecessors have landed and whose track has nothing
in flight. Tracks are independent; order inside a track is not.

Landed state is the one thing nothing records yet. `issues/github-issues.json` tracks what has
been filed, not what has merged, so for now keep that by hand or extend that file.

### 2. File the issue

```
.venv/bin/python render-improvement-harness/scripts/gh_issue.py <issue-id> --create
```

Renders the body from the investigation and files it, recording the number so later issues can
cross-reference it. Add `--evidence 1,3` to embed a subset of the crops, which is worth doing when
a deck's content is someone's real presentation.

### 3. Create the worktree

```
wt switch --create fix/pptx-<slug> --base feat/pptx-screenshot
```

Base on `feat/pptx-screenshot`, not on `main`. That branch is the rasterizer, proposed upstream as
pull request 264 and still open; `crates/pptx-raster` is absent from `main`, so a branch cut from
`main` cannot render a slide and step 5 would have nothing to measure. It carries the rasterizer
and none of the harness, which is what a fix wants. Move to `main` once 264 merges, and never base
on the harness branch, which carries hundreds of report files that must not reach a pull request. The hooks copy the build cache, the Python environment and the decks,
then rebuild the binding against the new worktree. Roughly a minute. Add `-x claude` to open an
agent in the worktree.

### 4. Fix

The brief is the issue folder. `report.md` names the confirmed cause and the files; the
`Verification` section states what should change and what should remain. `possible-solution.md`
gives the approach, the risks and the tests to add. Most of these areas have no test at all today,
so a fix usually adds one rather than updating one.

### 5. Verify

Build, run the crate tests, then:

```
.venv/bin/python render-improvement-harness/scripts/verify_fix.py <issue-id> \
    --engine ~/GitHub/fork/betteroffice.fix-pptx-<slug>
```

Run this from the harness worktree. The fix worktree carries no harness, so the decks, the
reference renders and the baseline all live here while only the compiled engine comes from there,
imported ahead of the local one.

It re-renders just the slides the issue names and prints, per slide, the diff before the fix, the
diff after it, and the delta. It also writes a labelled three-pane image per slide, LibreOffice
reference then BetterOffice before then BetterOffice after, which is exactly what the project's
pull request template wants in its Before/After section. Artifacts land in `issues/<id>/verify/`,
which is gitignored.

**Do not read the delta as a verdict.** The script prints, for every affected slide, which other
issues also have findings there, because a residual is usually another issue rather than a failed
fix. Some fixes are documented to make a slide worse until a dependent one lands: resolving
`grpFill` turns custom-geometry icons into solid rectangles until their paths are parsed. Judge
against the issue's own `Verification` section.

Running it against an unmodified engine prints a delta of zero everywhere, which is a useful check
that the baseline still reproduces.

### 6. Review

A person reads the diff and the three-pane images. This gate stays.

### 7. Open the pull request

The agent can write the title, the summary bullets, the test-plan checkboxes, the repro file link
and attach the images from step 5.

**One field is not the agent's.** The project's own contributor guide says the TL;DR is written by
a human, always, and that agents must leave it empty and never fill it in. Honour that.

### 8. Record and refresh the baseline

When the pull request merges, mark the issue landed so its successors unblock, then re-run the
harness on the decks it touched:

```
.venv/bin/python render-improvement-harness/scripts/pipeline.py <deck-id> --skip-lo
```

and commit the new `diff-summary.json`. Skipping this is how the loop drifts: every later
verification would compare against a baseline that predates the fix, and deltas would be measured
from the wrong place.

## What is worth automating, and what is not

| step | who |
|---|---|
| 1 select, 2 file, 3 worktree | script or one command |
| 4 fix | agent, with the issue folder as its brief |
| 5 verify | script for the numbers and images, agent to read them against the issue |
| 6 review, and the TL;DR in 7 | person |
| 7 pull request body, 8 record | agent and script |

**Pace it by review capacity, not by fix throughput.** The repository already carries other open
issues and a small number of maintainers. Landing a couple and watching the response beats opening
a queue that nobody has time to read.

**Run at most one or two tracks at a time.** Three files carry most of the work, so branches
contend even when the issues are logically independent, and the text track invalidates raster
goldens on every step.
