---
"@betteroffice/rust-crates": patch
"@betteroffice/docx": patch
---

Widow and orphan control now follows what the document authors. `w:widowControl` (§17.3.1.44) was parsed and written back, but nothing carried it from the paragraph properties into layout: the placement walk applied the rule to every paragraph of four lines or more, so a document that turns it off — Word's own `w:widowControl w:val="0"`, common in styles imported from typesetting templates — still had its paragraphs pushed whole onto the next page rather than split at the page boundary the author asked for.

The flag is a toggle that defaults on, so it inherits down the `basedOn` chain and only an authored off is worth carrying. It is now seeded onto the paragraph like `keepNext` and `keepLines`, resolving against the style chain and doc defaults, and reaches the layout attributes only when it resolves to false. Because absence is what encodes the default, applying a style clears the flag unless the new style authors an off of its own.
