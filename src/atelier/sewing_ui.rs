//! Shared sewing draft for 2D/3D: ranges and ordered sides remain material references.
use super::{authoring_ui::Tool, board::same_edge, *};
use marvelous_cloth::{
    authoring::Command,
    garment::seam::{BoundaryCurve, BoundaryRef},
};

#[derive(Default)]
pub(super) struct SewingDraft {
    pub chain_mode: bool,
    pub sides: [Vec<BoundaryRef>; 2],
    pub active: usize,
    pub editing: Option<String>,
    pub partial: bool,
    pub start: Option<(BoundaryRef, f64)>,
}
impl Atelier {
    /// Closest point in screen space, converted back to normalized rest arclength.
    pub(super) fn boundary_parameter(
        &self,
        edge: &BoundaryRef,
        rect: egui::Rect,
        pointer: egui::Pos2,
        in_3d: bool,
    ) -> f64 {
        let Some(i) = self
            .scene
            .garment
            .panels
            .iter()
            .position(|p| p.id == edge.panel)
        else {
            return 0.;
        };
        let panel = &self.scene.garment.panels[i];
        let ids = if let Some(name) = &edge.edge {
            panel.boundaries.get(name).cloned().unwrap_or_default()
        } else {
            self.project.template.panels[i].boundary_vertices(edge.side, panel.vertices.start)
        };
        let matrix = self.camera.matrix(rect.width() / rect.height());
        let project = |id: usize| {
            if in_3d {
                super::gizmo::project_matrix(matrix, rect, self.scene.state.x[id])
            } else {
                Some(self.authoring.board.screen(
                    rect,
                    self.board_origin(i)
                        + marvelous_cloth::authoring::rotate_2d(
                            self.scene.garment.mesh.uv[id],
                            self.board_angle(i),
                        ),
                ))
            }
        };
        let mut total = 0.;
        let mut best = (f32::INFINITY, 0.);
        for pair in ids.windows(2) {
            let length =
                self.scene.garment.mesh.uv[pair[0]].distance(self.scene.garment.mesh.uv[pair[1]]);
            if let (Some(a), Some(b)) = (project(pair[0]), project(pair[1])) {
                let v = b - a;
                let mut t = ((pointer - a).dot(v) / v.length_sq().max(1e-12)).clamp(0., 1.);
                let distance = pointer.distance(a + v * t);
                if in_3d {
                    let wa = (matrix * self.scene.state.x[pair[0]].as_vec3().extend(1.)).w;
                    let wb = (matrix * self.scene.state.x[pair[1]].as_vec3().extend(1.)).w;
                    t = (t / wb) / ((1. - t) / wa + t / wb);
                }
                if distance < best.0 {
                    best = (distance, total + length * f64::from(t));
                }
            }
            total += length;
        }
        if total > 0. {
            (best.1 / total).clamp(0., 1.)
        } else {
            0.
        }
    }
    pub(super) fn paint_sewing_ranges(&mut self, ui: &egui::Ui, rect: egui::Rect, in_3d: bool) {
        let painter = ui.painter().with_clip_rect(rect);
        let mut sides = self.sewing_sides();
        let draft = sides.iter().enumerate().flat_map(|(s, rs)| {
            rs.iter()
                .enumerate()
                .map(move |(i, r)| (r.clone(), Color32::from_rgb(248, 117, 188), Some((s, i))))
        });
        let stored = self.project.template.seams.iter().flat_map(|s| {
            s.references()
                .map(|r| (r.clone(), super::seam_links::seam_color(&s.id), None))
        });
        let ranges: Vec<_> = stored.chain(draft).collect();
        let mut updated = false;
        for (r, color, handle) in ranges {
            let Some(curve) = self.display_boundary(&r, false) else {
                continue;
            };
            let i = self
                .scene
                .garment
                .panels
                .iter()
                .position(|p| p.id == r.panel)
                .unwrap();
            let matrix = self.camera.matrix(rect.width() / rect.height());
            let screen = |t| {
                let weights = curve.point(t);
                if in_3d {
                    let p: glam::DVec3 = weights
                        .iter()
                        .map(|(v, w)| self.scene.state.x[*v] * *w)
                        .sum();
                    let (origin, dir) = self
                        .camera
                        .ray(super::gizmo::project_matrix(matrix, rect, p)?, rect);
                    if self
                        .scene
                        .collider
                        .occludes_ray(origin, dir, p.distance(origin) - 0.0001)
                    {
                        return None;
                    }
                    super::gizmo::project_matrix(matrix, rect, p)
                } else {
                    let uv: glam::DVec2 = weights
                        .iter()
                        .map(|(v, w)| self.scene.garment.mesh.uv[*v] * *w)
                        .sum();
                    Some(self.authoring.board.screen(
                        rect,
                        self.board_origin(i)
                            + marvelous_cloth::authoring::rotate_2d(uv, self.board_angle(i)),
                    ))
                }
            };
            let points: Vec<_> = (0..=32).map(|k| screen(k as f64 / 32.)).collect();
            for pair in points.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    painter.line_segment([*a, *b], Stroke::new(3., color));
                }
            }
            for end in 0..2 {
                let Some(pos) = screen(end as f64) else {
                    continue;
                };
                painter.circle_filled(pos, if handle.is_some() { 5. } else { 3. }, color);
                if let Some((side, index)) = handle {
                    let label = format!(
                        "{}{}.{}",
                        if side == 0 { "A" } else { "B" },
                        index + 1,
                        if side == 1 && self.authoring.reverse {
                            1 - end
                        } else {
                            end
                        }
                    );
                    painter.text(
                        pos + Vec2::new(7., -7.),
                        egui::Align2::LEFT_BOTTOM,
                        label,
                        egui::FontId::proportional(12.),
                        color,
                    );
                    if !self.authoring.radial.blocked
                        && self.authoring.tool == Tool::Sew
                        && rect.contains(pos)
                    {
                        let response = ui.interact(
                            egui::Rect::from_center_size(pos, Vec2::splat(14.)).intersect(rect),
                            ui.id().with(("sew-end", side, index, end)),
                            egui::Sense::drag(),
                        );
                        if response.dragged()
                            && let Some(pointer) = response.interact_pointer_pos()
                        {
                            let t = self.boundary_parameter(&r, rect, pointer, in_3d);
                            let physical_end = if r.reversed { 1 - end } else { end };
                            let range = &mut sides[side][index].range;
                            if physical_end == 0 {
                                range[0] = t.min(range[1] - 1e-6).max(0.);
                            } else {
                                range[1] = t.max(range[0] + 1e-6).min(1.);
                            }
                            updated = true;
                        }
                    }
                }
            }
        }
        if updated {
            if self.authoring.sewing.chain_mode {
                self.authoring.sewing.sides = sides;
            } else {
                self.authoring.ends = sides.into_iter().flatten().collect();
            }
        }
    }
    pub(super) fn sewing_sides(&self) -> [Vec<BoundaryRef>; 2] {
        if self.authoring.sewing.chain_mode {
            self.authoring.sewing.sides.clone()
        } else {
            [
                self.authoring.ends.first().cloned().into_iter().collect(),
                self.authoring.ends.get(1).cloned().into_iter().collect(),
            ]
        }
    }
    pub(super) fn sewing_curve(
        &self,
        refs: &[BoundaryRef],
        reverse: bool,
    ) -> Option<BoundaryCurve> {
        BoundaryCurve::concatenate(
            refs.iter()
                .map(|r| self.display_boundary(r, false))
                .collect::<Option<Vec<_>>>()?,
            reverse,
        )
        .ok()
    }
    pub(super) fn pick_sewing_boundary(&mut self, mut edge: BoundaryRef, t: f64) {
        if self.authoring.tool == Tool::Sew && self.authoring.sewing.partial {
            if let Some((start, s)) = self.authoring.sewing.start.take() {
                if !same_edge(&start, &edge) {
                    self.authoring.sewing.start = Some((edge, t));
                    self.status = "Start moved to this boundary · click its end".into();
                    return;
                }
                if (s - t).abs() < 1e-6 {
                    self.status = "Choose two different positions on the boundary".into();
                    return;
                }
                edge.range = [s.min(t), s.max(t)];
                edge.reversed = s > t;
            } else {
                self.authoring.sewing.start = Some((edge, t));
                self.status = "Partial sewing · click the other endpoint on this boundary".into();
                return;
            }
        }
        self.select_boundary(edge);
    }
    pub(super) fn begin_seam_edit(&mut self, id: &str) {
        let Some(s) = self
            .project
            .template
            .seams
            .iter()
            .find(|s| s.id == id)
            .cloned()
        else {
            return;
        };
        self.set_authoring_tool(Tool::Sew, false);
        self.authoring.reverse = s.reverse_b;
        self.authoring.ease = s.allow_ease;
        self.authoring.ends = vec![s.a.clone(), s.b.clone()];
        self.authoring.sewing = SewingDraft {
            chain_mode: !s.a_tail.is_empty() || !s.b_tail.is_empty(),
            sides: [
                s.side(false).cloned().collect(),
                s.side(true).cloned().collect(),
            ],
            editing: Some(s.id),
            ..Default::default()
        };
        self.prefs.workspace.inspector_open = true;
    }
    pub(super) fn sewing_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sewing");
        if let Some(id) = &self.authoring.sewing.editing {
            ui.label(format!("Editing {id} · apply to commit"));
        }
        let mut chain = self.authoring.sewing.chain_mode;
        let chain_button = ui.checkbox(&mut chain, "Multiple edges per side");
        self.smoke_target_ui(ui, "sew_chain", chain_button.rect);
        if chain_button.changed() {
            let sides = self.sewing_sides();
            if chain {
                self.authoring.sewing.sides = sides;
            } else if sides.iter().any(|s| s.len() > 1) {
                self.status = "Remove extra segments before switching to single-edge sewing".into();
                chain = true;
            } else {
                self.authoring.ends = sides.into_iter().flatten().collect();
            }
            self.authoring.sewing.chain_mode = chain;
        }
        if ui
            .checkbox(
                &mut self.authoring.sewing.partial,
                "Pick partial range (two clicks)",
            )
            .changed()
        {
            self.authoring.sewing.start = None;
        }
        if chain {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.authoring.sewing.active, 0, "Add to A");
                let button = ui.selectable_value(&mut self.authoring.sewing.active, 1, "Add to B");
                self.smoke_target_ui(ui, "sew_side_b", button.rect);
            });
            ui.small(
                "Click edges in sewing order. Enter switches A/B. Reverse changes one segment.",
            );
        } else {
            ui.small("Select A then B in either view. Ranges use rest length, in mm.");
        }
        let mut sides = self.sewing_sides();
        for (side, refs) in sides.iter_mut().enumerate() {
            let mut remove = None;
            let mut move_up = None;
            for (i, r) in refs.iter_mut().enumerate() {
                ui.push_id((side, i), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}{} · {} / {}",
                            if side == 0 { "A" } else { "B" },
                            i + 1,
                            r.panel,
                            r.edge.as_deref().unwrap_or("edge")
                        ));
                        if chain
                            && i > 0
                            && ui
                                .small_button("Up")
                                .on_hover_text("Move earlier in the chain")
                                .clicked()
                        {
                            move_up = Some(i);
                        }
                        if ui
                            .small_button("×")
                            .on_hover_text("Remove this segment from the draft")
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                    let mut full = r.clone();
                    full.range = [0., 1.];
                    if let Some(c) = self.display_boundary(&full, false) {
                        let length = c.length * 1000.;
                        let mut mm = r.range.map(|t| t * length);
                        ui.horizontal(|ui| {
                            ui.label("From");
                            ui.add(
                                egui::DragValue::new(&mut mm[0])
                                    .range(0. ..=length)
                                    .speed(0.5)
                                    .suffix(" mm"),
                            );
                            ui.label("To");
                            let to = ui.add(
                                egui::DragValue::new(&mut mm[1])
                                    .range(0. ..=length)
                                    .speed(0.5)
                                    .suffix(" mm"),
                            );
                            self.smoke_target_ui(ui, &format!("sew_to_{side}_{i}"), to.rect);
                        });
                        if mm[1] > mm[0] + 1e-5 {
                            r.range = mm.map(|m| m / length);
                        }
                        ui.horizontal(|ui| {
                            let mut selected = (r.range[1] - r.range[0]) * length;
                            ui.label("Length");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut selected)
                                        .range(0.01..=(length * (1. - r.range[0])))
                                        .speed(0.5)
                                        .suffix(" mm"),
                                )
                                .changed()
                            {
                                r.range[1] = r.range[0] + selected / length;
                            }
                            ui.checkbox(&mut r.reversed, "Reverse");
                        });
                    }
                });
            }
            if let Some(i) = remove {
                refs.remove(i);
            } else if let Some(i) = move_up {
                refs.swap(i, i - 1);
            }
        }
        let lengths = sides
            .each_ref()
            .map(|s| self.sewing_curve(s, false).map_or(0., |c| c.length));
        ui.label(format!(
            "A {:.1} mm / B {:.1} mm",
            lengths[0] * 1000.,
            lengths[1] * 1000.
        ));
        ui.checkbox(&mut self.authoring.reverse, "Reverse the entire B side");
        let easing = ui.checkbox(&mut self.authoring.ease, "Allow easing (different lengths)");
        self.smoke_target_ui(ui, "sew_easing", easing.rect);
        if (lengths[0] - lengths[1]).abs() > 1e-5 && !self.authoring.ease {
            ui.colored_label(
                Color32::GOLD,
                "Lengths differ. Adjust the ranges or explicitly allow easing.",
            );
        }
        let ready = sides.iter().all(|s| !s.is_empty());
        if chain {
            self.authoring.sewing.sides = sides;
        } else {
            self.authoring.ends = sides.into_iter().flatten().collect();
        }
        ui.horizontal(|ui| {
            let button = ui.add_enabled(
                ready,
                egui::Button::new(if self.authoring.sewing.editing.is_some() {
                    "Apply seam"
                } else {
                    "Commit the seam"
                }),
            );
            self.smoke_target_ui(ui, "sew", button.rect);
            if button.clicked() {
                self.commit_seam();
            }
            if ui.button("Cancel draft").clicked() {
                self.authoring.ends.clear();
                self.authoring.sewing = Default::default();
            }
        });
        ui.separator();
    }
    pub(super) fn commit_seam(&mut self) -> bool {
        let [a, b] = self.sewing_sides();
        if a.is_empty() || b.is_empty() {
            return false;
        }
        if self.edit(Command::SewChain {
            seam: self.authoring.sewing.editing.clone(),
            a,
            b,
            reverse: self.authoring.reverse,
            ease: self.authoring.ease,
        }) {
            self.authoring.sewing = Default::default();
            self.status = "Seam committed · Simulate to assemble".into();
            true
        } else {
            false
        }
    }
}
