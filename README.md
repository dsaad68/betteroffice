# Render evidence

Before/after renders backing the open pull requests against
[openooxml/betteroffice](https://github.com/openooxml/betteroffice). Each image shows one slide three
ways: the LibreOffice reference, BetterOffice before the fix, and BetterOffice after it. Files ending
`-detail` are magnified crops of the region the fix changes.

Renders are produced at 96 dpi with Liberation, Carlito and Caladea aliased to the metric-compatible
Microsoft families, so the reference and the candidate shape text with the same metrics.

## Decks

Every deck here is a PowerPoint design template whose slides carry only placeholder content — "Lorem
ipsum", "ADD TEXT", "Add Some Brief To Explain". None contains business data, personal data or
customer material.

| deck | template | where to get it |
|---|---|---|
| `green-solutions` | Unique Way To Showcase Your Green Solutions (PowerPoint School) | [document 4.zip](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip) |
| `swot-analysis` | SWOT Analysis Slide Design Template (PowerPoint School) | [document 4.zip](https://github.com/Suparnapaul393/PowerPoint-Sample/blob/master/document%204.zip) |
| `tpl-double-exposure-business-templates` | Double Exposure Business | [free-powerpoint-templates-design.com](http://www.free-powerpoint-templates-design.com) |
| `tpl-innovative-way-create-charts-graphs` | Innovative Way To Create Charts & Graphs (PowerPoint School) | PowerPoint School |
| `tpl-modern-business-infographic-presentation` | Modern Business Infographic (PowerPoint School) | PowerPoint School |
| `tpl-attractive-presentation-slide-animation-by-s` | Attractive Presentation Slide with animation (PowerPoint School) | PowerPoint School |
| `tpl-write-and-quote-slide-business-presentation` | Quote Slide for Business Presentation (PowerPoint School) | PowerPoint School |
| `cc-durand` | student presentation on Jean-Nicolas-Louis Durand, by Mayank | [Internet Archive](https://archive.org/details/jeannicolaslouisdurand), Public Domain Mark 1.0 |

Deck files are not redistributed here; only the renders are. The one non-template deck,
`cc-durand`, appears as a tight crop of a Durand engraving — a 19th-century work in the public
domain — chosen so no part of the slide carrying the author's name or student number is shown.

## Test corpora used locally

Two open benchmark corpora were used to find and confirm these defects, and neither is the source of
any image in this repository. Recording them because their licence position is easy to misread:

- [microsoft/ppteval](https://github.com/microsoft/ppteval) — MIT for the code. It ships no decks;
  they are hydrated from the Internet Archive and each carries its own Creative Commons licence,
  listed in that repo's `ATTRIBUTION.md`.
- [PPTArena](https://huggingface.co/datasets/mofengenden/PPTArena) — the dataset card states MIT, but
  the decks are third-party presentations with no per-file provenance.
