//! Sandboxed SVG rasterisation into the straight-alpha RGBA buffer the other
//! image formats produce.

use resvg::usvg;
use tiny_skia::{IntSize, Pixmap, PremultipliedColorU8, Transform};

/// One SVG document's source bytes.
pub const MAX_SVG_BYTES: usize = 4_194_304;
/// Elements one SVG document may nest. Both the XML parser and `usvg`'s
/// converter recurse over nesting, so past this a document is a stack overflow
/// rather than an error.
pub const MAX_SVG_DEPTH: usize = 64;
/// Nodes the XML parser will materialise for one document.
pub const MAX_SVG_NODES: u32 = 1_048_576;
/// One rasterised SVG's longest side.
pub const MAX_SVG_RASTER_DIM: u32 = 8_192;
/// How far above its intrinsic size an SVG rasterises, so a picture frame
/// larger than the document still has pixels to stretch.
const SVG_SUPERSAMPLE: u32 = 4;

/// Why the sandbox declined a document. Structural only: no markup, no
/// attribute value and no reference target is carried out of the decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgRefusal {
    /// Not UTF-8, or the root element is not `svg`.
    NotSvg,
    /// Past [`MAX_SVG_BYTES`].
    DocumentTooLarge,
    /// Carries a `<!DOCTYPE>`, so it may declare entities.
    DoctypeDeclared,
    /// Past [`MAX_SVG_DEPTH`].
    TooDeeplyNested,
    /// References something outside itself: a network or filesystem href, or a
    /// `url()` that is not a same-document fragment.
    ExternalReference,
    /// Malformed, past [`MAX_SVG_NODES`], or without a usable intrinsic size.
    Unparsable,
    /// Rasterises past [`MAX_SVG_RASTER_DIM`].
    RasterTooLarge,
}

/// A parsed SVG and the raster it will render into.
pub struct SvgImage {
    tree: usvg::Tree,
    size: IntSize,
}

/// Whether `bytes` are worth handing to [`parse`]. `image`'s sniffer never
/// claims SVG, so this is the only thing that routes a picture here.
pub fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let head = head.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(head);
    let start = head.iter().position(|byte| !byte.is_ascii_whitespace());
    start.is_some_and(|start| head[start] == b'<')
        && head.windows(4).any(|window| window == b"<svg")
}

/// Parses under the sandbox: no DTD, no external reference, bounded document,
/// nesting and output raster.
pub fn parse(bytes: &[u8]) -> Result<SvgImage, SvgRefusal> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(SvgRefusal::DocumentTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SvgRefusal::NotSvg)?;
    audit_nesting(bytes)?;
    let document = usvg::roxmltree::Document::parse_with_options(
        text,
        usvg::roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_SVG_NODES,
            ..Default::default()
        },
    )
    .map_err(|error| match error {
        usvg::roxmltree::Error::DtdDetected => SvgRefusal::DoctypeDeclared,
        _ => SvgRefusal::Unparsable,
    })?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err(SvgRefusal::NotSvg);
    }
    audit_references(root)?;
    let tree =
        usvg::Tree::from_xmltree(&document, &sandbox()).map_err(|_| SvgRefusal::Unparsable)?;
    let size = raster_size(tree.size())?;
    Ok(SvgImage { tree, size })
}

impl SvgImage {
    /// Pixels the render will allocate, for the caller's decode budget.
    pub fn pixels(&self) -> u64 {
        u64::from(self.size.width()) * u64::from(self.size.height())
    }

    /// Straight-alpha RGBA, matching what the raster formats hand back.
    pub fn render(&self) -> Option<(Vec<u8>, IntSize)> {
        let mut pixmap = Pixmap::new(self.size.width(), self.size.height())?;
        let scale = Transform::from_scale(
            self.size.width() as f32 / self.tree.size().width(),
            self.size.height() as f32 / self.tree.size().height(),
        );
        resvg::render(&self.tree, scale, &mut pixmap.as_mut());
        let mut data = pixmap.take();
        let (pixels, _) = data.as_chunks_mut::<4>();
        for pixel in pixels {
            let straight = PremultipliedColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3])
                .map(|color| color.demultiply());
            if let Some(color) = straight {
                *pixel = [color.red(), color.green(), color.blue(), color.alpha()];
            }
        }
        Some((data, self.size))
    }
}

/// Neither resolver may reach a file or a network, whatever the audit missed.
fn sandbox() -> usvg::Options<'static> {
    usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

/// Element nesting, bounded before any parser sees the document. Runs on the
/// bytes because reaching it through a parsed tree is already too late.
fn audit_nesting(bytes: &[u8]) -> Result<(), SvgRefusal> {
    let mut index = 0;
    let mut depth = 0usize;
    while let Some(open) = bytes[index..].iter().position(|byte| *byte == b'<') {
        index += open + 1;
        let rest = &bytes[index..];
        if rest.starts_with(b"!--") {
            index = after(bytes, index, b"-->")?;
        } else if rest.starts_with(b"![CDATA[") {
            index = after(bytes, index, b"]]>")?;
        } else if rest.starts_with(b"!") {
            return Err(SvgRefusal::DoctypeDeclared);
        } else if rest.starts_with(b"?") {
            index = after(bytes, index, b"?>")?;
        } else {
            let closing = rest.starts_with(b"/");
            let (end, empty) = tag_end(bytes, index)?;
            index = end;
            if closing {
                depth = depth.saturating_sub(1);
            } else if !empty {
                depth += 1;
                if depth > MAX_SVG_DEPTH {
                    return Err(SvgRefusal::TooDeeplyNested);
                }
            }
        }
    }
    Ok(())
}

/// Index just past `needle`.
fn after(bytes: &[u8], from: usize, needle: &[u8]) -> Result<usize, SvgRefusal> {
    bytes[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| from + at + needle.len())
        .ok_or(SvgRefusal::Unparsable)
}

/// Index just past a tag's `>`, and whether the tag closed itself. A quoted
/// attribute value may hold a `>` of its own.
fn tag_end(bytes: &[u8], from: usize) -> Result<(usize, bool), SvgRefusal> {
    let mut quote: Option<u8> = None;
    let mut previous = 0u8;
    for (offset, byte) in bytes[from..].iter().enumerate() {
        match (quote, *byte) {
            (Some(open), seen) if seen == open => quote = None,
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'>') => return Ok((from + offset + 1, previous == b'/')),
            _ => {}
        }
        previous = *byte;
    }
    Err(SvgRefusal::Unparsable)
}

/// Walks the element tree iteratively, so auditing a hostile document cannot
/// itself overflow the stack.
fn audit_references(root: usvg::roxmltree::Node<'_, '_>) -> Result<(), SvgRefusal> {
    let mut node = root;
    loop {
        for attribute in node.attributes() {
            if !reference_is_local(attribute.name(), attribute.value()) {
                return Err(SvgRefusal::ExternalReference);
            }
        }
        if let Some(child) = node.first_element_child() {
            node = child;
            continue;
        }
        loop {
            if node.id() == root.id() {
                return Ok(());
            }
            if let Some(sibling) = node.next_sibling_element() {
                node = sibling;
                break;
            }
            let Some(parent) = node.parent_element() else {
                return Ok(());
            };
            node = parent;
        }
    }
}

/// An `href` may only name a fragment of this document, and a `url()` anywhere
/// in a value may only do the same.
fn reference_is_local(name: &str, value: &str) -> bool {
    if name == "href" && !value.starts_with('#') {
        return false;
    }
    let mut rest = value;
    while let Some(open) = rest.find("url(") {
        rest = &rest[open + 4..];
        let target = rest.trim_start().trim_start_matches(['"', '\'']);
        if !target.starts_with('#') {
            return false;
        }
    }
    true
}

/// The raster an intrinsic size renders into, supersampled where the bound
/// leaves room.
fn raster_size(size: usvg::Size) -> Result<IntSize, SvgRefusal> {
    let width = size.width();
    let height = size.height();
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(SvgRefusal::Unparsable);
    }
    let limit = MAX_SVG_RASTER_DIM as f32;
    let (width, height) = (width.ceil().max(1.0), height.ceil().max(1.0));
    if width > limit || height > limit {
        return Err(SvgRefusal::RasterTooLarge);
    }
    let factor = (1..=SVG_SUPERSAMPLE)
        .rev()
        .find(|factor| width * *factor as f32 <= limit && height * *factor as f32 <= limit)
        .unwrap_or(1);
    IntSize::from_wh(width as u32 * factor, height as u32 * factor).ok_or(SvgRefusal::Unparsable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(bytes: &[u8]) -> Option<SvgRefusal> {
        parse(bytes).err()
    }

    fn document(body: &str) -> String {
        format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">{body}</svg>"##)
    }

    #[test]
    fn a_bounded_document_rasterizes_above_its_intrinsic_size() {
        let source = document(r##"<rect width="96" height="96" fill="#00ff00"/>"##);
        let image = parse(source.as_bytes()).expect("parse");
        let (data, size) = image.render().expect("render");
        assert_eq!((size.width(), size.height()), (384, 384));
        assert_eq!(image.pixels(), 384 * 384);
        assert_eq!(data.len(), 384 * 384 * 4);
        assert_eq!(&data[..4], &[0, 255, 0, 255]);
    }

    #[test]
    fn an_intrinsic_size_at_the_bound_rasterizes_without_supersampling() {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{dim}" height="{dim}"><rect width="{dim}" height="{dim}" fill="#000"/></svg>"##,
            dim = MAX_SVG_RASTER_DIM
        );
        let size = parse(source.as_bytes()).expect("parse").size;
        assert_eq!((size.width(), size.height()), (8192, 8192));
    }

    #[test]
    fn a_document_past_the_byte_bound_is_refused() {
        let padding = " ".repeat(MAX_SVG_BYTES);
        let source = document(&format!("<desc>{padding}</desc>"));
        assert_eq!(
            refusal(source.as_bytes()),
            Some(SvgRefusal::DocumentTooLarge)
        );
    }

    #[test]
    fn nesting_past_the_depth_bound_is_refused() {
        let nest =
            |depth: usize| document(&format!("{}{}", "<g>".repeat(depth), "</g>".repeat(depth)));
        assert_eq!(
            refusal(nest(MAX_SVG_DEPTH + 1).as_bytes()),
            Some(SvgRefusal::TooDeeplyNested)
        );
        assert!(parse(nest(MAX_SVG_DEPTH - 1).as_bytes()).is_ok());
    }

    #[test]
    fn siblings_comments_and_quoted_angle_brackets_do_not_count_as_nesting() {
        let body = concat!(
            r##"<!-- <g><g><g> --><![CDATA[<g><g>]]>"##,
            r##"<desc title="a &gt; b"/><path d="M 0 0"/><path d="M 1 1"/>"##
        );
        let source = document(&body.repeat(64));
        assert!(parse(source.as_bytes()).is_ok());
    }

    #[test]
    fn a_prologue_that_mentions_a_tag_is_not_the_root_element() {
        let prologue = concat!(
            r##"<!-- exported by <svg width="1"><g><g><g> -->"##,
            r##"<?xml-stylesheet href="a.css" type="text/css"?>"##
        );
        let body = r##"<rect width="96" height="96" fill="#00ff00"/>"##;
        assert!(parse(format!("{prologue}{}", document(body)).as_bytes()).is_ok());
        assert_eq!(
            refusal(format!("{prologue}<html><svg/></html>").as_bytes()),
            Some(SvgRefusal::NotSvg)
        );
    }

    #[test]
    fn a_declared_entity_is_refused_before_it_can_expand() {
        let source = concat!(
            r##"<!DOCTYPE svg [<!ENTITY secret SYSTEM "file:///etc/passwd">]>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96"><desc>&secret;</desc></svg>"##
        );
        assert_eq!(
            refusal(source.as_bytes()),
            Some(SvgRefusal::DoctypeDeclared)
        );
    }

    #[test]
    fn a_reference_outside_the_document_is_refused() {
        for body in [
            r##"<image href="https://example.invalid/pixel.png" width="96" height="96"/>"##,
            r##"<image xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="../../secret.png" width="96" height="96"/>"##,
            r##"<rect width="96" height="96" fill="url(https://example.invalid/paint.svg#g)"/>"##,
            r##"<rect width="96" height="96" style="fill:url('/etc/paint.svg#g')"/>"##,
        ] {
            assert_eq!(
                refusal(document(body).as_bytes()),
                Some(SvgRefusal::ExternalReference),
                "{body}"
            );
        }
    }

    #[test]
    fn a_same_document_reference_is_kept() {
        let body = concat!(
            r##"<defs><linearGradient id="g"><stop offset="0" stop-color="#f00"/></linearGradient></defs>"##,
            r##"<rect width="96" height="96" fill="url(#g)"/><use href="#g"/>"##
        );
        assert!(parse(document(body).as_bytes()).is_ok());
    }

    #[test]
    fn an_output_raster_past_the_dimension_bound_is_refused() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"><rect width="100000" height="100000" fill="#000"/></svg>"##;
        assert_eq!(refusal(source.as_bytes()), Some(SvgRefusal::RasterTooLarge));
    }

    #[test]
    fn malformed_and_foreign_documents_are_refused_without_panicking() {
        assert_eq!(refusal(b"<svg><g></svg>"), Some(SvgRefusal::Unparsable));
        assert_eq!(refusal(b"<html><svg/></html>"), Some(SvgRefusal::NotSvg));
        assert_eq!(refusal(&[0xff, 0xfe, 0x3c, 0x73]), Some(SvgRefusal::NotSvg));
    }

    #[test]
    fn only_a_document_that_opens_as_markup_reaches_the_parser() {
        assert!(looks_like_svg(
            b"  <?xml version=\"1.0\"?><svg xmlns=\"x\"/>"
        ));
        assert!(looks_like_svg(b"\xef\xbb\xbf<svg/>"));
        assert!(!looks_like_svg(b"\x89PNG\r\n\x1a\n"));
        assert!(!looks_like_svg(b"not markup, mentions <svg> in prose"));
        assert!(!looks_like_svg(b""));
    }
}
