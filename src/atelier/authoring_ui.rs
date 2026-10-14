//! Egui adapter for the CPU authoring commands; no physical model lives here.
use super::*;
use glam::{DVec2, DVec3};
use marvelous_cloth::{
    authoring::{self, Command, Edit, History},
    garment::{
        meshing::MeshingSettings,
        seam::{BoundaryRef, Side},
        template::Placement,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Tool {
    #[default]
    Grab,
    Place,
    Draw,
    Sew,
}
#[derive(Default)]
pub(super) struct AuthoringUi {
    pub tool: Tool,
    pub selected: Option<String>,
    pub history: History,
    pub draft: Vec<DVec2>,
    pub replace: bool,
    pub ends: Vec<BoundaryRef>,
    pub reverse: bool,
    pub ease: bool,
    pub preview: Option<Placement>,
    pub rotate: bool,
    pub drag: Option<super::gizmo::Drag>,
}

pub(super) fn segment_distance(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let d = b - a;
    p.distance(a + d * ((p - a).dot(d) / d.length_sq().max(1e-12)).clamp(0., 1.))
}

impl Atelier {
    pub(super) fn install_edit(&mut self, edit: Edit) {
        self.project.template = edit.template;
        self.project.state = None;
        self.project.mesh_signature = None;
        self.project.prearrange = false;
        self.scene.state = edit.garment.initial_state.clone();
        self.scene.garment = std::sync::Arc::new(edit.garment);
        self.scene.next_dt = self.settings.dt;
        self.generation += 1;
        self.playing = false;
        self.pending = false;
        self.metrics = None;
        self.grab = None;
        self.drag_plane = None;
        self.authoring.preview = None;
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
        self.status = "Édition validée · arrangement réinitialisé · prêt à simuler".into();
    }
    pub(super) fn edit(&mut self, command: Command) -> bool {
        match authoring::apply(&self.project.template, command) {
            Ok(edit) => {
                self.authoring.history.record(self.project.template.clone());
                self.install_edit(edit);
                true
            }
            Err(e) => {
                self.status = e;
                false
            }
        }
    }
    fn undo_edit(&mut self, redo: bool) {
        self.cancel_preview();
        match self.authoring.history.restore(&self.project.template, redo) {
            Ok(edit) => self.install_edit(edit),
            Err(e) => self.status = e,
        }
    }
    pub(super) fn cancel_preview(&mut self) {
        if self.authoring.preview.take().is_some() {
            self.scene.state = self.scene.garment.initial_state.clone();
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
            self.generation += 1;
        }
        self.playing = false;
        for i in range.vertices.clone() {
            let uv = self.scene.garment.mesh.uv[i];
            self.scene.state.x[i] = placement.transform(DVec3::new(uv.x, -uv.y, 0.));
        }
        self.authoring.preview = Some(placement);
    }
    pub(super) fn commit_placement(&mut self) {
        if let (Some(panel), Some(placement)) = (
            self.authoring.selected.clone(),
            self.authoring.preview.clone(),
        ) && !self.edit(Command::Place { panel, placement })
        {
            self.cancel_preview();
        }
    }
    pub(super) fn authoring_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let previous = self.authoring.tool;
            for (tool, label) in [
                (Tool::Grab, "Saisir"),
                (Tool::Place, "Placer 3D"),
                (Tool::Draw, "Dessiner"),
                (Tool::Sew, "Coudre"),
            ] {
                let button = ui.selectable_value(&mut self.authoring.tool, tool, label);
                self.smoke_target(label, button.rect.center());
            }
            if self.authoring.tool != previous {
                self.cancel_preview();
                self.grab = None;
                self.drag_plane = None;
                if self.authoring.tool != Tool::Grab {
                    self.playing = false;
                    self.pending = false;
                    self.generation += 1;
                    self.scene.state = self.scene.garment.initial_state.clone();
                    self.metrics = None;
                    self.status = "Mode édition · retour à l'arrangement de repos".into();
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
    pub(super) fn authoring_properties(&mut self, ui: &mut egui::Ui) {
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
        ui.horizontal(|ui| {
            let duplicate = ui.button("Dupliquer");
            self.smoke_target("duplicate", duplicate.rect.center());
            if duplicate.clicked() {
                self.edit(Command::Duplicate {
                    panel: panel.id.clone(),
                });
            }
            if ui.button("Supprimer").clicked() {
                self.edit(Command::RemovePanel { panel: panel.id });
            }
        });
        ui.separator();
    }
    fn create_polygon(&mut self) {
        let command = Command::Polygon {
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
            self.authoring.replace = false;
            self.authoring.tool = Tool::Place;
        }
    }
    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        ui.label("Contour · clics successifs, puis premier point");
        ui.small("Carré de 1 m · grille 50 mm · aimant 10 mm");
        let replace_button = ui.checkbox(
            &mut self.authoring.replace,
            "Nouveau vêtement (remplace les panneaux)",
        );
        self.smoke_target("replace", replace_button.rect.center());
        let size = ui.available_width().clamp(100., 500.);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
        let canvas = rect.shrink(10.);
        let scale = canvas.width();
        let point = |p: DVec2| canvas.min + Vec2::new(p.x as f32, p.y as f32) * scale;
        ui.painter()
            .rect_filled(rect, 4., Color32::from_rgb(46, 53, 59));
        for i in 0..=20 {
            let v = i as f32 / 20.;
            for line in [
                [
                    canvas.min + Vec2::new(v * scale, 0.),
                    canvas.min + Vec2::new(v * scale, scale),
                ],
                [
                    canvas.min + Vec2::new(0., v * scale),
                    canvas.min + Vec2::new(scale, v * scale),
                ],
            ] {
                ui.painter()
                    .line_segment(line, Stroke::new(0.5, Color32::from_gray(85)));
            }
        }
        for (key, uv) in [
            ("point0", DVec2::new(0.15, 0.15)),
            ("point1", DVec2::new(0.45, 0.15)),
            ("point2", DVec2::new(0.45, 0.55)),
            ("point3", DVec2::new(0.15, 0.55)),
        ] {
            self.smoke_target(key, point(uv));
        }
        let mut close = false;
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
            && canvas.contains(pos)
        {
            if self.authoring.draft.len() >= 3 && pos.distance(point(self.authoring.draft[0])) < 10.
            {
                close = true;
            } else if self.authoring.draft.len() < 64 {
                let v = (pos - canvas.min) / scale;
                let p = (DVec2::new(v.x as f64, v.y as f64) * 100.).round() / 100.;
                if self.authoring.draft.last() != Some(&p) {
                    self.authoring.draft.push(p);
                }
            }
        }
        for pair in self.authoring.draft.windows(2) {
            ui.painter().line_segment(
                [point(pair[0]), point(pair[1])],
                Stroke::new(2., Color32::LIGHT_BLUE),
            );
        }
        for (i, p) in self.authoring.draft.iter().enumerate() {
            ui.painter()
                .circle_filled(point(*p), if i == 0 { 5. } else { 3. }, Color32::GOLD);
        }
        if let (Some(last), Some(pos)) = (self.authoring.draft.last(), response.hover_pos()) {
            ui.painter()
                .line_segment([point(*last), pos], Stroke::new(1., Color32::GRAY));
        }
        ui.horizontal(|ui| {
            let create = ui.add_enabled(
                self.authoring.draft.len() >= 3,
                egui::Button::new("Créer le panneau"),
            );
            self.smoke_target("create", create.rect.center());
            close |= create.clicked();
            if ui.button("Retirer point").clicked() {
                self.authoring.draft.pop();
            }
            if ui.button("Effacer").clicked() {
                self.authoring.draft.clear();
            }
        });
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.authoring.draft.pop();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                close = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.authoring.draft.clear();
            }
        }
        if close {
            self.create_polygon();
        }
        ui.separator();
    }
    fn sewing_controls(&mut self, ui: &mut egui::Ui) {
        ui.small("Choisir deux bords dans les patrons ; 0 ↔ 0, 1 ↔ 1.");
        let mut lengths = vec![];
        for (i, e) in self.authoring.ends.iter().enumerate() {
            ui.label(format!(
                "{} : {} / {}",
                if i == 0 { "A" } else { "B" },
                e.panel,
                e.edge.as_deref().unwrap_or("?")
            ));
            if let Some(poly) = self
                .project
                .template
                .panels
                .iter()
                .find(|p| p.id == e.panel)
                .and_then(|p| p.polygon.as_ref())
                && let Some(k) = poly.edges.iter().position(|s| Some(s) == e.edge.as_ref())
            {
                lengths.push(poly.points[k].distance(poly.points[(k + 1) % poly.points.len()]));
            }
        }
        if lengths.len() == 2 {
            ui.label(format!(
                "{:.1} / {:.1} mm · écart {:.1} mm",
                lengths[0] * 1000.,
                lengths[1] * 1000.,
                (lengths[0] - lengths[1]).abs() * 1000.
            ));
        }
        ui.checkbox(&mut self.authoring.reverse, "Inverser le sens B");
        ui.checkbox(
            &mut self.authoring.ease,
            "Autoriser l'embu (longueurs différentes)",
        );
        ui.horizontal(|ui| {
            let sew = ui.add_enabled(
                self.authoring.ends.len() == 2,
                egui::Button::new("Créer la couture"),
            );
            self.smoke_target("sew", sew.rect.center());
            if sew.clicked() {
                self.edit(Command::Sew {
                    a: self.authoring.ends[0].clone(),
                    b: self.authoring.ends[1].clone(),
                    reverse: self.authoring.reverse,
                    ease: self.authoring.ease,
                });
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
    pub(super) fn patterns(&mut self, ui: &mut egui::Ui) {
        ui.strong("PATRONS 2D");
        ui.small("Repos en mètres · édition réinitialise la simulation");
        ui.separator();
        if self.authoring.tool == Tool::Draw {
            self.draw_canvas(ui);
        }
        if self.authoring.tool == Tool::Sew {
            self.sewing_controls(ui);
        }
        let mut selected = None;
        let mut edge_click = None;
        for panel in &self.project.template.panels {
            ui.push_id(&panel.id, |ui| {
                if ui
                    .selectable_label(
                        self.authoring.selected.as_ref() == Some(&panel.id),
                        &panel.name,
                    )
                    .clicked()
                {
                    selected = Some(panel.id.clone());
                }
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 260.),
                    egui::Sense::click(),
                );
                ui.painter()
                    .rect_filled(rect, 4., Color32::from_rgb(174, 177, 181));
                let scale = ((rect.width() - 30.) / panel.pattern.width as f32)
                    .min((rect.height() - 30.) / panel.pattern.height as f32);
                let origin = rect.center()
                    - Vec2::new(panel.pattern.width as f32, panel.pattern.height as f32)
                        * scale
                        * 0.5;
                let point = |p: DVec2| origin + Vec2::new(p.x as f32, p.y as f32) * scale;
                let range = self
                    .scene
                    .garment
                    .panels
                    .iter()
                    .find(|p| p.id == panel.id)
                    .unwrap();
                let mut mesh = egui::Mesh::default();
                for tri in &self.scene.garment.mesh.triangles[range.triangles.clone()] {
                    let index = mesh.vertices.len() as u32;
                    for p in tri.rest {
                        mesh.colored_vertex(point(p), Color32::from_gray(246));
                    }
                    mesh.add_triangle(index, index + 1, index + 2);
                }
                ui.painter().add(egui::Shape::mesh(mesh));
                if self.wire {
                    for tri in &self.scene.garment.mesh.triangles[range.triangles.clone()] {
                        for i in 0..3 {
                            ui.painter().line_segment(
                                [point(tri.rest[i]), point(tri.rest[(i + 1) % 3])],
                                Stroke::new(0.4, Color32::GRAY),
                            );
                        }
                    }
                }
                if response.clicked() {
                    selected = Some(panel.id.clone());
                }
                if let Some(poly) = &panel.polygon {
                    let nearest = response.hover_pos().and_then(|p| {
                        poly.edges
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                (
                                    i,
                                    segment_distance(
                                        p,
                                        point(poly.points[i]),
                                        point(poly.points[(i + 1) % poly.points.len()]),
                                    ),
                                )
                            })
                            .filter(|(_, d)| *d < 9.)
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(i, _)| i)
                    });
                    for (i, name) in poly.edges.iter().enumerate() {
                        let a = point(poly.points[i]);
                        let b = point(poly.points[(i + 1) % poly.points.len()]);
                        if i == 0
                            && let Some(smoke) = &mut self.authoring_smoke
                        {
                            smoke.target(&panel.id, a + (b - a) * 0.5);
                        }
                        let chosen = self
                            .authoring
                            .ends
                            .iter()
                            .position(|e| e.panel == panel.id && e.edge.as_ref() == Some(name));
                        let sewn = self.project.template.seams.iter().any(|s| {
                            [&s.a, &s.b]
                                .iter()
                                .any(|e| e.panel == panel.id && e.edge.as_ref() == Some(name))
                        });
                        let color = if chosen.is_some() {
                            Color32::from_rgb(180, 40, 170)
                        } else if nearest == Some(i) {
                            Color32::from_rgb(0, 150, 210)
                        } else if sewn {
                            Color32::from_rgb(210, 120, 20)
                        } else {
                            Color32::from_rgb(54, 116, 152)
                        };
                        ui.painter().line_segment(
                            [a, b],
                            Stroke::new(if chosen.is_some() { 4. } else { 2. }, color),
                        );
                        if let Some(k) = chosen {
                            let reverse = k == 1 && self.authoring.reverse;
                            for (p, end) in [(a, usize::from(reverse)), (b, usize::from(!reverse))]
                            {
                                ui.painter().text(
                                    p,
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}{end}", if k == 0 { "A" } else { "B" }),
                                    egui::FontId::proportional(13.),
                                    Color32::BLACK,
                                );
                            }
                        }
                        if nearest == Some(i) {
                            response.clone().on_hover_text(format!(
                                "{name} · {:.1} mm",
                                poly.points[i].distance(poly.points[(i + 1) % poly.points.len()])
                                    * 1000.
                            ));
                            if response.clicked() && self.authoring.tool == Tool::Sew {
                                edge_click = Some(BoundaryRef {
                                    panel: panel.id.clone(),
                                    side: Side::Top,
                                    edge: Some(name.clone()),
                                    range: [0., 1.],
                                });
                            }
                        }
                    }
                }
                ui.small(format!(
                    "{} triangles · droit-fil {:.0}°",
                    range.triangles.len(),
                    panel.grain_degrees
                ));
                ui.add_space(10.);
            });
        }
        if let Some(id) = selected {
            self.cancel_preview();
            self.authoring.selected = Some(id);
        }
        if let Some(edge) = edge_click {
            if self.authoring.ends.len() == 2 {
                self.authoring.ends.clear();
            }
            self.authoring.ends.push(edge);
        }
    }
}
