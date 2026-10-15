//! View bars for the 3D garment window and the 2D pattern board.
//!
//! Presentation only. The 3D bar moves the orbit camera, frames the scene and
//! switches the projection; the 2D bar moves the board's view transform. Neither
//! writes material coordinates, rest geometry, grain, masses or cache signatures,
//! and no control here commits an authoring command.
use super::{authoring_ui::Tool, board::Board, *};
use crate::render::{Camera, PresetView};
use glam::{DVec3, Vec3};

/// World-space bounds the 3D bar can frame, in metres.
#[derive(Clone, Copy, Default)]
pub(super) struct ViewBounds {
    /// The garment plus the mannequin.
    pub all: Option<(Vec3, Vec3)>,
    /// The selected panel, when it is part of the compiled garment.
    pub selection: Option<(Vec3, Vec3)>,
}

/// Bounds of a point set, or `None` when it is empty or entirely non-finite.
fn bounds_of<'a>(points: impl IntoIterator<Item = &'a DVec3>) -> Option<(Vec3, Vec3)> {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    let mut seen = false;
    for p in points {
        let p = p.as_vec3();
        if !p.is_finite() {
            continue;
        }
        lo = lo.min(p);
        hi = hi.max(p);
        seen = true;
    }
    seen.then_some((lo, hi))
}

/// Centre and bounding-sphere radius of a box.
fn center_radius(bounds: Option<(Vec3, Vec3)>) -> Option<(Vec3, f32)> {
    bounds.map(|(lo, hi)| ((lo + hi) * 0.5, ((hi - lo) * 0.5).length().max(1e-3)))
}

/// On-screen binding hint for the 3D viewport, matching the active tool. Every
/// arm ends with the same navigation bindings, because the 3D camera responds to
/// them in every tool.
pub(super) fn hint_3d(tool: Tool) -> &'static str {
    match tool {
        Tool::Place => {
            "G: move · R: rotate · X/Y/Z rings · Shift: 15° · Esc: cancel · right drag: orbit · middle drag: pan · wheel: zoom toward the cursor"
        }
        Tool::Sew => {
            "Click boundaries to sew · right drag: orbit · middle drag: pan · wheel: zoom toward the cursor"
        }
        Tool::Grab => {
            "Left: grab the fabric · right drag: orbit · middle drag: pan · wheel: zoom toward the cursor"
        }
        Tool::Edit | Tool::Draw | Tool::Rectangle | Tool::InternalLine => {
            "Select a boundary · right drag: orbit · middle drag: pan · wheel: zoom toward the cursor"
        }
    }
}

/// On-screen binding hint for the 2D board navigation.
pub(super) const HINT_2D_VIEW: &str = "Alt + left drag: rotate · middle or right drag: pan · wheel: zoom toward the cursor · [ and ]: rotate 15° · Home: reset view";

impl Atelier {
    /// Framing bounds for the 3D view bar.
    pub(super) fn view_bounds(&self) -> ViewBounds {
        let garment = bounds_of(self.scene.state.x.iter());
        let mannequin = bounds_of(self.scene.collider.surface.vertices.iter());
        let all = match (garment, mannequin) {
            (Some((a, b)), Some((c, d))) => Some((a.min(c), b.max(d))),
            (garment, mannequin) => garment.or(mannequin),
        };
        let selection = self.authoring.selected.as_ref().and_then(|id| {
            let index = self
                .project
                .template
                .panels
                .iter()
                .position(|p| &p.id == id)?;
            let range = self.scene.garment.panels.get(index)?;
            bounds_of(self.scene.state.x[range.vertices.clone()].iter())
        });
        ViewBounds { all, selection }
    }
    /// Draw the 3D view bar for this workspace. Call it once per frame, above
    /// the 3D canvas; the frame request is applied when the canvas rectangle is
    /// known.
    pub(super) fn show_view_bar(&mut self, ui: &mut egui::Ui) {
        let bounds = self.view_bounds();
        view_bar(ui, &mut self.camera, bounds);
    }
}

/// 3D view bar: preset directions, framing, restore and the projection toggle.
pub(super) fn view_bar(ui: &mut egui::Ui, camera: &mut Camera, bounds: ViewBounds) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("View").small());
        for preset in PresetView::ALL {
            if ui
                .selectable_label(false, preset.label())
                .on_hover_text(format!(
                    "Look at the model from the {} direction",
                    preset.label()
                ))
                .clicked()
            {
                camera.apply_preset(preset);
            }
        }
        ui.separator();
        let all = center_radius(bounds.all);
        if ui
            .add_enabled(all.is_some(), egui::Button::new("Frame all"))
            .on_hover_text("Frame the garment and the mannequin")
            .clicked()
            && let Some((center, radius)) = all
        {
            camera.request_frame(center, radius);
        }
        let selection = center_radius(bounds.selection);
        if ui
            .add_enabled(selection.is_some(), egui::Button::new("Frame selection"))
            .on_hover_text("Frame the selected piece")
            .clicked()
            && let Some((center, radius)) = selection
        {
            camera.request_frame(center, radius);
        }
        ui.separator();
        if ui
            .add_enabled(camera.has_saved_pose(), egui::Button::new("Restore"))
            .on_hover_text("Swap back to the view from before the last preset or frame")
            .clicked()
        {
            camera.restore_pose();
        }
        let projection = if camera.orthographic {
            "Orthographic"
        } else {
            "Perspective"
        };
        if ui
            .selectable_label(camera.orthographic, projection)
            .on_hover_text("Switch between the matched perspective and orthographic projections")
            .clicked()
        {
            camera.toggle_projection();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.small(format!("distance {:.2} m", camera.distance));
        });
    });
}

/// 2D board bar: view rotation, fit and reset. Presentation only.
pub(super) fn board_bar(ui: &mut egui::Ui, board: &mut Board) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Board view").small());
        if ui
            .button("Rotate -90°")
            .on_hover_text("Turn the board a quarter turn anticlockwise")
            .clicked()
        {
            board.rotate_view_by(-90.);
        }
        if ui
            .button("Rotate +90°")
            .on_hover_text("Turn the board a quarter turn clockwise")
            .clicked()
        {
            board.rotate_view_by(90.);
        }
        let mut degrees = board.view_rotation_degrees();
        if ui
            .add(
                egui::DragValue::new(&mut degrees)
                    .speed(1.)
                    .range(-180.0..=180.0)
                    .suffix("°"),
            )
            .on_hover_text("Board view angle in degrees")
            .changed()
        {
            board.set_view_rotation_degrees(degrees);
        }
        ui.separator();
        if ui
            .button("Fit all")
            .on_hover_text("Fit every piece in the window")
            .clicked()
        {
            board.request_fit();
        }
        if ui
            .button("Reset view")
            .on_hover_text("Reset the rotation and fit the pieces again")
            .clicked()
        {
            board.reset_view();
        }
        ui.separator();
        ui.label(egui::RichText::new(HINT_2D_VIEW).small());
    });
}
