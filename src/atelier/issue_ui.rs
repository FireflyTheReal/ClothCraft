//! Paused, visibility-aware face painting. Mesh indices live only in this UI cache.
use super::*;
use std::collections::BTreeSet;

pub(super) struct IssueUi {
    pub faces: BTreeSet<usize>,
    pub radius: f32,
    note: String,
    previous: Option<egui::Pos2>,
    pub last_export: Option<PathBuf>,
}
impl Default for IssueUi {
    fn default() -> Self {
        Self {
            faces: BTreeSet::new(),
            radius: 18.,
            note: String::new(),
            previous: None,
            last_export: None,
        }
    }
}
impl IssueUi {
    pub fn invalidate(&mut self) {
        self.faces.clear();
        self.previous = None;
    }
}

impl Atelier {
    pub(super) fn inspect_motion(&mut self) {
        let Some(motion) = self.metrics.as_ref().and_then(|m| m.motion.as_ref()) else {
            return;
        };
        let g = &self.scene.garment;
        let vertices: BTreeSet<_> = motion
            .hotspots
            .iter()
            .filter_map(|h| {
                let p = g.panels.iter().find(|p| p.id == h.panel)?;
                p.vertices
                    .clone()
                    .find(|&v| g.mesh.uv[v].distance_squared(h.material_point_m) < 1e-20)
            })
            .collect();
        self.issue.faces = g
            .mesh
            .triangles
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.vertices.iter().any(|v| vertices.contains(v)).then_some(i))
            .collect();
        self.issue.note = format!(
            "Persistent oscillation: {} vertices; global high-pass RMS {:.4} mm; maximum vertex RMS {:.4} mm. Inspect numerical jitter versus physical motion.",
            motion.oscillating_vertices,
            motion.highpass_rms_m * 1000.,
            motion.highpass_max_m * 1000.
        );
        self.set_authoring_tool(Tool::Inspect, false);
    }

    pub(super) fn issue_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("{} faces marked", self.issue.faces.len()));
            ui.add(egui::Slider::new(&mut self.issue.radius, 4. ..=48.).text("Brush px"));
            if ui.small_button("Clear").clicked() {
                self.issue.invalidate();
            }
        });
        ui.small("Paint visible cloth · Shift erases · right drag orbits · simulation paused");
        ui.add(
            egui::TextEdit::singleline(&mut self.issue.note)
                .hint_text("What goes wrong here? (optional)")
                .char_limit(2000),
        );
        ui.horizontal(|ui| {
            let export = ui.button("Export report");
            self.smoke_target("issue_export", export.rect.center());
            if export.clicked() {
                self.export_issue(ui.ctx());
            }
            if let Some(path) = &self.issue.last_export
                && ui.small_button("Copy path").clicked()
            {
                ui.ctx().copy_text(path.display().to_string());
            }
            if ui.small_button("Done").clicked() {
                self.set_authoring_tool(Tool::Edit, false);
            }
        });
        ui.small("Local capture only. Share the exported folder path to request debugging.");
        if !self.authoring.draft.is_empty()
            || !self.authoring.ends.is_empty()
            || self.authoring.sewing.sides.iter().any(|s| !s.is_empty())
        {
            ui.small("Uncommitted drawing and sewing previews are not included in the report.");
        }
    }

    pub(super) fn export_issue(&mut self, ctx: &egui::Context) {
        // Capture the displayed accepted state, never a later worker result.
        self.playing = false;
        self.generation += 1;
        self.pending = false;
        self.submit(ctx, false);
        let root = PathBuf::from("output/diagnostics");
        let result = marvelous_cloth::issue_report::export(
            &root,
            marvelous_cloth::issue_report::Capture {
                project: &self.project,
                scene: &self.scene,
                settings: &self.settings,
                marked: &self.issue.faces,
                note: &self.issue.note,
                backend: if self.gpu {
                    "hybrid-cpu-ogc-gpu-vbd"
                } else {
                    "cpu-vbd-ogc"
                },
                metrics: self.metrics.as_ref(),
            },
        );
        match result {
            Ok(path) => {
                let path = std::path::absolute(&path).unwrap_or(path);
                ctx.copy_text(path.display().to_string());
                self.status = format!("Report saved · path copied · {}", path.display());
                self.issue.last_export = Some(path);
            }
            Err(e) => self.status = format!("Report failed: {e}"),
        }
    }

    pub(super) fn issue_brush(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
    ) {
        if self
            .authoring_smoke
            .as_ref()
            .is_some_and(|s| s.wants("issue_face"))
        {
            for triangle in &self.scene.garment.mesh.triangles {
                let world = triangle
                    .vertices
                    .iter()
                    .map(|v| self.scene.state.x[*v] / 3.)
                    .sum::<glam::DVec3>();
                if let Some(at) = self.camera.project(world.as_vec3(), rect)
                    && rect.shrink(30.).contains(at)
                {
                    let (origin, direction) = self.camera.ray(at, rect);
                    if marvelous_cloth::picking::visible_cloth_face(&self.scene, origin, direction)
                        .is_some()
                    {
                        self.smoke_target("issue_face", at);
                        break;
                    }
                }
            }
        }
        if self.authoring.tool != Tool::Inspect
            || self.authoring.radial.blocked
            || self.quick.blocked
        {
            return;
        }
        let Some(pointer) = response.hover_pos() else {
            self.issue.previous = None;
            return;
        };
        let erase = ui.input(|i| i.modifiers.shift);
        let color = if erase {
            Color32::WHITE
        } else {
            Color32::from_rgb(255, 100, 65)
        };
        ui.painter().with_clip_rect(rect).circle_stroke(
            pointer,
            self.issue.radius,
            Stroke::new(1.5, color),
        );
        if !(response.dragged_by(egui::PointerButton::Primary)
            || response.clicked()
            || response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()))
        {
            self.issue.previous = None;
            return;
        }
        if self
            .issue
            .previous
            .is_some_and(|p| p.distance(pointer) < 2.)
        {
            return;
        }
        let from = self.issue.previous.unwrap_or(pointer);
        let steps =
            ((from.distance(pointer) / (self.issue.radius * 0.5)).ceil() as usize).clamp(1, 16);
        for k in 1..=steps {
            let center = from.lerp(pointer, k as f32 / steps as f32);
            for sample in 0..9 {
                let at = if sample == 0 {
                    center
                } else {
                    center
                        + Vec2::angled((sample - 1) as f32 * std::f32::consts::TAU / 8.)
                            * self.issue.radius
                            * 0.7
                };
                if !rect.contains(at) {
                    continue;
                }
                let (origin, direction) = self.camera.ray(at, rect);
                if let Some(face) =
                    marvelous_cloth::picking::visible_cloth_face(&self.scene, origin, direction)
                {
                    if erase {
                        self.issue.faces.remove(&face);
                    } else {
                        self.issue.faces.insert(face);
                    }
                }
            }
        }
        self.issue.previous = Some(pointer);
        ui.ctx().request_repaint();
    }
}
