//! Cursor-local commands backed by the same transactional authoring operations.
use super::*;
use marvelous_cloth::{
    authoring::Command,
    garment::shapes::{BooleanOp, MirrorAxis},
};

#[derive(Default)]
pub(super) struct QuickActions {
    at: Option<egui::Pos2>,
    pub blocked: bool,
    keep_cutter: bool,
    opened_frame: u64,
}
impl QuickActions {
    pub fn open(&mut self, ctx: &egui::Context, at: egui::Pos2) {
        self.at = Some(at);
        self.blocked = true;
        self.opened_frame = ctx.cumulative_frame_nr();
    }
}

pub(super) fn boolean_pair(
    active: Option<&String>,
    selected: &[String],
) -> Option<(String, String)> {
    let a = active?;
    if selected.len() != 2 || !selected.contains(a) {
        return None;
    }
    Some((a.clone(), selected.iter().find(|id| *id != a)?.clone()))
}

impl Atelier {
    pub(super) fn quick_actions(&mut self, ctx: &egui::Context) {
        self.quick.blocked = self.quick.at.is_some();
        if !ctx.egui_wants_keyboard_input()
            && !self.authoring.board.gesture_active()
            && self.authoring.drag.is_none()
            && self.grab.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Q))
        {
            self.authoring.radial.close();
            if self.quick.at.is_some() {
                self.quick.at = None;
                return;
            }
            self.quick.open(
                ctx,
                ctx.pointer_hover_pos()
                    .unwrap_or(ctx.content_rect().center()),
            );
        }
        let Some(at) = self.quick.at else {
            return;
        };
        let mut open = true;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused);
        let selected = self.selected_panels();
        let pair = boolean_pair(self.authoring.selected.as_ref(), &selected);
        let active = self.authoring.selected.clone();
        let bounds = ctx.content_rect();
        let at = egui::pos2(
            at.x.min(bounds.right() - 345.).max(bounds.left()),
            at.y.min(bounds.bottom() - 470.).max(bounds.top()),
        );
        let window = egui::Window::new("Quick actions · Q")
            .id(egui::Id::new("quick-actions"))
            .order(egui::Order::Foreground).fixed_pos(at).default_width(320.)
            .resizable(false).collapsible(false).open(&mut open)
            .show(ctx, |ui| {
                ui.strong("DRAW IN 2D");
                ui.horizontal_wrapped(|ui| {
                    for (tool, icon, name) in [
                        (Tool::Draw, icons::Icon::Pen, "Pen"),
                        (Tool::Rectangle, icons::Icon::Rectangle, "Rectangle"),
                        (Tool::Ellipse, icons::Icon::Ellipse, "Ellipse"),
                        (Tool::Sector, icons::Icon::Arc, "Arc"),
                        (Tool::InternalLine, icons::Icon::Pen, "Internal line"),
                        (Tool::Measure, icons::Icon::Measure, "Measure"),
                    ] {
                        let button = icons::tool_button(ui, icon, name, self.authoring.tool == tool, "Choose, then drag on the 2D board. Shift makes a circle or square.");
                        self.smoke_target(&format!("quick_{name}"), button.rect.center());
                        if button.clicked() { self.set_authoring_tool(tool, false); close = true; }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Arc sweep");
                    ui.add(egui::DragValue::new(&mut self.authoring.drafting.sweep).range(5. ..=355.).suffix("°"));
                });
                ui.separator();
                ui.strong("MIRROR ACTIVE PIECE");
                if let Some(panel) = &active {
                    self.linked_mirror_properties(ui, panel);
                }
                ui.add_enabled_ui(active.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        for (axis, label) in [(MirrorAxis::X, "Flip X"), (MirrorAxis::Y, "Flip Y")] {
                            if icons::tool_button(ui, icons::Icon::Mirror, label, false, "Reflect in local pattern coordinates; one undoable edit").clicked() {
                                close = self.edit(Command::Mirror { panel: active.clone().unwrap(), axis, copy_side: None });
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Copy");
                        for (axis, side, label) in [(MirrorAxis::X, -1, "← −X"), (MirrorAxis::X, 1, "+X →"), (MirrorAxis::Y, -1, "↓ −Y"), (MirrorAxis::Y, 1, "+Y ↑")] {
                            let button = ui.button(label).on_hover_text("Create and select an independent mirrored copy beside the source");
                            if axis == MirrorAxis::X && side == 1 { self.smoke_target("quick_copy_x", button.rect.center()); }
                            if button.clicked() {
                                close = self.edit(Command::Mirror { panel: active.clone().unwrap(), axis, copy_side: Some(side) });
                            }
                        }
                    });
                });
                ui.separator();
                ui.strong("COMBINE TWO SELECTED PIECES");
                if let Some((a, b)) = &pair {
                    ui.small(format!("A: {a}    B: {b}"));
                    if ui.small_button("Swap A / B").clicked() { self.authoring.selected = Some(b.clone()); }
                } else {
                    ui.small("Shift-click two pieces. The active piece is A.");
                }
                ui.add_enabled_ui(pair.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        for (op, icon, name) in [(BooleanOp::Union, icons::Icon::Union, "Union"), (BooleanOp::Difference, icons::Icon::Subtract, "A − B"), (BooleanOp::Intersection, icons::Icon::Intersect, "Intersect")] {
                            let button = icons::tool_button(ui, icon, name, false, "Uses the two selected pieces' 2D overlap. Failed edits leave the document intact.");
                            self.smoke_target(&format!("quick_{name}"), button.rect.center());
                            if button.clicked() {
                                let (a, b) = pair.clone().unwrap();
                                close = self.edit(Command::Boolean { a, b, op, keep_tool: self.quick.keep_cutter });
                            }
                        }
                    });
                    ui.checkbox(&mut self.quick.keep_cutter, "Keep B");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if icons::tool_button(ui, icons::Icon::Sew, "Sew", self.authoring.tool == Tool::Sew, "Choose boundaries in either view · F: flip · E: ease").clicked() {
                        self.set_authoring_tool(Tool::Sew, false); close = true;
                    }
                    if icons::tool_button(ui, icons::Icon::Warning, "Mark issue · D", self.authoring.tool == Tool::Inspect, "Pause and paint problem faces in 3D").clicked() {
                        self.set_authoring_tool(Tool::Inspect, false); close = true;
                    }
                });
                if let Some(error) = &self.authoring.error { ui.colored_label(Color32::from_rgb(210, 70, 55), error); }
            });
        if ctx.cumulative_frame_nr() > self.quick.opened_frame
            && window.as_ref().is_some_and(|w| {
                ctx.input(|i| i.pointer.any_click())
                    && ctx
                        .pointer_hover_pos()
                        .is_some_and(|p| !w.response.rect.contains(p))
            })
        {
            close = true;
        }
        if close || !open {
            self.quick.at = None;
        } // blocked remains true on the closing frame.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boolean_operands_use_active_piece_not_sorted_selection_order() {
        let selected = vec!["a".into(), "z".into()];
        assert_eq!(
            boolean_pair(Some(&"z".into()), &selected),
            Some(("z".into(), "a".into()))
        );
        assert!(boolean_pair(Some(&"missing".into()), &selected).is_none());
        assert!(boolean_pair(Some(&"a".into()), &selected[..1]).is_none());
    }
}
