//! Read-only board measurements. These points are presentation coordinates,
//! not persistent garment constraints or distances on the draped surface.
use super::*;
use glam::DVec2;

#[derive(Default)]
pub(super) struct Tape {
    start: Option<DVec2>,
    end: Option<DVec2>,
}
impl Tape {
    fn click(&mut self, point: DVec2) {
        if self.start.is_none() || self.end.is_some() {
            self.start = Some(point);
            self.end = None;
        } else {
            self.end = Some(point);
        }
    }
    pub(super) fn segment(&self, hover: Option<DVec2>) -> Option<[DVec2; 2]> {
        Some([self.start?, self.end.or(hover)?])
    }
}
impl Atelier {
    pub(super) fn measure_board(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let hover = self.authoring.board.hover_snap.map(|s| s.point);
        if response.clicked()
            && let Some(point) = hover
        {
            self.authoring.measure.click(point);
        }
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.authoring.measure = Default::default();
        }
        let painter = ui.painter().with_clip_rect(rect);
        let text = if let Some([a, b]) = self.authoring.measure.segment(hover) {
            let delta = b - a;
            let a = self.authoring.board.screen(rect, a);
            let b = self.authoring.board.screen(rect, b);
            let color = ui.visuals().selection.stroke.color;
            painter.line_segment([a, b], Stroke::new(2., color));
            for point in [a, b] {
                painter.circle_filled(point, 4., color);
                painter.circle_stroke(point, 7., Stroke::new(1., color));
            }
            format!(
                "{:.1} mm  ·  ΔX {:+.1}  ΔY {:+.1} mm  ·  {:.1}°",
                delta.length() * 1000.,
                delta.x * 1000.,
                delta.y * 1000.,
                delta.y.atan2(delta.x).to_degrees()
            )
        } else {
            "Click the first point · then click the second point".into()
        };
        let galley = painter.layout_no_wrap(
            text,
            egui::FontId::proportional(14.),
            ui.visuals().text_color(),
        );
        let at = rect.left_top() + Vec2::splat(12.);
        painter.rect_filled(
            egui::Rect::from_min_size(at - Vec2::splat(6.), galley.size() + Vec2::splat(12.)),
            4.,
            ui.visuals().window_fill(),
        );
        painter.galley(at, galley, ui.visuals().text_color());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_clicks_lock_a_measurement_third_starts_another() {
        let mut tape = Tape::default();
        tape.click(DVec2::ZERO);
        assert_eq!(
            tape.segment(Some(DVec2::new(0.3, 0.4))).unwrap()[0],
            DVec2::ZERO
        );
        tape.click(DVec2::new(0.3, 0.4));
        let [a, b] = tape.segment(Some(DVec2::ONE)).unwrap();
        assert!((a.distance(b) - 0.5).abs() < 1e-12);
        tape.click(DVec2::ONE);
        assert!(tape.segment(None).is_none());
    }
}
