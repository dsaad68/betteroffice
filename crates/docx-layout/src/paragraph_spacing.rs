//! Paragraph spacing resolution.

use crate::types::{ParagraphBlock, ParagraphExtent, Run, TypesetRow};

fn is_empty_paragraph(block: &ParagraphBlock) -> bool {
    if block.runs.is_empty() {
        return true;
    }
    if block.runs.len() != 1 {
        return false;
    }
    match &block.runs[0] {
        Run::Text(r) => r.text.is_empty(),
        _ => false,
    }
}

/// Returns effective leading spacing.
pub fn get_spacing_before(block: &ParagraphBlock) -> f64 {
    let value = block
        .attrs
        .as_ref()
        .and_then(|a| a.spacing.as_ref())
        .and_then(|s| s.before)
        .unwrap_or(0.0);
    let explicit = block
        .attrs
        .as_ref()
        .and_then(|a| a.spacing_explicit.as_ref())
        .and_then(|e| e.before)
        .unwrap_or(false);
    if is_empty_paragraph(block) && !explicit {
        return 0.0;
    }
    value
}

/// Returns effective trailing spacing.
pub fn get_spacing_after(block: &ParagraphBlock) -> f64 {
    let value = block
        .attrs
        .as_ref()
        .and_then(|a| a.spacing.as_ref())
        .and_then(|s| s.after)
        .unwrap_or(0.0);
    let explicit = block
        .attrs
        .as_ref()
        .and_then(|a| a.spacing_explicit.as_ref())
        .and_then(|e| e.after)
        .unwrap_or(false);
    if is_empty_paragraph(block) && !explicit {
        return 0.0;
    }
    value
}

/// Stacked height of measured lines, float skips included.
pub fn lines_height(lines: &[TypesetRow]) -> f64 {
    lines.iter().fold(0.0, |sum, line| {
        sum + line.line_height + line.float_skip_before.unwrap_or(0.0)
    })
}

/// Vertical space a paragraph consumes in flow: effective spacing around its
/// lines. `ParagraphExtent::total_height` is not this height — the measurer
/// folds authored spacing into it unconditionally, while placement drops
/// style-inherited spacing on an empty paragraph.
pub fn paragraph_flow_height(block: &ParagraphBlock, measure: &ParagraphExtent) -> f64 {
    get_spacing_before(block) + lines_height(&measure.lines) + get_spacing_after(block)
}
