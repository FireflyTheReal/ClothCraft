//! Resizable workspace geometry. Presentation state never enters a garment file.
use eframe::egui::{self, Rect, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ViewMode {
    #[default]
    Split,
    Garment,
    Patterns,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct WorkspaceLayout {
    pub split: f32,
    pub inspector_width: f32,
    pub inspector_open: bool,
    pub mode: ViewMode,
}
impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            split: 0.58,
            inspector_width: 280.,
            inspector_open: true,
            mode: ViewMode::Split,
        }
    }
}
impl WorkspaceLayout {
    pub fn sanitize(&mut self) {
        self.split = if self.split.is_finite() {
            self.split.clamp(0.2, 0.8)
        } else {
            0.58
        };
        self.inspector_width = if self.inspector_width.is_finite() {
            self.inspector_width.clamp(240., 440.)
        } else {
            280.
        };
    }
    pub fn toggle_focus(&mut self, view: ViewMode) {
        self.mode = if self.mode == view {
            ViewMode::Split
        } else {
            view
        };
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Panes {
    pub garment: Option<Rect>,
    pub patterns: Option<Rect>,
    pub inspector: Option<Rect>,
    split_handle: Option<Rect>,
    inspector_handle: Option<Rect>,
}
const HANDLE: f32 = 7.;

pub(super) fn rectangles(area: Rect, layout: &WorkspaceLayout) -> Panes {
    let mut work = area;
    let mut inspector = None;
    let mut inspector_handle = None;
    if layout.inspector_open {
        let width = layout.inspector_width.min((area.width() - 540.).max(160.));
        let width = width.min(area.width() * 0.42);
        let x = area.right() - width;
        inspector = Some(Rect::from_min_max(egui::pos2(x, area.top()), area.max));
        inspector_handle = Some(Rect::from_min_max(
            egui::pos2(x - HANDLE, area.top()),
            egui::pos2(x, area.bottom()),
        ));
        work.max.x = x - HANDLE;
    }
    let (garment, patterns, split_handle) = match layout.mode {
        ViewMode::Garment => (Some(work), None, None),
        ViewMode::Patterns => (None, Some(work), None),
        ViewMode::Split => {
            let width = (work.width() - HANDLE).max(1.);
            let min = 240_f32.min(width * 0.4);
            let x = work.left() + (width * layout.split).clamp(min, width - min);
            (
                Some(Rect::from_min_max(work.min, egui::pos2(x, work.bottom()))),
                Some(Rect::from_min_max(
                    egui::pos2(x + HANDLE, work.top()),
                    work.max,
                )),
                Some(Rect::from_min_max(
                    egui::pos2(x, work.top()),
                    egui::pos2(x + HANDLE, work.bottom()),
                )),
            )
        }
    };
    Panes {
        garment,
        patterns,
        inspector,
        split_handle,
        inspector_handle,
    }
}

pub(super) fn arrange(ui: &mut egui::Ui, area: Rect, layout: &mut WorkspaceLayout) -> Panes {
    layout.sanitize();
    let panes = rectangles(area, layout);
    for (name, rect) in [
        ("workspace-split", panes.split_handle),
        ("inspector-split", panes.inspector_handle),
    ] {
        let Some(rect) = rect else { continue };
        let response = ui
            .interact(rect, ui.id().with(name), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
            .on_hover_text("Drag to resize · double-click to reset");
        if response.double_clicked() {
            if name == "workspace-split" {
                layout.split = 0.58;
            } else {
                layout.inspector_width = 280.;
            }
        } else if response.dragged()
            && let Some(p) = response.interact_pointer_pos()
        {
            if name == "workspace-split" {
                let right = panes.patterns.unwrap().right();
                layout.split = ((p.x - area.left() - HANDLE * 0.5)
                    / (right - area.left() - HANDLE))
                    .clamp(0.2, 0.8);
            } else {
                layout.inspector_width = (area.right() - p.x - HANDLE * 0.5).clamp(240., 440.);
            }
        }
        let color = if response.hovered() || response.dragged() {
            super::workspace::ACCENT
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        ui.painter().line_segment(
            [
                rect.center() - Vec2::new(0., 18.),
                rect.center() + Vec2::new(0., 18.),
            ],
            egui::Stroke::new(2., color),
        );
    }
    rectangles(area, layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panes_fit_small_and_large_windows_and_focus_preserves_split() {
        for width in [900., 1040., 1540., 2500.] {
            for split in [0.2, 0.58, 0.8] {
                let area = Rect::from_min_size(egui::pos2(50., 40.), Vec2::new(width, 650.));
                let mut layout = WorkspaceLayout {
                    split,
                    ..Default::default()
                };
                let p = rectangles(area, &layout);
                let a = p.garment.unwrap();
                let b = p.patterns.unwrap();
                let c = p.inspector.unwrap();
                for r in [a, b, c] {
                    assert!(area.contains_rect(r));
                    assert!(r.width() >= 160.);
                }
                assert!(a.right() < b.left() && b.right() < c.left());
                layout.toggle_focus(ViewMode::Garment);
                assert!(rectangles(area, &layout).patterns.is_none());
                layout.toggle_focus(ViewMode::Garment);
                assert_eq!(layout.split, split);
                assert_eq!(rectangles(area, &layout).patterns, Some(b));
            }
        }
    }
}
