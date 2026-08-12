---
"@betteroffice/rust-crates": patch
"@betteroffice/docx": patch
---

A keep-with-next group is now measured the way it is placed, so a heading bound to its follower stops moving to a page it did not need — and stops being stranded on one it did.

Two errors met in the old estimate. The measurer folds a paragraph's space-before and space-after into `ParagraphExtent.totalHeight`, and the estimate added both again on top of it, so every member was charged twice for its spacing and groups that fit the remaining space were pushed to a fresh page, leaving a short page behind. Against that, the estimate summed every spacing edge, while placement collapses adjacent edges to the larger of the two — Word's own rule — so a group also under-counted the gap wherever its follower's space-before exceeded the spacing above it, and a heading whose space-after is smaller than the body style's space-before could be left at the foot of a page with its follower overleaf. That is the defect keepNext exists to prevent.

The group now stacks through one shared helper that models the collapse at every boundary: the gap above the group (against the spacing the previous block deferred), the gaps between members, and the gap above the follower's witness line, which also picks up the float skip a wrapped line opens with. A table, image or text box follower is placed with no leading spacing of its own, so its gap is whatever the group's tail deferred. The break policy weighs each geometry against the height measured for it — a fresh page defers nothing, the cursor may.

Column balancing stacks the terminal continuous section through the same helper, so its balanced columns are no longer inflated by double-counted or uncollapsed spacing.
