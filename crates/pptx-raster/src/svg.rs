//! Sandboxed SVG rasterisation into the straight-alpha RGBA buffer the other
//! image formats produce. The sandbox bounds what conversion and rendering cost
//! by construction: the bytes are scanned before `roxmltree` parses them, the
//! document is audited before `usvg` builds anything, and the tree `usvg` builds
//! is priced before `resvg` paints it. Every bound below is a share of one
//! envelope, and a document past any of them is refused.

mod audit;
mod cost;
mod markup;
mod reference;
mod style;

use std::panic::{AssertUnwindSafe, catch_unwind};

use resvg::usvg;
use tiny_skia::{IntSize, Pixmap, PremultipliedColorU8, Transform};

use crate::MAX_IMAGE_PIXELS;

/// Peak memory one SVG decode may use beyond its output raster and group
/// layers, which the caller charges to the slide's image budget.
pub const SVG_MEMORY_ENVELOPE: u64 = 100 << 20;
/// Single-core time one SVG decode may take, in nanoseconds of a release
/// build; the work units below are each about a nanosecond.
pub const SVG_TIME_ENVELOPE: u64 = 1_000_000_000;

/// Bytes `roxmltree` keeps per node or attribute, its final shrink included.
const XML_ITEM_BYTES: u64 = 80;
/// Bytes of `usvg`'s element and tree node per expanded element; a `use` of a
/// `symbol` with its viewport clip, the costliest, measures about 970.
const NODE_BYTES: u64 = 1_024;
/// Bytes per expanded markup byte: `usvg` stores each attribute instance in
/// 32 bytes and each byte of path data as up to 4.5 bytes of points.
const BYTE_BYTES: u64 = 8;
/// Bytes of the declaration lists `simplecss` copies per selector and byte of
/// a block: 40 per declaration of at least four bytes.
const STYLE_COPY_BYTES: u64 = 10;
/// Bytes of outline per byte of path data, which `usvg` strokes whole to
/// measure a shape: about 190 per segment with round joins, and at most one
/// segment per byte.
const STROKE_BYTES: u64 = 256;
/// Time the byte scan, `roxmltree` and the audit take per source byte.
const SOURCE_NS: u64 = 16;
/// Time `usvg` takes to build one expanded element.
const NODE_NS: u64 = 1_024;
/// Time `usvg` takes per expanded markup byte: path data parses and strokes
/// its outline for bounds at about 30 ns a byte.
const BYTE_NS: u64 = 32;
/// Time `resvg` takes per unit of [`MAX_SVG_RENDER_WORK`]: an anti-aliased,
/// alpha-blended pixel.
const RENDER_NS: u64 = 4;

/// One document's bytes, twice what [`MAX_SVG_EXPANDED_BYTES`] lets it draw
/// so an editor's private markup, which the audit skips, still fits: the byte
/// scan, `roxmltree` and the audit take about 67 ms over them, and
/// `roxmltree` copies at most as many bytes again.
pub const MAX_SVG_BYTES: usize = 1 << 22;
/// Elements one document may nest, in its markup or once its references are
/// expanded. The parsers, `usvg`'s converter and `resvg` recurse over it at
/// about 3 KiB of stack a level: 64 levels fit a 512 KiB thread stack twice.
pub const MAX_SVG_DEPTH: usize = 64;
/// Nodes `roxmltree` will build: 10 MiB of the memory envelope.
pub const MAX_SVG_NODES: u32 = 1 << 17;
/// Attributes across the markup: 10 MiB of the memory envelope.
pub const MAX_SVG_ATTRIBUTES: usize = 1 << 17;
/// Attributes on one element. `roxmltree` compares each with every earlier
/// one, so the markup costs at most 64 comparisons an attribute.
pub const MAX_SVG_ELEMENT_ATTRIBUTES: usize = 64;
/// Namespace declarations in scope at one element, and distinct ones in the
/// document: `roxmltree` looks each element and prefixed attribute up across
/// those in scope, and inserts each distinct one into a sorted list.
pub const MAX_SVG_NAMESPACES: usize = 64;
/// Elements a document expands into once every `use` and clip path
/// instantiates its target: 32 MiB of the memory envelope and 34 ms.
pub const MAX_SVG_EXPANDED_NODES: u64 = 1 << 15;
/// Markup bytes expansion hands `usvg`, which re-reads a target's attributes
/// and path data once per instance, and walks its markup once per `use`:
/// 16 MiB of the memory envelope and 67 ms.
pub const MAX_SVG_EXPANDED_BYTES: u64 = 1 << 21;
/// Path data one shape may carry, which `usvg` strokes whole to measure it:
/// 16 MiB of outline, the envelope's share for any one path's transients.
pub const MAX_SVG_PATH_BYTES: usize = 1 << 16;
/// That share: what the edges, outline and dashes of any one path may take
/// while `usvg` measures it or `resvg` paints it.
const SVG_TRANSIENT_BYTES: u64 = MAX_SVG_PATH_BYTES as u64 * STROKE_BYTES;
/// Stops one gradient may carry. `usvg` drops equal offsets by shifting the
/// list, quadratic in its length, and `tiny-skia` tests every stop per pixel.
pub const MAX_SVG_GRADIENT_STOPS: usize = 256;
/// Bytes of the gradient copies `usvg` makes per shape: one for every shape
/// that paints with a gradient in its own box's units or with a `use`'s
/// context paint, each ~256 bytes plus 12 a stop. 8 MiB of the envelope.
pub const MAX_SVG_PAINT_BYTES: u64 = 1 << 23;
/// What `usvg` spends collecting distinct gradients and clip paths, which it
/// compares each reference against every one collected so far at about a
/// quarter nanosecond each: 67 ms of the time envelope.
pub const MAX_SVG_COLLECT_WORK: u64 = 1 << 26;
/// `<style>` elements plus the rules they declare. `simplecss` re-sorts every
/// rule after each sheet, so the sorts stay under ten million comparisons.
pub const MAX_SVG_STYLE_RULES: usize = 1_024;
/// Simple selectors (a type, `.class` or `#id`) one rule may compound, each
/// an attribute lookup when `simplecss` tests the rule.
pub const MAX_SVG_SELECTOR_PARTS: usize = 16;
/// Bytes of one selector, which bounds the names each lookup compares.
pub const MAX_SVG_SELECTOR_BYTES: usize = 256;
/// Selectors times the bytes of the block they share: `simplecss` copies a
/// block's declarations once per selector of its list. 2.5 MiB of the
/// envelope.
pub const MAX_SVG_STYLE_COPIES: u64 = 1 << 18;
/// What `simplecss` and `usvg` spend on CSS, in units of about a nanosecond:
/// rescans of every stylesheet and `style` attribute, selector tests, and
/// declarations applied, across the expanded document. 201 ms of the time
/// envelope.
pub const MAX_SVG_STYLE_WORK: u64 = 3 << 26;
/// Group layers (opacity, clip, blend, isolation) one render may stack. Each
/// is a raster charged to the slide's image budget with the output.
pub const MAX_SVG_LAYER_DEPTH: usize = 8;
/// Painted pixels, gradient stops weighted in, as a multiple of the output
/// raster: a cheap first refusal before [`MAX_SVG_RENDER_WORK`] is summed.
pub const MAX_SVG_OVERDRAW: u64 = 64;
/// Painting work in painted-pixel units, each about 4 ns: 537 ms, the rest of
/// the time envelope. Past it the raster supersamples less, and a render still
/// past it at its intrinsic size is refused.
pub const MAX_SVG_RENDER_WORK: u64 = 4 * MAX_IMAGE_PIXELS;
/// One rasterised SVG's longest side. The raster is outside the envelope: the
/// caller charges it, with its layers, to the slide's image budget.
pub const MAX_SVG_RASTER_DIM: u32 = 8_192;
/// How far above its intrinsic size an SVG rasterises, so a picture frame
/// larger than the document still has pixels to stretch.
const SVG_SUPERSAMPLE: u32 = 4;

const _: () = assert!(
    MAX_SVG_BYTES as u64
        + (MAX_SVG_NODES as u64 + MAX_SVG_ATTRIBUTES as u64) * XML_ITEM_BYTES
        + MAX_SVG_EXPANDED_NODES * NODE_BYTES
        + MAX_SVG_EXPANDED_BYTES * BYTE_BYTES
        + MAX_SVG_PATH_BYTES as u64 * STROKE_BYTES
        + MAX_SVG_PAINT_BYTES
        + MAX_SVG_STYLE_COPIES * STYLE_COPY_BYTES
        <= SVG_MEMORY_ENVELOPE
);
const _: () = assert!(
    MAX_SVG_BYTES as u64 * SOURCE_NS
        + MAX_SVG_EXPANDED_NODES * NODE_NS
        + MAX_SVG_EXPANDED_BYTES * BYTE_NS
        + MAX_SVG_STYLE_WORK
        + MAX_SVG_COLLECT_WORK
        + MAX_SVG_RENDER_WORK * RENDER_NS
        <= SVG_TIME_ENVELOPE
);

/// Why the sandbox declined a document. Structural only: no markup, no
/// attribute value and no reference target is carried out of the decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgRefusal {
    /// Not UTF-8, or the root element is not `svg`.
    NotSvg,
    /// Past [`MAX_SVG_BYTES`], or markup past [`MAX_SVG_ELEMENT_ATTRIBUTES`],
    /// [`MAX_SVG_ATTRIBUTES`] or [`MAX_SVG_NAMESPACES`].
    DocumentTooLarge,
    /// Carries a `<!DOCTYPE>`, so it may declare entities.
    DoctypeDeclared,
    /// Past [`MAX_SVG_DEPTH`], in the markup or once references are expanded.
    TooDeeplyNested,
    /// References something outside itself: a network or filesystem href, or a
    /// `url()` that is not a same-document fragment.
    ExternalReference,
    /// Malformed, past [`MAX_SVG_NODES`], or without a usable intrinsic size.
    Unparsable,
    /// Rasterises past [`MAX_SVG_RASTER_DIM`].
    RasterTooLarge,
    /// An element outside the drawing allowlist, a reference into content the
    /// audit skips, or a mask, filter, pattern, image or text node in the tree.
    UnsupportedElement,
    /// A stylesheet past [`MAX_SVG_STYLE_RULES`] or beyond plain type, class and
    /// id rules within [`MAX_SVG_SELECTOR_PARTS`] and [`MAX_SVG_SELECTOR_BYTES`],
    /// a `filter` in any form, or a clip path inherited from whatever element a
    /// copy lands under.
    UnsupportedStyle,
    /// A reference that leads back to itself.
    ReferenceCycle,
    /// Past [`MAX_SVG_EXPANDED_NODES`], [`MAX_SVG_EXPANDED_BYTES`],
    /// [`MAX_SVG_STYLE_WORK`], [`MAX_SVG_STYLE_COPIES`], [`MAX_SVG_PAINT_BYTES`]
    /// or [`MAX_SVG_COLLECT_WORK`] once references are expanded, or a gradient
    /// past [`MAX_SVG_GRADIENT_STOPS`] or a shape past [`MAX_SVG_PATH_BYTES`].
    ExpansionTooLarge,
    /// Group layers past [`MAX_SVG_LAYER_DEPTH`], or a clip path that is
    /// itself clipped.
    TooManyLayers,
    /// Paints past [`MAX_SVG_OVERDRAW`] or [`MAX_SVG_RENDER_WORK`].
    RenderTooCostly,
    /// `usvg` or `resvg` panicked.
    Panicked,
}

/// A parsed SVG and the raster it will render into.
pub struct SvgImage {
    tree: usvg::Tree,
    size: IntSize,
    pixels: u64,
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

/// Parses under the sandbox: no DTD, no external reference, an allowlisted and
/// bounded document, and a render priced before it runs.
pub fn parse(bytes: &[u8]) -> Result<SvgImage, SvgRefusal> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(SvgRefusal::DocumentTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SvgRefusal::NotSvg)?;
    markup::scan(bytes)?;
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
    if document.root_element().tag_name().name() != "svg" {
        return Err(SvgRefusal::NotSvg);
    }
    audit::audit(&document)?;
    let tree = guarded(|| usvg::Tree::from_xmltree(&document, &sandbox()))?
        .map_err(|_| SvgRefusal::Unparsable)?;
    let (size, pixels) = raster(&tree)?;
    Ok(SvgImage { tree, size, pixels })
}

impl SvgImage {
    /// Pixels the render will allocate, for the caller's decode budget: the
    /// output raster plus the deepest stack of group layers and clip masks.
    pub fn pixels(&self) -> u64 {
        self.pixels
    }

    /// The document's own size in CSS px, which a tiled fill repeats at.
    pub fn intrinsic(&self) -> (f32, f32) {
        (self.tree.size().width(), self.tree.size().height())
    }

    /// Straight-alpha RGBA, matching what the raster formats hand back.
    pub fn render(&self) -> Result<(Vec<u8>, IntSize), SvgRefusal> {
        let mut pixmap =
            Pixmap::new(self.size.width(), self.size.height()).ok_or(SvgRefusal::RasterTooLarge)?;
        let scale = Transform::from_scale(
            self.size.width() as f32 / self.tree.size().width(),
            self.size.height() as f32 / self.tree.size().height(),
        );
        guarded(|| resvg::render(&self.tree, scale, &mut pixmap.as_mut()))?;
        let mut data = pixmap.take();
        let (pixels, _) = data.as_chunks_mut::<4>();
        for pixel in pixels {
            let straight = PremultipliedColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3])
                .map(|color| color.demultiply());
            if let Some(color) = straight {
                *pixel = [color.red(), color.green(), color.blue(), color.alpha()];
            }
        }
        Ok((data, self.size))
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

/// Runs a `usvg` or `resvg` step with a panic mapped to a refusal. The panic
/// hook is left alone, so the panic is still reported the usual way.
fn guarded<T>(step: impl FnOnce() -> T) -> Result<T, SvgRefusal> {
    catch_unwind(AssertUnwindSafe(step)).map_err(|_| SvgRefusal::Panicked)
}

/// The raster an intrinsic size renders into and the pixels it allocates: the
/// largest supersample whose output and layers fit [`MAX_IMAGE_PIXELS`] and
/// whose painting fits [`MAX_SVG_RENDER_WORK`]. At the intrinsic size only the
/// painting refuses; a raster too large for the budget is the caller's to skip.
fn raster(tree: &usvg::Tree) -> Result<(IntSize, u64), SvgRefusal> {
    let width = tree.size().width();
    let height = tree.size().height();
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(SvgRefusal::Unparsable);
    }
    let limit = MAX_SVG_RASTER_DIM as f32;
    let (width, height) = (width.ceil().max(1.0), height.ceil().max(1.0));
    if width > limit || height > limit {
        return Err(SvgRefusal::RasterTooLarge);
    }
    let mut factor = SVG_SUPERSAMPLE;
    loop {
        let (scaled_width, scaled_height) = (width * factor as f32, height * factor as f32);
        if factor > 1 && (scaled_width > limit || scaled_height > limit) {
            factor -= 1;
            continue;
        }
        let size = IntSize::from_wh(scaled_width as u32, scaled_height as u32)
            .ok_or(SvgRefusal::Unparsable)?;
        let cost = cost::measure(tree, size)?;
        let affordable = cost.work <= MAX_SVG_RENDER_WORK;
        if affordable && cost.pixels <= MAX_IMAGE_PIXELS {
            return Ok((size, cost.pixels));
        }
        if factor == 1 {
            if !affordable {
                return Err(SvgRefusal::RenderTooCostly);
            }
            return Ok((size, cost.pixels));
        }
        factor -= 1;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn refusal(bytes: &[u8]) -> Option<SvgRefusal> {
        parse(bytes).err()
    }

    pub(crate) fn document(body: &str) -> String {
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
            r##"<use href="https://example.invalid/sprite.svg#icon"/>"##,
            r##"<linearGradient xmlns:xlink="http://www.w3.org/1999/xlink" id="g" xlink:href="../../secret.svg#g"/>"##,
            r##"<rect width="96" height="96" fill="url(https://example.invalid/paint.svg#g)"/>"##,
            r##"<rect width="96" height="96" style="fill:url('/etc/paint.svg#g')"/>"##,
            r##"<rect width="96" height="96" fill="URL(http://example.invalid/paint.svg#g)"/>"##,
            r##"<rect width="96" height="96" style="clip-path:Url( data:image/svg+xml,x)"/>"##,
            r##"<style>rect{fill:uRl(https://example.invalid/paint.svg#g)}</style><rect width="96" height="96"/>"##,
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
        assert_eq!(
            refusal(br#"<svg xmlns="urn:not-svg" viewBox="0 0 4 4"/>"#),
            Some(SvgRefusal::NotSvg)
        );
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

    pub(crate) fn marker_chain(vertices: usize, levels: usize) -> String {
        let mut d = String::from("M0 0");
        for index in 1..vertices {
            d.push_str(&format!(" L{} {}", index % 10, index / 10));
        }
        let mut defs = String::from(
            r##"<marker id="m0" markerWidth="1" markerHeight="1" overflow="visible"><rect width="1" height="1" fill="#f00"/></marker>"##,
        );
        for level in 1..=levels {
            defs.push_str(&format!(
                r##"<marker id="m{level}" markerWidth="1" markerHeight="1" overflow="visible" markerUnits="userSpaceOnUse"><path d="{d}" fill="none" stroke="#000" marker-mid="url(#m{})"/></marker>"##,
                level - 1
            ));
        }
        document(&format!(
            r##"<defs>{defs}</defs><path d="{d}" fill="none" stroke="#000" marker-mid="url(#m{levels})"/>"##
        ))
    }

    #[test]
    fn a_marker_chain_is_refused_before_usvg_multiplies_it() {
        assert_eq!(
            refusal(marker_chain(6, 12).as_bytes()),
            Some(SvgRefusal::UnsupportedElement)
        );
    }

    #[test]
    fn a_marker_sized_to_overflow_is_refused_instead_of_panicking() {
        let source = concat!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">"##,
            r##"<marker id="m" markerWidth="1e30" markerHeight="1e30" viewBox="0 0 1 1"><rect width="1" height="1"/></marker>"##,
            r##"<path d="M0 0L5 5" stroke="#000" stroke-width="1e30" marker-end="url(#m)"/></svg>"##
        );
        assert_eq!(
            refusal(source.as_bytes()),
            Some(SvgRefusal::UnsupportedElement)
        );
    }

    #[test]
    fn elements_outside_the_allowlist_are_refused() {
        for body in [
            r##"<filter id="f"><feGaussianBlur stdDeviation="9"/></filter>"##,
            r##"<mask id="m"><rect width="9" height="9"/></mask>"##,
            r##"<pattern id="p" width="1" height="1"><rect width="1" height="1"/></pattern>"##,
            r##"<foreignObject width="9" height="9"/>"##,
            r##"<script>alert(1)</script>"##,
            r##"<switch><rect width="9" height="9"/></switch>"##,
            r##"<text><textPath href="#p">x</textPath></text>"##,
            r##"<rect width="9" height="9"><animate attributeName="x" to="9"/></rect>"##,
            r##"<desc><marker id="m"/></desc>"##,
        ] {
            assert_eq!(
                refusal(document(body).as_bytes()),
                Some(SvgRefusal::UnsupportedElement),
                "{body}"
            );
        }
    }

    #[test]
    fn metadata_and_foreign_markup_are_skipped_but_never_instantiated() {
        let skipped = concat!(
            r##"<metadata><rdf:RDF xmlns:rdf="urn:rdf"><filter id="f"/></rdf:RDF><marker id="m"/></metadata>"##,
            r##"<sodipodi:namedview xmlns:sodipodi="urn:sodipodi" id="base"><marker/></sodipodi:namedview>"##,
            r##"<rect width="96" height="96" fill="#00f"/>"##
        );
        assert!(parse(document(skipped).as_bytes()).is_ok());
        for body in [
            r##"<metadata><g id="x"><rect width="9" height="9"/></g></metadata><use href="#x"/>"##,
            r##"<x:y xmlns:x="urn:x"><svg:g xmlns:svg="http://www.w3.org/2000/svg" id="x"/></x:y><use href="#x"/>"##,
        ] {
            assert_eq!(
                refusal(document(body).as_bytes()),
                Some(SvgRefusal::UnsupportedElement),
                "{body}"
            );
        }
    }

    pub(crate) fn use_fan_out(levels: usize, uses: usize) -> String {
        let mut defs = String::from(r##"<rect id="l0" width="1" height="1" fill="#f00"/>"##);
        for level in 1..=levels {
            let copies = format!(r##"<use href="#l{}"/>"##, level - 1).repeat(uses);
            defs.push_str(&format!(r##"<g id="l{level}">{copies}</g>"##));
        }
        document(&format!(r##"<defs>{defs}</defs><use href="#l{levels}"/>"##))
    }

    #[test]
    fn an_exponential_use_fan_out_is_refused_before_it_expands() {
        assert_eq!(
            refusal(use_fan_out(10, 10).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
        assert!(parse(use_fan_out(2, 10).as_bytes()).is_ok());
    }

    pub(crate) fn use_chain(hops: usize) -> String {
        let mut defs = String::from(r##"<g id="g0"><rect width="9" height="9" fill="#f00"/></g>"##);
        for hop in 1..=hops {
            defs.push_str(&format!(
                r##"<g id="g{hop}"><use href="#g{}"/></g>"##,
                hop - 1
            ));
        }
        document(&format!(r##"<defs>{defs}</defs><use href="#g{hops}"/>"##))
    }

    #[test]
    fn a_use_chain_past_the_depth_bound_is_refused() {
        assert_eq!(
            refusal(use_chain(MAX_SVG_DEPTH).as_bytes()),
            Some(SvgRefusal::TooDeeplyNested)
        );
        assert_eq!(
            refusal(use_chain(256).as_bytes()),
            Some(SvgRefusal::TooDeeplyNested)
        );
        assert!(parse(use_chain(8).as_bytes()).is_ok());
    }

    #[test]
    fn a_reference_cycle_is_refused() {
        for body in [
            r##"<g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g>"##,
            r##"<g id="a"><rect width="9" height="9"/><use href="#a"/></g>"##,
            r##"<linearGradient id="a" href="#b"/><linearGradient id="b" href="#c"/><linearGradient id="c" href="#b"/><rect width="9" height="9" fill="url(#a)"/>"##,
            r##"<clipPath id="c"><rect width="9" height="9" clip-path="url(#c)"/></clipPath>"##,
        ] {
            assert_eq!(
                refusal(document(body).as_bytes()),
                Some(SvgRefusal::ReferenceCycle),
                "{body}"
            );
        }
    }

    #[test]
    fn a_reference_cycle_is_found_through_targets_exactly_as_usvg_reads_them() {
        let clips = |ids: [&str; 3], references: [&str; 3]| {
            let mut body = String::new();
            for (id, reference) in ids.iter().zip(references) {
                body.push_str(&format!(
                    r##"<clipPath id="{id}" {reference}><rect width="9" height="9"/></clipPath>"##
                ));
            }
            body.push_str(&format!(
                r##"<rect width="9" height="9" clip-path="url(#{})"/>"##,
                ids[0]
            ));
            document(&body)
        };
        for source in [
            clips(
                ["a&#9;", "b&#9;", "c&#9;"],
                [
                    r##"clip-path="url(#b&#9;)""##,
                    r##"clip-path="url(#c&#9;)""##,
                    r##"clip-path="url(#a&#9;)""##,
                ],
            ),
            clips(
                ["a&#9;x", "b&#9;x", "c&#9;x"],
                [
                    r##"clip-path="url('#b&#9;x')""##,
                    r##"clip-path=" url( &quot;#c&#9;x &quot; ) ""##,
                    r##"style="clip-path:url(#a&#9;x)""##,
                ],
            ),
            document(concat!(
                r##"<linearGradient id="a" href="#b&#9;"/><linearGradient id="b&#9;" href="#c&#9;"/>"##,
                r##"<linearGradient id="c&#9;" xlink:href="#b&#9;" xmlns:xlink="http://www.w3.org/1999/xlink"/>"##,
                r##"<rect width="9" height="9" fill="url(#a)"/>"##
            )),
        ] {
            assert_eq!(
                refusal(source.as_bytes()),
                Some(SvgRefusal::ReferenceCycle),
                "{source}"
            );
        }
    }

    #[test]
    fn a_clip_path_inherited_into_a_copy_is_refused() {
        for body in [
            r##"<g clip-path="url(#t)"><clipPath id="x" clip-path="inherit"><rect width="9" height="9"/></clipPath></g><clipPath id="t"><rect width="9" height="9" clip-path="url(#x)"/></clipPath>"##,
            r##"<rect width="9" height="9" style="clip-path: inherit"/>"##,
            r##"<style>.a{clip-path:inherit}</style><rect class="a" width="9" height="9"/>"##,
        ] {
            assert_eq!(
                refusal(document(body).as_bytes()),
                Some(SvgRefusal::UnsupportedStyle),
                "{body}"
            );
        }
    }

    #[test]
    fn a_reference_only_reaches_the_kind_of_element_usvg_converts_for_it() {
        let body = concat!(
            r##"<g id="x"><rect width="96" height="96" fill="url(#x)" clip-path="url(#x)"/></g>"##,
            r##"<linearGradient id="g"><stop offset="0" stop-color="#0f0"/></linearGradient>"##,
            r##"<clipPath id="c"><rect width="96" height="96" fill="url(#g)"/></clipPath>"##
        );
        assert!(parse(document(body).as_bytes()).is_ok());
    }

    #[test]
    fn the_audit_reads_a_long_reference_list_in_one_pass() {
        let list = "url(#".repeat(500_000);
        for (attribute, outcome) in [
            ("fill", Some(SvgRefusal::ExternalReference)),
            ("clip-path", Some(SvgRefusal::ExternalReference)),
            ("style", Some(SvgRefusal::UnsupportedStyle)),
            ("data-x", Some(SvgRefusal::ExpansionTooLarge)),
        ] {
            let value = if attribute == "style" {
                format!("fill:{list}")
            } else {
                list.clone()
            };
            let source = document(&format!(
                r##"<rect width="9" height="9" {attribute}="{value}"/>"##
            ));
            assert!(source.len() < MAX_SVG_BYTES);
            let started = std::time::Instant::now();
            assert_eq!(refusal(source.as_bytes()), outcome, "{attribute}");
            let elapsed = started.elapsed();
            assert!(
                elapsed < std::time::Duration::from_secs(5),
                "{attribute}: {elapsed:?}"
            );
        }
    }

    #[test]
    fn attributes_are_bounded_before_roxmltree_compares_them_pairwise() {
        let element = |count: usize| {
            let attributes: String = (0..count).map(|index| format!(" a{index}=\"\"")).collect();
            document(&format!(r##"<rect width="9" height="9"{attributes}/>"##))
        };
        assert!(parse(element(MAX_SVG_ELEMENT_ATTRIBUTES - 2).as_bytes()).is_ok());
        let started = std::time::Instant::now();
        assert_eq!(
            refusal(element(150_000).as_bytes()),
            Some(SvgRefusal::DocumentTooLarge)
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn a_use_is_charged_for_walking_its_whole_target() {
        for filler in ["<x:a/>", "<!---->", "x"] {
            let source = document(&format!(
                r##"<defs><g id="t" xmlns:x="urn:x">{}<rect width="1" height="1"/></g></defs>{}"##,
                filler.repeat(20_000),
                r##"<use href="#t"/>"##.repeat(10_000)
            ));
            assert_eq!(
                refusal(source.as_bytes()),
                Some(SvgRefusal::ExpansionTooLarge),
                "{filler}"
            );
        }
    }

    #[test]
    fn a_shape_is_held_to_the_path_data_usvg_strokes_whole() {
        let path = |segments: usize| {
            document(&format!(
                r##"<path d="M0 0{}" stroke="#000" stroke-width="0.1"/><polygon points="{}"/>"##,
                "h1".repeat(segments),
                "1 1 ".repeat(segments / 2)
            ))
        };
        assert!(parse(path(MAX_SVG_PATH_BYTES / 2 - 4).as_bytes()).is_ok());
        assert_eq!(
            refusal(path(MAX_SVG_PATH_BYTES / 2).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
    }

    #[test]
    fn a_path_is_priced_for_the_edges_and_outline_it_draws() {
        let zigzag = |segments: usize, stroke: &str| {
            document(&format!(
                r##"<path d="M40 40{}" fill="none" stroke="#000" {stroke}/>"##,
                "l9 9l-9-9".repeat(segments / 2)
            ))
        };
        assert!(parse(zigzag(200, r#"stroke-width="4""#).as_bytes()).is_ok());
        for stroke in [
            r#"stroke-width="4""#,
            r#"stroke-width="4" stroke-linejoin="round""#,
            r#"stroke-width="4" stroke-dasharray="0.1 0.1""#,
        ] {
            let started = std::time::Instant::now();
            assert_eq!(
                refusal(zigzag(12_000, stroke).as_bytes()),
                Some(SvgRefusal::RenderTooCostly),
                "{stroke}"
            );
            assert!(started.elapsed() < std::time::Duration::from_secs(5));
        }
        assert!(
            parse(zigzag(12_000, r#"stroke-width="0.1""#).as_bytes()).is_ok(),
            "a hairline is drawn without an outline"
        );
    }

    #[test]
    fn a_stroke_too_wide_for_the_rasteriser_is_refused_before_it_panics() {
        let fuzzed = concat!(
            r##"<svg><g d="e_2"><g id="e1-2"><path d="M .23.11.08  0L5 5" stroke="#000" "##,
            r##"stroke-width="1e30" arke=""/></g></g></svg>"##
        );
        assert_eq!(
            refusal(fuzzed.as_bytes()),
            Some(SvgRefusal::RenderTooCostly)
        );
    }

    #[test]
    fn a_clip_path_chain_counts_every_instance() {
        let mut defs = String::from(
            r##"<clipPath id="c0" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath>"##,
        );
        for level in 1..=8 {
            let children = format!(
                r##"<rect width="1" height="1" clip-path="url(#c{})"/>"##,
                level - 1
            )
            .repeat(10);
            defs.push_str(&format!(
                r##"<clipPath id="c{level}" clipPathUnits="objectBoundingBox">{children}</clipPath>"##
            ));
        }
        let source = document(&format!(
            r##"<defs>{defs}</defs><rect width="96" height="96" clip-path="url(#c8)"/>"##
        ));
        assert_eq!(
            refusal(source.as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
    }

    #[test]
    fn path_data_reread_per_use_counts_against_the_expansion() {
        let d = "M0 0L1 1".repeat(20_000);
        let uses = r##"<use href="#p"/>"##.repeat(64);
        let source = document(&format!(
            r##"<defs><path id="p" d="{d}" stroke="#000"/></defs>{uses}"##
        ));
        assert!(source.len() < MAX_SVG_BYTES);
        assert_eq!(
            refusal(source.as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
    }

    #[test]
    fn a_reference_to_a_repeated_id_counts_every_element_that_carries_it() {
        let targets = r##"<linearGradient id="x"/>"##.repeat(1_000);
        let references = "url(#x) ".repeat(200);
        let source = document(&format!(
            r##"<defs>{targets}</defs><rect width="9" height="9" style="fill:{references}"/>"##
        ));
        assert_eq!(
            refusal(source.as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
    }

    #[test]
    fn a_stylesheet_is_held_to_plain_rules_and_a_matching_budget() {
        let deep = format!(
            "<style>.x {} {{fill:red}}</style>{}<rect width=\"9\" height=\"9\"/>{}",
            "g ".repeat(32),
            "<g>".repeat(60),
            "</g>".repeat(60)
        );
        assert_eq!(
            refusal(document(&deep).as_bytes()),
            Some(SvgRefusal::UnsupportedStyle)
        );
        let rules: String = (0..1_000)
            .map(|index| format!(".c{index}{{fill:red}}"))
            .collect();
        let rects = r##"<rect width="9" height="9"/>"##.repeat(20_000);
        let wide = document(&format!("<style>{rules}</style>{rects}"));
        assert_eq!(
            refusal(wide.as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
        let many: String = (0..MAX_SVG_STYLE_RULES)
            .map(|index| format!(".c{index}{{fill:red}}"))
            .collect();
        assert_eq!(
            refusal(document(&format!("<style>{many}</style>")).as_bytes()),
            Some(SvgRefusal::UnsupportedStyle)
        );
    }

    #[test]
    fn a_selector_bomb_is_refused_before_anything_matches_it() {
        let bomb = |parts: usize, elements: usize| {
            document(&format!(
                "<style>{}{{fill:red}}</style>{}",
                ".a".repeat(parts),
                r##"<g class="a"/>"##.repeat(elements)
            ))
        };
        let started = std::time::Instant::now();
        assert_ne!(refusal(bomb(1_000_000, 90_000).as_bytes()), None);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        for (parts, outcome) in [
            (800_000, SvgRefusal::ExpansionTooLarge),
            (5_000, SvgRefusal::UnsupportedStyle),
        ] {
            let source = bomb(parts, 20_000);
            assert!(source.len() < MAX_SVG_BYTES);
            let started = std::time::Instant::now();
            assert_eq!(refusal(source.as_bytes()), Some(outcome), "{parts}");
            assert!(started.elapsed() < std::time::Duration::from_secs(5));
        }
    }

    #[test]
    fn rules_times_elements_times_parts_is_charged_before_usvg_matches_them() {
        let styled = |elements: usize| {
            let rules: String = (0..300)
                .map(|index| format!(".x{index}{}{{fill:red}}", ".a".repeat(15)))
                .collect();
            document(&format!(
                "<style>{rules}</style>{}",
                r##"<g class="a"/>"##.repeat(elements)
            ))
        };
        assert!(parse(styled(100).as_bytes()).is_ok());
        assert_eq!(
            refusal(styled(10_000).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
    }

    #[test]
    fn css_text_is_charged_the_rescans_simplecss_makes_of_it() {
        let style = |declarations: usize| {
            document(&format!(
                r##"<rect width="9" height="9" style="{}"/>"##,
                "fill:red;".repeat(declarations)
            ))
        };
        assert!(parse(style(100).as_bytes()).is_ok());
        assert_eq!(
            refusal(style(8_000).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
        let sheet = document(&format!(
            "<style>.a{{{}}}</style>",
            "fill:red;".repeat(8_000)
        ));
        assert_eq!(
            refusal(sheet.as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
        let copied = document(&format!(
            "<defs><rect id=\"r\" width=\"9\" height=\"9\" style=\"{}\"/></defs>{}",
            "fill:red;".repeat(400),
            r##"<use href="#r"/>"##.repeat(2_000)
        ));
        assert_eq!(
            refusal(copied.as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge),
            "each copy re-reads its style attribute"
        );
    }

    #[test]
    fn a_filter_in_any_form_is_refused() {
        for body in [
            r##"<rect width="9" height="9" filter="blur(4)"/>"##,
            r##"<rect width="9" height="9" style="fill:red;filter:drop-shadow(1 1 1 red)"/>"##,
            r##"<style>rect{filter:blur(4)}</style><rect width="9" height="9"/>"##,
        ] {
            assert_eq!(
                refusal(document(body).as_bytes()),
                Some(SvgRefusal::UnsupportedStyle),
                "{body}"
            );
        }
        assert!(
            parse(document(r##"<rect width="9" height="9" filter="none"/>"##).as_bytes()).is_ok()
        );
    }

    fn gradient_fills(stops: usize, fills: usize) -> String {
        let stops: String = (0..stops)
            .map(|index| {
                let offset = index as f32 / stops as f32;
                format!(r##"<stop offset="{offset}" stop-color="#f00"/>"##)
            })
            .collect();
        let fills = r##"<rect width="96" height="96" fill="url(#g)"/>"##.repeat(fills);
        document(&format!(
            r##"<linearGradient id="g">{stops}</linearGradient>{fills}"##
        ))
    }

    #[test]
    fn a_gradient_past_the_stop_bound_is_refused() {
        assert_eq!(
            refusal(gradient_fills(MAX_SVG_GRADIENT_STOPS + 1, 1).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
        assert!(parse(gradient_fills(MAX_SVG_GRADIENT_STOPS, 1).as_bytes()).is_ok());
    }

    fn inherited(gradient: &str, stops: usize, paint: &str, shapes: usize) -> String {
        let stops: String = (0..stops)
            .map(|index| {
                let offset = index as f32 / stops as f32;
                format!(r##"<stop offset="{offset}" stop-color="#f00"/>"##)
            })
            .collect();
        document(&format!(
            r##"<linearGradient id="g" {gradient}>{stops}</linearGradient><g {paint}="url(#g)">{}</g>"##,
            r##"<rect width="1" height="1"/>"##.repeat(shapes)
        ))
    }

    #[test]
    fn an_inherited_gradient_is_charged_per_shape_that_paints_with_it() {
        let started = std::time::Instant::now();
        assert_ne!(
            refusal(inherited("", MAX_SVG_GRADIENT_STOPS, "fill", 90_000).as_bytes()),
            None
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        for paint in ["fill", "stroke"] {
            let source = inherited("", MAX_SVG_GRADIENT_STOPS, paint, 3_000);
            assert_eq!(
                refusal(source.as_bytes()),
                Some(SvgRefusal::ExpansionTooLarge),
                "{paint}"
            );
            assert!(parse(inherited("", MAX_SVG_GRADIENT_STOPS, paint, 16).as_bytes()).is_ok());
        }
        let shared = inherited(r##"gradientUnits="userSpaceOnUse""##, 2, "fill", 20_000);
        assert!(
            parse(shared.as_bytes()).is_ok(),
            "a gradient in user units is shared, not copied"
        );
        assert_eq!(
            refusal(inherited("", 2, "fill", 20_000).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge),
            "every copy is compared against every other when usvg collects them"
        );
        assert!(parse(inherited("", 2, "fill", 2_000).as_bytes()).is_ok());
    }

    #[test]
    fn a_context_paint_is_charged_per_use_that_hands_it_down() {
        let copies = |uses: usize| {
            let stops: String = (0..MAX_SVG_GRADIENT_STOPS)
                .map(|index| format!(r##"<stop offset="{}" stop-color="#f00"/>"##, index))
                .collect();
            document(&format!(
                concat!(
                    r##"<linearGradient id="g" gradientUnits="userSpaceOnUse">{}</linearGradient>"##,
                    r##"<defs><g id="r"><rect width="1" height="1" fill="context-fill"/></g></defs>{}"##
                ),
                stops,
                r##"<use href="#r" fill="url(#g)"/>"##.repeat(uses)
            ))
        };
        assert!(parse(copies(4).as_bytes()).is_ok());
        assert_eq!(
            refusal(copies(3_000).as_bytes()),
            Some(SvgRefusal::ExpansionTooLarge)
        );
    }

    #[test]
    fn full_canvas_fills_past_the_overdraw_bound_are_refused() {
        let fills = r##"<rect width="96" height="96" fill="#f00"/>"##.repeat(10_000);
        assert_eq!(
            refusal(document(&fills).as_bytes()),
            Some(SvgRefusal::RenderTooCostly)
        );
        assert_eq!(
            refusal(gradient_fills(MAX_SVG_GRADIENT_STOPS, 2).as_bytes()),
            Some(SvgRefusal::RenderTooCostly),
            "each stop is a test per painted pixel"
        );
        let few = r##"<rect width="96" height="96" fill="#f00"/>"##.repeat(32);
        assert!(parse(document(&few).as_bytes()).is_ok());
    }

    #[test]
    fn dashes_count_against_the_render_work() {
        let dashed =
            r##"<path d="M0 48H100000" stroke="#000" stroke-dasharray="0.5 0.5"/>"##.repeat(20);
        assert_eq!(
            refusal(document(&dashed).as_bytes()),
            Some(SvgRefusal::RenderTooCostly)
        );
        let few = r##"<path d="M0 48H96" stroke="#000" stroke-dasharray="4 4"/>"##;
        assert!(parse(document(few).as_bytes()).is_ok());
    }

    pub(crate) fn opacity_nest(depth: usize) -> String {
        document(&format!(
            r##"{}<rect width="96" height="96" fill="#0000ff"/>{}"##,
            r#"<g opacity="0.99">"#.repeat(depth),
            "</g>".repeat(depth)
        ))
    }

    #[test]
    fn nested_layers_past_the_bound_are_refused_and_within_it_charge_their_rasters() {
        assert_eq!(
            refusal(opacity_nest(MAX_SVG_LAYER_DEPTH + 1).as_bytes()),
            Some(SvgRefusal::TooManyLayers)
        );
        let image = parse(opacity_nest(MAX_SVG_LAYER_DEPTH).as_bytes()).expect("parse");
        let layers = MAX_SVG_LAYER_DEPTH as u64;
        assert!(
            (384 * 384 + layers * 388 * 388..=384 * 384 + layers * 390 * 390)
                .contains(&image.pixels()),
            "each layer is the canvas grown two pixels a side: {}",
            image.pixels()
        );
        let (data, _) = image.render().expect("render");
        assert_eq!(data[2], 255, "the nested rect still draws");
    }

    #[test]
    fn a_clip_too_large_to_stack_steps_the_supersample_down() {
        let source = concat!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="960" viewBox="0 0 1920 960">"##,
            r##"<defs><clipPath id="c"><path d="M0 0H1920V960H0Z"/></clipPath></defs>"##,
            r##"<g clip-path="url(#c)"><rect width="1920" height="960" fill="#00adef"/></g></svg>"##
        );
        let image = parse(source.as_bytes()).expect("parse");
        assert_eq!((image.size.width(), image.size.height()), (3840, 1920));
        assert!(image.pixels() <= MAX_IMAGE_PIXELS);
    }

    #[test]
    fn a_panicking_step_is_a_refusal() {
        assert_eq!(guarded(|| 7), Ok(7));
        assert_eq!(
            guarded(|| -> u8 { panic!("usvg fell over") }),
            Err(SvgRefusal::Panicked)
        );
    }
}
