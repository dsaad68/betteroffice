---
"@betteroffice/rust-crates": patch
"@betteroffice/docx": patch
---

A keep-with-next group no longer measures taller than it is. The measurer folds a paragraph's space-before and space-after into `ParagraphExtent.totalHeight`, and the group estimate added both again on top of it, so a heading bound to its follower was charged twice for its spacing. Groups that fit the remaining space were declared too tall for it and pushed to a fresh page, leaving a short page behind and inflating the page count of any document whose headings carry spacing — which is most of them.

Column balancing counted the same spacing twice when stacking the terminal continuous section, making its balanced columns taller than the content they hold.

Both now stack a paragraph the way placement does: effective spacing before, the measured lines, effective spacing after — one shared helper, so the estimate and the placement it predicts cannot drift apart again.
