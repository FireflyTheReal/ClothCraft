//! World-axis placement handles. Preview is rigid; only release recompiles.
use super::{
    authoring_ui::{Tool, segment_distance},
    *,
};
use glam::DVec3;
use marvelous_cloth::{authoring, garment::template::Placement};

pub(super) struct Drag {
    axis: DVec3,
    pivot: DVec3,
    center: DVec3,
    start: Placement,
    pointer: egui::Pos2,
    screen_axis: Vec2,
    radius: f64,
    rotate: bool,
    vector: DVec3,
}
const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
const COLORS: [Color32; 3] = [
    Color32::from_rgb(248, 95, 94),
    Color32::from_rgb(104, 220, 134),
    Color32::from_rgb(98, 160, 255),
];
fn project(camera: &Camera, rect: egui::Rect, p: DVec3) -> Option<egui::Pos2> {
    let clip = camera.matrix(rect.width() / rect.height()) * p.as_vec3().extend(1.);
    if clip.w <= 0. {
        return None;
    }
    let p = clip.truncate() / clip.w;
    Some(egui::pos2(
        rect.center().x + p.x * rect.width() * 0.5,
        rect.center().y - p.y * rect.height() * 0.5,
    ))
}
fn ring(
    camera: &Camera,
    rect: egui::Rect,
    center: DVec3,
    axis: usize,
    radius: f64,
) -> Vec<egui::Pos2> {
    let a = AXES[(axis + 1) % 3];
    let b = AXES[(axis + 2) % 3];
    (0..=64)
        .filter_map(|i| {
            let angle = i as f64 * std::f64::consts::TAU / 64.;
            project(
                camera,
                rect,
                center + radius * (angle.cos() * a + angle.sin() * b),
            )
        })
        .collect()
}
impl Atelier {
    fn gizmo_basis(&self) -> Option<(Placement, DVec3, DVec3, f64)> {
        let panel = self
            .project
            .template
            .panels
            .iter()
            .find(|p| Some(&p.id) == self.authoring.selected.as_ref())?;
        let placement = self
            .authoring
            .preview
            .clone()
            .unwrap_or_else(|| panel.placement.clone());
        let pivot = DVec3::new(panel.pattern.width * 0.5, -panel.pattern.height * 0.5, 0.);
        let center = placement.transform(pivot);
        Some((
            placement,
            pivot,
            center,
            self.camera.distance as f64 * 0.055,
        ))
    }
    pub(super) fn place_interaction(&mut self, response: &egui::Response, rect: egui::Rect) {
        if self.authoring.tool != Tool::Place {
            return;
        }
        if response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_preview();
            return;
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(pointer) = response.ctx.input(|i| i.pointer.press_origin())
            && let Some((start, pivot, center, radius)) = self.gizmo_basis()
            && let Some(screen_center) = project(&self.camera, rect, center)
        {
            let mut best = 11.;
            let mut hit = None;
            for (i, axis) in AXES.iter().enumerate() {
                let Some(tip) = project(&self.camera, rect, center + radius * axis) else {
                    continue;
                };
                let screen_axis = tip - screen_center;
                let rotate = self.authoring.rotate;
                let (origin, dir) = self.camera.ray(pointer, rect);
                let vector = picking::ray_plane(origin, dir, center, *axis)
                    .map(|p| p - center)
                    .unwrap_or_default();
                let distance = if rotate {
                    if dir.dot(*axis).abs() < 0.08 || vector.length_squared() < 1e-8 {
                        continue;
                    }
                    ring(&self.camera, rect, center, i, radius)
                        .windows(2)
                        .map(|p| segment_distance(pointer, p[0], p[1]))
                        .fold(f32::INFINITY, f32::min)
                } else {
                    if screen_axis.length() < 16. {
                        continue;
                    }
                    segment_distance(pointer, screen_center + screen_axis * 0.15, tip)
                };
                if distance < best {
                    best = distance;
                    hit = Some(Drag {
                        axis: *axis,
                        pivot,
                        center,
                        start: start.clone(),
                        pointer,
                        screen_axis,
                        radius,
                        rotate,
                        vector,
                    });
                }
            }
            self.authoring.drag = hit;
        }
        if response.dragged_by(egui::PointerButton::Primary)
            && let (Some(drag), Some(pointer)) =
                (&self.authoring.drag, response.interact_pointer_pos())
        {
            let placement = if drag.rotate {
                let (origin, dir) = self.camera.ray(pointer, rect);
                picking::ray_plane(origin, dir, drag.center, drag.axis).map(|p| {
                    let v = p - drag.center;
                    let angle = drag
                        .axis
                        .dot(drag.vector.cross(v))
                        .atan2(drag.vector.dot(v));
                    authoring::rotate_about(&drag.start, drag.pivot, drag.axis, angle)
                })
            } else {
                let mut p = drag.start.clone();
                let delta =
                    (pointer - drag.pointer).dot(drag.screen_axis) / drag.screen_axis.length_sq();
                p.translation += drag.axis * (delta as f64 * drag.radius);
                Some(p)
            };
            if let Some(p) = placement {
                self.preview_placement(p);
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) && self.authoring.drag.is_some() {
            self.commit_placement();
            self.authoring.drag = None;
        }
        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let (origin, dir) = self.camera.ray(pointer, rect);
            let mut best = f64::INFINITY;
            let mut selected = None;
            for panel in &self.scene.garment.panels {
                for t in &self.scene.garment.mesh.triangles[panel.triangles.clone()] {
                    if let Some((d, _)) = picking::ray_triangle(
                        origin,
                        dir,
                        t.vertices.map(|i| self.scene.state.x[i]),
                    ) && d < best
                    {
                        best = d;
                        selected = Some(panel.id.clone());
                    }
                }
            }
            if self.scene.collider.surface.triangles.iter().any(|t| {
                picking::ray_triangle(
                    origin,
                    dir,
                    t.map(|i| self.scene.collider.surface.vertices[i]),
                )
                .is_some_and(|(d, _)| d < best - 1e-5)
            }) {
                selected = None;
            }
            if let Some(id) = selected
                && self.authoring.selected.as_ref() != Some(&id)
            {
                self.cancel_preview();
                self.authoring.selected = Some(id);
            }
        }
    }
    pub(super) fn paint_gizmo(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if self.authoring.tool != Tool::Place {
            return;
        }
        let Some((placement, _, center, radius)) = self.gizmo_basis() else {
            return;
        };
        let Some(screen_center) = project(&self.camera, rect, center) else {
            return;
        };
        if let Some(tip) = project(&self.camera, rect, center + radius * DVec3::X) {
            self.smoke_target("gizmo_x", screen_center + (tip - screen_center) * 0.70);
        }
        if self.authoring.rotate {
            let target = center + radius * (DVec3::X + DVec3::Y) * std::f64::consts::FRAC_1_SQRT_2;
            if let Some(p) = project(&self.camera, rect, target) {
                self.smoke_target("ring_z", p);
            }
        }
        let painter = ui.painter().with_clip_rect(rect);
        if let Some(panel) = self
            .project
            .template
            .panels
            .iter()
            .find(|p| Some(&p.id) == self.authoring.selected.as_ref())
        {
            if let Some(poly) = &panel.polygon {
                for i in 0..poly.points.len() {
                    let to_screen = |k: usize| {
                        let uv = poly.points[k];
                        project(
                            &self.camera,
                            rect,
                            placement.transform(DVec3::new(uv.x, -uv.y, 0.)),
                        )
                    };
                    if let (Some(a), Some(b)) =
                        (to_screen(i), to_screen((i + 1) % poly.points.len()))
                    {
                        painter.line_segment([a, b], Stroke::new(1.5, Color32::GOLD));
                    }
                }
            }
            painter.text(
                screen_center + Vec2::new(0., -12.),
                egui::Align2::CENTER_BOTTOM,
                &panel.name,
                egui::FontId::proportional(13.),
                Color32::GOLD,
            );
        }
        for (i, axis) in AXES.iter().enumerate() {
            let color = if self
                .authoring
                .drag
                .as_ref()
                .is_some_and(|d| d.axis == *axis)
            {
                Color32::WHITE
            } else {
                COLORS[i]
            };
            if self.authoring.rotate {
                let points = ring(&self.camera, rect, center, i, radius);
                painter.add(egui::Shape::line(points, Stroke::new(2.5, color)));
            } else if let Some(tip) = project(&self.camera, rect, center + radius * axis) {
                painter.arrow(screen_center, tip - screen_center, Stroke::new(3., color));
                painter.text(
                    tip,
                    egui::Align2::CENTER_BOTTOM,
                    ["X", "Y", "Z"][i],
                    egui::FontId::proportional(14.),
                    color,
                );
            }
        }
        painter.circle_filled(screen_center, 4., Color32::GOLD);
    }
}
