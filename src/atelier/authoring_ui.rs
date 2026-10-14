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
    pub rotate: bool,
    pub drag: Option<super::gizmo::Drag>,
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
        self.authoring.drag = None;
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
            "Édition validée · pièces inchangées conservées · prêt à simuler"
        } else {
            "Disposition 2D modifiée · état 3D conservé"
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
        ui.horizontal(|ui| {
            let previous = self.authoring.tool;
            for (tool, label) in [
                (Tool::Edit, "Modifier"),
                (Tool::Grab, "Saisir"),
                (Tool::Place, "Placer 3D"),
                (Tool::Draw, "Plume"),
                (Tool::Rectangle, "Rectangle"),
                (Tool::Sew, "Coudre"),
            ] {
                let button = ui.selectable_value(&mut self.authoring.tool, tool, label);
                self.smoke_target(label, button.rect.center());
            }
            if self.authoring.tool != previous {
                self.cancel_preview();
                self.authoring.board.clear_preview();
                self.grab = None;
                self.drag_plane = None;
                if self.authoring.tool != Tool::Grab {
                    self.playing = false;
                    self.pending = false;
                    self.generation += 1;
                    self.metrics = None;
                    self.status = if self.authoring.tool == Tool::Place {
                        "Placement du panneau sélectionné · autres pièces conservées"
                    } else {
                        "Mode édition · simulation en pause"
                    }
                    .into();
                }
            }
            ui.separator();
            let undo = ui.add_enabled(
                self.authoring.history.can_undo(),
                egui::Button::new("Annuler"),
            );
            self.smoke_target("undo", undo.rect.center());
            if undo.clicked() {
                self.undo_edit(false);
            }
            let redo = ui.add_enabled(
                self.authoring.history.can_redo(),
                egui::Button::new("Rétablir"),
            );
            self.smoke_target("redo", redo.rect.center());
            if redo.clicked() {
                self.undo_edit(true);
            }
        });
    }
    fn history_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
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
        if self.authoring.tool == Tool::Sew || !self.authoring.ends.is_empty() {
            self.sewing_controls(ui);
        }
        if let Some(error) = &self.authoring.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        self.edge_properties(ui);
        ui.strong("Panneau actif");
        let Some(panel) = self
            .project
            .template
            .panels
            .iter()
            .find(|p| Some(&p.id) == self.authoring.selected.as_ref())
            .cloned()
        else {
            ui.small("Sélectionner un patron en 2D ou en 3D.");
            return;
        };
        ui.label(&panel.name);
        ui.small(format!(
            "{:.0} × {:.0} mm",
            panel.pattern.width * 1000.,
            panel.pattern.height * 1000.
        ));
        ui.horizontal(|ui| {
            let duplicate = ui.button("Dupliquer");
            self.smoke_target("duplicate", duplicate.rect.center());
            if duplicate.clicked() {
                self.edit(Command::Duplicate {
                    panel: panel.id.clone(),
                });
            }
            if ui.button("Supprimer").clicked() {
                self.edit(Command::RemovePanel {
                    panel: panel.id.clone(),
                });
            }
        });
        if self.authoring.tool != Tool::Place {
            ui.small("Placer 3D : translation et rotation du panneau.");
            return;
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.authoring.rotate, false, "Déplacer");
            let turn = ui.selectable_value(&mut self.authoring.rotate, true, "Tourner");
            self.smoke_target("turn", turn.rect.center());
        });
        let focus = ui.button("Cadrer le panneau");
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
            self.preview_placement(p);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.authoring.preview.is_some(),
                    egui::Button::new("Appliquer"),
                )
                .clicked()
            {
                self.commit_placement();
            }
            if ui.button("Abandonner").clicked() {
                self.cancel_preview();
            }
        });
        ui.small("Gizmo : glisser un axe / anneau ; relâcher pour valider. Échap annule.");
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
            ui.colored_label(Color32::GOLD, "Aperçu seulement — couture à valider");
        }
        ui.small("Choisir deux bords en 2D ou 3D ; 0 ↔ 0, 1 ↔ 1.");
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
                "{:.1} / {:.1} mm · écart {:.1} mm",
                lengths[0] * 1000.,
                lengths[1] * 1000.,
                (lengths[0] - lengths[1]).abs() * 1000.
            ));
            if (lengths[0] - lengths[1]).abs() > 1e-5 && !self.authoring.ease {
                ui.colored_label(
                    Color32::GOLD,
                    "Longueurs différentes : ajuster le patron ou autoriser l'embu ci-dessous.",
                );
            }
        }
        ui.checkbox(&mut self.authoring.reverse, "Inverser le sens B");
        ui.checkbox(
            &mut self.authoring.ease,
            "Autoriser l'embu (longueurs différentes)",
        );
        ui.horizontal(|ui| {
            let sew = ui.add_enabled(
                self.authoring.ends.len() == 2,
                egui::Button::new("Valider la couture"),
            );
            self.smoke_target("sew", sew.rect.center());
            if sew.clicked() {
                self.commit_seam();
            }
            if ui.button("Effacer sélection").clicked() {
                self.authoring.ends.clear();
            }
        });
        ui.collapsing("Coutures existantes", |ui| {
            let mut remove = None;
            for seam in &self.project.template.seams {
                ui.horizontal(|ui| {
                    if ui.small_button("×").clicked() {
                        remove = Some(seam.id.clone());
                    }
                    ui.label(&seam.id)
                        .on_hover_text(format!("{} ↔ {}", seam.a.panel, seam.b.panel));
                });
            }
            if let Some(seam) = remove {
                self.edit(Command::RemoveSeam { seam });
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
            self.status = "Couture créée · Simuler pour assembler les pièces".into();
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
