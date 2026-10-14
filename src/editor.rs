use crate::render::{Camera, Viewport};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use marvelous_cloth::{
    model::{Mesh, Scene, State},
    project::{self, Project},
    solver::{Metrics, ReferenceSolver},
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};

struct Work {
    generation: u64,
    mesh: Arc<Mesh>,
    state: State,
    project: Project,
    ctx: egui::Context,
}
struct Done {
    generation: u64,
    result: Result<(State, Metrics), String>,
}

pub fn run(project: Project, output: PathBuf) -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1000.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MARVELOUS CLOTH | Atelier FEM",
        options,
        Box::new(move |cc| Ok(Box::new(Editor::new(cc, project, output)?))),
    )
}

struct Editor {
    active: Project,
    draft: Project,
    mesh: Arc<Mesh>,
    state: State,
    metrics: Option<Metrics>,
    viewport: Viewport,
    camera: Camera,
    playing: bool,
    wire: bool,
    heat: bool,
    contacts: bool,
    generation: u64,
    pending: bool,
    send: mpsc::Sender<Work>,
    receive: mpsc::Receiver<Done>,
    project_path: String,
    output: PathBuf,
    status: String,
    selected_edge: Option<usize>,
}
impl Editor {
    fn new(
        cc: &eframe::CreationContext<'_>,
        project: Project,
        output: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (mesh, state) = project.build()?;
        let render_state = cc
            .wgpu_render_state
            .clone()
            .ok_or("wgpu renderer unavailable")?;
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let (send, work) = mpsc::channel::<Work>();
        let (result, receive) = mpsc::channel::<Done>();
        std::thread::Builder::new()
            .name("cloth-cpu-solver".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                while let Ok(mut job) = work.recv() {
                    let solved = ReferenceSolver::step(
                        &job.mesh,
                        &mut job.state,
                        &job.project.material,
                        &job.project.solver,
                        job.project.sphere.as_ref(),
                    )
                    .map(|m| (job.state, m));
                    if result
                        .send(Done {
                            generation: job.generation,
                            result: solved,
                        })
                        .is_err()
                    {
                        break;
                    }
                    job.ctx.request_repaint();
                }
            })?;
        let project_path = output.join("project.json").to_string_lossy().into_owned();
        Ok(Self {
            active: project.clone(),
            draft: project,
            mesh: Arc::new(mesh),
            state,
            metrics: None,
            viewport: Viewport::new(render_state),
            camera: Camera::default(),
            playing: false,
            wire: true,
            heat: false,
            contacts: false,
            generation: 0,
            pending: false,
            send,
            receive,
            project_path,
            output,
            status: "Prêt. Play démarre la simulation CPU.".into(),
            selected_edge: None,
        })
    }
    fn reset(&mut self, project: Project) {
        match project.build() {
            Ok((mesh, state)) => {
                self.generation += 1;
                self.mesh = Arc::new(mesh);
                self.state = state;
                self.active = project.clone();
                self.draft = project;
                self.metrics = None;
                self.playing = false;
                self.selected_edge = None;
                self.status = "Patron appliqué, simulation remise au repos.".into();
            }
            Err(e) => self.status = e,
        }
    }
    fn step(&mut self, ctx: &egui::Context) {
        if self.pending {
            return;
        }
        let job = Work {
            generation: self.generation,
            mesh: self.mesh.clone(),
            state: self.state.clone(),
            project: self.active.clone(),
            ctx: ctx.clone(),
        };
        if self.send.send(job).is_ok() {
            self.pending = true;
        } else {
            self.playing = false;
            self.status = "Le worker CPU est arrêté.".into();
        }
    }
    fn poll(&mut self) {
        while let Ok(done) = self.receive.try_recv() {
            self.pending = false;
            if done.generation != self.generation {
                continue;
            }
            match done.result {
                Ok((state, m)) => {
                    if !m.converged {
                        self.playing = false;
                        self.status="Tolérance non atteinte : pause. Réduire dt ou augmenter les itérations.".into();
                    }
                    self.state = state;
                    self.metrics = Some(m);
                }
                Err(e) => {
                    self.status = e;
                    self.playing = false;
                }
            }
        }
    }
    fn save(&mut self) {
        let path = PathBuf::from(&self.project_path);
        let result = (|| {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            self.active.save(&path)
        })();
        self.status = result.map_or_else(
            |e| e,
            |_| format!("Projet actif sauvegardé : {}", path.display()),
        );
    }
    fn export(&mut self) {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            fs::create_dir_all(&self.output)?;
            fs::write(
                self.output.join("cloth.obj"),
                project::obj(&self.mesh, &self.state),
            )?;
            if let Some(m) = &self.metrics {
                fs::write(
                    self.output.join("last-metrics.json"),
                    serde_json::to_string_pretty(m)?,
                )?;
            }
            Ok(())
        })();
        self.status = result.map_or_else(
            |e| e.to_string(),
            |_| format!("OBJ et métriques exportés : {}", self.output.display()),
        );
    }
    fn controls(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("ATELIER");
            ui.label("Un patron · FEM CPU f64");
            ui.add_space(12.0);
            ui.strong("Scène de départ");
            let old = self.draft.scene;
            ui.radio_value(&mut self.draft.scene, Scene::Sphere, "Drapé sur sphère");
            ui.radio_value(
                &mut self.draft.scene,
                Scene::Hanging,
                "Suspendu par deux coins",
            );
            if old != self.draft.scene {
                self.draft.sphere = if self.draft.scene == Scene::Sphere {
                    Some(Default::default())
                } else {
                    None
                };
            }
            ui.separator();
            ui.strong("Patron de repos");
            ui.add(egui::Slider::new(&mut self.draft.pattern.width, 0.1..=2.0).text("Largeur · m"));
            ui.add(
                egui::Slider::new(&mut self.draft.pattern.height, 0.1..=2.0).text("Hauteur · m"),
            );
            ui.add(egui::Slider::new(&mut self.draft.pattern.nx, 2..=32).text("Cellules X"));
            ui.add(egui::Slider::new(&mut self.draft.pattern.ny, 2..=32).text("Cellules Y"));
            ui.add(
                egui::Slider::new(&mut self.draft.material.grain_degrees, 0.0..=180.0)
                    .text("Droit-fil · °"),
            );
            ui.separator();
            ui.strong("Matière · unités SI");
            let m = &mut self.draft.material;
            ui.add(
                egui::Slider::new(&mut m.warp, 10.0..=3000.0)
                    .logarithmic(true)
                    .text("Chaîne · N/m"),
            );
            ui.add(
                egui::Slider::new(&mut m.weft, 10.0..=3000.0)
                    .logarithmic(true)
                    .text("Trame · N/m"),
            );
            ui.add(
                egui::Slider::new(&mut m.shear, 1.0..=500.0)
                    .logarithmic(true)
                    .text("Cisaillement"),
            );
            ui.add(
                egui::Slider::new(&mut m.bend_warp, 1e-6..=0.01)
                    .logarithmic(true)
                    .text("Flexion chaîne · N m"),
            );
            ui.add(
                egui::Slider::new(&mut m.bend_weft, 1e-6..=0.01)
                    .logarithmic(true)
                    .text("Flexion trame · N m"),
            );
            ui.add(
                egui::Slider::new(&mut m.compression_ratio, 0.01..=1.0).text("Ratio compression"),
            );
            ui.add(egui::Slider::new(&mut m.density, 0.02..=1.0).text("Masse · kg/m²"));
            ui.add(
                egui::Slider::new(&mut m.thickness, 0.0001..=0.01)
                    .logarithmic(true)
                    .text("Épaisseur · m"),
            );
            ui.separator();
            ui.strong("Référence · Euler implicite");
            ui.add(egui::Slider::new(&mut self.draft.solver.dt, 0.001..=0.04).text("Pas · s"));
            ui.add(
                egui::Slider::new(&mut self.draft.solver.gravity.y, -20.0..=0.0)
                    .text("Gravité Y · m/s²"),
            );
            ui.add(
                egui::Slider::new(&mut self.draft.solver.damping, 0.0..=5.0).text("Damping · 1/s"),
            );
            ui.add(
                egui::Slider::new(&mut self.draft.solver.newton_iterations, 5..=100)
                    .text("Itérations Newton"),
            );
            if let Some(s) = &mut self.draft.sphere {
                ui.add(egui::Slider::new(&mut s.radius, 0.1..=0.5).text("Rayon sphère · m"));
                ui.add(
                    egui::Slider::new(&mut s.stiffness, 1e3..=1e6)
                        .logarithmic(true)
                        .text("Contact · N/m³"),
                );
            }
            ui.add_space(8.0);
            if ui.button("Appliquer les paramètres et Reset").clicked() {
                self.reset(self.draft.clone());
            }
            ui.small(
                "Les réglages sont appliqués ensemble. Reset reconstruit le patron et sa masse.",
            );
            ui.separator();
            ui.strong("Projet JSON");
            ui.text_edit_singleline(&mut self.project_path);
            ui.horizontal(|ui| {
                if ui.button("Sauvegarder").clicked() {
                    self.save();
                }
                if ui.button("Charger").clicked() {
                    match Project::load(&PathBuf::from(&self.project_path)) {
                        Ok(p) => self.reset(p),
                        Err(e) => self.status = e,
                    }
                }
            });
            if ui.button("Exporter OBJ + métriques").clicked() {
                self.export();
            }
            ui.small(
                "Sauvegarde du patron actif et des réglages ; l'état simulé s'exporte en OBJ.",
            );
        });
    }
    fn pattern(&mut self, ui: &mut egui::Ui) {
        ui.strong("PATRON 2D");
        ui.label("Configuration de repos · mètres");
        let size = Vec2::new(
            ui.available_width(),
            (ui.available_height() - 48.0).max(120.0),
        );
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, Color32::from_rgb(22, 28, 36));
        let p = &self.active.pattern;
        let scale = ((rect.width() - 60.0) / p.width as f32)
            .min((rect.height() - 80.0) / p.height as f32)
            .max(1.0);
        let origin = rect.center() - Vec2::new(p.width as f32, p.height as f32) * scale * 0.5;
        let point = |u: glam::DVec2| origin + Vec2::new(u.x as f32, u.y as f32) * scale;
        for t in &self.mesh.triangles {
            let [a, b, c] = t.vertices.map(|i| point(self.mesh.uv[i]));
            for pair in [[a, b], [b, c], [c, a]] {
                painter.line_segment(pair, Stroke::new(0.5, Color32::from_rgb(48, 82, 92)));
            }
        }
        let corners = [
            origin,
            origin + Vec2::new(p.width as f32 * scale, 0.0),
            origin + Vec2::new(p.width as f32, p.height as f32) * scale,
            origin + Vec2::new(0.0, p.height as f32 * scale),
        ];
        for i in 0..4 {
            let pair = [corners[i], corners[(i + 1) % 4]];
            let selected = self.selected_edge == Some(i);
            painter.line_segment(
                pair,
                Stroke::new(
                    if selected { 3.0 } else { 1.5 },
                    if selected {
                        Color32::GOLD
                    } else {
                        Color32::from_rgb(88, 204, 213)
                    },
                ),
            );
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let edge = pair[1] - pair[0];
                let t = ((pos - pair[0]).dot(edge) / edge.length_sq()).clamp(0.0, 1.0);
                if pos.distance(pair[0] + t * edge) < 10.0 {
                    self.selected_edge = Some(i);
                }
            }
        }
        let angle = self.active.material.grain_degrees.to_radians() as f32;
        let d = Vec2::new(angle.cos(), angle.sin()) * 55.0;
        painter.arrow(rect.center() - 0.5 * d, d, Stroke::new(2.0, Color32::GOLD));
        painter.text(
            rect.center() + Vec2::new(0.0, 40.0),
            egui::Align2::CENTER_CENTER,
            "CHAÎNE",
            egui::FontId::proportional(11.0),
            Color32::GOLD,
        );
        painter.text(
            origin - Vec2::new(-p.width as f32 * scale * 0.5, 18.0),
            egui::Align2::CENTER_CENTER,
            format!("{:.0} mm", p.width * 1000.0),
            egui::FontId::proportional(12.0),
            Color32::LIGHT_GRAY,
        );
        for (i, pin) in self.mesh.pins.iter().enumerate() {
            if pin.is_some() {
                painter.circle_filled(point(self.mesh.uv[i]), 4.5, Color32::LIGHT_RED);
            }
        }
        if let Some(i) = self.selected_edge {
            ui.label(format!(
                "Bord {} sélectionné · {:.0} mm",
                i + 1,
                if i % 2 == 0 {
                    p.width * 1000.0
                } else {
                    p.height * 1000.0
                }
            ));
        } else {
            ui.small("Cliquer sur un bord pour le sélectionner.");
        }
    }
}
impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("MARVELOUS CLOTH")
                        .strong()
                        .size(19.0)
                        .color(Color32::from_rgb(100, 220, 218)),
                );
                ui.separator();
                if ui
                    .button(if self.playing { "Pause" } else { "Play" })
                    .clicked()
                {
                    self.playing = !self.playing;
                }
                if ui
                    .add_enabled(!self.pending && !self.playing, egui::Button::new("Step"))
                    .clicked()
                {
                    self.step(&ctx);
                }
                if ui.button("Reset").clicked() {
                    self.reset(self.active.clone());
                }
                ui.separator();
                ui.checkbox(&mut self.wire, "Maillage");
                ui.checkbox(&mut self.heat, "Déformation");
                ui.checkbox(&mut self.contacts, "Contacts");
                ui.label(format!("t = {:.3} s", self.state.time));
                if self.pending {
                    ui.spinner();
                }
            });
        });
        egui::Panel::bottom("status").show(ui,|ui| {
            ui.label(&self.status);
            if let Some(m)=&self.metrics {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(if m.converged {Color32::LIGHT_GREEN} else {Color32::YELLOW},if m.converged {"Convergé"} else {"Budget atteint"});
                    ui.label(format!("Résidu {:.2e} N / {:.2e} · Newton {} · PCG {} · CPU {:.1} ms",m.residual_rms_n,m.tolerance_n,m.newton_iterations,m.pcg_iterations,m.step_ms));
                    ui.label(format!("Déformation max {:.2}% · pénétration sommets {:.3} mm",100.0*m.max_strain,1000.0*m.max_vertex_penetration_m));
                });
            }
            ui.small("Contact sphère par pénalité · pas de self-collision · grille sans collision · FEM CPU, affichage wgpu");
        });
        egui::Panel::left("settings")
            .default_size(295.0)
            .resizable(true)
            .show(ui, |ui| self.controls(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(2, |cols| {
                self.pattern(&mut cols[0]);
                let ui = &mut cols[1];
                ui.strong("SIMULATION 3D");
                ui.label("Glisser : orbite · clic droit : pan · molette : zoom");
                let size = ui.available_size().max(Vec2::splat(1.0));
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                self.camera.interact(&response);
                let pixels = rect.size() * ctx.pixels_per_point();
                let texture = self.viewport.draw(
                    [pixels.x as u32, pixels.y as u32],
                    &self.camera,
                    &self.mesh,
                    &self.state,
                    &self.active,
                    self.wire,
                    self.heat,
                    self.contacts,
                );
                ui.painter().image(
                    texture,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                if self.heat {
                    ui.painter().text(
                        rect.left_bottom() + Vec2::new(10.0, -18.0),
                        egui::Align2::LEFT_BOTTOM,
                        "|allongement - 1| : cyan 0% → rouge 10%+",
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }
            });
        });
        if self.playing {
            self.step(&ctx);
        }
        if self.pending {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}
