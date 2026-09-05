---
"@betteroffice/pptx": minor
"@betteroffice/rust-crates": minor
---

Read, write, thread and remove PowerPoint comments in PPTX decks.

Both wire formats are parsed: the ISO 29500 legacy pair (`ppt/commentAuthors.xml`
plus a `ppt/comments/commentN.xml` per slide) and the `[MS-PPTX]` modern threaded
pair (`ppt/authors.xml` plus `ppt/comments/modernCommentN.xml`). Writing picks
one: a deck keeps whichever flavour it arrived with, a deck with no comments
defaults to legacy, and `setCommentFlavor` switches a comment-free deck.
PowerPoint fixes a file to one system at its first comment and never mixes them,
so a package never carries both — switching drops the other flavour's parts,
relationships and content-type overrides.

`addComment` anchors a comment to a slide; `replyToComment` and
`setCommentStatus` need modern comments, since legacy `p:cm` carries neither a
reply list nor a status, and are rejected on a legacy deck. `removeComment`
takes a thread root's replies with it, and dropping a slide's last comment
removes its part, its relationship and its content-type override. Comment
positions cross the API in EMU. Legacy comments convert to 1/576-inch master
units; modern comments store EMU directly. Timestamps are caller-supplied, so the engine stays clock-free
and two peers replaying the same edits converge byte for byte.

Minor rather than patch: `DeckSnapshot` gains `comments` and `commentFlavor`,
`ParseLimits` gains `max_comments`, `DeckWrite` gains `comments`, and `EditError`
gains `CommentNotFound` and `InvalidComment` — so anything that _constructs_ a
`ParseLimits` or `DeckWrite` by literal, or matches `EditError` exhaustively,
must be updated. The deck schema moves to v3; v1 and v2 collaboration updates
load through the migration path, but restoring their existing comments and
interoperating with v2 clients remain unresolved.

Redaction now also scrubs author names, initials and user IDs from the modern
`ppt/authors.xml` part, which it previously passed through untouched.
