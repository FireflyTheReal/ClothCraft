//! One metric drafting board. Layout is presentation; rest contours remain local.
use super::{
    authoring_ui::{Tool, segment_distance},
    *,
};
use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command},
    garment::{
        polygon::PolygonPattern,
        seam::{BoundaryRef, Side},
    },
};

pub(super) struct Board {
    pub center: DVec2,
    pub scale: f32,
    pub fitted: bool,
    pub snap: bool,
    drag: Option<Drag>,
    pub shape: Option<(String, PolygonPattern)>,
    pub layout: Option<(String, DVec2)>,
    pub rotation: Option<(String, f64)>,
}
impl Default for Board {
    fn default() -> Self {
        Self {
            center: DVec2::ZERO,
            scale: 300.,
            fitted: false,
            snap: true,
            drag: None,
            shape: None,
            layout: None,
            rotation: None,
        }
    }
}
impl Board {
    pub fn clear_preview(&mut self) {
        self.drag = None;
        self.shape = None;
        self.layout = None;
        self.rotation = None;
    }
    pub(super) fn screen(&self, rect: egui::Rect, p: DVec2) -> egui::Pos2 {
        rect.center()
            + Vec2::new((p.x - self.center.x) as f32, (p.y - self.center.y) as f32) * self.scale
    }
    fn world(&self, rect: egui::Rect, p: egui::Pos2) -> DVec2 {
        self.center
            + DVec2::new(
                (p.x - rect.center().x) as f64,
                (p.y - rect.center().y) as f64,
            ) / self.scale as f64
    }
    fn snapped(&self, p: DVec2) -> DVec2 {
        if self.snap {
            (p * 100.).round() / 100.
        } else {
            p
        }
    }
}
enum Drag {
    Draft(usize),
    Rectangle(DVec2),
    Layout {
        panel: String,
        start: DVec2,
        pointer: DVec2,
    },
    Rotate {
        panel: String,
        pivot: DVec2,
        center: DVec2,
        vector: DVec2,
        angle: f64,
    },
    Shape {
        panel: String,
        polygon: PolygonPattern,
        part: Part,
        origin: DVec2,
        pointer: DVec2,
        angle: f64,
    },
}
#[derive(Clone)]
enum Part {
    Body,
    Anchor(usize),
    Edge(usize),
    Path(String),
    LineAnchor(usize, usize),
    LineHandle(usize, usize),
    Handle(usize, usize),
    Rotation,
}
struct Hit {
    index: usize,
    part: Part,
}
fn inside(p: DVec2, poly: &[DVec2]) -> bool {
    let mut yes = false;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            yes = !yes;
        }
    }
    yes
}
pub(super) fn same_edge(a: &BoundaryRef, b: &BoundaryRef) -> bool {
    a.panel == b.panel && a.edge == b.edge && (a.edge.is_some() || a.side == b.side)
}
impl Atelier {
    pub(super) fn board_angle(&self, i: usize) -> f64 {
        let panel = &self.project.template.panels[i];
        self.authoring
            .board
            .rotation
            .as_ref()
            .filter(|(id, _)| id == &panel.id)
            .map(|(_, angle)| *angle)
            .unwrap_or(panel.layout_rotation_degrees)
    }
    fn rotation_handle(&self, i: usize) -> DVec2 {
        let p = &self.project.template.panels[i];
        DVec2::new(
            p.pattern.width * 0.5,
            -32. / self.authoring.board.scale as f64,
        )
    }
    pub(super) fn board_origin(&self, i: usize) -> DVec2 {
        let p = &self.project.template.panels[i];
        self.authoring
            .board
            .layout
            .as_ref()
            .filter(|(id, _)| id == &p.id)
            .map(|(_, p)| *p)
            .unwrap_or_else(|| authoring::layout(&self.project.template, i))
    }
    fn board_polygon(&self, i: usize) -> Option<&PolygonPattern> {
        let p = &self.project.template.panels[i];
        self.authoring
            .board
            .shape
            .as_ref()
            .filter(|(id, _)| id == &p.id)
            .map(|(_, poly)| poly)
            .or(p.polygon.as_ref())
    }
    fn fit_board(&mut self, rect: egui::Rect) {
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        for (i, p) in self.project.template.panels.iter().enumerate() {
            let origin = authoring::layout(&self.project.template, i);
            for corner in [
                DVec2::ZERO,
                DVec2::new(p.pattern.width, 0.),
                DVec2::new(p.pattern.width, p.pattern.height),
                DVec2::new(0., p.pattern.height),
            ] {
                let q = origin + authoring::rotate_2d(corner, p.layout_rotation_degrees);
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
        self.authoring.board.center = (lo + hi) * 0.5;
        self.authoring.board.scale = ((rect.width() - 80.) / (hi - lo).x.max(0.1) as f32)
            .min((rect.height() - 80.) / (hi - lo).y.max(0.1) as f32)
            .clamp(60., 1600.);
        self.authoring.board.fitted = true;
    }
    fn board_hit(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<Hit> {
        let mut edge = None;
        let mut body = None;
        let mut best = 9.;
        for i in (0..self.project.template.panels.len()).rev() {
            let p = &self.project.template.panels[i];
            let origin = self.board_origin(i);
            let angle = self.board_angle(i);
            let point = |uv| {
                self.authoring
                    .board
                    .screen(rect, origin + authoring::rotate_2d(uv, angle))
            };
            if self.authoring.tool == Tool::Edit
                && self.authoring.selected.as_ref() == Some(&p.id)
                && point(self.rotation_handle(i)).distance(pos) < 11.
            {
                return Some(Hit {
                    index: i,
                    part: Part::Rotation,
                });
            }
            let Some(poly) = self.board_polygon(i) else {
                continue;
            };
            if self.authoring.tool == Tool::Edit && self.authoring.selected.as_ref() == Some(&p.id)
            {
                for (l, line) in poly.lines.iter().enumerate() {
                    if self
                        .authoring
                        .edge
                        .as_ref()
                        .is_some_and(|e| e.panel == p.id && e.edge.as_ref() == Some(&line.id))
                    {
                        for (k, uv) in line.points.iter().enumerate() {
                            for (at, part) in [
                                (*uv, Part::LineAnchor(l, k)),
                                (*uv + line.tangents[k], Part::LineHandle(l, k)),
                            ] {
                                if point(at).distance(pos) < 8. {
                                    return Some(Hit { index: i, part });
                                }
                            }
                        }
                    }
                }
                for (k, uv) in poly.points.iter().enumerate() {
                    if point(*uv).distance(pos) < 8. {
                        return Some(Hit {
                            index: i,
                            part: Part::Anchor(k),
                        });
                    }
                }
                if let Some(e) = self.authoring.edge.as_ref().filter(|e| e.panel == p.id)
                    && let Some(k) = poly
                        .edges
                        .iter()
                        .position(|name| Some(name) == e.edge.as_ref())
                    && let Some(controls) = poly.curves.get(&poly.edges[k])
                {
                    for (j, uv) in controls.iter().enumerate() {
                        if point(*uv).distance(pos) < 9. {
                            return Some(Hit {
                                index: i,
                                part: Part::Handle(k, j),
                            });
                        }
                    }
                }
            }
            let mut contour = vec![];
            for k in 0..poly.points.len() {
                let Ok(points) = poly.edge_points(k) else {
                    continue;
                };
                for pair in points.windows(2) {
                    let d = segment_distance(pos, point(pair[0]), point(pair[1]));
                    if d < best {
                        best = d;
                        edge = Some(Hit {
                            index: i,
                            part: Part::Edge(k),
                        });
                    }
                }
                contour.extend_from_slice(&points[..points.len() - 1]);
            }
            if body.is_none()
                && inside(
                    authoring::rotate_2d(self.authoring.board.world(rect, pos) - origin, -angle),
                    &contour,
                )
                && !poly.holes.iter().any(|h| {
                    inside(
                        authoring::rotate_2d(
                            self.authoring.board.world(rect, pos) - origin,
                            -angle,
                        ),
                        &h.points,
                    )
                })
            {
                body = Some(Hit {
                    index: i,
                    part: Part::Body,
                });
            }
            if let Ok(paths) = poly.paths() {
                for (name, points) in paths.into_iter().skip(poly.edges.len()) {
                    for pair in points.windows(2) {
                        let d = segment_distance(pos, point(pair[0]), point(pair[1]));
                        if d < best {
                            best = d;
                            edge = Some(Hit {
                                index: i,
                                part: Part::Path(name.clone()),
                            });
                        }
                    }
                }
            }
        }
        edge.or(body)
    }
    pub(super) fn select_boundary(&mut self, edge: BoundaryRef) {
        self.cancel_preview();
        self.authoring.selected = Some(edge.panel.clone());
        self.authoring.edge = Some(edge.clone());
        if self.authoring.tool == Tool::Sew {
            if self.authoring.ends.len() == 2 {
                self.authoring.ends.clear();
            }
            if !self.authoring.ends.iter().any(|e| same_edge(e, &edge)) {
                self.authoring.ends.push(edge);
            }
        }
    }
    pub(super) fn edge_properties(&mut self, ui: &mut egui::Ui) {
        let Some(edge) = self.authoring.edge.clone() else {
            return;
        };
        let Some(p) = self
            .project
            .template
            .panels
            .iter()
            .find(|p| p.id == edge.panel)
        else {
            return;
        };
        let Some(mut poly) = p.polygon.clone() else {
            return;
        };
        let Some(k) = poly
            .edges
            .iter()
            .position(|n| Some(n) == edge.edge.as_ref())
        else {
            return;
        };
        ui.label(format!("Bord : {}", poly.edges[k]));
        let length = poly
            .edge_points(k)
            .unwrap_or_default()
            .windows(2)
            .map(|p| p[0].distance(p[1]))
            .sum::<f64>();
        ui.small(format!(
            "{:.1} mm · même sélection en 2D / 3D",
            length * 1000.
        ));
        if self.authoring.tool == Tool::Edit {
            let curved = poly.curves.contains_key(&poly.edges[k]);
            if ui
                .button(if curved {
                    "Convertir en segment droit"
                } else {
                    "Ajouter poignées Bézier"
                })
                .clicked()
            {
                if curved {
                    poly.curves.remove(&poly.edges[k]);
                } else {
                    let a = poly.points[k];
                    let b = poly.points[(k + 1) % poly.points.len()];
                    poly.curves.insert(
                        poly.edges[k].clone(),
                        [a.lerp(b, 1. / 3.), a.lerp(b, 2. / 3.)],
                    );
                }
                self.edit(Command::Shape {
                    panel: edge.panel,
                    polygon: poly,
                });
            }
            ui.small("Glisser les points ou poignées sur le plan 2D.");
        }
        ui.separator();
    }
    pub(super) fn patterns(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("PATRONAGE 2D");
            if ui.small_button("Tout cadrer").clicked() {
                self.authoring.board.fitted = false;
            }
            let new = ui.small_button("Nouveau");
            self.smoke_target("replace", new.rect.center());
            if new.clicked() {
                self.playing = false;
                self.pending = false;
                self.generation += 1;
                self.grab = None;
                self.authoring.replace = true;
                self.authoring.tool = Tool::Draw;
                self.authoring.draft.clear();
                self.authoring.tangents.clear();
                self.authoring.board.clear_preview();
                self.authoring.board.center = DVec2::splat(0.5);
                self.authoring.board.scale = 400.;
                self.authoring.board.fitted = true;
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.authoring.board.snap, "Aimant 10 mm");
            if matches!(
                self.authoring.tool,
                Tool::Draw | Tool::Rectangle | Tool::InternalLine
            ) {
                let internal = self.authoring.tool == Tool::InternalLine;
                let create = ui.add_enabled(
                    self.authoring.draft.len() >= if internal { 2 } else { 3 },
                    egui::Button::new(if internal {
                        "Terminer la ligne"
                    } else {
                        "Fermer le contour"
                    }),
                );
                self.smoke_target("create", create.rect.center());
                if create.clicked() {
                    if internal {
                        self.finish_internal_line();
                    } else {
                        self.create_polygon();
                    }
                }
                if ui.small_button("Annuler tracé").clicked() {
                    self.authoring.draft.clear();
                    self.authoring.tangents.clear();
                    self.authoring.replace = false;
                    self.authoring.board.clear_preview();
                }
            } else {
                ui.small(format!(
                    "{} pièces · molette : zoom · milieu : déplacer",
                    self.project.template.panels.len()
                ));
            }
        });
        ui.small(match self.authoring.tool {
            Tool::Draw => {
                "Plume : cliquer = coin · cliquer-glisser = Bézier · premier point = fermer"
            }
            Tool::Rectangle => "Rectangle : glisser entre deux coins opposés",
            Tool::InternalLine => "Dans le panneau actif : cliquer / glisser = ligne / courbe · Entrée = terminer · rester à l’intérieur",
            Tool::Sew => "Choisir deux bords en 2D ou 3D, puis créer la couture →",
            Tool::Edit => {
                "Poignée ronde : tourner en 2D · intérieur : déplacer · Maj : angles de 15°"
            }
            _ => "Sélection liée à la 3D · Plume ou Rectangle pour créer du tissu",
        });
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size().max(Vec2::splat(1.)),
            egui::Sense::click_and_drag(),
        );
        if !self.authoring.board.fitted {
            self.fit_board(rect);
        }
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
        {
            self.authoring.board.center -= ui.input(|i| {
                let d = i.pointer.delta();
                DVec2::new(d.x as f64, d.y as f64)
            }) / self.authoring.board.scale as f64;
        }
        if let Some(pos) = response.hover_pos() {
            let before = self.authoring.board.world(rect, pos);
            let zoom = ui.input(|i| i.smooth_scroll_delta.y);
            self.authoring.board.scale =
                (self.authoring.board.scale * (zoom * 0.002).exp()).clamp(40., 2400.);
            let after = self.authoring.board.world(rect, pos);
            self.authoring.board.center += before - after;
        }
        self.board_events(&response, rect);
        self.paint_board(ui, rect, &response);
    }
    fn board_events(&mut self, response: &egui::Response, rect: egui::Rect) {
        let pos = response.interact_pointer_pos();
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(start) = response.ctx.input(|i| i.pointer.press_origin())
        {
            let p = self
                .authoring
                .board
                .snapped(self.authoring.board.world(rect, start));
            match self.authoring.tool {
                Tool::Draw | Tool::InternalLine => {
                    if self.authoring.draft.len() < 64 {
                        let k = self.authoring.draft.len();
                        self.authoring.draft.push(p);
                        self.authoring.tangents.push(DVec2::ZERO);
                        self.authoring.board.drag = Some(Drag::Draft(k));
                    }
                }
                Tool::Rectangle => self.authoring.board.drag = Some(Drag::Rectangle(p)),
                Tool::Edit => {
                    if let Some(hit) = self.board_hit(rect, start) {
                        let panel = &self.project.template.panels[hit.index];
                        self.authoring.selected = Some(panel.id.clone());
                        if matches!(hit.part, Part::Body) {
                            self.authoring.edge = None;
                        }
                        self.authoring.board.drag = match hit.part {
                            Part::Rotation => {
                                let pivot =
                                    DVec2::new(panel.pattern.width, panel.pattern.height) * 0.5;
                                let angle = self.board_angle(hit.index);
                                let center = self.board_origin(hit.index)
                                    + authoring::rotate_2d(pivot, angle);
                                Some(Drag::Rotate {
                                    panel: panel.id.clone(),
                                    pivot,
                                    center,
                                    vector: self.authoring.board.world(rect, start) - center,
                                    angle,
                                })
                            }
                            Part::Body => Some(Drag::Layout {
                                panel: panel.id.clone(),
                                start: self.board_origin(hit.index),
                                pointer: p,
                            }),
                            Part::Anchor(_)
                            | Part::Handle(_, _)
                            | Part::LineAnchor(_, _)
                            | Part::LineHandle(_, _) => Some(Drag::Shape {
                                panel: panel.id.clone(),
                                polygon: panel.polygon.clone().unwrap(),
                                part: hit.part,
                                origin: self.board_origin(hit.index),
                                pointer: p,
                                angle: self.board_angle(hit.index),
                            }),
                            _ => None,
                        };
                    }
                }
                _ => {}
            }
        }
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = pos
        {
            let p = self
                .authoring
                .board
                .snapped(self.authoring.board.world(rect, pos));
            match &self.authoring.board.drag {
                Some(Drag::Draft(k)) => self.authoring.tangents[*k] = p - self.authoring.draft[*k],
                Some(Drag::Rectangle(start)) => {
                    let lo = start.min(p);
                    let hi = start.max(p);
                    self.authoring.draft =
                        vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
                    self.authoring.tangents = vec![DVec2::ZERO; 4];
                }
                Some(Drag::Layout {
                    panel,
                    start,
                    pointer,
                }) => self.authoring.board.layout = Some((panel.clone(), *start + p - *pointer)),
                Some(Drag::Rotate {
                    panel,
                    pivot,
                    center,
                    vector,
                    angle,
                }) => {
                    let v = self.authoring.board.world(rect, pos) - *center;
                    if v.length_squared() > 1e-10 {
                        let mut degrees =
                            *angle + vector.perp_dot(v).atan2(vector.dot(v)).to_degrees();
                        if response.ctx.input(|i| i.modifiers.shift) {
                            degrees = (degrees / 15.).round() * 15.;
                        }
                        self.authoring.board.layout = Some((
                            panel.clone(),
                            *center - authoring::rotate_2d(*pivot, degrees),
                        ));
                        self.authoring.board.rotation = Some((panel.clone(), degrees));
                    }
                }
                Some(Drag::Shape {
                    panel,
                    polygon,
                    part,
                    origin,
                    pointer,
                    angle,
                }) => {
                    let mut poly = polygon.clone();
                    match *part {
                        Part::Anchor(k) => {
                            let delta = authoring::rotate_2d(p - *pointer, -*angle);
                            poly.points[k] += delta;
                            if let Some(c) = poly.curves.get_mut(&poly.edges[k]) {
                                c[0] += delta;
                            }
                            let prev = (k + poly.points.len() - 1) % poly.points.len();
                            if let Some(c) = poly.curves.get_mut(&poly.edges[prev]) {
                                c[1] += delta;
                            }
                        }
                        Part::Handle(k, j) => {
                            poly.curves.get_mut(&poly.edges[k]).unwrap()[j] =
                                authoring::rotate_2d(p - *origin, -*angle)
                        }
                        Part::LineAnchor(l, k) => {
                            poly.lines[l].points[k] += authoring::rotate_2d(p - *pointer, -*angle)
                        }
                        Part::LineHandle(l, k) => {
                            poly.lines[l].tangents[k] =
                                authoring::rotate_2d(p - *origin, -*angle) - poly.lines[l].points[k]
                        }
                        _ => {}
                    }
                    self.authoring.board.shape = Some((panel.clone(), poly));
                }
                _ => {}
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            match self.authoring.board.drag.take() {
                Some(Drag::Rotate { .. }) => {
                    if let (Some((panel, origin)), Some((_, rotation_degrees))) = (
                        self.authoring.board.layout.take(),
                        self.authoring.board.rotation.take(),
                    ) {
                        self.edit(Command::BoardTransform {
                            panel,
                            origin,
                            rotation_degrees,
                        });
                    }
                }
                Some(Drag::Rectangle(_)) => {
                    if self.authoring.draft.len() == 4
                        && self.authoring.draft[0].distance(self.authoring.draft[2]) > 0.02
                    {
                        self.create_polygon();
                    }
                }
                Some(Drag::Layout { .. }) => {
                    if let Some((panel, origin)) = self.authoring.board.layout.take() {
                        self.edit(Command::Layout { panel, origin });
                    }
                }
                Some(Drag::Shape { .. }) => {
                    if let Some((panel, polygon)) = self.authoring.board.shape.take() {
                        self.edit(Command::Shape { panel, polygon });
                    }
                }
                _ => {}
            }
        }
        if response.clicked()
            && let Some(pos) = pos
        {
            if matches!(self.authoring.tool, Tool::Draw | Tool::InternalLine) {
                let close = self.authoring.tool == Tool::Draw
                    && self.authoring.draft.len() >= 3
                    && self
                        .authoring
                        .board
                        .screen(rect, self.authoring.draft[0])
                        .distance(pos)
                        < 10.;
                if close {
                    self.create_polygon();
                } else if self.authoring.draft.len() < 64 {
                    let p = self
                        .authoring
                        .board
                        .snapped(self.authoring.board.world(rect, pos));
                    if self.authoring.draft.last() != Some(&p) {
                        self.authoring.draft.push(p);
                        self.authoring.tangents.push(DVec2::ZERO);
                    }
                }
            } else if let Some(hit) = self.board_hit(rect, pos) {
                let panel = &self.project.template.panels[hit.index];
                let id = panel.id.clone();
                let name = match hit.part {
                    Part::Edge(k) => Some(panel.polygon.as_ref().unwrap().edges[k].clone()),
                    Part::Path(name) => Some(name),
                    _ => None,
                };
                if let Some(name) = name {
                    self.select_boundary(BoundaryRef {
                        panel: id,
                        side: Side::Top,
                        edge: Some(name),
                        range: [0., 1.],
                    });
                } else {
                    self.cancel_preview();
                    self.authoring.selected = Some(id);
                    self.authoring.edge = None;
                }
            }
        }
        if !response.ctx.egui_wants_keyboard_input() {
            if response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.authoring.board.clear_preview();
                self.authoring.draft.clear();
                self.authoring.tangents.clear();
                self.authoring.replace = false;
            }
            if response.ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.authoring.draft.pop();
                self.authoring.tangents.pop();
            }
            if matches!(self.authoring.tool, Tool::Draw | Tool::InternalLine)
                && response.ctx.input(|i| i.key_pressed(egui::Key::Enter))
            {
                if self.authoring.tool == Tool::InternalLine {
                    self.finish_internal_line();
                } else {
                    self.create_polygon();
                }
            }
        }
        if let Some(pos) = response.hover_pos()
            && let Some(hit) = self.board_hit(rect, pos)
        {
            let panel = &self.project.template.panels[hit.index];
            let name = match hit.part {
                Part::Edge(k) => Some(panel.polygon.as_ref().unwrap().edges[k].clone()),
                Part::Path(name) => Some(name),
                _ => None,
            };
            self.authoring.hover_edge = name.map(|name| BoundaryRef {
                panel: panel.id.clone(),
                side: Side::Top,
                edge: Some(name),
                range: [0., 1.],
            });
        }
    }
    fn paint_board(&mut self, ui: &egui::Ui, rect: egui::Rect, response: &egui::Response) {
        let mut smoke_targets: Vec<(String, egui::Pos2)> = vec![];
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0., Color32::from_rgb(40, 46, 52));
        let board = &self.authoring.board;
        let spacing = if board.scale < 140. { 0.25 } else { 0.05 };
        let lo = board.world(rect, rect.min);
        let hi = board.world(rect, rect.max);
        for k in (lo.x / spacing).floor() as i32..=(hi.x / spacing).ceil() as i32 {
            painter.line_segment(
                [
                    board.screen(rect, DVec2::new(k as f64 * spacing, lo.y)),
                    board.screen(rect, DVec2::new(k as f64 * spacing, hi.y)),
                ],
                Stroke::new(0.5, Color32::from_gray(64)),
            );
        }
        for k in (lo.y / spacing).floor() as i32..=(hi.y / spacing).ceil() as i32 {
            painter.line_segment(
                [
                    board.screen(rect, DVec2::new(lo.x, k as f64 * spacing)),
                    board.screen(rect, DVec2::new(hi.x, k as f64 * spacing)),
                ],
                Stroke::new(0.5, Color32::from_gray(64)),
            );
        }
        if !self.authoring.replace {
            for (i, panel) in self.project.template.panels.iter().enumerate() {
                let origin = self.board_origin(i);
                let angle = self.board_angle(i);
                let point = |uv| board.screen(rect, origin + authoring::rotate_2d(uv, angle));
                let selected = self.authoring.selected.as_ref() == Some(&panel.id);
                let range = &self.scene.garment.panels[i];
                let mut mesh = egui::Mesh::default();
                let mut wire = Vec::new();
                // Transform each material vertex once; indices are shared by its triangles.
                mesh.vertices.reserve(range.vertices.len());
                mesh.indices.reserve(range.triangles.len() * 3);
                for &uv in &self.scene.garment.mesh.uv[range.vertices.clone()] {
                    mesh.colored_vertex(
                        point(uv),
                        if selected {
                            Color32::from_rgb(221, 234, 240)
                        } else {
                            Color32::from_gray(193)
                        },
                    );
                }
                for tri in &self.scene.garment.mesh.triangles[range.triangles.clone()] {
                    let [a, b, c] = tri.vertices.map(|v| (v - range.vertices.start) as u32);
                    mesh.add_triangle(a, b, c);
                    if self.wire {
                        for k in 0..3 {
                            wire.push(egui::Shape::line_segment(
                                [point(tri.rest[k]), point(tri.rest[(k + 1) % 3])],
                                Stroke::new(0.4, Color32::GRAY),
                            ));
                        }
                    }
                }
                painter.add(egui::Shape::mesh(mesh));
                painter.extend(wire);
                smoke_targets.push((
                    format!("body_{}", panel.id),
                    point(DVec2::new(panel.pattern.width, panel.pattern.height) * 0.5),
                ));
                if selected && self.authoring.tool == Tool::Edit {
                    let handle = point(self.rotation_handle(i));
                    painter.line_segment(
                        [point(DVec2::new(panel.pattern.width * 0.5, 0.)), handle],
                        Stroke::new(1.5, Color32::GOLD),
                    );
                    painter.circle_filled(handle, 8., Color32::from_rgb(40, 46, 52));
                    painter.circle_stroke(handle, 8., Stroke::new(2., Color32::GOLD));
                    painter.text(
                        handle + Vec2::new(14., 0.),
                        egui::Align2::LEFT_CENTER,
                        format!("↻ {:.0}°", angle.rem_euclid(360.)),
                        egui::FontId::proportional(12.),
                        Color32::GOLD,
                    );
                    smoke_targets.push(("rotate_2d".into(), handle));
                }
                let Some(poly) = self.board_polygon(i) else {
                    continue;
                };
                for (k, (name, points)) in poly.paths().unwrap_or_default().iter().enumerate() {
                    let edge = BoundaryRef {
                        panel: panel.id.clone(),
                        side: Side::Top,
                        edge: Some(name.clone()),
                        range: [0., 1.],
                    };
                    let end = self.authoring.ends.iter().position(|e| same_edge(e, &edge));
                    let active = self.seam_edge_highlighted(&edge);
                    let hover = self
                        .authoring
                        .hover_edge
                        .as_ref()
                        .is_some_and(|e| same_edge(e, &edge));
                    let sewn = self
                        .project
                        .template
                        .seams
                        .iter()
                        .find(|s| same_edge(&s.a, &edge) || same_edge(&s.b, &edge));
                    let color = if end.is_some() {
                        Color32::from_rgb(248, 117, 188)
                    } else if active || hover {
                        Color32::GOLD
                    } else if let Some(seam) = sewn {
                        super::seam_links::seam_color(&seam.id)
                    } else {
                        Color32::from_rgb(79, 155, 195)
                    };
                    {
                        painter.add(egui::Shape::line(
                            points.iter().map(|p| point(*p)).collect(),
                            Stroke::new(if active || end.is_some() { 3. } else { 1.5 }, color),
                        ));
                        if let Some(n) = end {
                            for (p, t) in [(points[0], 0), (points[points.len() - 1], 1)] {
                                let t = if n == 1 && self.authoring.reverse {
                                    1 - t
                                } else {
                                    t
                                };
                                painter.text(
                                    point(p),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}{t}", if n == 0 { "A" } else { "B" }),
                                    egui::FontId::proportional(13.),
                                    Color32::BLACK,
                                );
                            }
                        }
                        if k == 0 {
                            let p = marvelous_cloth::garment::curves::arclength_point(points, 0.5);
                            smoke_targets.push((panel.id.clone(), point(p)));
                        }
                    }
                    if selected
                        && self.authoring.tool == Tool::Edit
                        && active
                        && let Some([a, b]) = poly.curves.get(name)
                    {
                        for (j, (uv, control)) in [
                            (poly.points[k], *a),
                            (poly.points[(k + 1) % poly.points.len()], *b),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            painter.line_segment(
                                [point(uv), point(control)],
                                Stroke::new(1., Color32::LIGHT_BLUE),
                            );
                            painter.circle_filled(point(control), 5., Color32::LIGHT_BLUE);
                            smoke_targets.push((format!("handle_{j}"), point(control)));
                        }
                    }
                }
                if selected && self.authoring.tool == Tool::Edit {
                    for line in &poly.lines {
                        if self.authoring.edge.as_ref().is_some_and(|e| {
                            e.panel == panel.id && e.edge.as_ref() == Some(&line.id)
                        }) {
                            for (uv, tangent) in line.points.iter().zip(&line.tangents) {
                                painter.circle_filled(point(*uv), 4., Color32::GOLD);
                                let h = *uv + *tangent;
                                painter.line_segment(
                                    [point(*uv), point(h)],
                                    Stroke::new(1., Color32::LIGHT_BLUE),
                                );
                                painter.circle_stroke(
                                    point(h),
                                    6.,
                                    Stroke::new(1., Color32::LIGHT_BLUE),
                                );
                            }
                        }
                    }
                    for (k, uv) in poly.points.iter().enumerate() {
                        painter.circle_filled(point(*uv), 4., Color32::WHITE);
                        painter.circle_stroke(point(*uv), 4., Stroke::new(1., Color32::BLACK));
                        smoke_targets.push((format!("anchor_{k}"), point(*uv)));
                    }
                }
                painter.text(
                    point(DVec2::new(panel.pattern.width * 0.5, -0.035)),
                    egui::Align2::CENTER_BOTTOM,
                    &panel.name,
                    egui::FontId::proportional(13.),
                    if selected {
                        Color32::GOLD
                    } else {
                        Color32::WHITE
                    },
                );
            }
        }
        self.paint_seam_links(&painter, rect, false);
        let point = |uv| board.screen(rect, uv);
        for k in 0..self.authoring.draft.len().saturating_sub(1) {
            let p = [
                self.authoring.draft[k],
                self.authoring.draft[k] + self.authoring.tangents[k],
                self.authoring.draft[k + 1] - self.authoring.tangents[k + 1],
                self.authoring.draft[k + 1],
            ];
            if let Ok(samples) = marvelous_cloth::garment::curves::flatten(p, 0.00025) {
                painter.add(egui::Shape::line(
                    samples.into_iter().map(point).collect(),
                    Stroke::new(2., Color32::LIGHT_BLUE),
                ));
            }
        }
        for (k, p) in self.authoring.draft.iter().enumerate() {
            painter.circle_filled(point(*p), if k == 0 { 6. } else { 4. }, Color32::GOLD);
            if self.authoring.tangents[k].length_squared() > 1e-10 {
                let h = point(*p + self.authoring.tangents[k]);
                painter.line_segment([point(*p), h], Stroke::new(1., Color32::LIGHT_BLUE));
                painter.circle_filled(h, 4., Color32::LIGHT_BLUE);
            }
        }
        if self.authoring.tool == Tool::Rectangle && self.authoring.draft.len() == 4 {
            painter.line_segment(
                [
                    point(self.authoring.draft[3]),
                    point(self.authoring.draft[0]),
                ],
                Stroke::new(2., Color32::LIGHT_BLUE),
            );
        }
        if let (Some(last), Some(pos)) = (self.authoring.draft.last(), response.hover_pos()) {
            painter.line_segment([point(*last), pos], Stroke::new(1., Color32::GRAY));
        }
        painter.text(
            rect.left_bottom() + Vec2::new(12., -12.),
            egui::Align2::LEFT_BOTTOM,
            format!("Grille {:.0} mm · {:.0} px/m", spacing * 1000., board.scale),
            egui::FontId::proportional(12.),
            Color32::GRAY,
        );
        let targets = [
            ("point0", DVec2::new(0.15, 0.15)),
            ("point1", DVec2::new(0.45, 0.15)),
            ("point2", DVec2::new(0.45, 0.55)),
            ("point3", DVec2::new(0.15, 0.55)),
        ]
        .map(|(k, p)| (k, point(p)));
        for (key, p) in smoke_targets {
            self.smoke_target(&key, p);
        }
        for (key, p) in targets {
            self.smoke_target(key, p);
        }
    }
}
