//! Garment-CAD workspace presentation for the CPU FEM reference.
//!
//! The arrangement follows the workspace shape of a professional garment CAD
//! tool: a menu bar, a tool strip, a narrow palette on the left, the 3D garment
//! window and the white 2D pattern window side by side, a tabbed property
//! inspector docked on the right, and a status bar along the bottom.
//!
//! Every colour, glyph, label and layout choice in this file is original to this
//! project. No CLO3D asset, icon, artwork, font or screenshot is used, and no
//! CLO3D control is imitated at the pixel level; the shape of a docked viewport
//! workspace is a generic CAD convention shared by many applications.
//!
//! Presentation only: the worker thread, solver calls, file I/O and camera
//! interaction below are unchanged from the previous layout.
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

/// Chrome colours: light panels, dark docked-window title bars.
const CHROME: Color32 = Color32::from_rgb(0xEC, 0xEC, 0xEC);
const CHROME_DEEP: Color32 = Color32::from_rgb(0xDD, 0xDD, 0xDD);
const TITLE_BAR: Color32 = Color32::from_rgb(0x4A, 0x4E, 0x54);
const TEXT: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1A);
const ACCENT: Color32 = Color32::from_rgb(0x1E, 0x88, 0xE5);
const ACCENT_DEEP: Color32 = Color32::from_rgb(0x15, 0x65, 0xC0);
const SELECT: Color32 = Color32::from_rgb(0xE8, 0x6A, 0x17);
/// Neutral backdrop behind the rendered 3D window, matching the wgpu clear
/// colour in `render.rs` so no seam shows before the first frame lands.
const VIEW_BG: Color32 = Color32::from_rgb(0x99, 0xA3, 0xB3);
const PATTERN_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const PATTERN_GRID: Color32 = Color32::from_rgb(0xE3, 0xE8, 0xEF);
const PATTERN_MESH: Color32 = Color32::from_rgb(0xB6, 0xC1, 0xCE);
const PATTERN_LINE: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x1B);

const TABS: [&str; 4] = ["Propriétés", "Objets", "Matière", "Simulation"];

/// Light chrome with a blue accent, applied to both egui themes so the
/// workspace looks the same whatever the host reports.
fn style(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.button_padding = Vec2::new(8.0, 3.0);
    style.spacing.interact_size.y = 23.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    style.visuals.panel_fill = CHROME;
    style.visuals.window_fill = Color32::from_rgb(0xF7, 0xF7, 0xF7);
    style.visuals.faint_bg_color = CHROME_DEEP;
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT_DEEP);
    style.visuals.widgets.inactive.bg_fill = CHROME_DEEP;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0xCB, 0xDD, 0xF0);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(0xAE, 0xCC, 0xEC);
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
}

/// Docked-window caption strip, the way a CAD workspace titles each viewport.
fn title_bar(ui: &mut egui::Ui, title: &str, hint: &str) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, TITLE_BAR);
    painter.text(
        rect.left_center() + Vec2::new(7.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(11.5),
        Color32::from_rgb(0xF2, 0xF2, 0xF2),
    );
    if !hint.is_empty() {
        painter.text(
            rect.right_center() - Vec2::new(7.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            hint,
            egui::FontId::proportional(10.5),
            Color32::from_rgb(0xBE, 0xC4, 0xCC),
        );
    }
}

/// A palette entry that shows its own on/off state.
fn palette_toggle(ui: &mut egui::Ui, on: &mut bool, label: &str, hint: &str) {
    let text = RichText::new(label)
        .size(11.5)
        .color(if *on { Color32::WHITE } else { TEXT });
    let button = egui::Button::new(text).fill(if *on { ACCENT } else { CHROME_DEEP });
    if ui
        .add_sized(Vec2::new(ui.available_width(), 26.0), button)
        .on_hover_text(hint)
        .clicked()
    {
        *on = !*on;
    }
}

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
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([1100.0, 700.0]),
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
    tab: usize,
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
        style(&cc.egui_ctx);
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
            status: "Prêt. Le menu Simulation démarre le solveur FEM CPU.".into(),
            selected_edge: None,
            tab: 0,
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
    fn load(&mut self) {
        match Project::load(&PathBuf::from(&self.project_path)) {
            Ok(p) => self.reset(p),
            Err(e) => self.status = e,
        }
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
    fn playing_label(&self) -> &'static str {
        if self.playing {
            "⏸ Pause"
        } else {
            "▶ Simuler"
        }
    }

    // ---------------------------------------------------------------- chrome

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.menu_button("Fichier", |ui| {
                if ui.button("Sauvegarder le projet").clicked() {
                    self.save();
                    ui.close();
                }
                if ui.button("Charger le projet").clicked() {
                    self.load();
                    ui.close();
                }
                ui.separator();
                if ui.button("Exporter OBJ + métriques").clicked() {
                    self.export();
                    ui.close();
                }
                ui.separator();
                if ui.button("Quitter").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Édition", |ui| {
                if ui.button("Appliquer les paramètres et Reset").clicked() {
                    self.reset(self.draft.clone());
                    ui.close();
                }
                if ui.button("Réinitialiser la vue 3D").clicked() {
                    self.camera = Camera::default();
                    ui.close();
                }
            });
            ui.menu_button("Affichage", |ui| {
                ui.checkbox(&mut self.wire, "Maillage");
                ui.checkbox(&mut self.heat, "Déformation");
                ui.checkbox(&mut self.contacts, "Contacts");
            });
            ui.menu_button("Simulation", |ui| {
                if ui.button(self.playing_label()).clicked() {
                    self.playing = !self.playing;
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.pending && !self.playing,
                        egui::Button::new("Avancer d'un pas"),
                    )
                    .clicked()
                {
                    self.step(&ctx);
                    ui.close();
                }
                if ui.button("Retour au repos").clicked() {
                    self.reset(self.active.clone());
                    ui.close();
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("FEM CPU f64")
                        .small()
                        .color(Color32::from_rgb(0x66, 0x66, 0x66)),
                );
                ui.separator();
                ui.label(
                    RichText::new("MARVELOUS CLOTH")
                        .strong()
                        .size(13.5)
                        .color(ACCENT_DEEP),
                );
            });
        });
    }

    fn tool_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            let play = ui.button(RichText::new(self.playing_label()).size(12.5));
            if play.clicked() {
                self.playing = !self.playing;
            }
            if ui
                .add_enabled(
                    !self.pending && !self.playing,
                    egui::Button::new(RichText::new("⏭ Pas").size(12.5)),
                )
                .clicked()
            {
                self.step(ctx);
            }
            if ui
                .button(RichText::new("↺ Reset").size(12.5))
                .on_hover_text("Reconstruit le patron et remet la simulation au repos")
                .clicked()
            {
                self.reset(self.active.clone());
            }
            ui.separator();
            if ui
                .button(RichText::new("Vue par défaut").size(12.5))
                .on_hover_text("Recadre la caméra 3D")
                .clicked()
            {
                self.camera = Camera::default();
            }
            ui.separator();
            ui.checkbox(&mut self.wire, "Maillage");
            ui.checkbox(&mut self.heat, "Déformation");
            ui.checkbox(&mut self.contacts, "Contacts");
            ui.separator();
            ui.label(format!("t = {:.3} s", self.state.time));
            if self.pending {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(self.scene_label())
                        .small()
                        .color(Color32::from_rgb(0x55, 0x55, 0x55)),
                );
            });
        });
    }

    fn scene_label(&self) -> &'static str {
        match self.active.scene {
            Scene::Sphere => "Scène : drapé sur sphère",
            Scene::Hanging => "Scène : suspendu par deux coins",
        }
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(
            RichText::new("AFFICHAGE")
                .size(10.0)
                .color(Color32::from_rgb(0x70, 0x70, 0x70)),
        );
        palette_toggle(
            ui,
            &mut self.wire,
            "Maillage",
            "Affiche les arêtes des triangles",
        );
        palette_toggle(
            ui,
            &mut self.heat,
            "Déform.",
            "Colore |allongement - 1| de cyan (0 %) à rouge (10 %+)",
        );
        palette_toggle(
            ui,
            &mut self.contacts,
            "Contacts",
            "Surligne les sommets en pénétration de contact",
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("VUE")
                .size(10.0)
                .color(Color32::from_rgb(0x70, 0x70, 0x70)),
        );
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 26.0),
                egui::Button::new(RichText::new("Défaut").size(11.5)).fill(CHROME_DEEP),
            )
            .on_hover_text("Recadre la caméra 3D")
            .clicked()
        {
            self.camera = Camera::default();
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (badge, color) = match &self.metrics {
                Some(m) if m.converged => ("Convergé", ACCENT_DEEP),
                Some(_) => ("Budget atteint", SELECT),
                None => ("Au repos", Color32::from_rgb(0x66, 0x66, 0x66)),
            };
            ui.label(RichText::new(badge).strong().color(color));
            ui.separator();
            ui.add(egui::Label::new(RichText::new(&self.status).size(11.5)).truncate())
                .on_hover_text(&self.status);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(m) = &self.metrics {
                    ui.label(
                        RichText::new(format!(
                            "Newton {} · PCG {} · {:.1} ms · déformation max {:.2} %",
                            m.newton_iterations,
                            m.pcg_iterations,
                            m.step_ms,
                            100.0 * m.max_strain
                        ))
                        .small(),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "résidu {:.2e} / {:.2e} N",
                            m.residual_rms_n, m.tolerance_n
                        ))
                        .small(),
                    );
                }
            });
        });
    }

    // ------------------------------------------------------------- inspector

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (i, name) in TABS.iter().enumerate() {
                if ui.selectable_label(self.tab == i, *name).clicked() {
                    self.tab = i;
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("inspector")
            .show(ui, |ui| match self.tab {
                0 => self.tab_properties(ui),
                1 => self.tab_objects(ui),
                2 => self.tab_fabric(ui),
                _ => self.tab_simulation(ui),
            });
    }

    fn tab_properties(&mut self, ui: &mut egui::Ui) {
        ui.strong("SCÈNE DE DÉPART");
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
        ui.strong("PATRON DE REPOS");
        ui.add(egui::Slider::new(&mut self.draft.pattern.width, 0.1..=2.0).text("Largeur · m"));
        ui.add(egui::Slider::new(&mut self.draft.pattern.height, 0.1..=2.0).text("Hauteur · m"));
        ui.add(egui::Slider::new(&mut self.draft.pattern.nx, 2..=32).text("Cellules X"));
        ui.add(egui::Slider::new(&mut self.draft.pattern.ny, 2..=32).text("Cellules Y"));
        ui.add(
            egui::Slider::new(&mut self.draft.material.grain_degrees, 0.0..=180.0)
                .text("Droit-fil · °"),
        );
        if let Some(s) = &mut self.draft.sphere {
            ui.separator();
            ui.strong("COLLISION · SPHÈRE");
            ui.add(egui::Slider::new(&mut s.radius, 0.1..=0.5).text("Rayon · m"));
            ui.add(
                egui::Slider::new(&mut s.stiffness, 1e3..=1e6)
                    .logarithmic(true)
                    .text("Contact · N/m³"),
            );
        }
        ui.add_space(8.0);
        if ui
            .add_sized(
                Vec2::new(ui.available_width(), 26.0),
                egui::Button::new(RichText::new("Appliquer et Reset").strong()),
            )
            .clicked()
        {
            self.reset(self.draft.clone());
        }
        ui.small("Les réglages sont appliqués ensemble. Reset reconstruit le patron et sa masse.");
    }

    fn tab_objects(&mut self, ui: &mut egui::Ui) {
        ui.strong("ARBORESCENCE");
        ui.collapsing("Patron 2D", |ui| {
            let p = &self.active.pattern;
            ui.label(format!("{} × {} cellules", p.nx, p.ny));
            ui.label(format!(
                "{:.0} × {:.0} mm",
                p.width * 1000.0,
                p.height * 1000.0
            ));
        });
        ui.collapsing("Maillage compilé", |ui| {
            ui.label(format!("{} sommets", self.mesh.uv.len()));
            ui.label(format!("{} triangles", self.mesh.triangles.len()));
            ui.label(format!("{} charnières", self.mesh.hinges.len()));
            let pins = self.mesh.pins.iter().filter(|p| p.is_some()).count();
            ui.label(format!("{pins} points fixes"));
        });
        ui.collapsing("Sélection", |ui| match self.selected_edge {
            Some(i) => {
                let p = &self.active.pattern;
                let mm = if i % 2 == 0 {
                    p.width * 1000.0
                } else {
                    p.height * 1000.0
                };
                ui.label(format!("Bord {} · {:.0} mm", i + 1, mm));
                if ui.button("Effacer la sélection").clicked() {
                    self.selected_edge = None;
                }
            }
            None => {
                ui.small("Aucun bord sélectionné. Cliquer un bord dans la fenêtre 2D.");
            }
        });
        ui.separator();
        ui.strong("PROJET JSON");
        ui.text_edit_singleline(&mut self.project_path);
        ui.horizontal(|ui| {
            if ui.button("Sauvegarder").clicked() {
                self.save();
            }
            if ui.button("Charger").clicked() {
                self.load();
            }
        });
        if ui.button("Exporter OBJ + métriques").clicked() {
            self.export();
        }
        ui.small("Sauvegarde du patron actif et des réglages ; l'état simulé s'exporte en OBJ.");
    }

    fn tab_fabric(&mut self, ui: &mut egui::Ui) {
        ui.strong("MATIÈRE · UNITÉS SI");
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
        ui.add(egui::Slider::new(&mut m.compression_ratio, 0.01..=1.0).text("Ratio compression"));
        ui.add(egui::Slider::new(&mut m.density, 0.02..=1.0).text("Masse · kg/m²"));
        ui.add(
            egui::Slider::new(&mut m.thickness, 0.0001..=0.01)
                .logarithmic(true)
                .text("Épaisseur · m"),
        );
        ui.separator();
        ui.small("Valeurs estimées, non calibrées sur mesures. Modifier la matière exige un Reset : la densité est intégrée à la masse compilée.");
    }

    fn tab_simulation(&mut self, ui: &mut egui::Ui) {
        ui.strong("RÉFÉRENCE · EULER IMPLICITE");
        ui.add(egui::Slider::new(&mut self.draft.solver.dt, 0.001..=0.04).text("Pas · s"));
        ui.add(
            egui::Slider::new(&mut self.draft.solver.gravity.y, -20.0..=0.0)
                .text("Gravité Y · m/s²"),
        );
        ui.add(egui::Slider::new(&mut self.draft.solver.damping, 0.0..=5.0).text("Damping · 1/s"));
        ui.add(
            egui::Slider::new(&mut self.draft.solver.newton_iterations, 5..=100)
                .text("Itérations Newton"),
        );
        ui.separator();
        ui.strong("SOLVEUR LINÉAIRE");
        ui.label("Opérateur nodal assemblé · 3×3");
        ui.label("Préconditionneur : block-Jacobi nodal");
        ui.small(
            "Le chemin matriciel-libre à point-Jacobi reste le repli si un bloc nodal n'est pas inversible.",
        );
        if let Some(m) = &self.metrics {
            ui.separator();
            ui.strong("DERNIER PAS");
            egui::Grid::new("metrics").num_columns(2).show(ui, |ui| {
                ui.label("Résidu");
                ui.label(format!("{:.3e} N", m.residual_rms_n));
                ui.end_row();
                ui.label("Tolérance");
                ui.label(format!("{:.3e} N", m.tolerance_n));
                ui.end_row();
                ui.label("Newton / PCG");
                ui.label(format!("{} / {}", m.newton_iterations, m.pcg_iterations));
                ui.end_row();
                ui.label("Allongement max");
                ui.label(format!("{:.2} %", 100.0 * m.max_strain));
                ui.end_row();
                ui.label("Pénétration");
                ui.label(format!("{:.3} mm", 1000.0 * m.max_vertex_penetration_m));
                ui.end_row();
                ui.label("Coût du pas");
                ui.label(format!("{:.2} ms", m.step_ms));
                ui.end_row();
            });
        }
        ui.separator();
        ui.small("Contact sphère par pénalité · pas de self-collision · grille sans collision · FEM CPU, affichage wgpu");
    }

    // -------------------------------------------------------------- viewports

    fn window_3d(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        title_bar(
            ui,
            "3D WINDOW",
            "glisser : orbite · droit : pan · molette : zoom",
        );
        let size = ui.available_size().max(Vec2::splat(1.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, VIEW_BG);
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
        painter.image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if self.heat {
            painter.text(
                rect.left_bottom() + Vec2::new(8.0, -14.0),
                egui::Align2::LEFT_BOTTOM,
                "|allongement - 1| : cyan 0 % → rouge 10 %+",
                egui::FontId::proportional(11.0),
                Color32::from_rgb(0x33, 0x33, 0x33),
            );
        }
    }

    fn window_2d(&mut self, ui: &mut egui::Ui) {
        let hint = match self.selected_edge {
            Some(i) => {
                let p = &self.active.pattern;
                let mm = if i % 2 == 0 {
                    p.width * 1000.0
                } else {
                    p.height * 1000.0
                };
                format!("bord {} · {:.0} mm", i + 1, mm)
            }
            None => "clic sur un bord pour le sélectionner".to_string(),
        };
        title_bar(ui, "2D PATTERN WINDOW", &hint);

        let avail = ui.available_size();
        let size = Vec2::new(avail.x.max(80.0), (avail.y - 6.0).max(120.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, PATTERN_BG);

        // Millimetre drafting grid, the way a pattern window rules its canvas.
        let step = 40.0_f32;
        let mut x = rect.left() + step;
        while x < rect.right() {
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                Stroke::new(1.0, PATTERN_GRID),
            );
            x += step;
        }
        let mut y = rect.top() + step;
        while y < rect.bottom() {
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                Stroke::new(1.0, PATTERN_GRID),
            );
            y += step;
        }
        for edge in [
            [rect.left_top(), rect.right_top()],
            [rect.right_top(), rect.right_bottom()],
            [rect.right_bottom(), rect.left_bottom()],
            [rect.left_bottom(), rect.left_top()],
        ] {
            painter.line_segment(edge, Stroke::new(1.0, Color32::from_rgb(0x8A, 0x8A, 0x8A)));
        }

        let p = &self.active.pattern;
        let scale = ((rect.width() - 90.0) / p.width as f32)
            .min((rect.height() - 110.0) / p.height as f32)
            .max(1.0);
        let origin = rect.center() - Vec2::new(p.width as f32, p.height as f32) * scale * 0.5;
        let point = |u: glam::DVec2| origin + Vec2::new(u.x as f32, u.y as f32) * scale;

        for t in &self.mesh.triangles {
            let [a, b, c] = t.vertices.map(|i| point(self.mesh.uv[i]));
            for pair in [[a, b], [b, c], [c, a]] {
                painter.line_segment(pair, Stroke::new(0.5, PATTERN_MESH));
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
                    if selected { 3.0 } else { 1.6 },
                    if selected { SELECT } else { PATTERN_LINE },
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

        // Warp direction marker.
        let angle = self.active.material.grain_degrees.to_radians() as f32;
        let d = Vec2::new(angle.cos(), angle.sin()) * 62.0;
        painter.arrow(rect.center() - 0.5 * d, d, Stroke::new(2.0, ACCENT_DEEP));
        painter.text(
            rect.center() + Vec2::new(0.0, 42.0),
            egui::Align2::CENTER_CENTER,
            "CHAÎNE",
            egui::FontId::proportional(11.0),
            ACCENT_DEEP,
        );

        // Dimension lines, drawn the way a pattern window annotates a piece.
        let dim = Color32::from_rgb(0x44, 0x44, 0x44);
        let top = origin.y - 26.0;
        painter.line_segment(
            [
                egui::pos2(origin.x, top),
                egui::pos2(origin.x + p.width as f32 * scale, top),
            ],
            Stroke::new(1.0, dim),
        );
        painter.text(
            egui::pos2(origin.x + p.width as f32 * scale * 0.5, top - 9.0),
            egui::Align2::CENTER_CENTER,
            format!("{:.0} mm", p.width * 1000.0),
            egui::FontId::proportional(11.0),
            dim,
        );
        let right = origin.x + p.width as f32 * scale + 26.0;
        painter.line_segment(
            [
                egui::pos2(right, origin.y),
                egui::pos2(right, origin.y + p.height as f32 * scale),
            ],
            Stroke::new(1.0, dim),
        );
        painter.text(
            egui::pos2(right + 6.0, origin.y + p.height as f32 * scale * 0.5),
            egui::Align2::LEFT_CENTER,
            format!("{:.0} mm", p.height * 1000.0),
            egui::FontId::proportional(11.0),
            dim,
        );

        for (i, pin) in self.mesh.pins.iter().enumerate() {
            if pin.is_some() {
                let at = point(self.mesh.uv[i]);
                painter.circle_filled(at, 5.0, Color32::WHITE);
                painter.circle_stroke(
                    at,
                    5.0,
                    Stroke::new(1.6, Color32::from_rgb(0xC8, 0x28, 0x28)),
                );
            }
        }
    }
}
impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        egui::Panel::top("menubar").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar").show(ui, |ui| self.tool_strip(ui, &ctx));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("palette")
            .default_size(58.0)
            .resizable(false)
            .show(ui, |ui| self.palette(ui));
        egui::Panel::right("inspector")
            .default_size(320.0)
            .size_range(260.0..=460.0)
            .resizable(true)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(2, |cols| {
                self.window_3d(&mut cols[0], &ctx);
                self.window_2d(&mut cols[1]);
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
