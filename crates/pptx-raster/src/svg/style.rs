//! The stylesheet subset the sandbox accepts: rules whose selectors are a lone
//! type, class or id compound. `simplecss` backtracks through combinators, so a
//! selector like `g g g … .x` over deep nesting is exponential before any budget
//! sees it; compounds match in constant time and name the elements they touch.

use resvg::usvg::roxmltree::{Document, Node};

use super::{MAX_SVG_STYLE_RULES, SvgRefusal};

/// Every rule the document's `<style>` elements declare, one per selector.
#[derive(Default)]
pub(super) struct StyleSheet<'a> {
    rules: Vec<Rule<'a>>,
    blocks: Vec<Block<'a>>,
}

struct Rule<'a> {
    tag: Option<&'a str>,
    classes: Vec<&'a str>,
    ids: Vec<&'a str>,
    block: usize,
}

struct Block<'a> {
    len: u64,
    references: Vec<&'a str>,
}

impl<'a> StyleSheet<'a> {
    /// Reads every `<style>` element `usvg` would apply, wherever it sits.
    pub(super) fn collect(document: &'a Document<'a>) -> Result<Self, SvgRefusal> {
        let mut sheet = StyleSheet::default();
        let mut sheets = 0;
        for node in document.descendants() {
            if !node.is_element()
                || node.tag_name().name() != "style"
                || node
                    .attribute("type")
                    .is_some_and(|kind| kind != "text/css")
            {
                continue;
            }
            sheets += 1;
            for text in node.children().filter(|child| child.is_text()) {
                sheet.parse(text.text().unwrap_or_default())?;
            }
            if sheets + sheet.rules.len() > MAX_SVG_STYLE_RULES {
                return Err(SvgRefusal::UnsupportedStyle);
            }
        }
        Ok(sheet)
    }

    pub(super) fn rules(&self) -> u64 {
        self.rules.len() as u64
    }

    /// Calls `matched` with the block length and same-document references of
    /// every rule that may apply to `node`: a superset of what `simplecss` matches.
    pub(super) fn each_match(&self, node: Node<'_, '_>, mut matched: impl FnMut(u64, &[&'a str])) {
        if self.rules.is_empty() {
            return;
        }
        let mut classes: Vec<&str> = node
            .attribute("class")
            .unwrap_or_default()
            .split_ascii_whitespace()
            .collect();
        classes.sort_unstable();
        let id = node.attribute("id");
        for rule in &self.rules {
            let applies = rule.tag.is_none_or(|tag| node.tag_name().name() == tag)
                && rule
                    .classes
                    .iter()
                    .all(|name| classes.binary_search(name).is_ok())
                && rule.ids.iter().all(|name| id == Some(*name));
            if applies {
                let block = &self.blocks[rule.block];
                matched(block.len, &block.references);
            }
        }
    }

    fn parse(&mut self, text: &'a str) -> Result<(), SvgRefusal> {
        if text.contains("/*") {
            return Err(SvgRefusal::UnsupportedStyle);
        }
        let mut rest = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
        while !rest.is_empty() {
            let open = rest.find('{').ok_or(SvgRefusal::UnsupportedStyle)?;
            let close = rest[open..].find('}').ok_or(SvgRefusal::UnsupportedStyle)? + open;
            let declarations = &rest[open + 1..close];
            if declarations.contains('{') || super::contains_ignore_case(declarations, "filter") {
                return Err(SvgRefusal::UnsupportedStyle);
            }
            let mut references = Vec::new();
            super::local_references(declarations, &mut references)?;
            let block = self.blocks.len();
            self.blocks.push(Block {
                len: declarations.len() as u64,
                references,
            });
            for selector in rest[..open].split(',') {
                let rule = compound(
                    selector.trim_matches(|c: char| c.is_ascii_whitespace()),
                    block,
                )
                .ok_or(SvgRefusal::UnsupportedStyle)?;
                if self.rules.len() >= MAX_SVG_STYLE_RULES {
                    return Err(SvgRefusal::UnsupportedStyle);
                }
                self.rules.push(rule);
            }
            rest = rest[close + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        }
        Ok(())
    }
}

/// `*`, or an optional type followed by `.class` and `#id` parts, nothing else.
fn compound(selector: &str, block: usize) -> Option<Rule<'_>> {
    let mut rule = Rule {
        tag: None,
        classes: Vec::new(),
        ids: Vec::new(),
        block,
    };
    if selector == "*" {
        return Some(rule);
    }
    let (tag, mut rest) = ident(selector);
    rule.tag = (!tag.is_empty()).then_some(tag);
    while let Some(sigil) = rest.chars().next().filter(|c| matches!(c, '.' | '#')) {
        let (name, after) = ident(&rest[1..]);
        if name.is_empty() {
            return None;
        }
        match sigil {
            '.' => rule.classes.push(name),
            _ => rule.ids.push(name),
        }
        rest = after;
    }
    if !rest.is_empty() {
        return None;
    }
    (rule.tag.is_some() || !rule.classes.is_empty() || !rule.ids.is_empty()).then_some(rule)
}

fn ident(text: &str) -> (&str, &str) {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(text.len());
    text.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(css: &str) -> Result<usize, SvgRefusal> {
        let mut sheet = StyleSheet::default();
        sheet.parse(css)?;
        Ok(sheet.rules.len())
    }

    #[test]
    fn office_and_illustrator_rules_parse() {
        let css = "\n.MsftOfcThm_Accent1_Fill_v2 {\n fill:#4472C4; \n}\n.st0,.st1{fill:url(#SVGID_1_);}\nrect#a.b{stroke:none}\n*{opacity:1}";
        assert_eq!(sheet(css), Ok(5));
    }

    #[test]
    fn combinators_comments_at_rules_and_filters_are_refused() {
        for css in [
            "g .x{fill:red}",
            "g>.x{fill:red}",
            "a+b{fill:red}",
            "[class]{fill:red}",
            "a:first-child{fill:red}",
            "@media print{a{fill:red}}",
            "/* c */a{fill:red}",
            ".a{fill:red",
            ".a{filter:blur(4px)}",
            ".a,{fill:red}",
            "*.a{fill:red}",
            "aé{fill:red}",
            ".a\u{e9}{fill:red}",
        ] {
            assert_eq!(sheet(css), Err(SvgRefusal::UnsupportedStyle), "{css}");
        }
        assert_eq!(
            sheet(".a{fill:URL(https://example.invalid/p.svg#g)}"),
            Err(SvgRefusal::ExternalReference)
        );
    }
}
