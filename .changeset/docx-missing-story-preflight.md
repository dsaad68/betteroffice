---
"@betteroffice/docx": patch
"@betteroffice/rust-crates": patch
---

DOCX layout now skips missing auxiliary stories during font preflight and header/footer measurement, allowing the body to paginate without an unavailable header or footer.
