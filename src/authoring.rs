//! Transactional authoring in material coordinates, independent of UI and solver.
//! Compile before accepting an edit. Reset physical state only when Edit requests it.
use crate::{
    garment::{
        CompiledGarment, GarmentTemplate,
        meshing::MeshingSettings,
        polygon::PolygonPattern,
        seam::{BoundaryRef, SeamSpec},
        template::{Fabric, FabricProperties, Panel, Placement},
    },
    model::{Pattern, State},
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
    BoardTransform {
        panel: String,
        origin: DVec2,
        rotation_degrees: f64,
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
    Fabric {
        panel: String,
        name: String,
        properties: FabricProperties,
        all_panels: bool,
    },
}

pub struct Edit {
    pub template: GarmentTemplate,
    pub garment: CompiledGarment,
    pub selected: Option<String>,
    pub reset_simulation: bool,
}

impl Edit {
    /// Keep the drape of panels whose compiled rest geometry and arrangement
    /// are unchanged. Match by panel ID and local indices, never global offsets.
    /// New/remeshed/moved panels start from their new arrangement. Solver/contact
    /// caches must still be rebuilt, even when all positions are preserved.
    pub fn state_after(&self, previous: &CompiledGarment, state: &State) -> State {
        let mut next = self.garment.initial_state.clone();
        next.time = state.time;
        if state.x.len() != previous.mesh.uv.len() || state.v.len() != state.x.len() {
            return next;
        }
        for new in &self.garment.panels {
            let Some(old) = previous.panels.iter().find(|p| p.id == new.id) else {
                continue;
            };
            let a = old.vertices.clone();
            let b = new.vertices.clone();
            if previous.mesh.uv[a.clone()] != self.garment.mesh.uv[b.clone()]
                || previous.initial_state.x[a.clone()] != self.garment.initial_state.x[b.clone()]
                || previous.mesh.pins[a.clone()] != self.garment.mesh.pins[b.clone()]
                || !previous.mesh.triangles[old.triangles.clone()]
                    .iter()
                    .map(|t| t.vertices.map(|i| i - a.start))
                    .eq(self.garment.mesh.triangles[new.triangles.clone()]
                        .iter()
                        .map(|t| t.vertices.map(|i| i - b.start)))
            {
                continue;
            }
            next.x[b.clone()].copy_from_slice(&state.x[a.clone()]);
            next.v[b].copy_from_slice(&state.v[a]);
        }
        next
    }
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
            p.layout_rotation_degrees = 0.;
        }
        serde_json::to_vec(&t).unwrap()
    };
    without_layout(a) == without_layout(b)
}
pub fn rotate_2d(p: DVec2, degrees: f64) -> DVec2 {
    let (s, c) = degrees.to_radians().sin_cos();
    DVec2::new(c * p.x - s * p.y, s * p.x + c * p.y)
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
    panel.layout_2d =
        Some(panel.layout_2d.unwrap_or_default() + rotate_2d(lo, panel.layout_rotation_degrees));
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
        Command::Fabric {
            panel,
            name,
            properties,
            all_panels,
        } => {
            properties.bind(0.).validate()?;
            if name.trim().is_empty() {
                return Err("Nom de tissu vide".into());
            }
            let index = template
                .panels
                .iter()
                .position(|p| p.id == panel)
                .ok_or("Panneau absent")?;
            let old_id = template.panels[index].fabric.clone();
            let shared = template
                .panels
                .iter()
                .filter(|p| p.fabric == old_id)
                .count()
                > 1;
            let id = if shared && !all_panels {
                let id = unused_id("fabric", template.fabrics.iter().map(|f| f.id.clone()));
                template.fabrics.push(Fabric {
                    id: id.clone(),
                    name,
                    properties,
                });
                id
            } else {
                let f = template
                    .fabrics
                    .iter_mut()
                    .find(|f| f.id == old_id)
                    .ok_or("Tissu absent")?;
                f.name = name;
                f.properties = properties;
                old_id
            };
            for (i, p) in template.panels.iter_mut().enumerate() {
                if all_panels || i == index {
                    p.fabric = id.clone();
                }
            }
            selected = Some(panel);
        }
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
                layout_rotation_degrees: 0.,
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
        Command::BoardTransform {
            panel,
            origin,
            rotation_degrees,
        } => {
            let p = template
                .panels
                .iter_mut()
                .find(|p| p.id == panel)
                .ok_or("Panneau absent")?;
            if !rotation_degrees.is_finite() {
                return Err("Rotation 2D invalide".into());
            }
            p.layout_2d = Some(origin);
            p.layout_rotation_degrees = rotation_degrees.rem_euclid(360.);
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
