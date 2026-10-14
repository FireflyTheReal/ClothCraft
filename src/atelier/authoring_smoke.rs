//! Explicit native GPU smoke: inject real egui pointer events into editor widgets.
use super::*;
use std::{collections::BTreeMap, time::Instant};

const ACTIONS: [(&str, Vec2); 20] = [
    ("Dessiner", Vec2::ZERO),
    ("replace", Vec2::ZERO),
    ("point0", Vec2::ZERO),
    ("point1", Vec2::ZERO),
    ("point2", Vec2::ZERO),
    ("point3", Vec2::ZERO),
    ("create", Vec2::ZERO),
    ("gizmo_x", Vec2::new(35., 0.)),
    ("duplicate", Vec2::ZERO),
    ("Coudre", Vec2::ZERO),
    ("panel_1", Vec2::ZERO),
    ("panel_2", Vec2::ZERO),
    ("sew", Vec2::ZERO),
    ("undo", Vec2::ZERO),
    ("redo", Vec2::ZERO),
    ("Placer 3D", Vec2::ZERO),
    ("focus", Vec2::ZERO),
    ("turn", Vec2::ZERO),
    ("ring_z", Vec2::new(25., 20.)),
    ("export", Vec2::ZERO),
];
pub(super) struct Smoke {
    targets: BTreeMap<String, egui::Pos2>,
    action: usize,
    tick: usize,
    origin: Option<egui::Pos2>,
    start: Instant,
    pub finished: bool,
    undo_observed: bool,
    redo_observed: bool,
}
impl Smoke {
    pub fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
            action: 0,
            tick: 0,
            origin: None,
            start: Instant::now(),
            finished: false,
            undo_observed: false,
            redo_observed: false,
        }
    }
    pub fn target(&mut self, name: &str, p: egui::Pos2) {
        self.targets.insert(name.into(), p);
    }
    pub fn input(&mut self, input: &mut egui::RawInput) {
        if self.action >= ACTIONS.len() {
            return;
        }
        let (key, delta) = ACTIONS[self.action];
        self.tick += 1;
        if self.tick == 3 {
            self.origin = self.targets.get(key).copied();
        }
        if (3..=6).contains(&self.tick)
            && let Some(origin) = self.origin
        {
            let pos = origin + if self.tick >= 4 { delta } else { Vec2::ZERO };
            input.events.push(egui::Event::PointerMoved(pos));
            if self.tick == 3 || self.tick == 6 {
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: self.tick == 3,
                    modifiers: Default::default(),
                });
            }
        }
        if self.tick >= 10 && self.origin.is_some() {
            self.action += 1;
            self.tick = 0;
            self.origin = None;
        } else if self.tick >= 10 {
            self.tick = 0;
        }
    }
}
impl Atelier {
    pub(super) fn smoke_target(&mut self, name: &str, p: egui::Pos2) {
        if let Some(smoke) = &mut self.authoring_smoke {
            smoke.targets.insert(name.into(), p);
        }
    }
    pub(super) fn authoring_smoke_tick(&mut self) {
        let Some(smoke) = &mut self.authoring_smoke else {
            return;
        };
        smoke.undo_observed |= smoke.action == 14 && self.project.template.seams.is_empty();
        smoke.redo_observed |= smoke.action == 15 && self.project.template.seams.len() == 1;
        if smoke.finished || (smoke.action < ACTIONS.len() && smoke.start.elapsed().as_secs() < 25)
        {
            return;
        }
        smoke.finished = true;
        let complete = smoke.action == ACTIONS.len();
        let moved = self
            .project
            .template
            .panels
            .first()
            .is_some_and(|p| p.placement.translation.x > 0.66);
        let rotated = self
            .project
            .template
            .panels
            .iter()
            .any(|p| p.placement.rotation_degrees.length() > 1.);
        let exported = FittingProject::load(&self.output.join("fitting-project.json"))
            .and_then(|p| p.compile().map(|_| p))
            .is_ok_and(|p| p.template.panels.len() == 2 && p.template.seams.len() == 1);
        let report = serde_json::json!({"passed":complete&&moved&&rotated&&exported&&smoke.undo_observed&&smoke.redo_observed&&self.project.template.seams.len()==1,
            "completed_pointer_actions":smoke.action,"expected_actions":ACTIONS.len(),"translated_with_gizmo":moved,"rotated_with_gizmo":rotated,
            "export_reload_compiles":exported,"undo_observed":smoke.undo_observed,"redo_observed":smoke.redo_observed,"panels":self.project.template.panels.len(),"seams":self.project.template.seams.len(),"status":self.status});
        if let Err(e) = std::fs::write(
            self.output.join("authoring.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        ) {
            eprintln!("Authoring smoke: {e}");
        }
    }
}
