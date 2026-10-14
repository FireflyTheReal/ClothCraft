//! Transactional authoring in material coordinates, independent of UI and solver.
//! Compile before accepting an edit. Reset physical state only when Edit requests it.
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
        tangents: Vec<DVec2>,
        meshing: MeshingSettings,
        fabric: String,
        replace: bool,
    },
    Place {
        panel: String,
        placement: Placement,
    },
    Shape {
        panel: String,
        polygon: PolygonPattern,
    },
    Layout {
        panel: String,
        origin: DVec2,
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
    pub reset_simulation: bool,
}

pub fn layout(template: &GarmentTemplate, index: usize) -> DVec2 {
    template.panels[index].layout_2d.unwrap_or_else(|| {
        DVec2::new(
            template.panels[..index]
                .iter()
                .map(|p| p.pattern.width + 0.15)
                .sum(),
            0.,
        )
    })
}
pub fn same_physics(a: &GarmentTemplate, b: &GarmentTemplate) -> bool {
    let without_layout = |t: &GarmentTemplate| {
        let mut t = t.clone();
        for p in &mut t.panels {
            p.layout_2d = None;
        }
        serde_json::to_vec(&t).unwrap()
    };
    without_layout(a) == without_layout(b)
}
fn normalize(panel: &mut Panel) {
    let poly = panel.polygon.as_mut().unwrap();
    let lo = poly
        .points
        .iter()
        .chain(poly.curves.values().flatten())
        .copied()
        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
    let hi = poly
        .points
        .iter()
        .chain(poly.curves.values().flatten())
        .copied()
        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
    for p in poly
        .points
        .iter_mut()
        .chain(poly.curves.values_mut().flatten())
        .chain(&mut panel.pin_points)
    {
        *p -= lo;
    }
    panel.layout_2d = Some(panel.layout_2d.unwrap_or_default() + lo);
    panel.placement.translation = panel.placement.transform(DVec3::new(lo.x, -lo.y, 0.));
    panel.pattern.width = (hi - lo).x;
    panel.pattern.height = (hi - lo).y;
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
            mut tangents,
            meshing,
            fabric,
            replace,
        } => {
            if !(3..=64).contains(&points.len()) || points.iter().any(|p| !p.is_finite()) {
                return Err("Dessiner 3 à 64 points finis".into());
            }
            if tangents.is_empty() {
                tangents.resize(points.len(), DVec2::ZERO);
            }
            if tangents.len() != points.len() || tangents.iter().any(|p| !p.is_finite()) {
                return Err("Invalid drawing tangents".into());
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
                tangents.reverse();
                for t in &mut tangents {
                    *t = -*t;
                }
            }
            if replace {
                template.name = "Custom garment".into();
                template.panels.clear();
                template.seams.clear();
            }
            let id = unused_id("panel", template.panels.iter().map(|p| p.id.clone()));
            let curves = (0..points.len())
                .filter_map(|i| {
                    let j = (i + 1) % points.len();
                    (tangents[i].length_squared() + tangents[j].length_squared() > 1e-20).then(
                        || {
                            (
                                format!("edge_{}", i + 1),
                                [points[i] + tangents[i], points[j] - tangents[j]],
                            )
                        },
                    )
                })
                .collect();
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
                    curves,
                }),
                fabric,
                grain_degrees: 90.,
                placement: Placement {
                    translation: if let Some(first) = template.panels.first() {
                        let delta = lo - layout(&template, 0);
                        first.placement.translation + DVec3::new(delta.x, -delta.y, 0.12)
                    } else {
                        DVec3::new(-size.x * 0.5, 1.5, 0.45)
                    },
                    ..Default::default()
                },
                pins: vec![],
                pin_points: vec![],
                layout_2d: Some(lo),
            });
            normalize(template.panels.last_mut().unwrap());
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
        Command::Shape { panel, polygon } => {
            let index = template
                .panels
                .iter()
                .position(|p| p.id == panel)
                .ok_or("Panneau absent")?;
            let origin = layout(&template, index);
            let p = &mut template.panels[index];
            let old = p
                .polygon
                .as_ref()
                .ok_or("Convertir le patron rectangulaire avant édition")?;
            if polygon.edges != old.edges || polygon.points.len() != old.points.len() {
                return Err("Les bords nommés doivent rester stables".into());
            }
            p.layout_2d = Some(origin);
            p.polygon = Some(polygon);
            normalize(p);
            selected = Some(panel);
        }
        Command::Layout { panel, origin } => {
            template
                .panels
                .iter_mut()
                .find(|p| p.id == panel)
                .ok_or("Panneau absent")?
                .layout_2d = Some(origin);
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
            let i = template.panels.iter().position(|p| p.id == panel).unwrap();
            copy.layout_2d = Some(layout(&template, i) + DVec2::new(copy.pattern.width + 0.15, 0.));
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
        reset_simulation: !same_physics(source, &template),
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
            reset_simulation: !same_physics(current, &template),
            template,
            garment,
            selected: None,
        })
    }
}
