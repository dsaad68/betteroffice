# Failure taxonomy

Base vocabulary for the `category` field of a finding. The comparator picks the closest one;
the taxonomist may add categories, but must record additions here in the same format.

| category | covers |
|---|---|
| `render-failure` | BetterOffice raised an error or produced no image for the slide |
| `unsupported-element` | an element in the XML has no visible counterpart at all (whole shape, picture, table, chart, diagram missing) |
| `text-layout` | line breaking, wrapping, alignment, vertical anchor, insets, paragraph spacing, line spacing |
| `text-autofit` | `normAutofit` font scale / line spacing reduction, `spAutoFit`, shrink-on-overflow |
| `text-run-props` | character spacing, baseline, caps, underline, strike, highlight, run-level fill |
| `text-bullets` | bullet glyph, numbering, indent / hanging, bullet color and size |
| `text-font` | wrong face, weight, italic, size, fallback family, missing glyphs |
| `text-inheritance` | text properties from placeholder, layout, master `txStyles`, or `lstStyle` not applied |
| `placeholder-inheritance` | position / size / geometry inherited from layout or master not applied |
| `master-background` | slide, layout, or master background fill or picture wrong; master shapes shown or hidden incorrectly |
| `theme-color` | scheme color mapping, `lumMod` / `lumOff` / `tint` / `shade` / `alpha`, color map overrides |
| `fill` | solid, gradient, pattern, or picture fill wrong on a shape (not text) |
| `line` | outline width, dash, color, compound, caps, joins, arrowheads, `noFill` outlines drawn |
| `geometry-preset` | a preset shape rendered with the wrong outline or adjust values |
| `geometry-custom` | `custGeom` path parsed or drawn wrong |
| `transform` | rotation, flip, offsets, or group child scaling (`chOff` / `chExt`) wrong |
| `picture` | crop, stretch, tile, transparency, recolor, or unsupported format (EMF, WMF, SVG, TIFF) |
| `table` | cell fills, borders, merges, widths, row heights, table style |
| `chart` | any chart drawing problem: series, axes, labels, legend, colors, plot area |
| `diagram` | SmartArt (`dgm`) parts not drawn or drawn from `drawing.xml` incorrectly |
| `effects` | shadow, glow, reflection, soft edge, 3D, scene effects |
| `z-order` | shapes drawn in the wrong order |
| `connector` | connector routing, endpoints, arrowheads |
| `media-ole` | embedded objects, video posters, audio icons |
| `hidden` | hidden shapes or hidden slides handled differently |
| `lo-suspect` | the reference is probably wrong; BetterOffice matches the XML better |
| `field-eval` | a field code (`a:fld`, e.g. `slidenum`, `datetime`) renders its cached fallback text instead of being evaluated |

Severity: `high` = a reader would misread the slide; `medium` = clearly wrong but readable;
`low` = cosmetic, only visible side by side.

Confidence (that BetterOffice, not LibreOffice, is wrong): `high` = the XML settles it;
`medium` = likely, from experience with PowerPoint; `low` = could go either way.
