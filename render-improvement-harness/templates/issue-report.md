---
id: <issue-id>
title: <imperative one-liner>
category: <taxonomy category>
impact: high | medium | low
effort: easy | medium | hard
confidence: high | medium | low
status: open
occurrences: <count of findings in the cluster>
decks: [<deck-id>, ...]
findings: [<deck-id>/<NN>/<k>, ...]
files: [<repo-relative paths most likely involved>]
---

## Symptom

<What a reader sees. Two or three sentences. Point at evidence-1.png and the others.>

## Evidence

| # | deck / slide | what it shows |
|---|---|---|
| 1 | <deck>/<NN> | <one line> |

## Root cause (hypothesis)

<Where in the renderer the behaviour comes from, with `path:line` references. Say what the code
does now and what the spec or PowerPoint does instead. Mark guesses as guesses.>

## Verification

<How to confirm the fix: which slides to re-render, what the diff should drop to, which
existing tests cover the area.>
