//! Mouse tool wheel. Short secondary click opens; secondary drag stays navigation.
use super::{
    authoring_ui::Tool,
    icons::{self, Icon},
    *,
};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

const INNER: f32 = 48.;
const OUTER: f32 = 144.;
const ITEMS: [(Tool, bool, Icon, &str, &str); 8] = [
    (
        Tool::Edit,
        false,
        Icon::Edit,
        "Edit",
        "Select pieces, points and handles",
    ),
    (
        Tool::Draw,
        false,
        Icon::Pen,
        "Pen",
        "Click corners · drag to draw curves",
    ),
    (
        Tool::Rectangle,
        false,
        Icon::Rectangle,
        "Rectangle",
        "Drag opposite corners to create cloth",
    ),
    (
        Tool::Sew,
        false,
        Icon::Sew,
        "Sew",
        "Connect named edges in 2D or 3D",
    ),
    (
        Tool::Place,
        false,
        Icon::Move3d,
        "Move 3D",
        "Arrange the active panel · G",
    ),
    (
        Tool::Place,
        true,
        Icon::Rotate3d,
        "Rotate 3D",
        "Rotate the active panel · R",
    ),
    (
        Tool::Grab,
        false,
        Icon::Grab,
        "Grab",
        "Pull the simulated cloth",
    ),
    (
        Tool::InternalLine,
        false,
        Icon::Pen,
        "Internal line",
        "Draw a sewing path on the active panel",
    ),
];

#[derive(Default)]
pub(super) struct Radial {
    center: Option<egui::Pos2>,
    pub blocked: bool,
}

fn sector(delta: Vec2) -> Option<usize> {
    if !(INNER..=OUTER + 8.).contains(&delta.length()) {
        return None;
    }
    Some(((delta.angle() + FRAC_PI_2 + FRAC_PI_4 * 0.5).rem_euclid(TAU) / FRAC_PI_4) as usize % 8)
}

fn wheel_center(at: egui::Pos2, bounds: egui::Rect) -> egui::Pos2 {
    let safe = bounds.shrink2(Vec2::new(OUTER + 24., OUTER + 42.));
    if safe.is_positive() {
        at.clamp(safe.min, safe.max)
    } else {
        bounds.center()
    }
}

impl Radial {
    pub fn open(&mut self, ctx: &egui::Context, at: egui::Pos2) {
        self.center = Some(wheel_center(at, ctx.content_rect()));
        self.blocked = true;
        ctx.request_repaint();
    }

    /// Returns a real tool, never a synthetic pointer event on the canvas.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        active: Tool,
        rotate: bool,
        has_panel: bool,
    ) -> Option<(Tool, bool)> {
        self.blocked = self.center.is_some();
        let center = self.center?;
        let bounds = ctx.content_rect();
        let hover = ctx
            .input(|i| i.pointer.hover_pos())
            .and_then(|p| sector(p - center));
        let enabled =
            |i: usize| has_panel || !matches!(ITEMS[i].0, Tool::Place | Tool::InternalLine);
        let mut chosen = None;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused);
        if ctx.input(|i| i.pointer.primary_clicked()) {
            chosen = hover.filter(|i| enabled(*i));
            close = true;
        }
        if ctx.input(|i| i.pointer.secondary_clicked()) {
            close = true;
        }
        for (i, key) in [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
        ]
        .into_iter()
        .enumerate()
        {
            if ctx.input(|v| v.key_pressed(key)) && enabled(i) {
                chosen = Some(i);
                close = true;
            }
        }
        egui::Area::new(egui::Id::new("tool-wheel-input"))
            .order(egui::Order::Foreground)
            .fixed_pos(bounds.min)
            .movable(false)
            .show(ctx, |ui| {
                ui.allocate_rect(bounds, egui::Sense::click_and_drag());
                let p = ui.painter();
                let dark = ui.visuals().dark_mode;
                let bg = if dark {
                    Color32::from_rgb(26, 32, 43)
                } else {
                    Color32::from_rgb(250, 252, 255)
                };
                let ink = if dark {
                    Color32::from_gray(235)
                } else {
                    Color32::from_rgb(32, 45, 65)
                };
                let accent = Color32::from_rgb(57, 145, 238);
                p.circle_filled(
                    center + Vec2::new(0., 5.),
                    OUTER + 10.,
                    Color32::from_black_alpha(35),
                );
                p.circle_filled(center, OUTER + 3., bg);
                p.circle_stroke(
                    center,
                    OUTER + 3.,
                    Stroke::new(1., ink.gamma_multiply(0.18)),
                );
                for (i, &(tool, rotation, icon, label, _)) in ITEMS.iter().enumerate() {
                    let selected = active == tool && (tool != Tool::Place || rotate == rotation);
                    let hovered = hover == Some(i) && enabled(i);
                    let glow = ctx.animate_bool_with_time(
                        egui::Id::new(("wheel-sector", i)),
                        hovered,
                        0.09,
                    );
                    let angle = -FRAC_PI_2 + i as f32 * FRAC_PI_4;
                    let from = angle - FRAC_PI_4 * 0.5 + 0.018;
                    let to = angle + FRAC_PI_4 * 0.5 - 0.018;
                    let fill = if hovered {
                        accent.gamma_multiply(0.23)
                    } else if selected {
                        accent.gamma_multiply(0.10)
                    } else {
                        ink.gamma_multiply(0.035)
                    };
                    let mut mesh = egui::Mesh::default();
                    for k in 0..=20 {
                        let dir = Vec2::angled(from + (to - from) * k as f32 / 20.);
                        mesh.colored_vertex(center + dir * (INNER + 3.), fill);
                        mesh.colored_vertex(center + dir * (OUTER - 3. + glow * 5.), fill);
                        if k < 20 {
                            let n = k * 2;
                            mesh.add_triangle(n, n + 1, n + 2);
                            mesh.add_triangle(n + 1, n + 3, n + 2);
                        }
                    }
                    p.add(egui::Shape::mesh(mesh));
                    let at = center + Vec2::angled(angle) * 98.;
                    let color = if !enabled(i) {
                        ink.gamma_multiply(0.3)
                    } else if hovered || selected {
                        accent
                    } else {
                        ink
                    };
                    icons::glyph(
                        p,
                        egui::Rect::from_center_size(at - Vec2::new(0., 9.), Vec2::splat(24.)),
                        icon,
                        color,
                        1.7,
                    );
                    p.text(
                        at + Vec2::new(0., 14.),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(11.5),
                        color,
                    );
                }
                p.circle_filled(center, INNER, bg);
                p.circle_stroke(center, INNER, Stroke::new(1., ink.gamma_multiply(0.12)));
                let title = hover.map(|i| ITEMS[i].3).unwrap_or("TOOLS");
                p.text(
                    center - Vec2::new(0., 6.),
                    egui::Align2::CENTER_CENTER,
                    title,
                    egui::FontId::proportional(12.),
                    ink,
                );
                p.text(
                    center + Vec2::new(0., 13.),
                    egui::Align2::CENTER_CENTER,
                    "Esc · cancel",
                    egui::FontId::proportional(10.),
                    ink.gamma_multiply(0.6),
                );
                let hint = hover
                    .map(|i| ITEMS[i].4)
                    .unwrap_or("Choose a tool · click or press 1–8");
                let r = egui::Rect::from_center_size(
                    center + Vec2::new(0., OUTER + 25.),
                    Vec2::new(330., 26.),
                );
                p.rect_filled(r, 8., bg);
                p.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    hint,
                    egui::FontId::proportional(11.),
                    ink,
                );
            });
        if close {
            self.center = None;
            ctx.request_repaint();
        }
        chosen.map(|i| (ITEMS[i].0, ITEMS[i].1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        wheel: &mut Radial,
        events: Vec<egui::Event>,
    ) -> Option<(Tool, bool)> {
        let mut chosen = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1100., 700.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                chosen = wheel.show(ui.ctx(), Tool::Edit, false, true);
            },
        );
        output.textures_delta.clear(); // Headless input test has no texture backend.
        chosen
    }
    #[test]
    fn wheel_consumes_selection_and_cancel_frames_without_a_gpu() {
        let ctx = egui::Context::default();
        let mut wheel = Radial::default();
        frame(&ctx, &mut wheel, vec![]);
        wheel.open(&ctx, egui::pos2(400., 350.));
        frame(&ctx, &mut wheel, vec![]);
        let pos = egui::pos2(500., 350.); // Rectangle sector, east.
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            &ctx,
            &mut wheel,
            vec![egui::Event::PointerMoved(pos), button(true)],
        );
        let selected = frame(&ctx, &mut wheel, vec![button(false)]);
        assert!(selected == Some((Tool::Rectangle, false)));
        assert!(wheel.blocked, "Selection release must not reach the canvas");
        frame(&ctx, &mut wheel, vec![]);
        assert!(!wheel.blocked);
        wheel.open(&ctx, pos);
        let selected = frame(
            &ctx,
            &mut wheel,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(selected.is_none());
        assert!(wheel.center.is_none());
        assert!(wheel.blocked);
    }
    #[test]
    fn radial_has_dead_zone_and_correct_sectors() {
        assert_eq!(sector(Vec2::ZERO), None);
        assert_eq!(sector(Vec2::new(0., -200.)), None);
        for i in 0..8 {
            assert_eq!(
                sector(Vec2::angled(-FRAC_PI_2 + i as f32 * FRAC_PI_4) * 100.),
                Some(i)
            );
        }
    }
    #[test]
    fn wheel_stays_inside_window_edges() {
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1100., 700.));
        for at in [bounds.min, bounds.max, bounds.center()] {
            let center = wheel_center(at, bounds);
            assert!(
                bounds.contains_rect(egui::Rect::from_center_size(center, Vec2::new(330., 370.)))
            );
        }
    }
}
