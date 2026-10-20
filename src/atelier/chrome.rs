//! Compact viewport headers and contextual inspector, independent of solver choice.
use super::*;
use crate::render::appearance::{AntiAliasing, Appearance};
use layout::ViewMode;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InspectorTab {
    Piece,
    Fabric,
    Simulation,
}

/// Shared chrome stays usable at the minimum split width; content belongs below it.
pub(super) struct ViewHeader {
    pub expand: bool,
    pub fit: bool,
    pub settings: egui::Response,
    pub camera: Option<egui::Response>,
}
pub(super) fn view_header(
    ui: &mut egui::Ui,
    title: &str,
    expanded: bool,
    is_3d: bool,
    help: &str,
) -> ViewHeader {
    let compact = ui.available_width() < 430.;
    ui.horizontal(|ui| {
        ui.strong(if compact {
            if is_3d { "3D" } else { "2D" }
        } else {
            title
        })
        .on_hover_text(help);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let expand = ui
                .small_button(if expanded { "Split" } else { "Expand" })
                .clicked();
            let settings = icons::dropdown_button(ui, icons::Icon::Settings, "Display options");
            let fit = icons::icon_only(ui, icons::Icon::Fit, 26., false, "Frame all").clicked();
            let camera = is_3d.then(|| {
                icons::dropdown_button(
                    ui,
                    icons::Icon::ViewIso,
                    "Camera · standard views, projection and framing",
                )
            });
            ViewHeader {
                expand,
                fit,
                settings,
                camera,
            }
        })
        .inner
    })
    .inner
}

/// Shared by the real popup and CPU-only presentation tests.
pub(super) fn appearance_controls(ui: &mut egui::Ui, value: &mut Appearance, msaa4: bool) {
    ui.set_min_width(240.);
    ui.strong("VIEWPORT DISPLAY");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Anti-aliasing");
        ui.selectable_value(&mut value.anti_aliasing, AntiAliasing::Off, "Off");
        ui.add_enabled_ui(msaa4, |ui| {
            ui.selectable_value(&mut value.anti_aliasing, AntiAliasing::Msaa4, "4× MSAA");
        })
        .response
        .on_hover_text("Smooth silhouette edges; uses more graphics memory");
    });
    if !msaa4 {
        ui.small("4× MSAA is unavailable on this adapter.");
    }
    ui.separator();
    ui.strong("Background");
    ui.horizontal(|ui| {
        for (name, top, bottom, floor) in [
            ("Studio", [60, 67, 78], [116, 126, 140], [98, 107, 119]),
            ("Light", [190, 199, 210], [231, 235, 240], [196, 203, 212]),
            ("Dark", [27, 30, 36], [65, 72, 83], [54, 61, 71]),
        ] {
            if ui.small_button(name).clicked() {
                value.top = top;
                value.bottom = bottom;
                value.floor_color = floor;
            }
        }
    });
    ui.checkbox(&mut value.gradient, "Gradient");
    ui.horizontal(|ui| {
        ui.color_edit_button_srgb(&mut value.top);
        ui.label(if value.gradient {
            "Top color"
        } else {
            "Background color"
        });
    });
    if value.gradient {
        ui.horizontal(|ui| {
            ui.color_edit_button_srgb(&mut value.bottom);
            ui.label("Bottom color");
        });
    }
    ui.separator();
    ui.checkbox(&mut value.floor, "Ground plane");
    ui.add_enabled_ui(value.floor, |ui| {
        ui.horizontal(|ui| {
            ui.color_edit_button_srgb(&mut value.floor_color);
            ui.label("Ground color");
        });
        ui.checkbox(&mut value.grid, "Ground grid");
        ui.checkbox(&mut value.shadow, "Soft ground shadow")
            .on_hover_text(
                "Soft contact-style preview under the model. Does not affect simulation.",
            );
        if value.shadow {
            ui.add(egui::Slider::new(&mut value.shadow_opacity, 0.0..=0.8).text("Opacity"));
        }
    });
    ui.separator();
    if ui.small_button("Reset display").clicked() {
        *value = Appearance::default();
    }
}

impl Atelier {
    pub(super) fn garment_header(&mut self, ui: &mut egui::Ui) {
        let header = view_header(
            ui,
            "3D GARMENT",
            self.prefs.workspace.mode == ViewMode::Garment,
            true,
            view_controls::hint_3d(self.authoring.tool),
        );
        if header.expand {
            self.prefs.workspace.toggle_focus(ViewMode::Garment);
        }
        if header.fit {
            let bounds = self.view_bounds();
            if let Some((lo, hi)) = bounds.all {
                self.camera
                    .request_frame((lo + hi) * 0.5, (hi - lo).length() * 0.5);
            }
        }
        egui::Popup::menu(&header.settings).show(|ui| {
            appearance_controls(ui, &mut self.prefs.viewport, self.viewport.supports_msaa4());
            ui.separator();
            ui.strong("Overlays");
            ui.checkbox(&mut self.clean_view, "Clean view · hide all annotations")
                .on_hover_text("Show only the shaded garment and avatar. Keeps your overlay settings for later.");
            ui.checkbox(&mut self.wire, "Wireframe");
            ui.checkbox(&mut self.heat, "Strain map");
            ui.checkbox(&mut self.seam_links, "Seam links");
        });
        egui::Popup::menu(&header.camera.unwrap()).show(|ui| {
            ui.set_max_width(330.);
            self.show_view_bar(ui);
        });
    }

    pub(super) fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("PROPERTIES");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("Hide inspector · restore from Workspace menu")
                    .clicked()
                {
                    self.prefs.workspace.inspector_open = false;
                }
            });
        });
        ui.separator();
        self.panel_browser(ui);
        ui.horizontal(|ui| {
            for (tab, label) in [
                (InspectorTab::Piece, "Piece"),
                (InspectorTab::Fabric, "Fabric"),
                (InspectorTab::Simulation, "Simulation"),
            ] {
                let button = ui.selectable_value(&mut self.inspector_tab, tab, label);
                if tab == InspectorTab::Fabric {
                    self.smoke_target("fabric_tab", button.rect.center());
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("inspector-body")
            .show(ui, |ui| match self.inspector_tab {
                InspectorTab::Piece => {
                    self.authoring_properties(ui);
                    self.drafting_tools(ui);
                }
                InspectorTab::Fabric => {
                    if let Some(panel) = self
                        .project
                        .template
                        .panels
                        .iter()
                        .find(|p| Some(&p.id) == self.authoring.selected.as_ref())
                        .cloned()
                    {
                        ui.label(&panel.name);
                        self.fabric_properties(ui, &panel);
                    } else {
                        ui.label("Select a piece to edit its fabric.");
                    }
                }
                InspectorTab::Simulation => {
                    ui.strong("Mesh resolution");
                    ui.add(egui::Slider::new(&mut self.mesh_mm, 8.0..=60.0).text("mm"));
                    if ui.button("Remesh").clicked() {
                        self.remesh();
                    }
                    ui.small(format!(
                        "{} vertices · {} triangles",
                        self.scene.state.x.len(),
                        self.scene.garment.mesh.triangles.len()
                    ));
                    ui.separator();
                    ui.strong("Solver settings");
                    ui.checkbox(&mut self.gpu, "GPU compute");
                    ui.add(
                        egui::Slider::new(&mut self.settings.iterations, 2..=128)
                            .text("Iterations"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.settings.damping, 0.0..=10.0).text("Damping"),
                    );
                    ui.collapsing("Advanced meshing", |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.anisotropy, 1.0..=4.0).text("Anisotropy"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.mesh_axis, 0.0..=180.0).text("Mesh axis"),
                        );
                    });
                    ui.collapsing("Diagnostics", |ui| {
                        ui.checkbox(&mut self.seams, "All seam samples");
                        ui.small("Hybrid VBD / OGC · frictionless · uncalibrated materials");
                        if let Some(m) = &self.metrics {
                            ui.small(format!(
                                "Step {:.2} ms · detection {:.2} ms · seam gap {:.2} mm",
                                m.step_ms,
                                m.detection_ms,
                                1000. * m.max_seam_gap_m
                            ));
                            ui.small(format!(
                                "Extension: 95% of rest area ≤ {:.1}% · max {:.1}%",
                                m.extension_p95_percent, m.extension_max_percent
                            ));
                        }
                    });
                    ui.collapsing("Motion / jitter", |ui| {
                        let mut inspect = false;
                        if let Some(m) = self.metrics.as_ref().and_then(|m| m.motion.as_ref()) {
                            ui.small(format!("High-pass RMS {:.3} mm · max {:.3} mm", m.highpass_rms_m * 1000., m.highpass_max_m * 1000.));
                            ui.small(format!("{} vertices with persistent oscillation", m.oscillating_vertices));
                            if m.observed_seconds < 0.75 { ui.small("Collecting physical-step history…"); }
                            inspect = ui.add_enabled(!m.hotspots.is_empty(), egui::Button::new("Inspect jitter hotspots"))
                                .on_hover_text("Pause and mark faces around up to 12 strongest detected vertices. Export report includes motion measurements.").clicked();
                        } else {
                            ui.small("Run simulation to measure motion.");
                        }
                        ui.small("Repeated reversals, not rendering FPS. Real fabric vibration can also trigger this indicator.");
                        if inspect { self.inspect_motion(); }
                    });
                }
            });
    }
}

impl Atelier {
    pub(super) fn garment_view(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.garment_header(ui);
        if let Some(error) = self.simulation_error.clone() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.colored_label(Color32::from_rgb(225, 120, 75), "SIMULATION PAUSED");
                ui.label(&error);
                ui.horizontal(|ui| {
                    if ui.small_button("Inspect problem faces").clicked() {
                        self.set_authoring_tool(Tool::Inspect, false);
                    }
                    if ui.small_button("Copy details").clicked() {
                        ui.ctx().copy_text(error);
                    }
                });
            });
        }
        self.sewing_strip(ui);
        if self.authoring.tool == Tool::Grab {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Grab");
                ui.add(egui::Slider::new(&mut self.grab_radius_px, 0.0..=150.0).text("Radius (px)"))
                    .on_hover_text("Size on screen. Zero uses the original point grab. The brush follows the picked piece's surface, avoiding nearby disconnected layers.");
                ui.add(egui::Slider::new(&mut self.grab_strength, 0.25..=20.).logarithmic(true).text("Strength"))
                    .on_hover_text("1× is the original point spring strength. Higher values pull harder; collision limits still apply. Changes apply to the next drag.");
            });
        }
        if self.authoring.tool == Tool::Inspect {
            self.issue_controls(ui);
        }
        ui.separator();
        self.viewport.appearance = self.prefs.viewport.clone();
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size().max(Vec2::splat(1.)),
            egui::Sense::click_and_drag(),
        );
        self.authoring.selection.garment_rect = Some(rect);
        if !self.authoring.radial.blocked && !self.quick.blocked {
            self.sewing_wheel(&response);
            if response.secondary_clicked()
                && !self.authoring.board.gesture_active()
                && self.authoring.drag.is_none()
                && self.grab.is_none()
                && let Some(at) = response.interact_pointer_pos()
            {
                self.authoring.radial.open(&ctx, at);
            }
            if self.authoring.tool == Tool::Grab {
                self.pick_and_drag(&response, rect);
            }
            self.place_interaction(&response, rect);
            self.edge_interaction(&response, rect);
        }
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
        if !self.authoring.radial.blocked && !self.quick.blocked {
            self.camera.garment_interact(&response);
        }
        let size = rect.size() * ctx.pixels_per_point();
        let texture = self.viewport.draw_scene(
            [size.x as u32, size.y as u32],
            &self.camera,
            DrawModel {
                marked_faces: if self.clean_view {
                    None
                } else {
                    Some(&self.issue.faces)
                },
                mesh: &self.scene.garment.mesh,
                state: &self.scene.state,
                materials: &self.scene.garment.materials,
                sphere: None,
                avatar: Some(&self.scene.collider.surface),
                collider: None,
                seams: if self.seams && !self.clean_view {
                    &self.scene.garment.seams
                } else {
                    &[]
                },
            },
            self.wire && !self.clean_view,
            self.heat && !self.clean_view,
            false,
        );
        ui.painter().image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        if !self.clean_view {
            self.issue_brush(ui, &response, rect);
            self.paint_edges_3d(ui, rect);
            self.paint_seam_links(&ui.painter().with_clip_rect(rect), rect, true);
            self.paint_sewing_ranges(ui, rect, true);
            self.paint_gizmo(ui, rect);
            self.paint_selected_points_3d(ui, rect);
            if self.authoring.tool == Tool::Grab
                && let Some(p) = response.hover_pos()
            {
                ui.painter().with_clip_rect(rect).circle_stroke(
                    p,
                    self.grab_radius_px.max(4.),
                    Stroke::new(1.5, Color32::from_rgb(255, 194, 64)),
                );
            }
        }
        if !self.clean_view
            && let Some(g) = &self.grab
        {
            let clip =
                self.camera.matrix(rect.width() / rect.height()) * g.target.as_vec3().extend(1.);
            if clip.w > 0. {
                let ndc = clip.truncate() / clip.w;
                let p = egui::pos2(
                    rect.center().x + ndc.x * rect.width() * 0.5,
                    rect.center().y - ndc.y * rect.height() * 0.5,
                );
                ui.painter()
                    .circle_stroke(p, 8., Stroke::new(2., Color32::from_rgb(255, 194, 64)));
            }
        }
    }
}
