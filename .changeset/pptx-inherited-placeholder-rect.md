---
'@betteroffice/pptx-react': patch
'@betteroffice/pptx': patch
'@betteroffice/rust-crates': patch
---

pptx: a placeholder that takes its geometry from its layout or master can now be dragged and resized from the frame it draws in. The shape snapshot carries the inherited transform next to the shape's own, which stays zero so a save still writes no `a:xfrm` the author never had, and the first move or resize materializes the complete rectangle through `setShapeRect` instead of a position-only move the renderer would overrule with the layout transform.
