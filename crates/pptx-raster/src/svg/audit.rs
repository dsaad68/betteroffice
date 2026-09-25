//! The document audit that runs before `usvg` sees a document: elements come
//! from an allowlist, every reference names a fragment of the document, and the
//! tree those references expand into is measured before anything builds it.

use std::collections::HashMap;

use resvg::usvg::roxmltree::{Document, Node};

use super::style::StyleSheet;
use super::{
    MAX_SVG_DEPTH, MAX_SVG_EXPANDED_BYTES, MAX_SVG_EXPANDED_NODES, MAX_SVG_GRADIENT_STOPS,
    MAX_SVG_STYLE_WORK, SvgRefusal, reference,
};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

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

/// How `usvg` follows a reference, which decides what its target must be.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Link {
    /// A `use` instantiates any element.
    Use,
    /// A `clip-path` converts a `clipPath`, and only that.
    Clip,
    /// A `fill` or `stroke` converts a gradient, and only that.
    Paint,
    /// A gradient's `href` reads the gradient it names for stops and
    /// attributes; the chain goes on only through gradients.
    Chain,
}

struct Element<'a> {
    node: Node<'a, 'a>,
    opaque: bool,
    children: Vec<usize>,
    links: Vec<(Link, &'a str)>,
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
        let (bytes, style) = audit_attributes(node, &mut links)?;
        elements.push(Element {
            node,
            opaque: name == "metadata",
            children: Vec::new(),
            links,
            bytes,
            style,
        });
        if let Some(Slot::Element(parent)) = parent {
            elements[parent].children.push(index);
        }
        slots[node.id().get_usize()] = Some(Slot::Element(index));
    }

    for element in &elements {
        if is_gradient(element.node) {
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
    let mut work = sheet.work();
    for element in &mut elements {
        element.style = element.style.saturating_add(sheet.tests(element.node));
        work = work.saturating_add(element.style);
    }
    if work > MAX_SVG_STYLE_WORK {
        return Err(SvgRefusal::ExpansionTooLarge);
    }
    elements[0].style = elements[0].style.saturating_add(sheet.work());
    let room = MAX_SVG_EXPANDED_NODES as usize;
    let mut links = 0usize;
    for element in &mut elements {
        let mut overflow = false;
        let insert = declaration_work(element.node);
        sheet.each_match(element.node, |declarations, references| {
            element.style = element
                .style
                .saturating_add(declarations.saturating_mul(insert));
            if element.links.len() + 2 * references.len() > room {
                overflow = true;
                return;
            }
            for &target in references {
                element.links.push((Link::Clip, target));
                element.links.push((Link::Paint, target));
            }
        });
        links += element.links.len();
        if overflow || links > room {
            return Err(SvgRefusal::ExpansionTooLarge);
        }
    }

    let mut edges = 0usize;
    let mut adjacency = Vec::with_capacity(elements.len());
    for element in &elements {
        let mut targets = element.children.clone();
        for &(link, id) in &element.links {
            for &node in ids.get(id).into_iter().flatten() {
                let Some(Slot::Element(target)) = slots[node] else {
                    return Err(SvgRefusal::UnsupportedElement);
                };
                if follows(link, elements[target].node) {
                    targets.push(target);
                }
                if edges + targets.len() > room {
                    return Err(SvgRefusal::ExpansionTooLarge);
                }
            }
        }
        edges += targets.len();
        if edges > room {
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

fn is_gradient(node: Node<'_, '_>) -> bool {
    matches!(node.tag_name().name(), "linearGradient" | "radialGradient")
}

/// Whether `usvg` goes on into `target` along a `link`: it converts only a
/// `clipPath` for a clip and only a gradient for a paint or a gradient chain.
fn follows(link: Link, target: Node<'_, '_>) -> bool {
    match link {
        Link::Use => true,
        Link::Clip => target.tag_name().name() == "clipPath",
        Link::Paint | Link::Chain => is_gradient(target),
    }
}

/// Markup bytes the element contributes and the style work a `style`
/// attribute costs per instance, collecting the references `usvg` follows
/// from it, each read by the parser `usvg` reads it with. An `href` is
/// followed only on `use` and on gradients: on `a` and `image` it is inert,
/// since nothing follows a link and both image resolvers return `None`.
fn audit_attributes<'a>(
    node: Node<'a, 'a>,
    links: &mut Vec<(Link, &'a str)>,
) -> Result<(u64, u64), SvgRefusal> {
    let name = node.tag_name().name();
    let mut bytes = name.len() as u64;
    let mut style = 0u64;
    let (mut href, mut xlink_href) = (None, None);
    for attribute in node.attributes() {
        let (local, value) = (attribute.name(), attribute.value());
        bytes += (local.len() + value.len()) as u64 + 4;
        if local == "filter" && value != "none" {
            return Err(SvgRefusal::UnsupportedStyle);
        }
        if local == "style" {
            let applied = (value.len() as u64).saturating_mul(declaration_work(node));
            style = style
                .saturating_add(super::style::rescans(value.len()))
                .saturating_add(applied);
            style_references(value, links)?;
            continue;
        }
        let namespace = attribute.namespace();
        if !matches!(namespace, None | Some(SVG_NS | XLINK_NS | XML_NS)) {
            continue;
        }
        match local {
            "href" if namespace.is_none() => href = href.or(Some(value)),
            "href" if namespace == Some(XLINK_NS) => xlink_href = xlink_href.or(Some(value)),
            "clip-path" if value == "inherit" => return Err(SvgRefusal::UnsupportedStyle),
            "clip-path" => {
                if let Some(target) = reference::func_iri(value)? {
                    links.push((Link::Clip, target));
                }
            }
            "fill" | "stroke" => {
                if let Some(target) = reference::paint(value)? {
                    links.push((Link::Paint, target));
                }
            }
            "mask" | "marker-start" | "marker-mid" | "marker-end" => {
                reference::func_iri(value)?;
            }
            _ => {}
        }
    }
    let link = match name {
        "use" => Some(Link::Use),
        "linearGradient" | "radialGradient" => Some(Link::Chain),
        _ => None,
    };
    if let (Some(link), Some(value)) = (link, href.or(xlink_href))
        && let Some(target) = reference::href(value)?
    {
        links.push((link, target));
    }
    for text in node.children().filter(|child| child.is_text()) {
        bytes += text.text().map_or(0, str::len) as u64;
    }
    Ok((bytes, style))
}

/// Style work per byte of declarations applied to one instance of `node`:
/// each declaration looks its property up among the element's attributes
/// and copies its value.
fn declaration_work(node: Node<'_, '_>) -> u64 {
    32 + node.attributes().len() as u64
}

/// The references a `style` attribute can make. Its declarations are not
/// split out, so every target counts as both a clip and a paint.
fn style_references<'a>(
    value: &'a str,
    links: &mut Vec<(Link, &'a str)>,
) -> Result<(), SvgRefusal> {
    super::style::screen(value)?;
    let mut targets = Vec::new();
    reference::css(value, &mut targets)?;
    for target in targets {
        links.push((Link::Clip, target));
        links.push((Link::Paint, target));
    }
    Ok(())
}

/// Sizes the tree `usvg` would build, where every reference instantiates its
/// target again: a memoised depth-first walk, iterative so a long chain cannot
/// overflow the stack, that refuses a cycle of any length and stops at the
/// first bound.
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
