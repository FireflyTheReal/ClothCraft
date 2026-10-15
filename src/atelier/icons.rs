//! Original vector glyph set for the atelier tool palettes.
//!
//! Every glyph here is drawn from scratch as plain stroke geometry for this
//! project. Nothing is downloaded, embedded, traced or adapted from another
//! product, icon set or image asset, so the set carries no third-party licence.
//!
//! Geometry is expressed in a normalised `0..1` square and mapped onto whatever
//! rectangle the caller passes, which keeps one glyph legible in a dense 16 px
//! palette row and crisp at 2x. Generation ([`geometry`]) is deliberately
//! separate from painting ([`paint`]) so tests can assert that every glyph is
//! non-empty and stays inside the unit square without a GPU.

// The palette is declared in full ahead of the toolbar wiring, so a few entries
// are still waiting for a call site. They are covered by tests as they land.
#![allow(dead_code)]

use eframe::egui::{
    self, Color32, CornerRadius, Pos2, Rect, Response, Shape, Stroke, StrokeKind, Vec2,
};

/// One drawing primitive in the normalised unit square.
///
/// Coordinates are `[x, y]` with `0..1` spanning the square; `y` grows downward,
/// matching egui's screen coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Prim {
    /// Straight segment from `a` to `b`.
    Line { a: [f32; 2], b: [f32; 2] },
    /// Open polyline through `points`.
    Path { points: Vec<[f32; 2]> },
    /// Stroked circle.
    Circle { center: [f32; 2], radius: f32 },
    /// Stroked rectangle.
    Rect { min: [f32; 2], max: [f32; 2] },
    /// Small filled marker: anchors, notch points, snap points.
    Dot { center: [f32; 2], radius: f32 },
}

/// Every glyph the atelier can draw.
///
/// The order is stable and each entry owns a fixed primitive count recorded in
/// the tests, so a glyph that silently loses its geometry fails a test instead
/// of reaching the user as a blank button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(super) enum Icon {
    Select,
    Edit,
    Pen,
    Rectangle,
    Sew,
    Move3d,
    Rotate3d,
    Grab,
    Undo,
    Redo,
    Duplicate,
    Mirror,
    Grid,
    Snap,
    Fit,
    ViewIso,
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
    ViewTop,
    Settings,
    Theme,
    Fabric,
    Simulate,
    Step,
    Reset,
    Export,
    Search,
    Info,
    Warning,
    Seam,
    Notch,
    Gradient,
}

#[cfg(test)]
const ALL: [Icon; 34] = [
    Icon::Select,
    Icon::Edit,
    Icon::Pen,
    Icon::Rectangle,
    Icon::Sew,
    Icon::Move3d,
    Icon::Rotate3d,
    Icon::Grab,
    Icon::Undo,
    Icon::Redo,
    Icon::Duplicate,
    Icon::Mirror,
    Icon::Grid,
    Icon::Snap,
    Icon::Fit,
    Icon::ViewIso,
    Icon::ViewFront,
    Icon::ViewBack,
    Icon::ViewLeft,
    Icon::ViewRight,
    Icon::ViewTop,
    Icon::Settings,
    Icon::Theme,
    Icon::Fabric,
    Icon::Simulate,
    Icon::Step,
    Icon::Reset,
    Icon::Export,
    Icon::Search,
    Icon::Info,
    Icon::Warning,
    Icon::Seam,
    Icon::Notch,
    Icon::Gradient,
];

// ---------------------------------------------------------------------------
// Geometry construction
// ---------------------------------------------------------------------------

fn seg(a: [f32; 2], b: [f32; 2]) -> Prim {
    Prim::Line { a, b }
}

fn path(points: &[[f32; 2]]) -> Prim {
    Prim::Path {
        points: points.to_vec(),
    }
}

/// Closed outline: repeats the first point so a stroked path returns to its start.
fn closed(points: &[[f32; 2]]) -> Prim {
    let mut points = points.to_vec();
    if let Some(first) = points.first().copied() {
        points.push(first);
    }
    Prim::Path { points }
}

fn circle(center: [f32; 2], radius: f32) -> Prim {
    Prim::Circle { center, radius }
}

fn stroke_rect(min: [f32; 2], max: [f32; 2]) -> Prim {
    Prim::Rect { min, max }
}

fn dot(center: [f32; 2], radius: f32) -> Prim {
    Prim::Dot { center, radius }
}

/// Sample a circular arc. Angles are in degrees, `y` grows downward.
fn arc(center: [f32; 2], radius: f32, start_deg: f32, end_deg: f32, segments: usize) -> Prim {
    let segments = segments.max(2);
    let mut points = Vec::with_capacity(segments + 1);
    for step in 0..=segments {
        let t = step as f32 / segments as f32;
        let angle = (start_deg + (end_deg - start_deg) * t).to_radians();
        points.push([
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]);
    }
    Prim::Path { points }
}

/// Sample a cubic Bezier curve.
fn cubic(p0: [f32; 2], c0: [f32; 2], c1: [f32; 2], p1: [f32; 2], segments: usize) -> Prim {
    let segments = segments.max(2);
    let mut points = Vec::with_capacity(segments + 1);
    for step in 0..=segments {
        let t = step as f32 / segments as f32;
        let u = 1.0 - t;
        points.push([
            u * u * u * p0[0]
                + 3.0 * u * u * t * c0[0]
                + 3.0 * u * t * t * c1[0]
                + t * t * t * p1[0],
            u * u * u * p0[1]
                + 3.0 * u * u * t * c0[1]
                + 3.0 * u * t * t * c1[1]
                + t * t * t * p1[1],
        ]);
    }
    Prim::Path { points }
}

/// Two-barb chevron whose vertex sits on `tip` and which opens away from `toward`.
///
/// `toward` is the direction the arrow points; it is normalised here so callers
/// can pass arc tangents directly.
fn arrow_head(tip: [f32; 2], toward: [f32; 2], length: f32, half_angle_deg: f32) -> Prim {
    let magnitude = (toward[0] * toward[0] + toward[1] * toward[1]).sqrt();
    let (dx, dy) = if magnitude > f32::EPSILON {
        (toward[0] / magnitude, toward[1] / magnitude)
    } else {
        (0.0, -1.0)
    };
    let back = (-dy).atan2(-dx);
    let half = half_angle_deg.to_radians();
    Prim::Path {
        points: vec![
            [
                tip[0] + length * (back + half).cos(),
                tip[1] + length * (back + half).sin(),
            ],
            tip,
            [
                tip[0] + length * (back - half).cos(),
                tip[1] + length * (back - half).sin(),
            ],
        ],
    }
}

/// The full primitive list for one icon, in the normalised unit square.
///
/// Pure data: no painter, no GPU and no fonts, which is what makes the glyphs
/// testable. Coordinates stay inside `0..1` including stroke extents.
pub(super) fn geometry(icon: Icon) -> Vec<Prim> {
    match icon {
        // Pointer arrow.
        Icon::Select => vec![closed(&[
            [0.30, 0.10],
            [0.30, 0.80],
            [0.44, 0.66],
            [0.55, 0.90],
            [0.67, 0.85],
            [0.56, 0.62],
            [0.74, 0.62],
        ])],
        // Pencil body, ferrule and sharpened edge.
        Icon::Edit => vec![
            closed(&[[0.30, 0.74], [0.62, 0.16], [0.80, 0.30], [0.48, 0.88]]),
            seg([0.56, 0.24], [0.74, 0.38]),
            seg([0.38, 0.72], [0.56, 0.86]),
        ],
        // Bezier pen: one curve between two editable anchors.
        Icon::Pen => vec![
            cubic([0.18, 0.80], [0.30, 0.18], [0.70, 0.82], [0.84, 0.22], 20),
            dot([0.18, 0.80], 0.07),
            dot([0.84, 0.22], 0.07),
        ],
        // Rectangle outline with a draggable corner handle.
        Icon::Rectangle => vec![
            stroke_rect([0.14, 0.20], [0.86, 0.80]),
            dot([0.86, 0.80], 0.055),
        ],
        // Two panel edges joined by a locking stitch.
        Icon::Sew => vec![
            seg([0.26, 0.14], [0.26, 0.86]),
            seg([0.74, 0.14], [0.74, 0.86]),
            seg([0.26, 0.30], [0.74, 0.42]),
            seg([0.74, 0.46], [0.26, 0.58]),
            seg([0.26, 0.62], [0.74, 0.74]),
        ],
        // Four-way translate handle with a centred origin.
        Icon::Move3d => vec![
            seg([0.14, 0.50], [0.86, 0.50]),
            arrow_head([0.10, 0.50], [-1.0, 0.0], 0.16, 30.0),
            arrow_head([0.90, 0.50], [1.0, 0.0], 0.16, 30.0),
            seg([0.50, 0.14], [0.50, 0.86]),
            arrow_head([0.50, 0.10], [0.0, -1.0], 0.16, 30.0),
            arrow_head([0.50, 0.90], [0.0, 1.0], 0.16, 30.0),
            dot([0.50, 0.50], 0.06),
        ],
        // Rotation ring around a vertical axis.
        Icon::Rotate3d => vec![
            arc([0.50, 0.50], 0.30, 140.0, 420.0, 28),
            arrow_head([0.65, 0.76], [-0.866, 0.5], 0.16, 30.0),
            seg([0.50, 0.14], [0.50, 0.86]),
        ],
        // Open hand grabbing fabric.
        Icon::Grab => vec![closed(&[
            [0.74, 0.82],
            [0.74, 0.48],
            [0.68, 0.42],
            [0.60, 0.42],
            [0.60, 0.32],
            [0.52, 0.32],
            [0.52, 0.26],
            [0.44, 0.26],
            [0.44, 0.30],
            [0.36, 0.30],
            [0.36, 0.42],
            [0.28, 0.42],
            [0.28, 0.60],
            [0.16, 0.66],
            [0.30, 0.82],
        ])],
        // Arrow curving back over the top with its head at the left.
        Icon::Undo => vec![
            arc([0.50, 0.58], 0.30, 180.0, 360.0, 20),
            arrow_head([0.20, 0.58], [0.0, 1.0], 0.16, 30.0),
        ],
        // Mirror of the undo arrow, head at the right.
        Icon::Redo => vec![
            arc([0.50, 0.58], 0.30, 180.0, 360.0, 20),
            arrow_head([0.80, 0.58], [0.0, 1.0], 0.16, 30.0),
        ],
        // One piece offset behind another.
        Icon::Duplicate => vec![
            stroke_rect([0.14, 0.14], [0.62, 0.62]),
            stroke_rect([0.38, 0.38], [0.86, 0.86]),
        ],
        // A shape and its reflection across a symmetry axis.
        Icon::Mirror => vec![
            seg([0.50, 0.12], [0.50, 0.88]),
            closed(&[[0.40, 0.28], [0.40, 0.72], [0.14, 0.50]]),
            closed(&[[0.60, 0.28], [0.60, 0.72], [0.86, 0.50]]),
        ],
        // Three-by-three draft grid.
        Icon::Grid => vec![
            stroke_rect([0.14, 0.14], [0.86, 0.86]),
            seg([0.38, 0.14], [0.38, 0.86]),
            seg([0.62, 0.14], [0.62, 0.86]),
            seg([0.14, 0.38], [0.86, 0.38]),
            seg([0.14, 0.62], [0.86, 0.62]),
        ],
        // Magnet: snaps markers onto a point.
        Icon::Snap => vec![
            seg([0.26, 0.74], [0.26, 0.44]),
            seg([0.74, 0.74], [0.74, 0.44]),
            arc([0.50, 0.44], 0.24, 180.0, 360.0, 16),
            seg([0.16, 0.74], [0.36, 0.74]),
            seg([0.64, 0.74], [0.84, 0.74]),
        ],
        // Corner brackets closing on the content, plus a centre mark.
        Icon::Fit => vec![
            path(&[[0.16, 0.32], [0.16, 0.16], [0.32, 0.16]]),
            path(&[[0.68, 0.16], [0.84, 0.16], [0.84, 0.32]]),
            path(&[[0.84, 0.68], [0.84, 0.84], [0.68, 0.84]]),
            path(&[[0.32, 0.84], [0.16, 0.84], [0.16, 0.68]]),
            dot([0.50, 0.50], 0.055),
        ],
        // Isometric box.
        Icon::ViewIso => vec![
            closed(&[[0.50, 0.12], [0.86, 0.32], [0.50, 0.52], [0.14, 0.32]]),
            closed(&[[0.14, 0.32], [0.50, 0.52], [0.50, 0.88], [0.14, 0.68]]),
            closed(&[[0.86, 0.32], [0.50, 0.52], [0.50, 0.88], [0.86, 0.68]]),
        ],
        // Face-on elevation with its centre mark.
        Icon::ViewFront => vec![
            stroke_rect([0.22, 0.20], [0.78, 0.80]),
            dot([0.50, 0.50], 0.07),
        ],
        // Same elevation seen from behind: the near edge becomes a hidden line.
        Icon::ViewBack => vec![
            stroke_rect([0.22, 0.20], [0.78, 0.80]),
            seg([0.22, 0.50], [0.42, 0.50]),
            seg([0.58, 0.50], [0.78, 0.50]),
        ],
        // Side elevation with the view direction leaving it.
        Icon::ViewLeft => vec![
            stroke_rect([0.62, 0.18], [0.86, 0.82]),
            seg([0.50, 0.50], [0.18, 0.50]),
            arrow_head([0.16, 0.50], [-1.0, 0.0], 0.16, 30.0),
        ],
        Icon::ViewRight => vec![
            stroke_rect([0.14, 0.18], [0.38, 0.82]),
            seg([0.50, 0.50], [0.82, 0.50]),
            arrow_head([0.84, 0.50], [1.0, 0.0], 0.16, 30.0),
        ],
        // Plan view: a flattened plate with the direction looking down onto it.
        Icon::ViewTop => vec![
            stroke_rect([0.18, 0.38], [0.82, 0.64]),
            seg([0.50, 0.14], [0.50, 0.34]),
            arrow_head([0.50, 0.36], [0.0, 1.0], 0.16, 30.0),
        ],
        // Gear: hub ring with six teeth.
        Icon::Settings => {
            let mut prims = vec![circle([0.50, 0.50], 0.22)];
            for index in 0..6 {
                let angle = index as f32 * 60.0_f32.to_radians();
                prims.push(seg(
                    [0.50 + 0.28 * angle.cos(), 0.50 + 0.28 * angle.sin()],
                    [0.50 + 0.42 * angle.cos(), 0.50 + 0.42 * angle.sin()],
                ));
            }
            prims
        }
        // Half-shaded disc: light and dark workspace themes.
        Icon::Theme => vec![
            circle([0.50, 0.50], 0.30),
            seg([0.50, 0.20], [0.50, 0.80]),
            seg([0.56, 0.36], [0.76, 0.36]),
            seg([0.56, 0.50], [0.78, 0.50]),
            seg([0.56, 0.64], [0.76, 0.64]),
        ],
        // Draped panel with two fold lines.
        Icon::Fabric => vec![
            closed(&[
                [0.14, 0.30],
                [0.32, 0.22],
                [0.50, 0.30],
                [0.68, 0.22],
                [0.86, 0.30],
                [0.86, 0.70],
                [0.68, 0.78],
                [0.50, 0.70],
                [0.32, 0.78],
                [0.14, 0.70],
            ]),
            seg([0.34, 0.27], [0.34, 0.73]),
            seg([0.66, 0.27], [0.66, 0.73]),
        ],
        // Play triangle.
        Icon::Simulate => vec![closed(&[[0.30, 0.18], [0.30, 0.82], [0.84, 0.50]])],
        // Single solver step.
        Icon::Step => vec![
            closed(&[[0.22, 0.20], [0.22, 0.80], [0.64, 0.50]]),
            seg([0.76, 0.20], [0.76, 0.80]),
        ],
        // Circular arrow returning to the start.
        Icon::Reset => vec![
            arc([0.50, 0.50], 0.30, 20.0, 320.0, 26),
            arrow_head([0.73, 0.31], [0.643, 0.766], 0.16, 30.0),
        ],
        // Tray with the payload leaving it.
        Icon::Export => vec![
            path(&[[0.18, 0.60], [0.18, 0.84], [0.82, 0.84], [0.82, 0.60]]),
            seg([0.50, 0.66], [0.50, 0.24]),
            arrow_head([0.50, 0.16], [0.0, -1.0], 0.16, 30.0),
        ],
        // Magnifier.
        Icon::Search => vec![circle([0.44, 0.42], 0.24), seg([0.61, 0.59], [0.84, 0.84])],
        // Information marker.
        Icon::Info => vec![
            circle([0.50, 0.50], 0.34),
            dot([0.50, 0.31], 0.05),
            seg([0.50, 0.44], [0.50, 0.70]),
        ],
        // Warning triangle.
        Icon::Warning => vec![
            closed(&[[0.50, 0.12], [0.90, 0.86], [0.10, 0.86]]),
            seg([0.50, 0.42], [0.50, 0.62]),
            dot([0.50, 0.72], 0.05),
        ],
        // Seam in section: two panels meeting, with stitch ticks.
        Icon::Seam => vec![
            path(&[[0.12, 0.30], [0.50, 0.66], [0.88, 0.30]]),
            seg([0.31, 0.46], [0.22, 0.62]),
            seg([0.50, 0.66], [0.50, 0.82]),
            seg([0.69, 0.46], [0.78, 0.62]),
        ],
        // Notch cut into a panel edge.
        Icon::Notch => vec![
            seg([0.12, 0.36], [0.38, 0.36]),
            seg([0.62, 0.36], [0.88, 0.36]),
            path(&[[0.38, 0.36], [0.50, 0.64], [0.62, 0.36]]),
        ],
        // Rising tonal ramp.
        Icon::Gradient => vec![
            seg([0.24, 0.34], [0.24, 0.66]),
            seg([0.41, 0.27], [0.41, 0.73]),
            seg([0.59, 0.20], [0.59, 0.80]),
            seg([0.76, 0.13], [0.76, 0.87]),
        ],
    }
}

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

/// Largest square centred inside `rect`.
///
/// Backing the glyph with a square rather than the raw rectangle keeps stroke
/// weight and proportions identical in a wide toolbar button and a square
/// icon-only button.
fn square_for(rect: Rect) -> Rect {
    let side = rect.width().min(rect.height()).max(0.0);
    Rect::from_center_size(rect.center(), Vec2::splat(side))
}

/// Map one normalised point onto a painter rectangle.
fn map(square: Rect, point: [f32; 2]) -> Pos2 {
    egui::pos2(
        square.left() + point[0] * square.width(),
        square.top() + point[1] * square.height(),
    )
}

/// Draw `prims` into `rect`, honouring the caller's colour and stroke weight.
fn paint(painter: &egui::Painter, rect: Rect, prims: &[Prim], color: Color32, stroke: f32) {
    let side = rect.width().min(rect.height());
    if !side.is_finite() || side <= 0.0 {
        return;
    }
    let stroke = if stroke.is_finite() && stroke > 0.0 {
        stroke
    } else {
        1.0
    };
    let style = Stroke::new(stroke, color);
    for prim in prims {
        match prim {
            Prim::Line { a, b } => {
                painter.line_segment([map(rect, *a), map(rect, *b)], style);
            }
            Prim::Path { points } if points.len() >= 2 => {
                let mapped: Vec<Pos2> = points.iter().map(|p| map(rect, *p)).collect();
                painter.add(Shape::line(mapped, style));
            }
            Prim::Path { .. } => {}
            Prim::Circle { center, radius } => {
                painter.circle_stroke(map(rect, *center), radius * side, style);
            }
            Prim::Rect { min, max } => {
                painter.rect_stroke(
                    Rect::from_min_max(map(rect, *min), map(rect, *max)),
                    CornerRadius::same(2),
                    style,
                    StrokeKind::Inside,
                );
            }
            Prim::Dot { center, radius } => {
                painter.circle_filled(map(rect, *center), (radius * side).max(1.0), color);
            }
        }
    }
}

/// Draw one glyph inside `rect`.
pub(super) fn glyph(
    painter: &egui::Painter,
    rect: egui::Rect,
    icon: Icon,
    color: Color32,
    stroke: f32,
) {
    let square = square_for(rect);
    if square.width() <= 0.0 {
        return;
    }
    paint(painter, square, &geometry(icon), color, stroke);
}

/// Background and icon colours for one button state.
fn button_fill(visuals: &egui::Visuals, active: bool, hovered: bool) -> Color32 {
    if active {
        visuals.selection.bg_fill
    } else if hovered {
        visuals.widgets.hovered.bg_fill
    } else {
        Color32::TRANSPARENT
    }
}

fn button_ink(visuals: &egui::Visuals, active: bool) -> Color32 {
    if active {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    }
}

/// Icon plus label, sized to its own text. Returns the click response.
pub(super) fn tool_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    active: bool,
    tip: &str,
) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let ink = ui.visuals().text_color();
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, ink);
    let height = ui.spacing().interact_size.y.max(26.0);
    let padding = 6.0;
    let icon_size = height - 10.0;
    let width = padding + icon_size + 6.0 + galley.size().x + padding;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let fill = button_fill(visuals, active, response.hovered());
        if fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
        }
        let icon_rect = Rect::from_min_size(
            egui::pos2(rect.left() + padding, rect.center().y - icon_size * 0.5),
            Vec2::splat(icon_size),
        );
        glyph(
            ui.painter(),
            icon_rect,
            icon,
            button_ink(visuals, active),
            1.6,
        );
        ui.painter().galley(
            egui::pos2(
                icon_rect.right() + 6.0,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            ink,
        );
    }
    with_tip(response, tip)
}

/// Square icon-only button for dense rows. Returns the click response.
pub(super) fn icon_only(
    ui: &mut egui::Ui,
    icon: Icon,
    size: f32,
    active: bool,
    tip: &str,
) -> Response {
    let size = if size.is_finite() && size > 0.0 {
        size
    } else {
        20.0
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let fill = button_fill(visuals, active, response.hovered());
        if fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
        }
        glyph(
            ui.painter(),
            rect.shrink(size * 0.18),
            icon,
            button_ink(visuals, active),
            1.6,
        );
    }
    with_tip(response, tip)
}

fn with_tip(response: Response, tip: &str) -> Response {
    if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Points that bound a primitive, including stroke extents.
    fn extent_points(prim: &Prim) -> Vec<[f32; 2]> {
        match prim {
            Prim::Line { a, b } => vec![*a, *b],
            Prim::Path { points } => points.clone(),
            Prim::Circle { center, radius } => vec![
                [center[0] - radius, center[1] - radius],
                [center[0] + radius, center[1] + radius],
            ],
            Prim::Rect { min, max } => vec![*min, *max, [min[0], max[1]], [max[0], min[1]]],
            Prim::Dot { center, radius } => vec![
                [center[0] - radius, center[1] - radius],
                [center[0] + radius, center[1] + radius],
            ],
        }
    }

    /// A primitive that would paint nothing visible.
    fn is_degenerate(prim: &Prim) -> bool {
        match prim {
            Prim::Line { a, b } => squared_distance(a, b) <= 1e-6,
            Prim::Path { points } => {
                points.len() < 2
                    || points
                        .iter()
                        .all(|p| squared_distance(p, &points[0]) <= 1e-6)
            }
            Prim::Circle { radius, .. } => *radius <= 1e-4,
            Prim::Rect { min, max } => (max[0] - min[0]) <= 1e-4 || (max[1] - min[1]) <= 1e-4,
            Prim::Dot { radius, .. } => *radius <= 1e-4,
        }
    }

    fn squared_distance(a: &[f32; 2], b: &[f32; 2]) -> f32 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        dx * dx + dy * dy
    }

    #[test]
    fn every_icon_draws_something() {
        for icon in ALL {
            let prims = geometry(icon);
            assert!(!prims.is_empty(), "{icon:?} generates no geometry");
            for prim in &prims {
                assert!(
                    !is_degenerate(prim),
                    "{icon:?} contains a primitive that would not paint: {prim:?}"
                );
            }
        }
    }

    #[test]
    fn every_primitive_stays_inside_the_unit_square() {
        for icon in ALL {
            for prim in geometry(icon) {
                for point in extent_points(&prim) {
                    assert!(
                        (0.0..=1.0).contains(&point[0]) && (0.0..=1.0).contains(&point[1]),
                        "{icon:?} leaves the unit square at {point:?} ({prim:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn primitive_counts_are_stable() {
        // Recorded per icon. If a glyph silently loses its geometry (or gains a
        // stray primitive), the count changes and this fails instead of the
        // blank button reaching the user.
        let expected: [(Icon, usize); 34] = [
            (Icon::Select, 1),
            (Icon::Edit, 3),
            (Icon::Pen, 3),
            (Icon::Rectangle, 2),
            (Icon::Sew, 5),
            (Icon::Move3d, 7),
            (Icon::Rotate3d, 3),
            (Icon::Grab, 1),
            (Icon::Undo, 2),
            (Icon::Redo, 2),
            (Icon::Duplicate, 2),
            (Icon::Mirror, 3),
            (Icon::Grid, 5),
            (Icon::Snap, 5),
            (Icon::Fit, 5),
            (Icon::ViewIso, 3),
            (Icon::ViewFront, 2),
            (Icon::ViewBack, 3),
            (Icon::ViewLeft, 3),
            (Icon::ViewRight, 3),
            (Icon::ViewTop, 3),
            (Icon::Settings, 7),
            (Icon::Theme, 5),
            (Icon::Fabric, 3),
            (Icon::Simulate, 1),
            (Icon::Step, 2),
            (Icon::Reset, 2),
            (Icon::Export, 3),
            (Icon::Search, 2),
            (Icon::Info, 3),
            (Icon::Warning, 3),
            (Icon::Seam, 4),
            (Icon::Notch, 3),
            (Icon::Gradient, 4),
        ];
        // A new variant without a recorded count is itself a regression.
        assert_eq!(expected.len(), ALL.len());
        let mut total = 0;
        for (slot, (icon, count)) in expected.iter().enumerate() {
            assert_eq!(*icon, ALL[slot], "recorded icons and ALL drifted apart");
            assert!(*count > 0, "{icon:?} is recorded as empty");
            let actual = geometry(*icon).len();
            assert_eq!(actual, *count, "{icon:?} primitive count changed");
            total += actual;
        }
        assert!(total >= 100, "the palette collapsed to {total} primitives");
    }

    #[test]
    fn the_degenerate_guard_rejects_bad_geometry() {
        // The guard must actually fail on a glyph that would render as nothing,
        // otherwise the tests above prove little.
        assert!(is_degenerate(&Prim::Line {
            a: [0.5, 0.5],
            b: [0.5, 0.5],
        }));
        assert!(is_degenerate(&Prim::Path { points: vec![] }));
        assert!(is_degenerate(&Prim::Path {
            points: vec![[0.4, 0.4], [0.4, 0.4]],
        }));
        assert!(is_degenerate(&Prim::Circle {
            center: [0.5, 0.5],
            radius: 0.0,
        }));
        assert!(is_degenerate(&Prim::Rect {
            min: [0.2, 0.2],
            max: [0.2, 0.8],
        }));
        assert!(is_degenerate(&Prim::Dot {
            center: [0.5, 0.5],
            radius: 0.0,
        }));
        // ...while accepting a healthy primitive of each kind.
        assert!(!is_degenerate(&Prim::Line {
            a: [0.2, 0.2],
            b: [0.8, 0.8],
        }));
        assert!(!is_degenerate(&circle([0.5, 0.5], 0.3)));
        assert!(!is_degenerate(&stroke_rect([0.2, 0.2], [0.8, 0.8])));
        assert!(!is_degenerate(&dot([0.5, 0.5], 0.05)));
    }

    #[test]
    fn glyphs_are_painted_into_a_centred_square_inside_the_caller_rect() {
        let wide = Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(100.0, 40.0));
        let square = square_for(wide);
        assert_eq!(square.width(), 40.0);
        assert_eq!(square.height(), 40.0);
        assert_eq!(square.center(), wide.center());

        for icon in ALL {
            for prim in geometry(icon) {
                for point in extent_points(&prim) {
                    let mapped = map(square, point);
                    assert!(
                        wide.contains(mapped),
                        "{icon:?} paints outside the rect it was given"
                    );
                }
            }
        }
    }

    #[test]
    fn palette_widgets_allocate_clickable_rects() {
        // Headless: layout only, no window, no GPU.
        let ctx = egui::Context::default();
        let mut sizes = Vec::new();
        // `run_ui` needs a root `Ui`; the returned output carries font texture
        // deltas that must be cleared, since there is no renderer to apply them.
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let labelled = tool_button(ui, Icon::Sew, "Sew", false, "Stitch two edges");
            let compact = icon_only(ui, Icon::Fit, 20.0, true, "Fit the view");
            sizes.push((labelled.rect.size(), compact.rect.size()));
        });
        output.drop_without_applying_deltas();
        let (labelled, compact) = sizes[0];
        assert!(labelled.x > 0.0 && labelled.y > 0.0);
        assert!(labelled.x > labelled.y, "the label should widen the button");
        assert_eq!(compact, Vec2::splat(20.0));
    }
}
