//! Small CPU drafting operations. Curves are flattened only for booleans.
use super::{
    domain::{Hole, area},
    meshing::MeshingSettings,
    polygon::{MAX_SEAM_ALLOWANCE_M, PolygonPattern},
};
use glam::DVec2;
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Rectangle,
    Ellipse,
    Sector,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorAxis {
    X,
    Y,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
}

pub fn primitive(
    kind: Primitive,
    size: DVec2,
    sweep_degrees: f64,
    meshing: MeshingSettings,
) -> Result<PolygonPattern, String> {
    if !size.is_finite() || size.min_element() < 0.005 || size.max_element() > 10. {
        return Err("Dimensions : 5 mm à 10 m".into());
    }
    let mut points = vec![];
    let mut controls = vec![];
    if kind == Primitive::Rectangle {
        points = vec![
            DVec2::ZERO,
            DVec2::new(size.x, 0.),
            size,
            DVec2::new(0., size.y),
        ];
    } else {
        let sweep = if kind == Primitive::Ellipse {
            360.
        } else {
            sweep_degrees
        };
        if !sweep.is_finite() || !(5. ..=355.).contains(&sweep) && kind == Primitive::Sector {
            return Err("Arc : angle entre 5° et 355°".into());
        }
        let count = (sweep / 90.).ceil() as usize;
        let step = sweep.to_radians() / count as f64;
        let center = size * 0.5;
        let pos = |a: f64| center + size * 0.5 * DVec2::new(a.cos(), a.sin());
        let tangent = |a: f64| size * 0.5 * DVec2::new(-a.sin(), a.cos());
        for i in 0..count {
            let a = i as f64 * step;
            let b = a + step;
            let k = 4. / 3. * (step / 4.).tan();
            points.push(pos(a));
            controls.push((i, [pos(a) + k * tangent(a), pos(b) - k * tangent(b)]));
        }
        if kind == Primitive::Sector {
            points.extend([pos(sweep.to_radians()), center]);
        }
    }
    let edges: Vec<_> = (0..points.len())
        .map(|i| format!("edge_{}", i + 1))
        .collect();
    let curves = controls
        .into_iter()
        .map(|(i, c)| (edges[i].clone(), c))
        .collect();
    Ok(PolygonPattern {
        points,
        edges,
        curves,
        subdivisions: 0,
        meshing: Some(meshing),
        holes: vec![],
        lines: vec![],
        target_lengths: Default::default(),
        notches: Default::default(),
        seam_allowances: Default::default(),
    })
}

/// Per-boundary outward offset of the sewing contour, with joined corners and
/// self-intersection cleanup. `None` when no seam allowance is stored.
///
/// The result follows one convention: outer rings are counter-clockwise
/// (positive signed area) and hole rings are clockwise. Hole rims are copied
/// unchanged; only outer boundaries can carry an allowance.
pub fn cutting_outline(poly: &PolygonPattern) -> Result<Option<Vec<Vec<DVec2>>>, String> {
    if poly.seam_allowances.is_empty() {
        return Ok(None);
    }
    if poly
        .seam_allowances
        .values()
        .any(|w| !w.is_finite() || *w <= 0. || *w > MAX_SEAM_ALLOWANCE_M)
    {
        return Err("Invalid seam allowance width".into());
    }
    let outer = poly.outer_points()?;
    if outer.len() < 3 {
        return Err("A cutting outline needs a sewing contour of at least 3 points".into());
    }
    // Positive material region: the CCW sewing contour, then the CW holes.
    let mut subject: Vec<Vec<[f64; 2]>> = vec![outer.iter().map(|p| p.to_array()).collect()];
    for hole in &poly.holes {
        subject.push(hole.points.iter().map(|p| p.to_array()).collect());
    }
    // One outward band per marked boundary. Each band is a CCW quad extended by
    // its own width along the edge, so adjacent bands overlap into a square
    // corner join; the positive fill rule unions those overlaps instead of
    // cancelling them, which needs no separate mitre step. The join is a square
    // approximation: an acute corner may overshoot the true miter by up to one
    // allowance width, always outward from the material.
    for (i, name) in poly.edges.iter().enumerate() {
        let Some(&width) = poly.seam_allowances.get(name) else {
            continue;
        };
        for pair in poly.edge_points(i)?.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let tangent = b - a;
            if tangent.length_squared() <= 1e-20 {
                continue;
            }
            let along = tangent.normalize();
            let offset = DVec2::new(along.y, -along.x) * width;
            let end = along * width;
            subject.push(vec![
                (a - end).to_array(),
                (a - end + offset).to_array(),
                (b + end + offset).to_array(),
                (b + end).to_array(),
            ]);
        }
    }
    if subject
        .iter()
        .flatten()
        .any(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 1000.))
    {
        return Err("Seam allowance produced out-of-range coordinates".into());
    }
    // Resolving the subject alone merges the overlapping positive bands and
    // subtracts the negative holes; no separate self-intersection pass is needed.
    let shapes = subject.overlay(
        &Vec::<Vec<[f64; 2]>>::new(),
        OverlayRule::Subject,
        FillRule::Positive,
    );
    let sewing_area = area(&outer);
    let mut rings = Vec::new();
    let mut cutting_area = 0.;
    for shape in shapes {
        for contour in shape {
            let points: Vec<DVec2> = contour.into_iter().map(DVec2::from_array).collect();
            let signed = area(&points);
            if signed.abs() < 1e-9 {
                continue;
            }
            if signed > 0. {
                cutting_area += signed;
            }
            rings.push(points);
        }
    }
    if rings.is_empty() || cutting_area + 1e-9 < sewing_area {
        return Err("Seam allowance did not grow the sewing contour".into());
    }
    Ok(Some(rings))
}

fn reverse_contour(points: &mut [DVec2], edges: &mut Vec<String>) {
    let n = points.len();
    points.reverse();
    *edges = (0..n).map(|i| edges[(2 * n - 2 - i) % n].clone()).collect();
}
/// Reflect material geometry, restoring contour winding without renaming edges.
/// Internal path parameter directions remain unchanged; contour directions reverse.
pub fn mirror(poly: &mut PolygonPattern, axis: MirrorAxis, size: DVec2) {
    let reflect = |p: &mut DVec2| match axis {
        MirrorAxis::X => p.x = size.x - p.x,
        MirrorAxis::Y => p.y = size.y - p.y,
    };
    for p in &mut poly.points {
        reflect(p);
    }
    for c in poly.curves.values_mut() {
        for p in c.iter_mut() {
            reflect(p);
        }
        c.swap(0, 1);
    }
    reverse_contour(&mut poly.points, &mut poly.edges);
    for h in &mut poly.holes {
        for p in &mut h.points {
            reflect(p);
        }
        reverse_contour(&mut h.points, &mut h.edges);
    }
    for line in &mut poly.lines {
        for p in &mut line.points {
            reflect(p);
        }
        for t in &mut line.tangents {
            match axis {
                MirrorAxis::X => t.x = -t.x,
                MirrorAxis::Y => t.y = -t.y,
            }
        }
    }
    // Reversing the contour flips the direction of every named boundary, so a
    // notch keeps its material position only if its arclength fraction inverts.
    // Allowance widths stay keyed by the same boundary names.
    for notches in poly.notches.values_mut() {
        for notch in notches.iter_mut() {
            notch.t = 1. - notch.t;
        }
        notches.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    if let Some(settings) = &mut poly.meshing {
        settings.axis_degrees = match axis {
            MirrorAxis::X => 180. - settings.axis_degrees,
            MirrorAxis::Y => -settings.axis_degrees,
        };
    }
}

pub fn boolean(
    a: &PolygonPattern,
    b: &PolygonPattern,
    op: BooleanOp,
) -> Result<Vec<PolygonPattern>, String> {
    let contours = |p: &PolygonPattern| -> Result<Vec<Vec<[f64; 2]>>, String> {
        let mut out = vec![
            p.outer_points()?
                .into_iter()
                .map(|p| p.to_array())
                .collect(),
        ];
        out.extend(
            p.holes
                .iter()
                .map(|h| h.points.iter().map(|p| p.to_array()).collect()),
        );
        Ok(out)
    };
    let a_paths = contours(a)?;
    let b_paths = contours(b)?;
    if a_paths
        .iter()
        .chain(&b_paths)
        .flatten()
        .any(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 1000.))
    {
        return Err("Booléen : coordonnées finies dans ±1000 m requises".into());
    }
    let rule = match op {
        BooleanOp::Union => OverlayRule::Union,
        BooleanOp::Difference => OverlayRule::Difference,
        BooleanOp::Intersection => OverlayRule::Intersect,
    };
    let result = a_paths.overlay(&b_paths, rule, FillRule::EvenOdd);
    if result.is_empty() {
        return Err("Résultat vide : aucune modification".into());
    }
    if result.len() > 32 {
        return Err("Booléen : maximum 32 pièces résultantes".into());
    }
    let mut out = vec![];
    for shape in result {
        let mut contours = shape
            .into_iter()
            .map(|p| p.into_iter().map(DVec2::from_array).collect::<Vec<_>>());
        let mut points = contours.next().ok_or("Résultat booléen invalide")?;
        if area(&points) < 0. {
            points.reverse();
        }
        let mut holes = vec![];
        for (i, mut points) in contours.enumerate() {
            if area(&points) > 0. {
                points.reverse();
            }
            holes.push(Hole {
                edges: (0..points.len())
                    .map(|j| format!("hole_{}_edge_{}", i + 1, j + 1))
                    .collect(),
                points,
            });
        }
        out.push(PolygonPattern {
            edges: (0..points.len())
                .map(|i| format!("edge_{}", i + 1))
                .collect(),
            points,
            curves: BTreeMap::new(),
            subdivisions: 0,
            meshing: Some(a.meshing_settings()),
            holes,
            lines: vec![],
            target_lengths: Default::default(),
            notches: Default::default(),
            seam_allowances: Default::default(),
        });
    }
    Ok(out)
}
