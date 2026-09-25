//! What rendering a converted tree costs at one raster size, measured on the
//! tree `usvg` resolved (CSS applied, references expanded) before `resvg` runs.

use resvg::usvg::{self, Node, Paint};
use tiny_skia::{IntSize, PathSegment, Point, Rect, Transform};

use super::{MAX_SVG_EXPANDED_NODES, MAX_SVG_LAYER_DEPTH, MAX_SVG_OVERDRAW, SvgRefusal};

/// Work units, each about one painted pixel, per drawn path before any pixel.
const PATH_WORK: u64 = 128;
/// Per device pixel of outline length the rasteriser walks for a fill.
const FILL_EDGE_WORK: f64 = 4.0;
/// The same for a stroke, which the stroker or hairline rasteriser walks.
const STROKE_EDGE_WORK: f64 = 16.0;
/// Per dash `tiny-skia` cuts, each stroked as its own contour.
const DASH_WORK: f64 = 128.0;

pub(super) struct Cost {
    /// The output raster plus the deepest stack of layers and clip masks.
    pub(super) pixels: u64,
    /// Painting work in units of one painted pixel.
    pub(super) work: u64,
}

/// A device-space rectangle in output pixels.
#[derive(Clone, Copy)]
struct Area {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Area {
    fn pixels(self) -> f64 {
        (self.right - self.left).max(0.0) * (self.bottom - self.top).max(0.0)
    }

    fn meet(self, other: Area) -> Area {
        Area {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        }
    }

    fn of(rect: Rect, transform: Transform) -> Option<Area> {
        let rect = rect.transform(transform)?;
        Some(Area {
            left: f64::from(rect.left()),
            top: f64::from(rect.top()),
            right: f64::from(rect.right()),
            bottom: f64::from(rect.bottom()),
        })
    }
}

struct Frame<'a> {
    group: &'a usvg::Group,
    surface: Area,
    live: u64,
    depth: usize,
}

#[derive(Default)]
struct Tally {
    nodes: u64,
    painted: f64,
    work: f64,
    peak: u64,
}

impl Tally {
    fn node(&mut self) -> Result<(), SvgRefusal> {
        self.nodes += 1;
        if self.nodes > MAX_SVG_EXPANDED_NODES {
            return Err(SvgRefusal::ExpansionTooLarge);
        }
        Ok(())
    }

    fn paint(&mut self, pixels: f64, paint: &Paint) -> Result<(), SvgRefusal> {
        let stops = match paint {
            Paint::Color(_) => 0,
            Paint::LinearGradient(gradient) => gradient.stops().len(),
            Paint::RadialGradient(gradient) => gradient.stops().len(),
            Paint::Pattern(_) => return Err(SvgRefusal::UnsupportedElement),
        };
        let painted = pixels * (1.0 + stops as f64 / 8.0);
        self.painted += painted;
        self.work += painted;
        Ok(())
    }
}

/// Refuses a tree whose render is unbounded in kind (masks, filters, nested
/// clips, layers past [`MAX_SVG_LAYER_DEPTH`], overdraw past
/// [`MAX_SVG_OVERDRAW`]) and prices the rest for a `size` raster. Layers are
/// sized the way `resvg` allocates them: the group's device bounds grown by two
/// pixels a side, clamped to five canvases a side around the parent layer.
pub(super) fn measure(tree: &usvg::Tree, size: IntSize) -> Result<Cost, SvgRefusal> {
    let (width, height) = (f64::from(size.width()), f64::from(size.height()));
    let scale = Transform::from_scale(
        size.width() as f32 / tree.size().width(),
        size.height() as f32 / tree.size().height(),
    );
    let canvas = Area {
        left: 0.0,
        top: 0.0,
        right: width,
        bottom: height,
    };
    let mut tally = Tally::default();
    let mut stack = vec![Frame {
        group: tree.root(),
        surface: canvas,
        live: 0,
        depth: 0,
    }];
    while let Some(frame) = stack.pop() {
        for node in frame.group.children() {
            tally.node()?;
            match node {
                Node::Group(group) => {
                    if group.mask().is_some() || !group.filters().is_empty() {
                        return Err(SvgRefusal::UnsupportedElement);
                    }
                    let mut inner = Frame { group, ..frame };
                    if group.should_isolate() {
                        inner.depth += 1;
                        if inner.depth > MAX_SVG_LAYER_DEPTH {
                            return Err(SvgRefusal::TooManyLayers);
                        }
                        let bounds = Area::of(group.abs_layer_bounding_box().to_rect(), scale);
                        let Some(layer) = layer(bounds, frame.surface, width, height) else {
                            continue;
                        };
                        let pixels = layer.pixels();
                        let masks = u64::from(group.clip_path().is_some());
                        tally.work += pixels * (1 + 2 * masks) as f64;
                        tally.painted += layer.meet(frame.surface).pixels();
                        inner.live = frame
                            .live
                            .saturating_add((pixels as u64).saturating_mul(1 + masks));
                        tally.peak = tally.peak.max(inner.live);
                        if let Some(clip) = group.clip_path() {
                            let place = scale
                                .pre_concat(group.abs_transform())
                                .pre_concat(clip.transform());
                            measure_clip(clip, place, layer, &mut tally)?;
                        }
                        inner.surface = layer;
                    }
                    stack.push(inner);
                }
                Node::Path(path) => {
                    if !path.is_visible() {
                        continue;
                    }
                    let place = scale.pre_concat(path.abs_transform());
                    tally.work += PATH_WORK as f64;
                    if let Some(fill) = path.fill() {
                        let area = Area::of(path.abs_bounding_box(), scale)
                            .map_or(frame.surface, |area| area.meet(frame.surface));
                        tally.paint(area.pixels(), fill.paint())?;
                        if area.pixels() > 0.0 {
                            tally.work += FILL_EDGE_WORK * length(path.data(), place);
                        }
                    }
                    if let Some(stroke) = path.stroke() {
                        let area = Area::of(path.abs_stroke_bounding_box(), scale)
                            .map_or(frame.surface, |area| area.meet(frame.surface));
                        tally.paint(area.pixels(), stroke.paint())?;
                        tally.work += STROKE_EDGE_WORK * length(path.data(), place);
                        tally.work += DASH_WORK * dashes(path.data(), stroke);
                    }
                }
                Node::Image(_) | Node::Text(_) => return Err(SvgRefusal::UnsupportedElement),
            }
        }
    }
    if tally.painted > (MAX_SVG_OVERDRAW as f64) * width * height {
        return Err(SvgRefusal::RenderTooCostly);
    }
    let output = u64::from(size.width()) * u64::from(size.height());
    Ok(Cost {
        pixels: output.saturating_add(tally.peak),
        work: if tally.work.is_finite() {
            tally.work as u64
        } else {
            u64::MAX
        },
    })
}

/// Where `resvg` allocates a group's layer: its device bounds grown two pixels
/// a side (one more for rounding), fitted to five canvases a side around the
/// surface it composites onto. `None` is a layer `resvg` skips.
fn layer(bounds: Option<Area>, parent: Area, width: f64, height: f64) -> Option<Area> {
    let reach = Area {
        left: parent.left - 2.0 * width,
        top: parent.top - 2.0 * height,
        right: parent.left + 3.0 * width,
        bottom: parent.top + 3.0 * height,
    };
    let grown = bounds
        .filter(|area| {
            [area.left, area.top, area.right, area.bottom]
                .iter()
                .all(|v| v.is_finite())
        })
        .map_or(reach, |area| Area {
            left: area.left.floor() - 2.0,
            top: area.top.floor() - 2.0,
            right: area.left.floor() + (area.right - area.left).ceil() + 3.0,
            bottom: area.top.floor() + (area.bottom - area.top).ceil() + 3.0,
        });
    let layer = grown.meet(reach);
    (layer.pixels() > 0.0).then_some(layer)
}

/// A clip path fills its children into a raster the size of the layer it
/// clips. One level only: a clip on a clip, or on a group inside one, stacks
/// another raster per level.
fn measure_clip(
    clip: &usvg::ClipPath,
    place: Transform,
    layer: Area,
    tally: &mut Tally,
) -> Result<(), SvgRefusal> {
    if clip.clip_path().is_some() {
        return Err(SvgRefusal::TooManyLayers);
    }
    let mut stack = vec![clip.root()];
    while let Some(group) = stack.pop() {
        for node in group.children() {
            tally.node()?;
            match node {
                Node::Group(group) => {
                    if group.clip_path().is_some() {
                        return Err(SvgRefusal::TooManyLayers);
                    }
                    stack.push(group);
                }
                Node::Path(path) => {
                    let area = Area::of(path.abs_bounding_box(), place)
                        .map_or(layer, |area| area.meet(layer));
                    tally.painted += area.pixels();
                    tally.work += PATH_WORK as f64
                        + area.pixels()
                        + FILL_EDGE_WORK
                            * length(path.data(), place.pre_concat(path.abs_transform()));
                }
                Node::Image(_) | Node::Text(_) => return Err(SvgRefusal::UnsupportedElement),
            }
        }
    }
    Ok(())
}

/// The control-polygon length of `data` under `transform`, which a curve never
/// exceeds.
fn length(data: &tiny_skia::Path, transform: Transform) -> f64 {
    let map = |point: Point| {
        (
            f64::from(transform.sx * point.x + transform.kx * point.y + transform.tx),
            f64::from(transform.ky * point.x + transform.sy * point.y + transform.ty),
        )
    };
    let distance =
        |(ax, ay): (f64, f64), (bx, by): (f64, f64)| ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
    let (mut start, mut current) = ((0.0, 0.0), (0.0, 0.0));
    let mut total = 0.0;
    for segment in data.segments() {
        let points: &[Point] = match &segment {
            PathSegment::MoveTo(point) => {
                start = map(*point);
                current = start;
                continue;
            }
            PathSegment::LineTo(point) => std::slice::from_ref(point),
            PathSegment::QuadTo(control, point) => &[*control, *point],
            PathSegment::CubicTo(first, second, point) => &[*first, *second, *point],
            PathSegment::Close => {
                total += distance(current, start);
                current = start;
                continue;
            }
        };
        for point in points {
            let next = map(*point);
            total += distance(current, next);
            current = next;
        }
    }
    total
}

/// Dashes `tiny-skia` cuts from a stroke: path length over the dash period, in
/// the path's own units, as its dasher counts them.
fn dashes(data: &tiny_skia::Path, stroke: &usvg::Stroke) -> f64 {
    let Some(array) = stroke.dasharray() else {
        return 0.0;
    };
    let period: f64 = array.iter().map(|value| f64::from(*value)).sum();
    if period.is_nan() || period <= 0.0 {
        return 0.0;
    }
    length(data, Transform::identity()) * (array.len() / 2) as f64 / period
}
