//! Transient selection and system clipboard adapters, separate from the document.
use super::*;
use glam::DVec2;
use marvelous_cloth::authoring::{Command, clipboard::PatternClipboard};
use std::collections::BTreeSet;

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(super) enum BoxTarget {
    #[default]
    Panels,
    Points,
}

#[derive(Default)]
pub(super) struct Selection {
    pub panels: BTreeSet<String>,
    pub target: BoxTarget,
    pub board_rect: Option<egui::Rect>,
    pub clipboard: Option<PatternClipboard>,
    pub paste_count: u32,
}

impl Atelier {
    pub(super) fn finish_marquee(
        &mut self,
        canvas: egui::Rect,
        area: egui::Rect,
        add: bool,
        points: bool,
    ) {
        if area.width() < 3. || area.height() < 3. {
            return;
        }
        if points {
            let Some((index, panel)) = self
                .project
                .template
                .panels
                .iter()
                .enumerate()
                .find(|(_, p)| self.authoring.selected.as_ref() == Some(&p.id))
            else {
                self.status = "Select a panel first, then box-select its contour points".into();
                return;
            };
            let Some(poly) = &panel.polygon else {
                return;
            };
            let mut selected =
                if add && self.authoring.board.selection_panel.as_ref() == Some(&panel.id) {
                    self.authoring.board.selection.clone()
                } else {
                    Vec::new()
                };
            for (k, uv) in poly.points.iter().enumerate() {
                let q = self.board_origin(index)
                    + marvelous_cloth::authoring::rotate_2d(*uv, self.board_angle(index));
                if area.contains(self.authoring.board.screen(canvas, q)) && !selected.contains(&k) {
                    selected.push(k);
                }
            }
            selected.sort_unstable();
            self.authoring.anchor = selected.first().map(|k| (panel.id.clone(), *k));
            self.authoring.board.selection_panel = Some(panel.id.clone());
            self.authoring.board.selection = selected;
            self.authoring.selection.panels.clear();
            self.status = format!(
                "{} contour points selected · drag a selected point or use arrow keys",
                self.authoring.board.selection.len()
            );
        } else {
            let mut selected: BTreeSet<_> = if add {
                self.selected_panels().into_iter().collect()
            } else {
                BTreeSet::new()
            };
            for (i, panel) in self.project.template.panels.iter().enumerate() {
                let project = |uv| {
                    self.authoring.board.screen(
                        canvas,
                        self.board_origin(i)
                            + marvelous_cloth::authoring::rotate_2d(uv, self.board_angle(i)),
                    )
                };
                let range = &self.scene.garment.panels[i];
                if self.scene.garment.mesh.triangles[range.triangles.clone()]
                    .iter()
                    .any(|t| {
                        triangle_in_rect(
                            t.vertices.map(|v| project(self.scene.garment.mesh.uv[v])),
                            area,
                        )
                    })
                {
                    selected.insert(panel.id.clone());
                }
            }
            self.authoring.selected = self
                .authoring
                .selected
                .take()
                .filter(|id| selected.contains(id))
                .or_else(|| selected.iter().next().cloned());
            self.authoring.selection.panels = selected;
            self.authoring.board.selection.clear();
            self.authoring.board.selection_panel = None;
            self.authoring.anchor = None;
            self.status = format!(
                "{} panels selected · drag to arrange in 2D · Ctrl+C to copy",
                self.selected_panels().len()
            );
        }
        self.authoring.edge = None;
    }
    pub(super) fn selected_panels(&self) -> Vec<String> {
        let s = &self.authoring.selection.panels;
        if self
            .authoring
            .selected
            .as_ref()
            .is_some_and(|id| s.contains(id))
        {
            s.iter()
                .filter(|id| self.project.template.panels.iter().any(|p| &p.id == *id))
                .cloned()
                .collect()
        } else {
            self.authoring.selected.clone().into_iter().collect()
        }
    }
    pub(super) fn panel_selected(&self, id: &str) -> bool {
        self.authoring.selected.as_deref() == Some(id)
            || (self
                .authoring
                .selected
                .as_ref()
                .is_some_and(|p| self.authoring.selection.panels.contains(p))
                && self.authoring.selection.panels.contains(id))
    }
    pub(super) fn select_panel(&mut self, id: String, toggle: bool) {
        let mut ids: BTreeSet<_> = if toggle {
            self.selected_panels().into_iter().collect()
        } else {
            BTreeSet::new()
        };
        if toggle && ids.contains(&id) {
            ids.remove(&id);
        } else {
            ids.insert(id.clone());
        }
        self.authoring.selected = if ids.contains(&id) {
            Some(id)
        } else {
            ids.iter().next().cloned()
        };
        self.authoring.selection.panels = ids;
        self.authoring.anchor = None;
        self.authoring.board.selection.clear();
        self.authoring.board.selection_panel = None;
        self.authoring.edge = None;
    }
    pub(super) fn selection_controls(&mut self, ui: &mut egui::Ui) {
        if self.authoring.tool != Tool::Edit {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.small("Box select");
            ui.selectable_value(
                &mut self.authoring.selection.target,
                BoxTarget::Panels,
                "Panels",
            );
            ui.selectable_value(
                &mut self.authoring.selection.target,
                BoxTarget::Points,
                "Points",
            );
            let count = self.selected_panels().len();
            ui.separator();
            if self.authoring.selection.target == BoxTarget::Points {
                ui.small(format!("{} points", self.authoring.board.selection.len()));
            } else {
                ui.small(format!("{count} panels"));
            }
            if ui
                .add_enabled(count > 0, egui::Button::new("Copy panels"))
                .on_hover_text("Ctrl+C · copy panels and seams between them")
                .clicked()
            {
                self.copy_panels(ui.ctx());
            }
            if ui
                .add_enabled(
                    self.authoring.selection.clipboard.is_some(),
                    egui::Button::new("Paste"),
                )
                .on_hover_text("Paste the last copied panels · Ctrl+V reads the system clipboard")
                .clicked()
                && let Some(clip) = self.authoring.selection.clipboard.clone()
            {
                self.paste_panels(clip, ui.ctx());
            }
        });
    }
    fn copy_panels(&mut self, ctx: &egui::Context) {
        match PatternClipboard::capture(&self.project.template, &self.selected_panels())
            .and_then(|clip| clip.to_text().map(|text| (clip, text)))
        {
            Ok((clip, text)) => {
                ctx.copy_text(text);
                self.status = format!(
                    "Copied {} panels and {} internal seams · Ctrl+V to paste",
                    clip.panel_count(),
                    clip.seam_count()
                );
                self.authoring.selection.clipboard = Some(clip);
                self.authoring.selection.paste_count = 0;
            }
            Err(error) => self.status = error,
        }
    }
    fn paste_panels(&mut self, clip: PatternClipboard, ctx: &egui::Context) {
        let board = &self.authoring.board;
        let cursor = ctx.input(|i| i.pointer.hover_pos());
        let center = self
            .authoring
            .selection
            .board_rect
            .zip(cursor)
            .filter(|(r, p)| r.contains(*p))
            .map(|(r, p)| board.world(r, p))
            .unwrap_or(board.center);
        let center = center + DVec2::splat(f64::from(self.authoring.selection.paste_count) * 0.03);
        let old: BTreeSet<_> = self
            .project
            .template
            .panels
            .iter()
            .map(|p| p.id.clone())
            .collect();
        if self.edit(Command::Paste {
            clipboard: clip.clone(),
            center,
        }) {
            self.set_authoring_tool(Tool::Edit, false);
            self.authoring.selection.panels = self
                .project
                .template
                .panels
                .iter()
                .filter(|p| !old.contains(&p.id))
                .map(|p| p.id.clone())
                .collect();
            self.authoring.selected = self.authoring.selection.panels.iter().next().cloned();
            self.authoring.anchor = None;
            self.authoring.edge = None;
            self.authoring.selection.clipboard = Some(clip);
            self.authoring.selection.paste_count += 1;
            self.status = "Panels pasted · internal seams retained · arrange copies in 3D before simulation · Ctrl+Z to undo".into();
        }
    }
    pub(super) fn clipboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input()
            || self.authoring.radial.blocked
            || self.authoring.board.gesture_active()
            || self.authoring.drag.is_some()
        {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        // eframe emits Copy/Paste events rather than ordinary C/V key presses.
        if events.iter().any(|e| matches!(e, egui::Event::Copy)) {
            self.copy_panels(ctx);
        }
        if let Some(text) = events.iter().find_map(|e| {
            if let egui::Event::Paste(s) = e {
                Some(s)
            } else {
                None
            }
        }) {
            match PatternClipboard::from_text(text) {
                Ok(clip) => self.paste_panels(clip, ctx),
                Err(error) => self.status = error,
            }
        }
    }
}

/// Separating axis test on actual cloth triangles, not their bounding boxes.
/// A selection rectangle wholly inside a cutout must not select the panel.
fn triangle_in_rect(triangle: [egui::Pos2; 3], rect: egui::Rect) -> bool {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let edges = [
        triangle[1] - triangle[0],
        triangle[2] - triangle[1],
        triangle[0] - triangle[2],
    ];
    for axis in [Vec2::X, Vec2::Y]
        .into_iter()
        .chain(edges.map(|v| Vec2::new(-v.y, v.x)))
    {
        let project = |p: egui::Pos2| p.to_vec2().dot(axis);
        let a = triangle.map(project);
        let b = corners.map(project);
        let min = |a: &[f32]| a.iter().copied().fold(f32::INFINITY, f32::min);
        let max = |a: &[f32]| a.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if max(&a) < min(&b) || max(&b) < min(&a) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marquee_uses_geometry_instead_of_aabb() {
        let t = [egui::pos2(0., 0.), egui::pos2(10., 0.), egui::pos2(0., 10.)];
        let r = |x, y, size| egui::Rect::from_min_size(egui::pos2(x, y), Vec2::splat(size));
        assert!(triangle_in_rect(t, r(1., 1., 2.)));
        assert!(triangle_in_rect(t, r(-1., -1., 12.)));
        assert!(triangle_in_rect(t, r(4., -1., 2.)));
        assert!(!triangle_in_rect(t, r(7., 7., 2.)));
    }
}
