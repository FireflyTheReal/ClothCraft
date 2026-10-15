//! Sparse correspondence overlays. Geometry comes from material-space attachments.
use super::{board::same_edge, *};
use glam::{DVec2, DVec3};
use marvelous_cloth::garment::seam::{BoundaryCurve, BoundaryRef, StitchGuide};

pub(super) fn seam_color(id: &str) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(104, 205, 220),
        Color32::from_rgb(237, 174, 105),
        Color32::from_rgb(185, 155, 230),
        Color32::from_rgb(137, 207, 151),
        Color32::from_rgb(233, 137, 166),
        Color32::from_rgb(130, 171, 238),
        Color32::from_rgb(209, 207, 123),
        Color32::from_rgb(218, 154, 219),
    ];
    let hash = id
        .bytes()
        .fold(2166136261_u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    COLORS[hash as usize % COLORS.len()]
}

impl Atelier {
    fn display_boundary(&self, edge: &BoundaryRef, reverse: bool) -> Option<BoundaryCurve> {
        let i = self
            .project
            .template
            .panels
            .iter()
            .position(|p| p.id == edge.panel)?;
        let panel = &self.scene.garment.panels[i];
        let ids = if let Some(name) = &edge.edge {
            panel.boundaries.get(name)?.clone()
        } else {
            self.project.template.panels[i].boundary_vertices(edge.side, panel.vertices.start)
        };
        BoundaryCurve::new(&self.scene.garment.mesh, ids, edge.range, reverse).ok()
    }
    pub(super) fn paint_seam_links(&self, painter: &egui::Painter, rect: egui::Rect, in_3d: bool) {
        if !in_3d && self.authoring.replace {
            return;
        }
        let matrix = self.camera.matrix(rect.width() / rect.height());
        let point = |attachment: &[(usize, f64); 2]| -> Option<egui::Pos2> {
            if in_3d {
                let p: DVec3 = attachment
                    .iter()
                    .map(|(v, w)| self.scene.state.x[*v] * *w)
                    .sum();
                super::gizmo::project_matrix(matrix, rect, p)
            } else {
                let i = self
                    .scene
                    .garment
                    .panels
                    .iter()
                    .position(|p| p.vertices.contains(&attachment[0].0))?;
                let uv: DVec2 = attachment
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
        let draw = |guides: &[StitchGuide], color: Color32, selected: bool, label: &str| {
            for (i, guide) in guides.iter().enumerate() {
                if !selected && i != 0 && i != guides.len() / 2 && i + 1 != guides.len() {
                    continue;
                }
                let (Some(a), Some(b)) = (point(&guide[0]), point(&guide[1])) else {
                    continue;
                };
                // Only a handful of links, independent of mesh density. Endpoint dots
                // remain readable when an assembled seam's gap is nearly zero.
                let stroke = Stroke::new(
                    if selected { 1.3 } else { 0.8 },
                    color.gamma_multiply(if selected { 0.85 } else { 0.35 }),
                );
                // Bound overlay tessellation even when pieces are far apart.
                let spacing = (a.distance(b) / 32.).max(10.);
                painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 5., spacing - 5.));
                let direction = if a.distance(b) > 0.01 {
                    (b - a).normalized()
                } else {
                    Vec2::X
                };
                let tick = Vec2::new(-direction.y, direction.x) * 3.;
                for end in [a, b] {
                    painter.line_segment([end - tick, end + tick], Stroke::new(1.2, color));
                    painter.circle_filled(end, if selected { 2.5 } else { 1.8 }, color);
                }
                if selected && i == guides.len() / 2 {
                    let middle = a.lerp(b, 0.5);
                    let text = painter.layout_no_wrap(
                        label.into(),
                        egui::FontId::proportional(11.),
                        color,
                    );
                    let box_rect =
                        egui::Rect::from_center_size(middle, text.size() + Vec2::new(10., 6.));
                    painter.rect_filled(box_rect, 3., Color32::from_rgb(0xF7, 0xF7, 0xF7));
                    painter.galley(box_rect.min + Vec2::new(5., 3.), text, color);
                }
            }
        };
        for seam in &self.scene.garment.seams {
            let Some(spec) = self.project.template.seams.iter().find(|s| s.id == seam.id) else {
                continue;
            };
            let selected = self
                .authoring
                .edge
                .as_ref()
                .is_some_and(|e| same_edge(e, &spec.a) || same_edge(e, &spec.b));
            let hover = self
                .authoring
                .hover_edge
                .as_ref()
                .is_some_and(|e| same_edge(e, &spec.a) || same_edge(e, &spec.b));
            if self.seam_links || selected || hover {
                draw(
                    &seam.guides,
                    seam_color(&seam.id),
                    selected || hover,
                    &seam.id,
                );
            }
        }
        if self.authoring.ends.len() == 2
            && let (Some(a), Some(b)) = (
                self.display_boundary(&self.authoring.ends[0], false),
                self.display_boundary(&self.authoring.ends[1], self.authoring.reverse),
            )
        {
            let guides: Vec<_> = (0..=8)
                .map(|i| [a.point(i as f64 / 8.), b.point(i as f64 / 8.)])
                .collect();
            draw(
                &guides,
                Color32::from_rgb(248, 117, 188),
                true,
                "Preview A ↔ B",
            );
        }
    }
}
