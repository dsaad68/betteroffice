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

pub fn lines_height(lines: &[TypesetRow]) -> f64 {
    lines.iter().fold(0.0, |sum, line| {
        sum + line.line_height + line.float_skip_before.unwrap_or(0.0)
    })
}

/// Mirrors paginator spacing collapse; paragraph `total_height` is ignored
/// because it already contains authored spacing.
#[derive(Debug, Clone, Default)]
pub struct FlowStack {
    height: f64,
    deferred: f64,
}

impl FlowStack {
    /// Starts at a cursor already carrying `deferred` trailing spacing.
    pub fn resuming(deferred: f64) -> Self {
        FlowStack {
            height: 0.0,
            deferred,
        }
    }

    pub fn open(&mut self, before: f64) {
        self.height += before.max(self.deferred);
        self.deferred = 0.0;
    }

    pub fn advance(&mut self, body: f64) {
        self.height += body;
    }

    pub fn close(&mut self, after: f64) {
        self.deferred = after;
    }

    pub fn push(&mut self, before: f64, body: f64, after: f64) {
        self.open(before);
        self.advance(body);
        self.close(after);
    }

    pub fn push_paragraph(&mut self, block: &ParagraphBlock, measure: &ParagraphExtent) {
        self.push(
            get_spacing_before(block),
            lines_height(&measure.lines),
            get_spacing_after(block),
        );
    }

    /// Height down to the last block's bottom. Trailing spacing is deferred,
    /// and a page or column break drops it, so it is not charged here.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Height including the spacing left deferred below the last block.
    pub fn height_with_trailing(&self) -> f64 {
        self.height + self.deferred
    }
}
