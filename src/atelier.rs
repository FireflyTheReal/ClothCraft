//! Garment workspace: library | 3D | 2D patterns | properties.
use crate::render::{Camera, DrawModel, Viewport};
use eframe::egui::{self, Color32, Stroke, Vec2};
use marvelous_cloth::{
    fitting::{FittingProject, FittingScene},
    garment::{export, presets::GarmentKind},
    picking,
    vbd::{Grab, VbdMetrics, VbdSettings},
};
use std::{path::PathBuf, sync::mpsc, time::Duration};
#[path = "atelier/authoring_smoke.rs"]
mod authoring_smoke;
#[path = "atelier/authoring_ui.rs"]
mod authoring_ui;
#[path = "atelier/board.rs"]
mod board;
#[path = "atelier/drafting_ui.rs"]
mod drafting_ui;
#[path = "atelier/edges_3d.rs"]
mod edges_3d;
#[path = "atelier/fabrics_ui.rs"]
mod fabrics_ui;
#[path = "atelier/gizmo.rs"]
mod gizmo;
#[path = "atelier/icons.rs"]
mod icons;
#[path = "atelier/perf.rs"]
mod perf;
#[path = "atelier/seam_links.rs"]
mod seam_links;
#[path = "atelier/seams_ui.rs"]
mod seams_ui;
#[path = "atelier/settings.rs"]
mod settings;
#[path = "atelier/view_controls.rs"]
mod view_controls;
#[path = "atelier/worker.rs"]
mod worker;
#[path = "atelier/workspace.rs"]
mod workspace;
use authoring_ui::{AuthoringUi, Tool};
use worker::Work;
fn camera() -> Camera {
    Camera {
        yaw: 0.10,
        pitch: 0.05,
        distance: 2.75,
        target: glam::Vec3::new(0., 0.95, 0.),
        ..Default::default()
    }
}
pub fn run(
    project: FittingProject,
    scene: FittingScene,
    output: PathBuf,
    smoke: bool,
    drag_test: bool,
    perf_test: bool,
    authoring_test: bool,
) -> Result<(), String> {
    let smoke_report = output.join("interaction.json");
    let authoring_report = output.join("authoring.json");
    let perf_report = output.join("ui-perf.json");
    let smoke_size = (smoke || drag_test || perf_test || authoring_test)
        .then(|| std::env::var("CLOTHCRAFT_SMOKE_SIZE").ok())
        .flatten()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
            ((1100..=3840).contains(&w) && (700..=2160).contains(&h))
                .then_some([w as f32, h as f32])
        });
    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/branding/clothcraft-icon.png"))
            .map_err(|error| format!("Cannot decode the bundled ClothCraft icon: {error}"))?;
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_inner_size(smoke_size.unwrap_or([1600., 980.]))
            .with_min_inner_size([1100., 700.]),
        ..Default::default()
    };
    eframe::run_native(
        "ClothCraft | Simulation Studio",
        options,
        Box::new(move |cc| {
            let render = cc.wgpu_render_state.clone().ok_or("wgpu unavailable")?;
            // Preferences choose the theme and the startup display defaults, so
            // they are loaded and applied before the first frame is drawn.
            // Applying them afterwards would show a flash of the wrong theme.
            let prefs = settings::load(&output.join(settings::FILE_NAME));
            settings::apply(&cc.egui_ctx, &prefs);
            // Copy the startup values out before `prefs` moves into the struct.
            let (wire, seam_links) = (prefs.wireframe, prefs.seam_links);
            let (dt, iterations) = (prefs.dt, prefs.iterations);
            let mut authoring = AuthoringUi {
                tool: if drag_test || perf_test {
                    Tool::Grab
                } else {
                    Tool::Edit
                },
                ..Default::default()
            };
            authoring.board.snap_mm = prefs.snap_mm;
            let (tx, receive) = worker::start(render.device.clone(), render.queue.clone())?;
            let m = project
                .template
                .panels
                .iter()
                .find_map(|p| p.polygon.as_ref())
                .map(|p| p.meshing_settings())
                .unwrap_or_default();
            Ok(Box::new(Atelier {
                authoring,
                prefs,
                settings_open: false,
                authoring_smoke: authoring_test.then(authoring_smoke::Smoke::new),
                mesh_mm: m.target_edge_m * 1000.,
                anisotropy: m.anisotropy,
                mesh_axis: m.axis_degrees,
                project,
                smoke_initial: if drag_test {
                    scene.state.x.clone()
                } else {
                    Vec::new()
                },
                scene,
                output,
                viewport: Viewport::new(render),
                camera: camera(),
                send: tx,
                receive,
                generation: 0,
                pending: false,
                playing: perf_test,
                wire,
                heat: false,
                seams: false,
                seam_links,
                gpu: true,
                settings: VbdSettings {
                    dt,
                    iterations,
                    ..Default::default()
                },
                metrics: None,
                status: "Ready · Space to start the simulation".into(),
                grab: None,
                drag_plane: None,
                smoke: smoke || drag_test || perf_test || authoring_test,
                perf: perf_test.then(perf::PerfRun::new),
                drag_test,
                smoke_point: None,
                smoke_tick: 0,
                smoke_grabbed: false,
                smoke_applied: false,
                smoke_captured: false,
                frames: 0,
            }))
        }),
    )
    .map_err(|e| e.to_string())?;
    if drag_test {
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(smoke_report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if report["passed"] != true {
            return Err(format!("UI drag smoke failed: {report}"));
        }
    }
    if authoring_test {
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(authoring_report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if report["passed"] != true {
            return Err(format!("Authoring smoke failed: {report}"));
        }
    }
    if perf_test {
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(perf_report).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if report["passed"] != true {
            return Err(format!("UI realtime check failed: {report}"));
        }
    }
    Ok(())
}
struct Atelier {
    authoring: AuthoringUi,
    /// Loaded from `output/preferences.json` before the first frame and saved
    /// whenever the preferences window reports a change.
    prefs: settings::Preferences,
    /// Whether the preferences window is currently open.
    settings_open: bool,
    authoring_smoke: Option<authoring_smoke::Smoke>,
    perf: Option<perf::PerfRun>,
    mesh_mm: f64,
    anisotropy: f64,
    mesh_axis: f64,
    project: FittingProject,
    scene: FittingScene,
    output: PathBuf,
    viewport: Viewport,
    camera: Camera,
    send: mpsc::Sender<Work>,
    receive: worker::Latest,
    generation: u64,
    pending: bool,
    playing: bool,
    wire: bool,
    heat: bool,
    seams: bool,
    seam_links: bool,
    gpu: bool,
    settings: VbdSettings,
    metrics: Option<VbdMetrics>,
    status: String,
    grab: Option<Grab>,
    drag_plane: Option<(glam::DVec3, glam::DVec3)>,
    smoke: bool,
    frames: u32,
    drag_test: bool,
    smoke_point: Option<egui::Pos2>,
    smoke_tick: u32,
    smoke_grabbed: bool,
    smoke_applied: bool,
    smoke_captured: bool,
    smoke_initial: Vec<glam::DVec3>,
}
impl Atelier {
    fn step(&mut self, ctx: &egui::Context) {
        self.submit(ctx, true);
    }
    fn submit(&mut self, ctx: &egui::Context, once: bool) {
        let scene = FittingScene {
            garment: self.scene.garment.clone(),
            collider: self.scene.collider.clone(),
            state: self.scene.state.clone(),
            quasistatic: false,
            next_dt: self.settings.dt,
        };
        self.pending = self
            .send
            .send(Work {
                generation: self.generation,
                playing: self.playing,
                once,
                scene,
                settings: self.settings.clone(),
                grab: self.grab.clone(),
                gpu: self.gpu,
                ctx: ctx.clone(),
            })
            .is_ok()
            && (self.playing || once);
    }
    fn load(&mut self, p: FittingProject) {
        match p.compile() {
            Ok(scene) => {
                if let Some(poly) = p.template.panels.iter().find_map(|p| p.polygon.as_ref()) {
                    let m = poly.meshing_settings();
                    self.mesh_mm = m.target_edge_m * 1000.;
                    self.anisotropy = m.anisotropy;
                    self.mesh_axis = m.axis_degrees;
                }
                self.authoring.history.record(self.project.template.clone());
                self.authoring.board.fitted = false;
                self.authoring.replace = false;
                self.authoring.draft.clear();
                self.authoring.tangents.clear();
                self.authoring.edge = None;
                self.authoring.board.clear_preview();
                self.authoring.preview = None;
                self.authoring.preview_state = None;
                self.authoring.error = None;
                self.authoring.fabric_draft = None;
                self.authoring.drag = None;
                self.authoring.ends.clear();
                self.authoring.selected = p.template.panels.first().map(|p| p.id.clone());
                self.authoring.drafting.line_panel = if self.authoring.tool == Tool::InternalLine {
                    self.authoring.selected.clone()
                } else {
                    None
                };
                self.scene = scene;
                self.project = p;
                self.generation += 1;
                self.playing = false;
                self.pending = false;
                self.metrics = None;
                self.grab = None;
                self.drag_plane = None;
                self.status = "New arrangement · run to assemble the seams".into();
            }
            Err(e) => self.status = e,
        }
    }
    fn preset(&mut self, kind: GarmentKind) {
        self.load(FittingProject::basic(
            kind,
            2,
            self.project.avatar_path.clone(),
        ));
    }
    fn reset(&mut self) {
        let mut p = self.project.clone();
        p.state = None;
        p.mesh_signature = None;
        p.prearrange = false;
        self.load(p);
    }
    fn remesh(&mut self) {
        let mut p = self.project.clone();
        p.state = None;
        p.mesh_signature = None;
        p.prearrange = false;
        for panel in &mut p.template.panels {
            if let Some(poly) = &mut panel.polygon {
                let mut m = poly.meshing_settings();
                m.target_edge_m = self.mesh_mm * 0.001;
                m.anisotropy = self.anisotropy;
                m.axis_degrees = self.mesh_axis;
                poly.meshing = Some(m);
            }
        }
        self.load(p);
    }
    fn export(&mut self) {
        self.commit_placement();
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(&self.output)?;
            export::write_artifacts(
                &self.project.template,
                &self.scene.garment,
                &self.scene.state,
                &self.output,
            )?;
            let mut p = self.project.clone();
            p.state = Some(self.scene.state.clone());
            p.save(&self.output.join("fitting-project.json"))?;
            Ok(())
        })();
        self.status = result.map_or_else(
            |e| e.to_string(),
            |_| format!("Export: {}", self.output.display()),
        );
    }
    fn pick_and_drag(&mut self, response: &egui::Response, rect: egui::Rect) {
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            let (origin, dir) = self.camera.ray(pos, rect);
            let mut best = f64::INFINITY;
            let mut picked = None;
            for t in &self.scene.garment.mesh.triangles {
                if let Some((d, w)) =
                    picking::ray_triangle(origin, dir, t.vertices.map(|i| self.scene.state.x[i]))
                    && d < best
                {
                    best = d;
                    picked = Some((t.vertices, w));
                }
            }
            if picked.is_some() && self.scene.collider.occludes_ray(origin, dir, best - 1e-5) {
                picked = None;
            }
            if let Some((vertices, weights)) = picked {
                let target = origin + best * dir;
                self.grab = Some(Grab {
                    vertices,
                    weights,
                    target,
                    stiffness: 150.,
                });
                self.drag_plane = Some((target, dir));
                self.playing = true;
            }
        }
        if response.dragged_by(egui::PointerButton::Primary)
            && let (Some(g), Some((point, normal)), Some(pos)) = (
                &mut self.grab,
                self.drag_plane,
                response.interact_pointer_pos(),
            )
        {
            let (origin, dir) = self.camera.ray(pos, rect);
            if let Some(target) = picking::ray_plane(origin, dir, point, normal) {
                g.target = target;
            }
        }
        if !response.ctx.input(|i| i.pointer.primary_down()) {
            self.grab = None;
            self.drag_plane = None;
        }
    }
}
impl eframe::App for Atelier {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Some(smoke) = &mut self.authoring_smoke {
            smoke.input(input);
        }
        if !self.drag_test {
            return;
        }
        let Some(origin) = self.smoke_point else {
            return;
        };
        self.smoke_tick += 1;
        let tick = self.smoke_tick;
        let pos = origin + Vec2::new((tick.saturating_sub(2).min(12) * 5) as f32, 0.);
        if tick <= 15 {
            input.events.push(egui::Event::PointerMoved(pos));
        }
        if tick == 2 || tick == 15 {
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: tick == 2,
                modifiers: Default::default(),
            });
        }
        if tick >= 15 {
            self.playing = false;
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frames += 1;
        self.authoring.hover_edge = None;
        let done = self.receive.lock().unwrap().take();
        if let Some(done) = done.filter(|d| d.generation == self.generation) {
            self.pending = false;
            match done.result {
                Ok((state, m)) => {
                    self.smoke_applied |= done.had_grab;
                    self.scene.state = state;
                    self.status = "Dynamic simulation · OGC contacts".into();
                    self.metrics = Some(m);
                }
                Err(e) => {
                    self.playing = false;
                    self.status = e;
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !ctx.egui_wants_keyboard_input() {
            if self.playing {
                self.playing = false;
            } else if self.prepare_simulation() {
                self.playing = true;
            }
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    workspace::ACCENT,
                    egui::RichText::new("CLOTHCRAFT").strong().size(18.),
                );
                ui.label(egui::RichText::new("ALPHA").small().color(Color32::GRAY))
                    .on_hover_text("Research prototype · most production features remain to be built");
                ui.separator();
                let play = ui.button(if self.playing {
                    "⏸ Pause"
                } else if self.authoring.ends.len() == 2 {
                    "▶ Sew and simulate"
                } else {
                    "▶ Simulate"
                });
                self.smoke_target("simulate", play.rect.center());
                if play.clicked() {
                    if self.playing {
                        self.playing = false;
                    } else if self.prepare_simulation() {
                        self.playing = true;
                    }
                }
                ui.menu_button("Simulation", |ui| {
                    if ui.add_enabled(!self.pending && !self.playing, egui::Button::new("Advance one step")).clicked()
                        && self.prepare_simulation() { self.step(&ctx); ui.close(); }
                    if ui.button("Reset the flat arrangement").on_hover_text("Resets the drape of every panel").clicked() {
                        self.reset(); ui.close();
                    }
                });
                let export_button = ui.button("Export").on_hover_text("Saves the JSON project, the patterns, the OBJ garment and the previews into the session folder.");
                self.smoke_target("export", export_button.rect.center());
                if export_button.clicked() {
                    self.export();
                }
                ui.separator();
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.wire, "Wireframe");
                    ui.checkbox(&mut self.heat, "Tension");
                    ui.checkbox(&mut self.seam_links, "2D + 3D seam links");
                    ui.checkbox(&mut self.seams, "All seam samples (diagnostic)");
                });
                ui.menu_button("Examples", |ui| {
                    if ui.button("T-shirt").clicked() {
                        self.preset(GarmentKind::Tshirt);
                        ui.close();
                    }
                    if ui.button("Dress").clicked() {
                        self.preset(GarmentKind::Dress);
                        ui.close();
                    }
                });
                self.online_patterns(ui);
                ui.separator();
                if ui
                    .button("Settings")
                    .on_hover_text("Preferences: theme, display and solver defaults")
                    .clicked()
                {
                    self.settings_open = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.small(format!("{} panels · {} seams", self.project.template.panels.len(), self.project.template.seams.len()));
                });
            });
        });
        let mut changed = false;
        settings::window(&ctx, &mut self.settings_open, &mut self.prefs, &mut changed);
        if changed {
            // Apply immediately rather than on the next start: the window edits
            // the live `Preferences`, so anything less would leave the theme
            // switch looking broken until a restart. `apply` is idempotent.
            settings::apply(&ctx, &self.prefs);
            // The board reads its step at startup, so push the edited value
            // through here too; otherwise the control would persist without
            // taking effect, which is a stored value pretending to be live.
            self.authoring.board.snap_mm = self.prefs.snap_mm;
            // A failed write must not interrupt the session: the status line
            // reports it so the user knows the preference did not stick.
            if let Err(error) = settings::save(&self.output.join(settings::FILE_NAME), &self.prefs)
            {
                self.status = format!("Preferences could not be saved: {error}");
            }
        }
        egui::Panel::top("authoring-tools").show(ui, |ui| self.authoring_toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(if self.playing { workspace::ACCENT } else { Color32::GRAY },
                    if self.playing { "Simulation" } else { "Pause" });
                ui.separator();
                ui.add(egui::Label::new(&self.status).truncate()).on_hover_text(&self.status);
            });
            if let Some(m) = &self.metrics {
                ui.small(format!(
                    "Seams: max gap {:.1} mm · Stretch: 95% of the surface ≤ {:.1}% (max {:.1}%) · Step {:.1} ms",
                    1000. * m.max_seam_gap_m, m.extension_p95_percent, m.extension_max_percent, m.step_ms
                ));
            }
        });
        egui::Panel::right("properties")
            .default_size(290.)
            .size_range(250.0..=420.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.strong("OBJECTS & PROPERTIES");
                    ui.separator();
                    self.panel_browser(ui);
                    self.authoring_properties(ui);
                    ui.collapsing("Fabric resolution", |ui| {
                        ui.add(egui::Slider::new(&mut self.mesh_mm, 8.0..=60.0).text("mm"));
                        if ui.button("Remesh").clicked() {
                            self.remesh();
                        }
                        ui.small(format!(
                            "{} vertices · {} triangles",
                            self.scene.state.x.len(),
                            self.scene.garment.mesh.triangles.len()
                        ));
                    });
                    ui.collapsing("Advanced settings", |ui| {
                        ui.checkbox(&mut self.gpu, "GPU compute");
                        ui.add(
                            egui::Slider::new(&mut self.settings.iterations, 2..=128)
                                .text("iterations"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.settings.damping, 0.0..=10.0)
                                .text("damping"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.anisotropy, 1.0..=4.0).text("anisotropy"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.mesh_axis, 0.0..=180.0).text("mesh axis"),
                        );
                        ui.small("Hybrid OGC · frictionless · uncalibrated material");
                        if let Some(m) = &self.metrics {
                            ui.small(format!(
                                "Step {:.2} ms · detection {:.2} ms · seams {:.2} mm",
                                m.step_ms,
                                m.detection_ms,
                                m.max_seam_gap_m * 1000.
                            ));
                        }
                    });
                    self.export_information(ui);
                });
            });
        egui::Panel::right("patterns")
            .default_size((ui.available_width() * 0.45).max(280.))
            .size_range(260.0..=900.0)
            .resizable(true)
            .show(ui, |ui| {
                self.patterns(ui);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("3D GARMENT");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("Overview")
                        .on_hover_text("Reframe the mannequin")
                        .clicked()
                    {
                        self.camera = camera();
                    }
                });
            });
            self.show_view_bar(ui);
            ui.small(view_controls::hint_3d(self.authoring.tool));
            let (rect, response) = ui.allocate_exact_size(
                ui.available_size().max(Vec2::splat(1.)),
                egui::Sense::click_and_drag(),
            );
            if self.authoring.tool == Tool::Grab {
                self.pick_and_drag(&response, rect);
            }
            self.place_interaction(&response, rect);
            self.edge_interaction(&response, rect);
            self.smoke_grabbed |= self.grab.is_some();
            if self.drag_test && self.smoke_point.is_none() {
                let aim = glam::DVec3::new(0., 1.12, 0.30);
                let front = &self.scene.garment.panels[0];
                let point = self.scene.garment.mesh.triangles[front.triangles.clone()]
                    .iter()
                    .map(|t| {
                        t.vertices
                            .iter()
                            .map(|i| self.scene.state.x[*i] / 3.)
                            .sum::<glam::DVec3>()
                    })
                    .min_by(|a, b| a.distance_squared(aim).total_cmp(&b.distance_squared(aim)))
                    .unwrap();
                let clip =
                    self.camera.matrix(rect.width() / rect.height()) * point.as_vec3().extend(1.);
                let ndc = clip.truncate() / clip.w;
                self.smoke_point = Some(egui::pos2(
                    rect.center().x + ndc.x * rect.width() * 0.5,
                    rect.center().y - ndc.y * rect.height() * 0.5,
                ));
            }
            self.camera.garment_interact(&response);
            let size = rect.size() * ctx.pixels_per_point();
            let texture = self.viewport.draw_scene(
                [size.x as u32, size.y as u32],
                &self.camera,
                DrawModel {
                    mesh: &self.scene.garment.mesh,
                    state: &self.scene.state,
                    materials: &self.scene.garment.materials,
                    sphere: None,
                    avatar: Some(&self.scene.collider.surface),
                    collider: None,
                    seams: if self.seams {
                        &self.scene.garment.seams
                    } else {
                        &[]
                    },
                },
                self.wire,
                self.heat,
                false,
            );
            ui.painter().image(
                texture,
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                Color32::WHITE,
            );
            self.paint_edges_3d(ui, rect);
            self.paint_seam_links(&ui.painter().with_clip_rect(rect), rect, true);
            self.paint_gizmo(ui, rect);
            if let Some(g) = &self.grab {
                let clip = self.camera.matrix(rect.width() / rect.height())
                    * g.target.as_vec3().extend(1.);
                if clip.w > 0. {
                    let ndc = clip.truncate() / clip.w;
                    let p = egui::pos2(
                        rect.center().x + ndc.x * rect.width() * 0.5,
                        rect.center().y - ndc.y * rect.height() * 0.5,
                    );
                    ui.painter().circle_stroke(
                        p,
                        8.,
                        Stroke::new(2., Color32::from_rgb(255, 194, 64)),
                    );
                }
            }
        });
        if let Some(perf) = &mut self.perf
            && let Some(report) = perf.tick(
                self.scene.state.time,
                ctx.cumulative_frame_nr(),
                self.settings.dt,
                self.metrics.is_some(),
            )
        {
            self.playing = false;
            if let Err(e) = std::fs::write(
                self.output.join("ui-perf.json"),
                serde_json::to_string_pretty(&report).unwrap(),
            ) {
                eprintln!("UI performance report: {e}");
            }
        }
        self.submit(&ctx, false);
        if self.playing {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.pending {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.authoring_smoke_tick();
        if self.smoke {
            let drag_finished = self.drag_test && self.smoke_tick >= 20 && !self.pending;
            if !self.smoke_captured
                && ((self.authoring_smoke.is_none()
                    && !self.drag_test
                    && self.perf.is_none()
                    && self.frames == 8)
                    || self.authoring_smoke.as_ref().is_some_and(|s| s.finished)
                    || drag_finished
                    || (self.authoring_smoke.is_none() && self.perf.is_none() && self.frames > 110)
                    || self.perf.as_ref().is_some_and(|p| p.finished))
            {
                self.smoke_captured = true;
                if self.drag_test {
                    let displacement = self
                        .scene
                        .state
                        .x
                        .iter()
                        .zip(&self.smoke_initial)
                        .map(|(a, b)| a.distance(*b))
                        .fold(0., f64::max);
                    let report = serde_json::json!({"passed":self.smoke_grabbed&&self.smoke_applied&&displacement>1e-5,"pointer_picked_cloth":self.smoke_grabbed,"accepted_step_with_grab":self.smoke_applied,"max_vertex_displacement_m":displacement,"status":self.status,"state_time_s":self.scene.state.time});
                    if let Err(e) = std::fs::write(
                        self.output.join("interaction.json"),
                        serde_json::to_string_pretty(&report).unwrap(),
                    ) {
                        eprintln!("Interaction report: {e}");
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Err(e) = image::save_buffer(
                        self.output.join("ui.png"),
                        &pixels,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("UI capture: {e}");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if (self.authoring_smoke.is_none() && self.perf.is_none() && self.frames > 120)
                || self.perf.as_ref().is_some_and(|p| p.expired())
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
}
