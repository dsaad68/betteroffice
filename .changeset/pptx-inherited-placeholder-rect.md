---
'@betteroffice/pptx-react': patch
'@betteroffice/pptx': patch
'@betteroffice/rust-crates': patch
---

pptx: a placeholder that takes its geometry from its layout or master can now be dragged and resized from the frame it draws in. The shape snapshot carries the inherited transform next to the shape's own, which stays zero so a save still writes no `a:xfrm` the author never had. The first geometry edit through `moveShape`, `resizeShape` or `setShapeRect` materializes the whole transform — offset, extent, rotation and flips — so a turned or flipped placeholder keeps its orientation and no caller can observe half of one. A placeholder still draws with the orientation it inherits before that first edit.
