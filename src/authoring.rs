//! Transactional authoring in material coordinates, independent of UI and solver.
//! Compile before accepting an edit. Callers must reset state and solver caches.
use crate::{
    garment::{
        CompiledGarment, GarmentTemplate,
        meshing::MeshingSettings,
        polygon::PolygonPattern,
        seam::{BoundaryRef, SeamSpec},
        template::{Panel, Placement},
    },
    model::Pattern,
};
use glam::{DQuat, DVec2, DVec3, EulerRot};

pub enum Command {
    Polygon {
        points: Vec<DVec2>,
        meshing: MeshingSettings,
        fabric: String,
        replace: bool,
    },
    Place {
        panel: String,
        placement: Placement,
    },
    Duplicate {
        panel: String,
    },
    RemovePanel {
        panel: String,
    },
    Sew {
        a: BoundaryRef,
        b: BoundaryRef,
        reverse: bool,
        ease: bool,
    },
    RemoveSeam {
        seam: String,
    },
}

pub struct Edit {
    pub template: GarmentTemplate,
    pub garment: CompiledGarment,
    pub selected: Option<String>,
}

fn unused_id(prefix: &str, ids: impl Iterator<Item = String>) -> String {
    let ids: std::collections::BTreeSet<_> = ids.collect();
    (1..)
        .map(|i| format!("{prefix}_{i}"))
        .find(|s| !ids.contains(s))
        .unwrap()
}

/// An error leaves the input byte-for-byte unchanged; no partial edit is visible.
pub fn apply(source: &GarmentTemplate, command: Command) -> Result<Edit, String> {
    let mut template = source.clone();
    let mut selected = None;
    match command {
        Command::Polygon {
            mut points,
            meshing,
            fabric,
            replace,
        } => {
            if !(3..=64).contains(&points.len()) || points.iter().any(|p| !p.is_finite()) {
                return Err("Dessiner 3 à 64 points finis".into());
            }
            let lo = points
                .iter()
                .copied()
                .fold(DVec2::splat(f64::INFINITY), DVec2::min);
            for p in &mut points {
                *p -= lo;
            }
            let size = points.iter().copied().fold(DVec2::ZERO, DVec2::max);
            let area: f64 = (0..points.len())
                .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
                .sum();
            if area < 0. {
                points.reverse();
            }
            if replace {
                template.name = "Custom garment".into();
                template.panels.clear();
                template.seams.clear();
            }
            let id = unused_id("panel", template.panels.iter().map(|p| p.id.clone()));
            template.panels.push(Panel {
                id: id.clone(),
                name: id.clone(),
                pattern: Pattern {
                    width: size.x,
                    height: size.y,
                    nx: 1,
                    ny: 1,
                },
                polygon: Some(PolygonPattern {
                    edges: (0..points.len())
                        .map(|i| format!("edge_{}", i + 1))
                        .collect(),
                    points,
                    subdivisions: 0,
                    meshing: Some(meshing),
                }),
                fabric,
                grain_degrees: 90.,
                placement: Placement {
                    translation: DVec3::new(0.65, 1.50, 0.35),
                    ..Default::default()
                },
                pins: vec![],
                pin_points: vec![],
            });
            template.version = 2;
            selected = Some(id);
        }
        Command::Place { panel, placement } => {
            template
                .panels
                .iter_mut()
                .find(|p| p.id == panel)
                .ok_or("Panneau absent")?
                .placement = placement;
            selected = Some(panel);
        }
        Command::Duplicate { panel } => {
            let mut copy = template
                .panels
                .iter()
                .find(|p| p.id == panel)
                .ok_or("Panneau absent")?
                .clone();
            copy.id = unused_id("panel", template.panels.iter().map(|p| p.id.clone()));
            copy.name = format!("{} copie", copy.name);
            copy.placement.translation.x += copy.pattern.width + 0.05;
            selected = Some(copy.id.clone());
            template.panels.push(copy);
        }
        Command::RemovePanel { panel } => {
            if template.panels.len() <= 1 {
                return Err("Conserver au moins un panneau".into());
            }
            if !template.panels.iter().any(|p| p.id == panel) {
                return Err("Panneau absent".into());
            }
            template.panels.retain(|p| p.id != panel);
            template
                .seams
                .retain(|s| s.a.panel != panel && s.b.panel != panel);
        }
        Command::Sew {
            a,
            b,
            reverse,
            ease,
        } => {
            let same_edge = |x: &BoundaryRef, y: &BoundaryRef| {
                x.panel == y.panel
                    && if x.edge.is_some() || y.edge.is_some() {
                        x.edge == y.edge
                    } else {
                        x.side == y.side
                    }
            };
            if same_edge(&a, &b) {
                return Err("Choisir deux bords distincts".into());
            }
            // This first editor supports full boundary segments only. Imported
            // partial seams stay intact; overlapping edits require free sewing.
            for end in [&a, &b] {
                if end.range != [0., 1.] {
                    return Err("L'éditeur attend un bord entier".into());
                }
                if template
                    .seams
                    .iter()
                    .any(|s| same_edge(end, &s.a) || same_edge(end, &s.b))
                {
                    return Err(
                        "Bord déjà cousu : supprimer sa couture avant de le réutiliser".into(),
                    );
                }
            }
            let id = unused_id("seam", template.seams.iter().map(|s| s.id.clone()));
            template.seams.push(SeamSpec {
                id,
                a,
                b,
                reverse_b: reverse,
                allow_ease: ease,
                notches: vec![],
                stiffness: 50_000.,
            });
        }
        Command::RemoveSeam { seam } => {
            if !template.seams.iter().any(|s| s.id == seam) {
                return Err("Couture absente".into());
            }
            template.seams.retain(|s| s.id != seam);
        }
    }
    let garment = template.compile()?;
    Ok(Edit {
        template,
        garment,
        selected,
    })
}

/// Rigid rotation around a material-space pivot. Rest UV and area never change.
pub fn rotate_about(placement: &Placement, pivot: DVec3, axis: DVec3, angle: f64) -> Placement {
    let r = placement.rotation_degrees * std::f64::consts::PI / 180.;
    let old = DQuat::from_euler(EulerRot::XYZ, r.x, r.y, r.z);
    let new = DQuat::from_axis_angle(axis, angle) * old;
    let (x, y, z) = new.to_euler(EulerRot::XYZ);
    Placement {
        translation: placement.transform(pivot) - new * pivot,
        rotation_degrees: DVec3::new(x, y, z) * 180. / std::f64::consts::PI,
    }
}

/// History stores editable templates, never GPU resources or transient indices.
#[derive(Default)]
pub struct History {
    undo: Vec<GarmentTemplate>,
    redo: Vec<GarmentTemplate>,
}
impl History {
    pub fn record(&mut self, old: GarmentTemplate) {
        if self.undo.len() == 32 {
            self.undo.remove(0);
        }
        self.undo.push(old);
        self.redo.clear();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn restore(&mut self, current: &GarmentTemplate, redo: bool) -> Result<Edit, String> {
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let template = from.last().ok_or("Historique vide")?;
        let garment = template.compile()?;
        let template = from.pop().unwrap();
        to.push(current.clone());
        Ok(Edit {
            template,
            garment,
            selected: None,
        })
    }
}
