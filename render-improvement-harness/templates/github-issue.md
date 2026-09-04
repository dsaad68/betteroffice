<!-- title: pptx: {{title}} -->
<!-- labels: bug -->

**Describe the bug**

{{symptom}}

Seen on {{occurrences}} slide{{occurrences_plural}} across {{deck_count}} deck{{deck_count_plural}} while comparing BetterOffice's raster output against LibreOffice on real-world decks. Impact {{impact}}, estimated effort {{effort}}, confidence that this is a BetterOffice defect rather than a LibreOffice quirk: {{confidence}}.

**Screenshots**

Each image is a crop of the same region rendered by LibreOffice (reference) and BetterOffice (candidate).

{{evidence}}

**To Reproduce**

Decks are from a public sample set; the slide numbers are 1-based.

{{repro_decks}}

Render a slide with the Python binding (fonts must be registered first; the harness registers Liberation Sans/Serif/Mono, Carlito and Caladea under the names Arial, Times New Roman, Courier New, Calibri and Cambria):

```python
import betteroffice_pptx as bo
deck = bo.Presentation.open_path("deck.pptx")
deck.register_font("Arial", open("LiberationSans-Regular.ttf", "rb").read())
deck.render_png({{repro_slide_index}}, scale=1.0).write("out.png")
```

**Expected behavior**

Match the reference render. {{expected}}

**Root cause**

{{root_cause}}

**Suggested fix**

{{approach}}

{{sketch}}

Risks and tests to add:

{{risks}}

**How to verify**

{{verification}}

**Additional context**

{{extra_sections}}

Related issues found in the same run: {{related}}

Files most likely involved: {{files}}

Found with a comparison harness that renders decks with both engines, pixel-diffs them, and traces each difference back to the OOXML and the code path. Full report with all findings: {{report_link}}. Methodology: {{gist_link}}. Line numbers link to the exact commit they were checked against.
