//! Deck comments: a flat side map keyed by comment id, with replies naming
//! their thread root.
//!
//! Ids come from the session's deterministic counter, never from a generated
//! GUID, so two peers that replay the same edits snapshot identically. The
//! GUIDs the modern wire format needs are derived from those ids at save time.

use pptx_parse::{CommentFlavor, PptxPackage};
use sha2::{Digest, Sha256};
use yrs::{Map, MapPrelim, MapRef, ReadTxn, Transact, TransactionMut, WriteTxn};

use crate::deck::{map_bool, map_number, map_string, required_map};
use crate::{
    COMMENTS, CommentReceipt, CommentSnapshot, DeckSession, EditCtx, EditError, EditResult, META,
};

/// Builds the stable id a seeded comment keeps for the deck's lifetime.
pub(crate) fn seeded_comment_id(index: usize, source_id: &str) -> String {
    format!("comment:{index}:{source_id}")
}

pub(crate) fn seed_comments(
    txn: &mut TransactionMut<'_>,
    package: &PptxPackage,
    slide_id_by_part: &dyn Fn(&str) -> Option<String>,
) -> EditResult<()> {
    let comments = txn.get_or_insert_map(COMMENTS);
    for (index, comment) in package.comments.iter().enumerate() {
        let Some(slide_id) = slide_id_by_part(&comment.slide_part_path) else {
            continue;
        };
        let author = package
            .comment_authors
            .iter()
            .find(|author| author.id == comment.author_id);
        let id = seeded_comment_id(index, &comment.id);
        let entry = comments.insert(txn, id.as_str(), MapPrelim::default());
        entry.insert(txn, "id", id.as_str());
        entry.insert(txn, "slideId", slide_id.as_str());
        entry.insert(
            txn,
            "author",
            author.map(|author| author.name.as_str()).unwrap_or(""),
        );
        entry.insert(
            txn,
            "initials",
            author.map(|author| author.initials.as_str()).unwrap_or(""),
        );
        entry.insert(txn, "text", comment.text.as_str());
        if let Some(created) = &comment.created {
            entry.insert(txn, "created", created.as_str());
        }
        entry.insert(txn, "x", comment.x_emu as f64);
        entry.insert(txn, "y", comment.y_emu as f64);
        if let Some(parent) = &comment.parent_id {
            let parent_index = package
                .comments
                .iter()
                .position(|candidate| &candidate.id == parent);
            if let Some(parent_index) = parent_index {
                entry.insert(
                    txn,
                    "parentId",
                    seeded_comment_id(parent_index, parent).as_str(),
                );
            }
        }
        entry.insert(
            txn,
            "resolved",
            comment.status.as_deref() == Some("resolved"),
        );
    }
    Ok(())
}

pub(crate) fn snapshot_comments<T: ReadTxn>(txn: &T) -> EditResult<Vec<CommentSnapshot>> {
    let comments = required_map(txn, COMMENTS)?;
    let mut output = Vec::new();
    for (id, value) in comments.iter(txn) {
        let Ok(entry) = value.cast::<MapRef>() else {
            return Err(EditError::InvalidState(format!(
                "comment {id} is not a map"
            )));
        };
        output.push(CommentSnapshot {
            id: id.to_owned(),
            slide_id: map_string(&entry, txn, "slideId").unwrap_or_default(),
            author: map_string(&entry, txn, "author").unwrap_or_default(),
            initials: map_string(&entry, txn, "initials").unwrap_or_default(),
            text: map_string(&entry, txn, "text").unwrap_or_default(),
            created: map_string(&entry, txn, "created"),
            x_emu: map_number(&entry, txn, "x").unwrap_or(0.0) as i64,
            y_emu: map_number(&entry, txn, "y").unwrap_or(0.0) as i64,
            parent_id: map_string(&entry, txn, "parentId"),
            resolved: map_bool(&entry, txn, "resolved").unwrap_or(false),
        });
    }
    output.sort_by(|left, right| {
        left.created
            .cmp(&right.created)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(output)
}

pub(crate) fn snapshot_flavor<T: ReadTxn>(txn: &T) -> EditResult<CommentFlavor> {
    let meta = required_map(txn, META)?;
    Ok(match map_string(&meta, txn, "commentFlavor").as_deref() {
        Some("modern") => CommentFlavor::Modern,
        _ => CommentFlavor::Legacy,
    })
}

pub(crate) fn flavor_key(flavor: CommentFlavor) -> &'static str {
    match flavor {
        CommentFlavor::Legacy => "legacy",
        CommentFlavor::Modern => "modern",
    }
}

/// A stable braced GUID for the modern wire format, derived from a deck-stable
/// id so it survives a save/reopen cycle unchanged.
pub(crate) fn derived_guid(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let hex: String = digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02X}"))
        .collect();
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

impl DeckSession {
    /// Anchors a comment to a slide. `created` is caller-supplied so the engine
    /// stays clock-free and reproducible.
    #[allow(clippy::too_many_arguments)]
    pub fn add_comment(
        &self,
        context: &EditCtx,
        slide_id: &str,
        author: &str,
        initials: &str,
        text: &str,
        created: &str,
        x_emu: i64,
        y_emu: i64,
    ) -> EditResult<CommentReceipt> {
        crate::model::validate_xml_text(text)?;
        crate::model::validate_xml_text(author)?;
        if text.is_empty() {
            return Err(EditError::InvalidComment("comment text is empty".into()));
        }
        let comment_id = self.next_id("comment");
        let mut txn = self.transact_for(context);
        crate::deck::slide_ref(&txn, slide_id)?;
        let comments = required_map(&txn, COMMENTS)?;
        let entry = comments.insert(&mut txn, comment_id.as_str(), MapPrelim::default());
        entry.insert(&mut txn, "id", comment_id.as_str());
        entry.insert(&mut txn, "slideId", slide_id);
        entry.insert(&mut txn, "author", author);
        entry.insert(&mut txn, "initials", initials);
        entry.insert(&mut txn, "text", text);
        entry.insert(&mut txn, "created", created);
        entry.insert(&mut txn, "x", x_emu as f64);
        entry.insert(&mut txn, "y", y_emu as f64);
        entry.insert(&mut txn, "resolved", false);
        Ok(CommentReceipt {
            comment_id,
            slide_id: slide_id.to_owned(),
            parent_id: None,
            resolved: false,
        })
    }

    /// Adds a reply to a thread. Legacy `p:cm` has no reply list, so this is
    /// rejected on a legacy deck.
    pub fn reply_to_comment(
        &self,
        context: &EditCtx,
        comment_id: &str,
        author: &str,
        initials: &str,
        text: &str,
        created: &str,
    ) -> EditResult<CommentReceipt> {
        crate::model::validate_xml_text(text)?;
        crate::model::validate_xml_text(author)?;
        if text.is_empty() {
            return Err(EditError::InvalidComment("reply text is empty".into()));
        }
        let reply_id = self.next_id("comment");
        let mut txn = self.transact_for(context);
        require_modern(&txn)?;
        let comments = required_map(&txn, COMMENTS)?;
        let parent = comment_ref(&comments, &txn, comment_id)?;
        if map_string(&parent, &txn, "parentId").is_some() {
            return Err(EditError::InvalidComment(
                "replies cannot be nested below a reply".into(),
            ));
        }
        let slide_id = map_string(&parent, &txn, "slideId").unwrap_or_default();
        let entry = comments.insert(&mut txn, reply_id.as_str(), MapPrelim::default());
        entry.insert(&mut txn, "id", reply_id.as_str());
        entry.insert(&mut txn, "slideId", slide_id.as_str());
        entry.insert(&mut txn, "author", author);
        entry.insert(&mut txn, "initials", initials);
        entry.insert(&mut txn, "text", text);
        entry.insert(&mut txn, "created", created);
        entry.insert(&mut txn, "x", 0.0);
        entry.insert(&mut txn, "y", 0.0);
        entry.insert(&mut txn, "parentId", comment_id);
        entry.insert(&mut txn, "resolved", false);
        Ok(CommentReceipt {
            comment_id: reply_id,
            slide_id,
            parent_id: Some(comment_id.to_owned()),
            resolved: false,
        })
    }

    /// Marks a thread resolved. Modern only — legacy has no status field, so
    /// resolving there means deleting.
    pub fn set_comment_status(
        &self,
        context: &EditCtx,
        comment_id: &str,
        resolved: bool,
    ) -> EditResult<CommentReceipt> {
        let mut txn = self.transact_for(context);
        require_modern(&txn)?;
        let comments = required_map(&txn, COMMENTS)?;
        let entry = comment_ref(&comments, &txn, comment_id)?;
        let slide_id = map_string(&entry, &txn, "slideId").unwrap_or_default();
        let parent_id = map_string(&entry, &txn, "parentId");
        entry.insert(&mut txn, "resolved", resolved);
        Ok(CommentReceipt {
            comment_id: comment_id.to_owned(),
            slide_id,
            parent_id,
            resolved,
        })
    }

    /// Removes a comment. Removing a thread root removes its replies too.
    pub fn remove_comment(
        &self,
        context: &EditCtx,
        comment_id: &str,
    ) -> EditResult<CommentReceipt> {
        let mut txn = self.transact_for(context);
        let comments = required_map(&txn, COMMENTS)?;
        let entry = comment_ref(&comments, &txn, comment_id)?;
        let slide_id = map_string(&entry, &txn, "slideId").unwrap_or_default();
        let parent_id = map_string(&entry, &txn, "parentId");
        let resolved = map_bool(&entry, &txn, "resolved").unwrap_or(false);
        let replies: Vec<String> = comments
            .iter(&txn)
            .filter_map(|(id, value)| {
                let entry = value.cast::<MapRef>().ok()?;
                (map_string(&entry, &txn, "parentId").as_deref() == Some(comment_id))
                    .then(|| id.to_owned())
            })
            .collect();
        for reply in replies {
            comments.remove(&mut txn, &reply);
        }
        comments.remove(&mut txn, comment_id);
        Ok(CommentReceipt {
            comment_id: comment_id.to_owned(),
            slide_id,
            parent_id,
            resolved,
        })
    }

    /// Switches which comment system the deck writes. Only legal while the deck
    /// has no comments — PowerPoint fixes the flavour at the first comment and
    /// the two bodies are not interchangeable.
    pub fn set_comment_flavor(
        &self,
        context: &EditCtx,
        flavor: CommentFlavor,
    ) -> EditResult<CommentFlavor> {
        let mut txn = self.transact_for(context);
        let comments = required_map(&txn, COMMENTS)?;
        if comments.len(&txn) > 0 {
            return Err(EditError::InvalidComment(
                "the comment flavour is fixed once a deck has comments".into(),
            ));
        }
        let meta = required_map(&txn, META)?;
        meta.insert(&mut txn, "commentFlavor", flavor_key(flavor));
        Ok(flavor)
    }

    pub fn comments(&self) -> EditResult<Vec<CommentSnapshot>> {
        snapshot_comments(&self.doc.transact())
    }

    pub fn comment_flavor(&self) -> EditResult<CommentFlavor> {
        snapshot_flavor(&self.doc.transact())
    }
}

fn require_modern<T: ReadTxn>(txn: &T) -> EditResult<()> {
    match snapshot_flavor(txn)? {
        CommentFlavor::Modern => Ok(()),
        CommentFlavor::Legacy => Err(EditError::InvalidComment(
            "legacy comments carry no replies or status; switch the deck to modern comments".into(),
        )),
    }
}

fn comment_ref<T: ReadTxn>(comments: &MapRef, txn: &T, comment_id: &str) -> EditResult<MapRef> {
    comments
        .get(txn, comment_id)
        .and_then(|value| value.cast::<MapRef>().ok())
        .ok_or_else(|| EditError::CommentNotFound(comment_id.to_owned()))
}
