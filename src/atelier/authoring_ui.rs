//! Egui adapter for the CPU authoring commands; no physical model lives here.
use super::*;
use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command, Edit, History},
    garment::{meshing::MeshingSettings, seam::BoundaryRef, template::Placement},
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Tool {
    Grab,
    #[default]
    Edit,
    Place,
    Draw,
    Rectangle,
    Sew,
}
#[derive(Default)]
pub(super) struct AuthoringUi {
    pub tool: Tool,
    pub selected: Option<String>,
    pub history: History,
    pub draft: Vec<DVec2>,
    pub tangents: Vec<DVec2>,
    pub board: super::board::Board,
    pub edge: Option<BoundaryRef>,
    pub hover_edge: Option<BoundaryRef>,
    pub replace: bool,
    pub ends: Vec<BoundaryRef>,
    pub reverse: bool,
    pub ease: bool,
    pub preview: Option<Placement>,
    pub preview_state: Option<marvelous_cloth::model::State>,
    pub error: Option<String>,
    pub fabric_draft: Option<super::fabrics_ui::FabricDraft>,
    pub rotate: bool,
    pub drag: Option<super::gizmo::Drag>,
    /// Contour point selected for numeric entry: (panel id, point index).
    pub anchor: Option<(String, usize)>,
    /// Panel the numeric size draft belongs to; `None` reloads from geometry.
    pub numeric_panel: Option<String>,
    pub width_mm: f64,
    pub height_mm: f64,
    pub numeric_anchor: Option<usize>,
    pub anchor_x_mm: f64,
    pub anchor_y_mm: f64,
}

pub(super) fn segment_distance(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let d = b - a;
    p.distance(a + d * ((p - a).dot(d) / d.length_sq().max(1e-12)).clamp(0., 1.))
}

impl Atelier {
    pub(super) fn install_edit(&mut self, edit: Edit) {
        if edit.reset_simulation {
            self.scene.state = edit.state_after(&self.scene.garment, &self.scene.state);
        }
        self.project.template = edit.template;
        self.project.state = None;
        self.project.mesh_signature = None;
        self.project.prearrange = false;
        if edit.reset_simulation {
            self.scene.garment = std::sync::Arc::new(edit.garment);
        }
        self.authoring.board.clear_preview();
        self.scene.next_dt = self.settings.dt;
        self.generation += 1;
        self.playing = false;
        self.pending = false;
        self.metrics = None;
        self.grab = None;
        self.drag_plane = None;
        self.authoring.preview = None;
        self.authoring.preview_state = None;
        self.authoring.error = None;
        self.authoring.fabric_draft = None;
        self.authoring.drag = None;
        // Force the numeric fields to re-read the edited geometry next frame.
        self.authoring.numeric_panel = None;
        self.authoring.numeric_anchor = None;
        self.authoring.ends.clear();
        if let Some(id) = edit.selected {
            self.authoring.selected = Some(id);
        }
        if !self
            .project
            .template
            .panels
            .iter()
            .any(|p| Some(&p.id) == self.authoring.selected.as_ref())
        {
            self.authoring.selected = self.project.template.panels.first().map(|p| p.id.clone());
        }
        self.status = if edit.reset_simulation {
            "Edit applied · unchanged panels retained · ready to simulate"
        } else {
            "2D layout changed · 3D state preserved"
        }
        .into();
    }
    pub(super) fn edit(&mut self, command: Command) -> bool {
        match authoring::apply(&self.project.template, command) {
            Ok(edit) => {
                self.authoring.history.record(self.project.template.clone());
                self.install_edit(edit);
                true
            }
            Err(e) => {
                self.authoring.error = Some(e.clone());
                self.status = e;
                false
            }
        }
    }
    fn undo_edit(&mut self, redo: bool) {
        self.cancel_preview();
        self.authoring.replace = false;
        self.authoring.draft.clear();
        self.authoring.tangents.clear();
        match self.authoring.history.restore(&self.project.template, redo) {
            Ok(edit) => self.install_edit(edit),
            Err(e) => self.status = e,
        }
    }
    pub(super) fn cancel_preview(&mut self) {
        if self.authoring.preview.take().is_some() {
            if let Some(state) = self.authoring.preview_state.take() {
                self.scene.state = state;
            }
            self.generation += 1;
        }
        self.authoring.drag = None;
    }
    pub(super) fn preview_placement(&mut self, placement: Placement) {
        let Some(id) = self.authoring.selected.as_ref() else {
            return;
        };
        let Some(range) = self.scene.garment.panels.iter().find(|p| &p.id == id) else {
            return;
        };
        if self.authoring.preview.is_none() {
            self.authoring.preview_state = Some(self.scene.state.clone());
            self.generation += 1;
        }
        self.playing = false;
        for i in range.vertices.clone() {
            let uv = self.scene.garment.mesh.uv[i];
            self.scene.state.x[i] = placement.transform(DVec3::new(uv.x, -uv.y, 0.));
            self.scene.state.v[i] = DVec3::ZERO;
        }
        self.authoring.preview = Some(placement);
    }
    pub(super) fn commit_placement(&mut self) -> bool {
        if let (Some(panel), Some(placement)) = (
            self.authoring.selected.clone(),
            self.authoring.preview.clone(),
        ) && !self.edit(Command::Place { panel, placement })
        {
            self.cancel_preview();
            return false;
        }
        true
    }
    pub(super) fn authoring_toolbar(&mut self, ui: &mut egui::Ui) {
        self.history_shortcuts(ui.ctx());
        ui.horizontal_wrapped(|ui| {
            // `key` is the smoke-target protocol name and must not change; `label`
            // is the translated text shown on the button.
            for (tool, key, label, rotate, help) in [
                (
                    Tool::Edit,
                    "Modifier",
                    "Edit",
                    false,
                    "Select, points and Bézier handles · 2D rotation with the round handle",
                ),
                (
                    Tool::Draw,
                    "Plume",
                    "Pen",
                    false,
                    "Click for a corner, click and drag for a curve",
                ),
                (
                    Tool::Rectangle,
                    "Rectangle",
                    "Rectangle",
                    false,
                    "Drag between two corners to create fabric",
                ),
                (
                    Tool::Sew,
                    "Coudre",
                    "Sew",
                    false,
                    "Choose two boundaries in the 2D or 3D views",
                ),
                (
                    Tool::Place,
                    "Déplacer 3D",
                    "Move 3D",
                    false,
                    "G · move the selected panel's flat arrangement",
                ),
                (
                    Tool::Place,
                    "Tourner 3D",
                    "Rotate 3D",
                    true,
                    "R · X/Y/Z rings · Shift for 15°",
                ),
                (
                    Tool::Grab,
                    "Saisir",
                    "Grab",
                    false,
                    "Drag the simulated cloth with the mouse",
                ),
            ] {
                if tool == Tool::Sew || (tool == Tool::Place && !rotate) || tool == Tool::Grab {
                    ui.separator();
                }
                let active = self.authoring.tool == tool
                    && (tool != Tool::Place || self.authoring.rotate == rotate);
                let button = ui.selectable_label(active, label).on_hover_text(help);
                self.smoke_target(key, button.rect.center());
                if button.clicked() {
                    self.set_authoring_tool(tool, rotate);
                }
            }
            ui.separator();
            let undo = ui.add_enabled(self.authoring.history.can_undo(), egui::Button::new("Undo"));
            self.smoke_target("undo", undo.rect.center());
            if undo.clicked() {
                self.undo_edit(false);
            }
            let redo = ui.add_enabled(self.authoring.history.can_redo(), egui::Button::new("Redo"));
            self.smoke_target("redo", redo.rect.center());
            if redo.clicked() {
                self.undo_edit(true);
            }
        });
    }
    pub(super) fn set_authoring_tool(&mut self, tool: Tool, rotate: bool) {
        if self.authoring.tool == tool && (tool != Tool::Place || self.authoring.rotate == rotate) {
            return;
        }
        self.cancel_preview();
        self.authoring.board.clear_preview();
        self.authoring.tool = tool;
        self.authoring.rotate = rotate;
        self.grab = None;
        self.drag_plane = None;
        if tool != Tool::Grab {
            self.playing = false;
            self.pending = false;
            self.generation += 1;
            self.metrics = None;
            self.status = if tool == Tool::Place {
                "Selected panel laid flat · other panels retained"
            } else {
                "Edit mode · simulation paused"
            }
            .into();
        }
    }
    fn history_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| !i.modifiers.command && !i.modifiers.alt && i.key_pressed(egui::Key::R)) {
            self.set_authoring_tool(Tool::Place, true);
        }
        if ctx.input(|i| !i.modifiers.command && !i.modifiers.alt && i.key_pressed(egui::Key::G)) {
            self.set_authoring_tool(Tool::Place, false);
        }
        if matches!(self.authoring.tool, Tool::Edit | Tool::Sew)
            && ctx.input(|i| i.key_pressed(egui::Key::Delete))
        {
            let seams = self.edge_seams();
            if seams.len() == 1 {
                self.remove_seam(seams[0].clone());
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            if self.authoring.tool == Tool::Draw && !self.authoring.draft.is_empty() {
                self.authoring.draft.pop();
                self.authoring.tangents.pop();
            } else {
                self.undo_edit(ctx.input(|i| i.modifiers.shift));
            }
        }
    }
    pub(super) fn authoring_properties(&mut self, ui: &mut egui::Ui) {
        self.existing_seams(ui);
        if self.authoring.tool == Tool::Sew || !self.authoring.ends.is_empty() {
            self.sewing_controls(ui);
        }
        if let Some(error) = &self.authoring.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        self.edge_properties(ui);
        ui.strong("Active panel");
        let Some(panel) = self
            .project
            .template
            .panels
            .iter()
            .find(|p| Some(&p.id) == self.authoring.selected.as_ref())
            .cloned()
        else {
            ui.small("Select a piece in the list or in a view. Pen / Rectangle to draw; Sew to assemble.");
            return;
        };
        ui.label(&panel.name);
        ui.small(format!(
            "{:.0} × {:.0} mm",
            panel.pattern.width * 1000.,
            panel.pattern.height * 1000.
        ));
        // Numeric entry stays in a draft until Apply, so one click is exactly one
        // document edit and one history entry. Millimetres convert by division:
        // 400 mm / 1000 is the exact f64 0.4, unlike a 0.001 multiplication.
        if self.authoring.numeric_panel.as_deref() != Some(panel.id.as_str()) {
            self.authoring.numeric_panel = Some(panel.id.clone());
            self.authoring.width_mm = panel.pattern.width * 1000.;
            self.authoring.height_mm = panel.pattern.height * 1000.;
            self.authoring.numeric_anchor = None;
        }
        ui.strong("Dimensions (mm)");
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut self.authoring.width_mm)
                    .speed(1.)
                    .suffix(" mm"),
            );
            ui.label("Height");
            ui.add(
                egui::DragValue::new(&mut self.authoring.height_mm)
                    .speed(1.)
                    .suffix(" mm"),
            );
        });
        if ui
            .button("Apply dimensions")
            .on_hover_text("Exact millimetre size; invalid values leave the document unchanged")
            .clicked()
        {
            let width = self.authoring.width_mm / 1000.;
            let height = self.authoring.height_mm / 1000.;
            self.edit(Command::Dimensions {
                panel: panel.id.clone(),
                width,
                height,
            });
        }
        if let Some(polygon) = panel.polygon.as_ref() {
            if let Some(k) = self
                .authoring
                .anchor
                .as_ref()
                .filter(|(id, k)| id == &panel.id && *k < polygon.points.len())
                .map(|(_, k)| *k)
            {
                if self.authoring.numeric_anchor != Some(k) {
                    self.authoring.numeric_anchor = Some(k);
                    self.authoring.anchor_x_mm = polygon.points[k].x * 1000.;
                    self.authoring.anchor_y_mm = polygon.points[k].y * 1000.;
                }
                ui.strong(format!("Point {} (mm)", k + 1));
                ui.horizontal(|ui| {
                    ui.label("X");
                    ui.add(
                        egui::DragValue::new(&mut self.authoring.anchor_x_mm)
                            .speed(1.)
                            .suffix(" mm"),
                    );
                    ui.label("Y");
                    ui.add(
                        egui::DragValue::new(&mut self.authoring.anchor_y_mm)
                            .speed(1.)
                            .suffix(" mm"),
                    );
                });
                if ui
                    .button("Apply point")
                    .on_hover_text(
                        "Material coordinates; the rest origin is re-based without moving \
                         the other points",
                    )
                    .clicked()
                {
                    let position = DVec2::new(
                        self.authoring.anchor_x_mm / 1000.,
                        self.authoring.anchor_y_mm / 1000.,
                    );
                    self.edit(Command::Anchor {
                        panel: panel.id.clone(),
                        anchor: k,
                        position,
                    });
                }
            } else {
                ui.small("Click a point on the 2D board to edit its exact position.");
            }
        }
        self.fabric_properties(ui, &panel);
        ui.horizontal(|ui| {
            let duplicate = ui.button("Duplicate");
            self.smoke_target("duplicate", duplicate.rect.center());
            if duplicate.clicked() {
                self.edit(Command::Duplicate {
                    panel: panel.id.clone(),
                });
            }
            if ui.button("Delete panel").clicked() {
                self.edit(Command::RemovePanel {
                    panel: panel.id.clone(),
                });
            }
        });
        if self.authoring.tool != Tool::Place {
            ui.small("Move 3D (G) / Rotate 3D (R) in the toolbar.");
            return;
        }
        ui.horizontal(|ui| {
            if ui
                .selectable_label(!self.authoring.rotate, "Move (G)")
                .clicked()
            {
                self.set_authoring_tool(Tool::Place, false);
            }
            if ui
                .selectable_label(self.authoring.rotate, "Rotate (R)")
                .clicked()
            {
                self.set_authoring_tool(Tool::Place, true);
            }
        });
        ui.small("Flat placement: this piece takes the shape of its pattern. The other pieces stay in place.");
        let focus = ui.button("Frame the panel");
        self.smoke_target("focus", focus.rect.center());
        if focus.clicked() {
            let placement = self.authoring.preview.as_ref().unwrap_or(&panel.placement);
            self.camera.target = placement
                .transform(DVec3::new(
                    panel.pattern.width * 0.5,
                    -panel.pattern.height * 0.5,
                    0.,
                ))
                .as_vec3();
            self.camera.distance =
                (panel.pattern.width.max(panel.pattern.height) * 3.).max(1.) as f32;
        }
        let mut p = self.authoring.preview.clone().unwrap_or(panel.placement);
        let before = p.clone();
        let mut changed = false;
        for (label, value, speed, suffix) in [
            ("X", &mut p.translation.x, 0.005, " m"),
            ("Y", &mut p.translation.y, 0.005, " m"),
            ("Z", &mut p.translation.z, 0.005, " m"),
            ("RX", &mut p.rotation_degrees.x, 1., "°"),
            ("RY", &mut p.rotation_degrees.y, 1., "°"),
            ("RZ", &mut p.rotation_degrees.z, 1., "°"),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                changed |= ui
                    .add(egui::DragValue::new(value).speed(speed).suffix(suffix))
                    .changed();
            });
        }
        if changed {
            // Numeric rotations use the same center pivot as the rings.
            let pivot = DVec3::new(panel.pattern.width * 0.5, -panel.pattern.height * 0.5, 0.);
            let rotated_pivot = p.transform(pivot) - p.translation;
            p.translation =
                before.transform(pivot) + (p.translation - before.translation) - rotated_pivot;
            self.preview_placement(p);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.authoring.preview.is_some(), egui::Button::new("Apply"))
                .clicked()
            {
                self.commit_placement();
            }
            if ui.button("Discard").clicked() {
                self.cancel_preview();
            }
        });
        ui.small("Drag an axis / ring · Shift: 15° angles · release: commit · Escape: cancel.");
        ui.separator();
    }
    pub(super) fn create_polygon(&mut self) {
        let command = Command::Polygon {
            tangents: self.authoring.tangents.clone(),
            points: self.authoring.draft.clone(),
            meshing: MeshingSettings {
                target_edge_m: self.mesh_mm * 0.001,
                anisotropy: self.anisotropy,
                axis_degrees: self.mesh_axis,
                ..Default::default()
            },
            fabric: self.project.template.fabrics[0].id.clone(),
            replace: self.authoring.replace,
        };
        if self.edit(command) {
            self.authoring.draft.clear();
            self.authoring.tangents.clear();
            self.authoring.replace = false;
            self.authoring.tool = Tool::Place;
        }
    }
    pub(super) fn sewing_controls(&mut self, ui: &mut egui::Ui) {
        if self.authoring.ends.len() == 2 {
            ui.colored_label(Color32::GOLD, "Preview only — seam not committed");
        }
        ui.small("Choose two boundaries in 2D or 3D; 0 ↔ 0, 1 ↔ 1.");
        let mut lengths = vec![];
        for (i, e) in self.authoring.ends.iter().enumerate() {
            ui.label(format!(
                "{} : {} / {}",
                if i == 0 { "A" } else { "B" },
                e.panel,
                e.edge.as_deref().unwrap_or("?")
            ));
            if let Some(range) = self.scene.garment.panels.iter().find(|p| p.id == e.panel)
                && let Some(ids) = e.edge.as_ref().and_then(|name| range.boundaries.get(name))
            {
                lengths.push(
                    ids.windows(2)
                        .map(|p| {
                            self.scene.garment.mesh.uv[p[0]]
                                .distance(self.scene.garment.mesh.uv[p[1]])
                        })
                        .sum::<f64>(),
                );
            }
        }
        if lengths.len() == 2 {
            ui.label(format!(
                "{:.1} / {:.1} mm · gap {:.1} mm",
                lengths[0] * 1000.,
                lengths[1] * 1000.,
                (lengths[0] - lengths[1]).abs() * 1000.
            ));
            if (lengths[0] - lengths[1]).abs() > 1e-5 && !self.authoring.ease {
                ui.colored_label(
                    Color32::GOLD,
                    "Different lengths: adjust the pattern or allow easing below.",
                );
            }
        }
        ui.checkbox(&mut self.authoring.reverse, "Reverse B's direction");
        ui.checkbox(&mut self.authoring.ease, "Allow easing (different lengths)");
        ui.horizontal(|ui| {
            let sew = ui.add_enabled(
                self.authoring.ends.len() == 2,
                egui::Button::new("Commit the seam"),
            );
            self.smoke_target("sew", sew.rect.center());
            if sew.clicked() {
                self.commit_seam();
            }
            if ui.button("Clear selection").clicked() {
                self.authoring.ends.clear();
            }
        });
        ui.separator();
    }
    pub(super) fn commit_seam(&mut self) -> bool {
        if self.authoring.ends.len() != 2 {
            return false;
        }
        if self.edit(Command::Sew {
            a: self.authoring.ends[0].clone(),
            b: self.authoring.ends[1].clone(),
            reverse: self.authoring.reverse,
            ease: self.authoring.ease,
        }) {
            self.status = "Seam created · Simulate to assemble the pieces".into();
            true
        } else {
            self.authoring.tool = Tool::Sew;
            false
        }
    }
    pub(super) fn prepare_simulation(&mut self) -> bool {
        if self.authoring.ends.len() == 2 && !self.commit_seam() {
            return false;
        }
        if !self.commit_placement() {
            return false;
        }
        self.authoring.tool = Tool::Grab;
        true
    }
}
