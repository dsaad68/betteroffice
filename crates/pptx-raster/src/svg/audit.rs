//! The document audit that runs before `usvg` sees a document: elements come
//! from an allowlist, every reference names a fragment of the document, and the
//! tree those references expand into is measured before anything builds it.

use std::collections::HashMap;

use resvg::usvg::roxmltree::{Document, Node};

use super::style::StyleSheet;
use super::{
    MAX_SVG_DEPTH, MAX_SVG_EXPANDED_BYTES, MAX_SVG_EXPANDED_NODES, MAX_SVG_GRADIENT_STOPS,
    MAX_SVG_STYLE_WORK, SvgRefusal, contains_ignore_case, local_references,
};

const SVG_NS: &str = "http://www.w3.org/2000/svg";

/// What a document may draw with. Office icons stay within `svg g defs style
/// path linearGradient stop`; markers, filters, masks, patterns, scripts and
/// animation are not here.
const ALLOWED: [&str; 24] = [
    "svg",
    "g",
    "defs",
    "title",
    "desc",
    "metadata",
    "style",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
    "symbol",
    "use",
    "a",
    "image",
    "text",
    "tspan",
];

#[derive(Clone, Copy)]
enum Slot {
    /// Inside `metadata` or a foreign namespace: `usvg` never builds it, so a
    /// reference into it is refused rather than audited.
    Skipped,
    Element(usize),
}

struct Element<'a> {
    node: Node<'a, 'a>,
    opaque: bool,
    children: Vec<usize>,
    links: Vec<&'a str>,
    bytes: u64,
    style: u64,
}

#[derive(Clone, Copy, Default)]
struct Expanded {
    nodes: u64,
    bytes: u64,
    style: u64,
    depth: u64,
}

/// Refuses a document outside the sandbox before `usvg` converts it.
pub(super) fn audit(document: &Document<'_>) -> Result<(), SvgRefusal> {
    let root = document.root_element();
    if !is_svg(root) {
        return Err(SvgRefusal::NotSvg);
    }
    let mut slots: Vec<Option<Slot>> = vec![None; document.descendants().count()];
    let mut ids: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut elements: Vec<Element<'_>> = Vec::new();
    for node in root.descendants().filter(|node| node.is_element()) {
        for attribute in node
            .attributes()
            .filter(|attribute| attribute.name() == "id")
        {
            ids.entry(attribute.value())
                .or_default()
                .push(node.id().get_usize());
        }
        let parent = node
            .parent_element()
            .and_then(|parent| slots[parent.id().get_usize()]);
        let skipped = match parent {
            Some(Slot::Element(parent)) => elements[parent].opaque,
            Some(Slot::Skipped) => true,
            None => false,
        };
        if skipped || !is_svg(node) {
            slots[node.id().get_usize()] = Some(Slot::Skipped);
            continue;
        }
        let name = node.tag_name().name();
        if !ALLOWED.contains(&name) {
            return Err(SvgRefusal::UnsupportedElement);
        }
        if elements.len() as u64 >= MAX_SVG_EXPANDED_NODES {
            return Err(SvgRefusal::ExpansionTooLarge);
        }
        let index = elements.len();
        let mut links = Vec::new();
        let bytes = audit_attributes(node, &mut links)?;
        elements.push(Element {
            node,
            opaque: name == "metadata",
            children: Vec::new(),
            links,
            bytes,
            style: 0,
        });
        if let Some(Slot::Element(parent)) = parent {
            elements[parent].children.push(index);
        }
        slots[node.id().get_usize()] = Some(Slot::Element(index));
    }

    for element in &elements {
        if matches!(
            element.node.tag_name().name(),
            "linearGradient" | "radialGradient"
        ) {
            let stops = element
                .children
                .iter()
                .filter(|child| elements[**child].node.tag_name().name() == "stop");
            if stops.count() > MAX_SVG_GRADIENT_STOPS {
                return Err(SvgRefusal::ExpansionTooLarge);
            }
        }
    }

    let sheet = StyleSheet::collect(document)?;
    let mut work = 0u64;
    for element in &mut elements {
        let class = element.node.attribute("class").map_or(0, str::len) as u64;
        element.style = sheet.rules().saturating_mul(1 + class / 16);
        work = work.saturating_add(element.style);
    }
    if work > MAX_SVG_STYLE_WORK {
        return Err(SvgRefusal::ExpansionTooLarge);
    }
    let room = MAX_SVG_EXPANDED_NODES as usize;
    let mut links = 0usize;
    for element in &mut elements {
        let mut overflow = false;
        sheet.each_match(element.node, |declarations, references| {
            element.style = element.style.saturating_add(1 + declarations);
            if element.links.len() + references.len() > room {
                overflow = true;
            } else {
                element.links.extend_from_slice(references);
            }
        });
        links += element.links.len();
        if overflow || links > room {
            return Err(SvgRefusal::ExpansionTooLarge);
        }
    }

    let mut edges = 0u64;
    let mut adjacency = Vec::with_capacity(elements.len());
    for element in &elements {
        let mut targets = element.children.clone();
        for link in &element.links {
            for &node in ids.get(link).into_iter().flatten() {
                match slots[node] {
                    Some(Slot::Element(target)) => targets.push(target),
                    _ => return Err(SvgRefusal::UnsupportedElement),
                }
            }
        }
        edges += targets.len() as u64;
        if edges > MAX_SVG_EXPANDED_NODES {
            return Err(SvgRefusal::ExpansionTooLarge);
        }
        adjacency.push(targets);
    }
    expand(&elements, &adjacency)
}

/// `usvg` reads an element as SVG when it has no namespace or the SVG one.
fn is_svg(node: Node<'_, '_>) -> bool {
    matches!(node.tag_name().namespace(), None | Some(SVG_NS))
}

/// Markup bytes the element contributes, collecting the ids it references. An
/// `href` on `a` or `image` is inert: nothing follows a link, and both image
/// resolvers return `None`.
fn audit_attributes<'a>(node: Node<'a, 'a>, links: &mut Vec<&'a str>) -> Result<u64, SvgRefusal> {
    let inert_href = matches!(node.tag_name().name(), "a" | "image");
    let mut bytes = node.tag_name().name().len() as u64;
    for attribute in node.attributes() {
        let (name, value) = (attribute.name(), attribute.value());
        bytes += (name.len() + value.len()) as u64 + 4;
        if name == "href" {
            if !inert_href {
                let target = value
                    .strip_prefix('#')
                    .ok_or(SvgRefusal::ExternalReference)?;
                links.push(target.split(' ').next().unwrap_or_default());
            }
            continue;
        }
        local_references(value, links)?;
        if (name == "filter" && value != "none")
            || (name == "style" && contains_ignore_case(value, "filter"))
        {
            return Err(SvgRefusal::UnsupportedStyle);
        }
    }
    for text in node.children().filter(|child| child.is_text()) {
        bytes += text.text().map_or(0, str::len) as u64;
    }
    Ok(bytes)
}

/// Sizes the tree `usvg` would build, where every reference instantiates its
/// target again: a memoised depth-first walk, iterative so a long chain cannot
/// overflow the stack, that refuses a cycle and stops at the first bound.
fn expand(elements: &[Element<'_>], adjacency: &[Vec<usize>]) -> Result<(), SvgRefusal> {
    const NEW: u8 = 0;
    const OPEN: u8 = 1;
    const DONE: u8 = 2;
    let mut state = vec![NEW; elements.len()];
    let mut sizes = vec![Expanded::default(); elements.len()];
    let mut stack = vec![(0usize, 0usize)];
    state[0] = OPEN;
    while let Some((index, next)) = stack.last_mut() {
        let index = *index;
        if let Some(&target) = adjacency[index].get(*next) {
            *next += 1;
            match state[target] {
                NEW => {
                    state[target] = OPEN;
                    stack.push((target, 0));
                }
                OPEN => return Err(SvgRefusal::ReferenceCycle),
                _ => {}
            }
            continue;
        }
        let own = &elements[index];
        let mut size = Expanded {
            nodes: 1,
            bytes: own.bytes,
            style: own.style,
            depth: 0,
        };
        for &target in &adjacency[index] {
            let inner = sizes[target];
            size.nodes = size.nodes.saturating_add(inner.nodes);
            size.bytes = size.bytes.saturating_add(inner.bytes);
            size.style = size.style.saturating_add(inner.style);
            size.depth = size.depth.max(inner.depth);
        }
        size.depth += 1;
        if size.nodes > MAX_SVG_EXPANDED_NODES
            || size.bytes > MAX_SVG_EXPANDED_BYTES
            || size.style > MAX_SVG_STYLE_WORK
        {
            return Err(SvgRefusal::ExpansionTooLarge);
        }
        if size.depth > MAX_SVG_DEPTH as u64 {
            return Err(SvgRefusal::TooDeeplyNested);
        }
        sizes[index] = size;
        state[index] = DONE;
        stack.pop();
    }
    Ok(())
}
