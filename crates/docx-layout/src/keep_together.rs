//! Keep-with-next grouping (`w:keepNext`, ECMA-376 §17.3.1.15).
//!
//! A run of consecutive keepNext paragraphs must share a page with the *start*
//! of whatever follows it. [`analyze_keep_with_next`] walks the measured blocks
//! once and returns each run keyed by its head, plus the interior members, so
//! the placement walk can skip blocks a group already accounted for.
//!
//! [`measure_keep_with_next_group`] turns a group into the height the contract
//! actually demands: every member paragraph plus the follower slice placement
//! requires before it can begin. The witness is one paragraph line normally,
//! two under widow control, every line under `w:keepLines`, a table's first row,
//! or the whole height of an in-flow object.
//!
//! The group is stacked through [`FlowStack`], so the budget is what placement
//! will consume rather than a sum of parts: every gap — above the head, between
//! members, and above the witness — collapses to the larger of the two spacings
//! that meet there, and style-inherited spacing on a blank paragraph is dropped
//! the way placement drops it. A paragraph follower brings its own leading
//! spacing to that last gap; an in-flow object is placed with none of its own,
//! so there the gap is whatever the tail deferred. Overlay followers add no
//! witness because placement leaves the pen unchanged.

use std::collections::{BTreeMap, BTreeSet};

use crate::floating_objects::{is_anchored_image_block, is_floating_text_box_block};
use crate::paragraph_spacing::{FlowStack, get_spacing_before, lines_height};
use crate::types::{BlockExtent, LayoutBlock, MeasuredBlock};

/// A maximal keep-with-next run and its follower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepWithNextGroup {
    /// Index of the run's leading paragraph.
    pub head_index: usize,
    /// Index of the run's final keep-with-next paragraph.
    pub tail_index: usize,
    /// Every block index that belongs to the run, in order.
    pub members: Vec<usize>,
    /// Index of the following flow block whose first unbreakable slice is the
    /// keep witness, or `None` at a forced/section break or EOF.
    pub follower: Option<usize>,
}

/// The two indexes placement needs: groups by head block, and every non-head
/// member so the walk does not re-evaluate them. Both iterate in ascending
/// block order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeepWithNextScan {
    /// Groups keyed by their leading block index.
    pub groups_by_head: BTreeMap<usize, KeepWithNextGroup>,
    /// Block indices that belong to a group but are not its head.
    pub interior_members: BTreeSet<usize>,
}

// true only for a paragraph block carrying a truthy keepNext flag
fn is_bound_paragraph(block: &LayoutBlock) -> bool {
    match block {
        LayoutBlock::Paragraph(p) => p.attrs.as_ref().and_then(|a| a.keep_next).unwrap_or(false),
        _ => false,
    }
}

/// Group every maximal run of consecutive keep-with-next paragraphs.
///
/// A run grows while the next block is another keep-with-next paragraph; it
/// ends at a break block, a non-paragraph block, a paragraph without keepNext,
/// or the end of the list. When the terminator is a plain paragraph it becomes
/// the run's follower, since the run must land on the follower's page.
pub fn analyze_keep_with_next(measured: &[MeasuredBlock]) -> KeepWithNextScan {
    let mut groups_by_head: BTreeMap<usize, KeepWithNextGroup> = BTreeMap::new();
    let mut interior_members: BTreeSet<usize> = BTreeSet::new();

    let mut cursor = 0usize;
    while cursor < measured.len() {
        if !is_bound_paragraph(&measured[cursor].block) {
            cursor += 1;
            continue;
        }

        let mut members: Vec<usize> = vec![cursor];
        let mut tail_index = cursor;
        let mut probe = cursor + 1;
        while probe < measured.len() && is_bound_paragraph(&measured[probe].block) {
            members.push(probe);
            tail_index = probe;
            probe += 1;
        }

        // A keep chain binds to the first unbreakable slice of any following
        // supported flow object. Forced/section breaks terminate it.
        let after_tail = tail_index + 1;
        let follower = if after_tail < measured.len()
            && matches!(
                measured[after_tail].block,
                LayoutBlock::Paragraph(_)
                    | LayoutBlock::Table(_)
                    | LayoutBlock::Image(_)
                    | LayoutBlock::Shape(_)
                    | LayoutBlock::Chart(_)
                    | LayoutBlock::TextBox(_)
            ) {
            Some(after_tail)
        } else {
            None
        };

        for k in 1..members.len() {
            interior_members.insert(members[k]);
        }
        groups_by_head.insert(
            cursor,
            KeepWithNextGroup {
                head_index: cursor,
                tail_index,
                members,
                follower,
            },
        );

        cursor = tail_index + 1;
    }

    KeepWithNextScan {
        groups_by_head,
        interior_members,
    }
}

/// The height a keep-with-next group needs, in each of the two geometries the
/// break policy weighs it against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeepWithNextHeight {
    /// From the cursor, where the head's leading spacing collapses against
    /// whatever the block above deferred.
    pub at_cursor: f64,
    /// At the top of a fresh page or column, where nothing is deferred.
    pub on_fresh_page: f64,
}

/// Vertical space (px) the group needs for its keepNext contract to hold on a
/// single page, from a cursor carrying `deferred_spacing`.
pub fn measure_keep_with_next_group(
    group: &KeepWithNextGroup,
    measured: &[MeasuredBlock],
    deferred_spacing: f64,
) -> KeepWithNextHeight {
    KeepWithNextHeight {
        at_cursor: stack_group(group, measured, deferred_spacing),
        on_fresh_page: stack_group(group, measured, 0.0),
    }
}

fn stack_group(group: &KeepWithNextGroup, measured: &[MeasuredBlock], deferred: f64) -> f64 {
    let mut stack = FlowStack::resuming(deferred);
    for &index in &group.members {
        let MeasuredBlock { block, measure } = &measured[index];
        let (LayoutBlock::Paragraph(block), BlockExtent::Paragraph(measure)) = (block, measure)
        else {
            continue;
        };
        stack.push_paragraph(block, measure);
    }
    if let Some(follower) = group.follower.and_then(|index| measured.get(index)) {
        if let Some((before, witness)) = witness_slice(follower) {
            stack.push(before, witness, 0.0);
        }
    }
    stack.height()
}

/// The follower's own leading spacing and the first slice bound to the group.
/// In-flow objects have no leading spacing of their own. Anchored images and
/// floating text boxes return no slice because they do not move the pen.
fn witness_slice(follower: &MeasuredBlock) -> Option<(f64, f64)> {
    match (&follower.block, &follower.measure) {
        (LayoutBlock::Paragraph(block), BlockExtent::Paragraph(measure)) => {
            let witness_lines = if paragraph_keeps_lines(&follower.block) {
                measure.lines.len()
            } else if measure.lines.len() >= 4
                && block
                    .attrs
                    .as_ref()
                    .and_then(|attrs| attrs.widow_control)
                    .unwrap_or(true)
            {
                2
            } else {
                measure.lines.len().min(1)
            };
            Some((
                get_spacing_before(block),
                lines_height(&measure.lines[..witness_lines]),
            ))
        }
        (_, BlockExtent::Table(table)) => {
            Some((0.0, table.rows.first().map_or(0.0, |row| row.height)))
        }
        (LayoutBlock::Image(block), BlockExtent::Image(image)) => {
            (!is_anchored_image_block(block)).then_some((0.0, image.height))
        }
        (_, BlockExtent::Shape(shape)) => Some((0.0, shape.height)),
        (_, BlockExtent::Chart(chart)) => Some((0.0, chart.height)),
        (LayoutBlock::TextBox(block), BlockExtent::TextBox(text_box)) => {
            (!is_floating_text_box_block(block)).then_some((0.0, text_box.height))
        }
        _ => None,
    }
}

/// Whether a paragraph forbids splitting its own lines across a page (keepLines).
pub fn paragraph_keeps_lines(block: &LayoutBlock) -> bool {
    match block {
        LayoutBlock::Paragraph(p) => p.attrs.as_ref().and_then(|a| a.keep_lines) == Some(true),
        _ => false,
    }
}

/// Whether a paragraph must begin on a fresh page (pageBreakBefore).
pub fn paragraph_breaks_before(block: &LayoutBlock) -> bool {
    match block {
        LayoutBlock::Paragraph(p) => {
            p.attrs.as_ref().and_then(|a| a.page_break_before) == Some(true)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::break_policy::{KeepWithNextFit, keep_with_next_group_must_advance};
    use crate::types::{
        BlockId, ParagraphAttrs, ParagraphBlock, ParagraphExtent, ParagraphSpacing, Run,
        RunFormatting, SpacingExplicit, TextRun, TypesetRow,
    };

    fn text_run(text: &str) -> Run {
        Run::Text(TextRun {
            fmt: RunFormatting::default(),
            text: text.to_string(),
            pm_start: None,
            pm_end: None,
            inline_sdt_widget: None,
        })
    }

    fn paragraph(runs: Vec<Run>, attrs: Option<ParagraphAttrs>) -> LayoutBlock {
        LayoutBlock::Paragraph(ParagraphBlock {
            sdt_groups: None,
            id: BlockId::Num(0.0),
            para_id: None,
            runs,
            attrs,
            pm_start: None,
            pm_end: None,
        })
    }

    fn make_paragraph_block(text: &str, keep_next: bool) -> LayoutBlock {
        paragraph(
            vec![text_run(text)],
            Some(ParagraphAttrs {
                keep_next: if keep_next { Some(true) } else { None },
                ..Default::default()
            }),
        )
    }

    /// A paragraph carrying authored spacing, optionally bound to what follows.
    fn spaced_paragraph(spacing: (f64, f64), keep_next: bool) -> LayoutBlock {
        paragraph(
            vec![text_run("text")],
            Some(ParagraphAttrs {
                keep_next: keep_next.then_some(true),
                spacing: Some(ParagraphSpacing {
                    before: Some(spacing.0),
                    after: Some(spacing.1),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        )
    }

    fn make_line(line_height: f64) -> TypesetRow {
        TypesetRow {
            line_height,
            ..Default::default()
        }
    }

    fn skipped_line(line_height: f64, float_skip_before: f64) -> TypesetRow {
        TypesetRow {
            line_height,
            float_skip_before: Some(float_skip_before),
            ..Default::default()
        }
    }

    // empty keepNext paragraph whose spacing is style-inherited (spacingExplicit
    // unset) — placement drops this spacing, so the group estimate must too
    fn make_empty_spaced_paragraph(
        spacing: (f64, f64),
        spacing_explicit: Option<(bool, bool)>,
    ) -> LayoutBlock {
        paragraph(
            vec![text_run("")],
            Some(ParagraphAttrs {
                keep_next: Some(true),
                spacing: Some(ParagraphSpacing {
                    before: Some(spacing.0),
                    after: Some(spacing.1),
                    ..Default::default()
                }),
                spacing_explicit: spacing_explicit.map(|(before, after)| SpacingExplicit {
                    before: Some(before),
                    after: Some(after),
                }),
                ..Default::default()
            }),
        )
    }

    /// Measures blocks the way the measurer does: `totalHeight` folds the
    /// paragraph's authored spacing in whether or not placement will paint it.
    fn to_measured_blocks(
        blocks: Vec<LayoutBlock>,
        lines: Vec<Vec<TypesetRow>>,
    ) -> Vec<MeasuredBlock> {
        assert_eq!(blocks.len(), lines.len());
        blocks
            .into_iter()
            .zip(lines)
            .map(|(block, lines)| {
                let (before, after) = match &block {
                    LayoutBlock::Paragraph(p) => p
                        .attrs
                        .as_ref()
                        .and_then(|attrs| attrs.spacing.as_ref())
                        .map_or((0.0, 0.0), |spacing| {
                            (spacing.before.unwrap_or(0.0), spacing.after.unwrap_or(0.0))
                        }),
                    _ => (0.0, 0.0),
                };
                let total_height = lines
                    .iter()
                    .map(|line| line.line_height + line.float_skip_before.unwrap_or(0.0))
                    .sum::<f64>()
                    + before
                    + after;
                MeasuredBlock {
                    block,
                    measure: BlockExtent::Paragraph(ParagraphExtent {
                        lines,
                        total_height,
                    }),
                }
            })
            .collect()
    }

    fn group_height(measured: &[MeasuredBlock], head: usize, deferred: f64) -> KeepWithNextHeight {
        let scan = analyze_keep_with_next(measured);
        let group = scan
            .groups_by_head
            .get(&head)
            .unwrap_or_else(|| panic!("group headed at block {head}"));
        measure_keep_with_next_group(group, measured, deferred)
    }

    #[test]
    fn ignores_style_inherited_spacing_on_an_empty_member_like_placement_does() {
        let measured = to_measured_blocks(
            vec![
                make_paragraph_block("Heading", true),
                make_empty_spaced_paragraph((150.0, 150.0), None),
                make_paragraph_block("Follower", false),
            ],
            vec![vec![make_line(20.0)], vec![], vec![make_line(20.0)]],
        );

        // heading line (20) + empty member (0, spacing suppressed) + follower first line (20)
        assert_eq!(group_height(&measured, 0, 0.0).at_cursor, 40.0);
    }

    #[test]
    fn keeps_counting_explicit_spacing_on_an_empty_member() {
        let measured = to_measured_blocks(
            vec![
                make_paragraph_block("Heading", true),
                make_empty_spaced_paragraph((150.0, 150.0), Some((true, true))),
                make_paragraph_block("Follower", false),
            ],
            vec![vec![make_line(20.0)], vec![], vec![make_line(20.0)]],
        );

        // direct formatting survives on empty paragraphs: 20 + 150 + 0 + 150 + 20
        assert_eq!(group_height(&measured, 0, 0.0).at_cursor, 340.0);
    }

    #[test]
    fn counts_member_spacing_once_when_the_measure_already_carries_it() {
        let measured = to_measured_blocks(
            vec![
                spaced_paragraph((10.0, 10.0), true),
                make_paragraph_block("Follower", false),
            ],
            vec![vec![make_line(20.0)], vec![make_line(20.0)]],
        );
        let BlockExtent::Paragraph(head) = &measured[0].measure else {
            panic!("paragraph measure");
        };
        assert_eq!(head.total_height, 40.0);

        // 10 before + 20 line + 10 after + 20 witness — not 10 + 40 + 10 + 20
        assert_eq!(group_height(&measured, 0, 0.0).at_cursor, 60.0);
    }

    #[test]
    fn collapses_the_gap_between_two_members() {
        let measured = to_measured_blocks(
            vec![
                spaced_paragraph((0.0, 10.0), true),
                spaced_paragraph((10.0, 0.0), true),
                make_paragraph_block("Follower", false),
            ],
            vec![
                vec![make_line(20.0)],
                vec![make_line(20.0)],
                vec![make_line(20.0)],
            ],
        );

        // the 10 between the members meets once, not twice: 20 + 10 + 20 + 20
        assert_eq!(group_height(&measured, 0, 0.0).at_cursor, 70.0);
    }

    #[test]
    fn charges_the_follower_the_larger_of_its_gap_and_the_tail_spacing() {
        let wider_follower = to_measured_blocks(
            vec![
                spaced_paragraph((0.0, 10.0), true),
                spaced_paragraph((30.0, 0.0), false),
            ],
            vec![vec![make_line(20.0)], vec![make_line(20.0)]],
        );
        let wider_tail = to_measured_blocks(
            vec![
                spaced_paragraph((0.0, 30.0), true),
                spaced_paragraph((10.0, 0.0), false),
            ],
            vec![vec![make_line(20.0)], vec![make_line(20.0)]],
        );

        assert_eq!(group_height(&wider_follower, 0, 0.0).at_cursor, 70.0);
        assert_eq!(group_height(&wider_tail, 0, 0.0).at_cursor, 70.0);
    }

    #[test]
    fn charges_the_float_skip_above_the_witness_line() {
        let measured = to_measured_blocks(
            vec![
                make_paragraph_block("Heading", true),
                make_paragraph_block("Follower", false),
            ],
            vec![vec![make_line(20.0)], vec![skipped_line(20.0, 15.0)]],
        );

        // placement charges the witness its float skip, so the budget must too
        assert_eq!(group_height(&measured, 0, 0.0).at_cursor, 55.0);
    }

    #[test]
    fn collapses_the_spacing_deferred_above_the_head() {
        let measured = to_measured_blocks(
            vec![
                spaced_paragraph((5.0, 0.0), true),
                make_paragraph_block("Follower", false),
            ],
            vec![vec![make_line(20.0)], vec![make_line(20.0)]],
        );

        let height = group_height(&measured, 0, 30.0);
        assert_eq!(height.at_cursor, 70.0);
        assert_eq!(height.on_fresh_page, 45.0);
    }

    #[test]
    fn charges_a_table_follower_only_the_gap_its_tail_defers() {
        let mut measured = to_measured_blocks(
            vec![spaced_paragraph((0.0, 12.0), true)],
            vec![vec![make_line(20.0)]],
        );
        measured.push(
            serde_json::from_value(serde_json::json!({
                "block": { "kind": "table", "id": 9, "rows": [], "columnWidths": [] },
                "measure": {
                    "kind": "table", "columnWidths": [], "totalWidth": 0, "totalHeight": 65,
                    "rows": [{ "height": 25, "cells": [] }, { "height": 40, "cells": [] }],
                },
            }))
            .expect("table measured block"),
        );

        // 20 line + 12 deferred + the first row only
        assert_eq!(group_height(&measured, 0, 0.0).at_cursor, 57.0);
    }

    #[test]
    fn does_not_advance_a_group_that_fits_once_inherited_empty_paragraph_spacing_is_dropped() {
        let measured = to_measured_blocks(
            vec![
                make_paragraph_block("Filler", false),
                make_paragraph_block("Heading", true),
                make_empty_spaced_paragraph((150.0, 150.0), None),
                make_paragraph_block("Follower", false),
            ],
            vec![
                vec![make_line(620.0)],
                vec![make_line(20.0)],
                vec![],
                vec![make_line(20.0)],
            ],
        );

        let scan = analyze_keep_with_next(&measured);
        let group = scan
            .groups_by_head
            .get(&1)
            .expect("group headed at block 1");
        assert_eq!(group.members, vec![1, 2]);
        assert_eq!(group.follower, Some(3));

        let height = measure_keep_with_next_group(group, &measured, 0.0);
        assert_eq!(height.at_cursor, 40.0);

        // content height 864 (1056 - 2*96); the 620px filler leaves 244 available
        assert!(!keep_with_next_group_must_advance(KeepWithNextFit {
            height,
            available_height: 244.0,
            page_content_height: 864.0,
            page_has_content: true,
        }));
    }
}
