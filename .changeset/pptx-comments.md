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
takes the replies visible to the deleting client with it. A concurrent reply
whose root was deleted becomes a root itself on every client and remains in the
saved file. Dropping a slide's last comment
removes its part, its relationship and its content-type override. Comment
positions cross the API in EMU. Legacy comments convert to 1/576-inch master
units; modern comments store EMU directly. Timestamps are caller-supplied, so the
engine stays clock-free and two peers replaying the same edits converge byte for
byte. Existing comment XML is patched in place: status edits retain GUIDs,
author identities, task metadata, formatting, anchors, unknown XML and reply
order. Untouched comment and author parts retain their exact source bytes.

Minor rather than patch: `DeckSnapshot` gains `comments` and `commentFlavor`,
`ParseLimits` gains `max_comments`, `DeckWrite` gains `comments`, and `EditError`
gains `CommentNotFound` and `InvalidComment` — so anything that _constructs_ a
`ParseLimits` or `DeckWrite` by literal, or matches `EditError` exhaustively,
must be updated. This branch moves the deck schema from v4 to v8. Versions v1,
v2, v3 and v4 load through migration; `open_from_update_with_source` imports source
comments deterministically when an older update has no comment model. Opening
without source bytes defers that import until the original file is attached.
The import runs once, so subsequent edits and deletions survive reopening.
Older clients reject the new schema, including updates for comment-free decks;
upgrade all collaborators before sharing new updates. Saved PPTX files remain
the file-exchange path for older clients.

The coordinated landing order assigns v4 to #277, v5 to #253, v6 to #269,
v7 to #285 and v8 to this feature. This branch includes #277; #253, #269 and
#285 were still pending at integration, so their reserved versions remain
unreadable until their migrations are integrated.

Redaction now also scrubs author names, initials and user IDs from the modern
`ppt/authors.xml` part, which it previously passed through untouched.
