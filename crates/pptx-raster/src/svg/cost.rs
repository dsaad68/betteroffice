//! What rendering a converted tree costs at one raster size, measured on the
//! tree `usvg` resolved (CSS applied, references expanded) before `resvg` runs.

use resvg::usvg::{self, Node, Paint};
use tiny_skia::{IntSize, PathSegment, Point, Rect, Transform};

use super::{
    MAX_SVG_EXPANDED_NODES, MAX_SVG_LAYER_DEPTH, MAX_SVG_OVERDRAW, SVG_TRANSIENT_BYTES, SvgRefusal,
};

/// Work units, each about one painted pixel, per drawn path before any pixel.
const PATH_WORK: u64 = 128;
/// Per device pixel of outline length the rasteriser walks for a fill.
const FILL_EDGE_WORK: f64 = 4.0;
/// The same for a stroke, which the stroker or hairline rasteriser walks.
const STROKE_EDGE_WORK: f64 = 16.0;
/// Per dash `tiny-skia` cuts, each stroked as its own contour.
const DASH_WORK: f64 = 128.0;
/// Per step of the rasteriser's insertion sort of its active edges, about a
/// nanosecond where a painted pixel is four.
const SORT_WORK: f64 = 0.25;
/// Edges the rasteriser builds per filled segment: a curve splits into up to
/// three pieces monotonic in y.
const FILL_EDGES: f64 = 3.0;
/// Edges of the outline the stroker builds per segment or dash: both sides
/// and the joins or caps between them.
const STROKE_EDGES: f64 = 4.0;
/// Bytes of edges the rasteriser builds per filled segment.
const FILL_BYTES: f64 = 192.0;
/// Bytes of outline and edges per stroked segment, round joins the costliest
/// at about 1.3 KiB.
const OUTLINE_BYTES: f64 = 1_536.0;
/// The same per dash, each its own contour with two caps.
const DASH_BYTES: f64 = 768.0;
/// Bytes of dashed path per dash of a hairline, which is never outlined.
const HAIRLINE_DASH_BYTES: f64 = 48.0;
/// Below this many edge pairs a path is charged every pair without sweeping
/// its rows.
const SWEEP_PAIRS: f64 = 1_048_576.0;
/// How far, in device pixels, a stroke's outline may reach past its path:
/// eight times the largest raster. `tiny-skia` rasterises in 16.16 fixed
/// point, and an outline far past that overflows its coverage runs.
const MAX_REACH: f64 = 65_536.0;

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
    /// The most any one path's edges, outline and dashes take while drawn.
    transient: f64,
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
                    let segments = path.data().verbs().len() as f64;
                    tally.work += PATH_WORK as f64;
                    if let Some(fill) = path.fill() {
                        let area = Area::of(path.abs_bounding_box(), scale)
                            .map_or(frame.surface, |area| area.meet(frame.surface));
                        tally.paint(area.pixels(), fill.paint())?;
                        if area.pixels() > 0.0 {
                            tally.work += FILL_EDGE_WORK * length(path.data(), place);
                            tally.work += SORT_WORK
                                * sorting(path.data(), place, frame.surface, 0.0, |_| FILL_EDGES);
                        }
                        tally.transient = tally.transient.max(segments * FILL_BYTES);
                    }
                    if let Some(stroke) = path.stroke() {
                        let area = Area::of(path.abs_stroke_bounding_box(), scale)
                            .map_or(frame.surface, |area| area.meet(frame.surface));
                        tally.paint(area.pixels(), stroke.paint())?;
                        tally.work += STROKE_EDGE_WORK * length(path.data(), place);
                        let dashes = dashes(path.data(), stroke);
                        tally.work += DASH_WORK * dashes;
                        let transient = if hairline(path, stroke, place) {
                            dashes * HAIRLINE_DASH_BYTES
                        } else {
                            let period = period(stroke);
                            let edges = |run: f64| {
                                STROKE_EDGES
                                    * (1.0 + 2.0 * period.map_or(0.0, |period| run / period))
                            };
                            let reach = reach(stroke, place);
                            if reach.is_nan() || reach > MAX_REACH {
                                return Err(SvgRefusal::RenderTooCostly);
                            }
                            tally.work += SORT_WORK
                                * sorting(path.data(), place, frame.surface, reach, edges);
                            segments * OUTLINE_BYTES + dashes * DASH_BYTES
                        };
                        tally.transient = tally.transient.max(transient);
                    }
                }
                Node::Image(_) | Node::Text(_) => return Err(SvgRefusal::UnsupportedElement),
            }
        }
    }
    if tally.painted > (MAX_SVG_OVERDRAW as f64) * width * height
        || tally.transient > SVG_TRANSIENT_BYTES as f64
    {
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
                    let drawn = place.pre_concat(path.abs_transform());
                    tally.painted += area.pixels();
                    tally.work += PATH_WORK as f64
                        + area.pixels()
                        + FILL_EDGE_WORK * length(path.data(), drawn)
                        + SORT_WORK * sorting(path.data(), drawn, layer, 0.0, |_| FILL_EDGES);
                    let segments = path.data().verbs().len() as f64;
                    tally.transient = tally.transient.max(segments * FILL_BYTES);
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

/// Steps the rasteriser may take insertion-sorting its active edges as it
/// fills a path: at most one per pair of edges, since two monotone edges swap
/// at most once, and at most every pair active together on each of the four
/// anti-aliasing sub-rows of every row. Each segment adds `edges(run)` edges,
/// `run` its length in the path's own units, over the rows its control points
/// span within `surface`, widened by `reach` for a stroke's outline; one with
/// no height to cross a sub-row adds none.
fn sorting(
    data: &tiny_skia::Path,
    place: Transform,
    surface: Area,
    reach: f64,
    edges: impl Fn(f64) -> f64,
) -> f64 {
    let mut spans = Vec::new();
    let mut total = 0.0;
    let (mut start, mut current) = (Point::zero(), Point::zero());
    for segment in data.segments() {
        let points: &[Point] = match &segment {
            PathSegment::MoveTo(point) => {
                start = *point;
                current = start;
                continue;
            }
            PathSegment::LineTo(point) => std::slice::from_ref(point),
            PathSegment::QuadTo(control, point) => &[*control, *point],
            PathSegment::CubicTo(first, second, point) => &[*first, *second, *point],
            PathSegment::Close => std::slice::from_ref(&start),
        };
        let mut run = 0.0;
        let mut rows = (f64::INFINITY, f64::NEG_INFINITY);
        let mut previous = current;
        for point in std::iter::once(&current).chain(points) {
            run += f64::from(point.distance(previous));
            previous = *point;
            let y = f64::from(place.ky * point.x + place.sy * point.y + place.ty);
            rows = (rows.0.min(y), rows.1.max(y));
        }
        current = previous;
        if rows.1 - rows.0 + 2.0 * reach < 0.25 {
            continue;
        }
        let count = edges(run);
        total += count;
        let top = (rows.0 - reach).floor().max(surface.top.floor());
        let bottom = (rows.1 + reach).ceil().min(surface.bottom.ceil());
        if top <= bottom {
            spans.push((top, count));
            spans.push((bottom + 1.0, -count));
        }
    }
    let pairs = 2.0 * total * total;
    if !pairs.is_finite() || pairs <= SWEEP_PAIRS {
        return pairs;
    }
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut active, mut row, mut crowded) = (0.0f64, f64::NEG_INFINITY, 0.0);
    for (at, change) in spans {
        if active > 0.0 {
            crowded += 4.0 * (at - row) * active * active;
        }
        active += change;
        row = at;
    }
    pairs.min(crowded)
}

/// Whether `tiny-skia` draws a stroke as a hairline, which it never outlines:
/// its width maps within a pixel and the path is anti-aliased.
fn hairline(path: &usvg::Path, stroke: &usvg::Stroke, place: Transform) -> bool {
    let width = stroke.width().get();
    let spread = |x: f32, y: f32| {
        let (x, y) = (
            (place.sx * x + place.kx * y).abs(),
            (place.ky * x + place.sy * y).abs(),
        );
        x.max(y) + x.min(y) / 2.0
    };
    path.rendering_mode().use_shape_antialiasing()
        && spread(width, 0.0) <= 1.0
        && spread(0.0, width) <= 1.0
}

/// How far a stroke's outline reaches past its path, in device pixels: half
/// its width, stretched by a miter or a square cap, and a pixel of rounding.
fn reach(stroke: &usvg::Stroke, place: Transform) -> f64 {
    let scale = f64::from((place.sx.abs() + place.kx.abs()).max(place.ky.abs() + place.sy.abs()));
    let miter = match stroke.linejoin() {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => f64::from(stroke.miterlimit().get()),
        _ => 1.0,
    };
    f64::from(stroke.width().get()) / 2.0 * scale * miter.max(std::f64::consts::SQRT_2) + 1.0
}

/// A stroke's dash period over the dashes it cuts in one, in the path's units.
fn period(stroke: &usvg::Stroke) -> Option<f64> {
    let array = stroke.dasharray()?;
    let period: f64 = array.iter().map(|value| f64::from(*value)).sum();
    (period.is_finite() && period > 0.0 && array.len() >= 2)
        .then(|| period / (array.len() / 2) as f64)
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
