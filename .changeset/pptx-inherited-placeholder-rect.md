---
'@betteroffice/pptx-react': patch
'@betteroffice/pptx': patch
'@betteroffice/python-pptx': patch
'@betteroffice/rust-crates': minor
---

pptx: a placeholder that takes its geometry from its layout or master draws with the rotation and flips it inherits, and can now be dragged and resized from the frame it is drawn in. Its snapshot carries that geometry as `inherited`, and the first `moveShape`, `resizeShape` or `setShapeRect` makes the whole transform its own, so the placeholder keeps its size and orientation live and after a save. The Rust crates add `Placeholder::matches`, the placeholder matching that rendering and editing share, and the facade re-exports `InheritedGeometry`.
