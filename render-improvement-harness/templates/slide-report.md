---
deck: <deck-id>
slide: <n>
verdict: match | minor | major | bo-render-failed
diff_pct: <fine_pct from diff-summary.json>
findings:
  - id: <deck-id>/<NN>/1
    category: <from taxonomy.md>
    severity: high | medium | low
    confidence: high | medium | low
    shape: "<name> (id <id>)"
    region: [x0, y0, x1, y1]
    summary: <one sentence, what differs>
    xml: <the attribute or element that proves it, one line>
---

## Reference vs candidate

<Two to five sentences. What the slide is, what the reference shows, what the candidate shows.>

## Findings

### 1. <summary>

- Where: <shape, region>
- Reference: <what LibreOffice draws>
- Candidate: <what BetterOffice draws>
- XML: <the relevant snippet, fenced, trimmed to the lines that matter>
- Why the candidate is wrong: <one or two sentences, cite the XML or the spec>

## Not reported

<Differences seen and deliberately ignored: anti-aliasing, sub-pixel offsets, LibreOffice quirks. One line each.>
