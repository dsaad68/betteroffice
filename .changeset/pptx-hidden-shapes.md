---
"@betteroffice/pptx": patch
"@betteroffice/rust-crates": patch
---

Skip hidden slide shapes and hidden groups' descendants when painting and hit-testing.

Deck schema 3 migrates existing version 1 and 2 documents by recovering hidden flags from stored package data. Older clients reject the new schema.

`ShapeSnapshot.hidden` is optional and omitted when false, preserving unchanged snapshot JSON. Only hidden shapes store a Yrs key; the schema stamp changes for all decks.
