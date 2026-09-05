# Hidden-shape fixtures

`hidden-shapes.pptx` is the repository's demo deck with `hidden="1"` added to
these `p:cNvPr` elements; every other ZIP part payload is unchanged:

| Slide | Snapshot shape ID | Name |
| --- | --- | --- |
| 1 | `slide:0:256:shape:0` | Cobalt rail |
| 1 | `slide:0:256:shape:8` | BetterOffice editor preview |
| 1 | `slide:0:256:shape:8.13` | PPTX tab |
| 2 | `slide:1:257:shape:4` | Format connector |
| 2 | `slide:1:257:shape:16` | Panel divider three |

Slide 1 loses 21 primitives: the cobalt rail, 14 child shapes, and six child
text boxes. Thirteen children have no hidden flag of their own. Slide 2 loses
the two marked shapes; slide 3 is unchanged.

`deck-schema-v2.snapshot.json` was serialized from the unmodified demo deck
using `DeckSession::snapshot()` and `serde_json::to_string` on
`387f2392c44e31e459264663fafd65581c8346a6` (schema 2).

`deck-schema-v2.update.bin` was generated on the same commit from the hidden
fixture with client ID 4343. Before calling `encode_state_as_update_v1()`:

1. Add a text box to `slide:1:257`, named `Persisted v2 textbox`, at
   `(100000, 100000, 2000000, 600000)` EMU, containing `persisted on v2` with
   default text style.
2. Insert `edited ` at offset 0 of `story:shape:4343:0:0`.
3. Remove `slide:1:257:shape:4`.
4. Move `slide:2:258` to index 0.

The persisted update contains no hidden shape-map keys. Migration must recover
four flags, preserve the edits, and leave the deleted shape absent.
