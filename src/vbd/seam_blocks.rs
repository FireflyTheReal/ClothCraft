//! Experimental coupled sewing-component refinement of the unchanged implicit objective.
//! Blocks come only from integrated sewing couplings, never panel/seam names.
//! Every incident term is counted once, including contacts shared by several block vertices.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct Block {
    vertices: Vec<usize>,
    triangles: Vec<usize>,
    hinges: Vec<usize>,
    stitches: Vec<usize>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SeamBlockMetrics {
    pub blocks: usize,
    pub attempts: usize,
    pub accepted: usize,
    pub bound_limited: usize,
    pub objective_decrease_j: f64,
    pub elapsed_ms: f64,
}

#[cfg(test)]
fn union(a: &[usize], b: &[usize]) -> Vec<usize> {
    let mut result: Vec<_> = a.iter().chain(b).copied().collect();
    result.sort_unstable();
    result.dedup();
    result
}

pub(super) fn blocks(solver: &VbdSolver) -> Vec<Block> {
    let n = solver.graph.len();
    let mut parent: Vec<_> = (0..n).collect();
    let mut used = vec![false; n];
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for stitch in &solver.stitches {
        let free: Vec<_> = stitch
            .terms
            .iter()
            .map(|&(v, _)| v)
            .filter(|&v| solver.garment.mesh.pins[v].is_none())
            .collect();
        if let Some(&first) = free.first() {
            for &v in &free {
                let a = root(&mut parent, first);
                let b = root(&mut parent, v);
                parent[b] = a;
                used[v] = true;
            }
        }
    }
    let mut components = BTreeMap::<usize, Vec<usize>>::new();
    for (v, &used) in used.iter().enumerate() {
        if used {
            components.entry(root(&mut parent, v)).or_default().push(v);
        }
    }
    components
        .into_values()
        .filter(|vs| vs.len() > 1)
        .map(|vertices| {
            let gather = |rows: &[Vec<usize>]| {
                let mut ids: Vec<_> = vertices
                    .iter()
                    .flat_map(|&v| rows[v].iter().copied())
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                ids
            };
            Block {
                triangles: gather(&solver.triangles),
                hinges: gather(&solver.hinges),
                stitches: gather(&solver.vertex_stitches),
                vertices,
            }
        })
        .collect()
}

struct Problem<'a> {
    solver: &'a VbdSolver,
    x: &'a [DVec3],
    contacts: &'a ContactSet,
    settings: &'a VbdSettings,
    grab: Option<&'a Grab>,
    state: &'a State,
}
impl Problem<'_> {
    fn energy<S: Scalar>(&self, block: &Block, contact_ids: &[usize], p: &[[S; 3]]) -> S {
        let get = |v: usize| match block.vertices.binary_search(&v) {
            Ok(i) => p[i],
            Err(_) => self.x[v].to_array().map(S::c),
        };
        let g = &self.solver.garment;
        let mut e = S::c(0.);
        for &i in &block.triangles {
            let t = &g.mesh.triangles[i];
            e = e + energy::membrane(t.vertices.map(get), t, &g.materials[t.material]);
        }
        for &i in &block.hinges {
            let h = &g.mesh.hinges[i];
            e = e + energy::bending(h.vertices.map(get), h, &g.materials[h.material]);
        }
        for &i in &block.stitches {
            let s = &self.solver.stitches[i];
            let mut gap = [S::c(0.); 3];
            for &(v, w) in &s.terms {
                let q = get(v);
                for k in 0..3 {
                    gap[k] = gap[k] + S::c(w) * q[k];
                }
            }
            e = e + S::c(0.5 * s.stiffness) * crate::math::dot(gap, gap);
        }
        for &i in contact_ids {
            let c = &self.contacts.contacts[i];
            e = e + c.energy(
                c.vertices.map(get),
                self.settings.contact.radius,
                self.settings.contact.stiffness,
            );
        }
        if let Some(grab) = self
            .grab
            .filter(|g| block.vertices.iter().any(|v| g.vertices.contains(v)))
        {
            let mut gap = grab.target.to_array().map(|q| S::c(-q));
            for (&v, w) in grab.vertices.iter().zip(grab.weights) {
                let q = get(v);
                for k in 0..3 {
                    gap[k] = gap[k] + S::c(w) * q[k];
                }
            }
            e = e + S::c(0.5 * grab.point_stiffness()) * crate::math::dot(gap, gap);
        }
        if let Some(grab) = self.grab {
            for &v in &block.vertices {
                if let Some(a) = grab.attachment(v) {
                    let gap =
                        crate::math::sub(get(v), (grab.target + a.offset).to_array().map(S::c));
                    e = e + S::c(0.5 * grab.patch_stiffness() * a.weight)
                        * crate::math::dot(gap, gap);
                }
            }
        }
        let dt = self.settings.dt;
        let damp = 1. + self.settings.damping * dt;
        for &v in &block.vertices {
            let target =
                self.state.x[v] + (self.state.v[v] * dt + self.settings.gravity * dt * dt) / damp;
            let gap = crate::math::sub(get(v), target.to_array().map(S::c));
            e = e + S::c(0.5 * g.mesh.masses[v] * damp / (dt * dt)) * crate::math::dot(gap, gap);
        }
        e
    }

    fn step(&self, block: &Block) -> (Vec<DVec3>, bool, f64) {
        let ids = &block.vertices;
        let old: Vec<_> = ids.iter().map(|&v| self.x[v]).collect();
        let mut contacts: Vec<_> = ids
            .iter()
            .flat_map(|&v| self.contacts.incident[v].iter().copied())
            .collect();
        contacts.sort_unstable();
        contacts.dedup();
        let dt = self.settings.dt;
        let damp = 1. + self.settings.damping * dt;
        let mut membrane_diagonal = vec![DMat3::ZERO; ids.len()];
        let mut membrane_rows = Vec::new();
        for &i in &block.triangles {
            let t = &self.solver.garment.mesh.triangles[i];
            let m = &self.solver.garment.materials[t.material];
            let (diagonal, rows) = membrane_model(t.vertices.map(|v| self.x[v]), t, m);
            for (j, &v) in t.vertices.iter().enumerate() {
                if let Ok(local) = ids.binary_search(&v) {
                    membrane_diagonal[local] += diagonal[j];
                }
            }
            for (k, vectors) in rows {
                let terms: Vec<_> = t
                    .vertices
                    .iter()
                    .zip(vectors)
                    .filter_map(|(&v, a)| ids.binary_search(&v).ok().map(|i| (i, a)))
                    .collect();
                if !terms.is_empty() && k > 0. {
                    membrane_rows.push((k, terms));
                }
            }
        }
        let assembled: Vec<_> = ids
            .par_iter()
            .enumerate()
            .map(|(local, &v)| {
                let jet = self.solver.local(
                    v,
                    std::array::from_fn(|k| Jet::<3>::variable(self.x[v][k], k)),
                    self.x,
                    self.contacts,
                    self.settings,
                    self.grab,
                );
                let inertia = self.solver.garment.mesh.masses[v] * damp / (dt * dt);
                let target = self.state.x[v]
                    + (self.state.v[v] * dt + self.settings.gravity * dt * dt) / damp;
                let g = DVec3::from_array(jet.g) + (self.x[v] - target) * inertia;
                // Keep material/contact local curvature, but assemble the complete
                // sewing matrix separately, including every off-diagonal coupling.
                let sewing_diagonal: f64 = self.solver.vertex_stitches[v]
                    .iter()
                    .map(|&i| {
                        let s = &self.solver.stitches[i];
                        let w = s.terms.iter().find(|&&(j, _)| j == v).unwrap().1;
                        s.stiffness * w * w
                    })
                    .sum();
                let mut h = DMat3::from_cols_array(&std::array::from_fn::<_, 9, _>(|i| {
                    jet.h[i % 3][i / 3]
                })) + DMat3::IDENTITY * (inertia - sewing_diagonal)
                    - membrane_diagonal[local];
                let a = h.to_cols_array_2d();
                let lower = (0..3)
                    .map(|i| {
                        a[i][i]
                            - (0..3)
                                .filter(|&j| j != i)
                                .map(|j| a[i][j].abs())
                                .sum::<f64>()
                    })
                    .fold(f64::INFINITY, f64::min);
                if lower < inertia * 1e-6 {
                    h += DMat3::IDENTITY * (inertia * 1e-6 - lower);
                }
                (h, g)
            })
            .collect();
        if assembled
            .iter()
            .any(|(h, g)| !h.is_finite() || !g.is_finite())
        {
            return (old, false, 0.);
        }
        let local_seams: Vec<_> = block
            .stitches
            .iter()
            .map(|&i| {
                let s = &self.solver.stitches[i];
                (
                    s.stiffness,
                    s.terms
                        .iter()
                        .filter_map(|&(v, w)| ids.binary_search(&v).ok().map(|i| (i, w)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let Some(d) = coupled_direction(&assembled, &local_seams, &membrane_rows) else {
            return (old, false, 0.);
        };
        let slope: f64 = assembled.iter().zip(&d).map(|((_, g), d)| g.dot(*d)).sum();
        if !slope.is_finite() || slope >= 0. {
            return (old, false, 0.);
        }
        let mut alpha = 1.0_f64;
        for (i, &v) in ids.iter().enumerate() {
            alpha = alpha.min(ball_step(
                old[i] - self.contacts.centers[v],
                d[i],
                self.contacts.radii[v],
            ));
        }
        let limited = alpha < 1.;
        let before = self.energy(
            block,
            &contacts,
            &old.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        );
        for _ in 0..12 {
            let p: Vec<_> = old.iter().zip(&d).map(|(p, d)| *p + alpha * (*d)).collect();
            let valid = ids
                .iter()
                .enumerate()
                .all(|(i, &v)| self.contacts.contains(v, p[i], 0.));
            if valid && alpha > 0. {
                let e = self.energy(
                    block,
                    &contacts,
                    &p.iter().map(|q| q.to_array()).collect::<Vec<_>>(),
                );
                if e.is_finite() && e <= before + 1e-4 * alpha * slope && e < before {
                    return (p, limited, before - e);
                }
            }
            alpha *= 0.5;
        }
        (old, limited, 0.)
    }
}

type LocalSeam = (f64, Vec<(usize, f64)>);
type RankRow = (f64, Vec<(usize, DVec3)>);
fn outer(a: DVec3) -> DMat3 {
    DMat3::from_cols(a * a.x, a * a.y, a * a.z)
}

/// Exact membrane diagonal and a PSD Gauss-Newton search approximation.
/// The omitted geometric curvature stays in the actual energy and gradient.
fn membrane_model(
    x: [DVec3; 3],
    t: &crate::model::Triangle,
    m: &crate::model::Material,
) -> ([DMat3; 3], [(f64, [DVec3; 3]); 3]) {
    let coefficients = [t.inv_dm.x_axis, t.inv_dm.y_axis].map(|q| [-q.x - q.y, q.x, q.y]);
    let (s, c) = m.grain_degrees.to_radians().sin_cos();
    let a: [f64; 3] = std::array::from_fn(|i| coefficients[0][i] * c + coefficients[1][i] * s);
    let b: [f64; 3] = std::array::from_fn(|i| -coefficients[0][i] * s + coefficients[1][i] * c);
    let w = (x[1] - x[0]) * a[1] + (x[2] - x[0]) * a[2];
    let v = (x[1] - x[0]) * b[1] + (x[2] - x[0]) * b[2];
    let ew = energy::axial(
        Jet::<1>::variable(0.5 * (w.length_squared() - 1.), 0),
        m.warp,
        m,
    );
    let ev = energy::axial(
        Jet::<1>::variable(0.5 * (v.length_squared() - 1.), 0),
        m.weft,
        m,
    );
    let gw = a.map(|a| w * a);
    let gv = b.map(|b| v * b);
    let gs: [DVec3; 3] = std::array::from_fn(|i| v * a[i] + w * b[i]);
    let diagonal = std::array::from_fn(|i| {
        t.area
            * (outer(gw[i]) * ew.h[0][0]
                + outer(gv[i]) * ev.h[0][0]
                + outer(gs[i]) * m.shear
                + DMat3::IDENTITY
                    * (ew.g[0] * a[i] * a[i]
                        + ev.g[0] * b[i] * b[i]
                        + 2. * m.shear * w.dot(v) * a[i] * b[i]))
    });
    (
        diagonal,
        [
            (t.area * ew.h[0][0].max(0.), gw),
            (t.area * ev.h[0][0].max(0.), gv),
            (t.area * m.shear, gs),
        ],
    )
}
/// Local SPD bending/contact approximation + coupled membrane GN + exact sewing.
/// PCG changes only the search direction; acceptance uses the full nonlinear
/// objective, including geometric membrane curvature and bending/contact couplings omitted here.
fn coupled_direction(
    blocks: &[(DMat3, DVec3)],
    seams: &[LocalSeam],
    rows: &[RankRow],
) -> Option<Vec<DVec3>> {
    let mut diagonal: Vec<_> = blocks.iter().map(|&(h, _)| h).collect();
    for (k, terms) in seams {
        for &(v, w) in terms {
            diagonal[v] += DMat3::IDENTITY * (*k * w * w);
        }
    }
    for (k, terms) in rows {
        for &(v, a) in terms {
            diagonal[v] += outer(a) * (*k);
        }
    }
    let inverse: Vec<_> = diagonal.into_iter().map(|h| h.inverse()).collect();
    if inverse.iter().any(|h| !h.is_finite()) {
        return None;
    }
    let mut r: Vec<_> = blocks.iter().map(|&(_, g)| -g).collect();
    let norm2: f64 = r.iter().map(|v| v.length_squared()).sum();
    let mut x = vec![DVec3::ZERO; r.len()];
    if norm2 == 0. {
        return Some(x);
    }
    let mut z: Vec<_> = r.iter().zip(&inverse).map(|(r, h)| *h * (*r)).collect();
    let mut p = z.clone();
    let mut rz: f64 = r.iter().zip(&z).map(|(a, b)| a.dot(*b)).sum();
    for _ in 0..64 {
        let mut ap: Vec<_> = blocks.iter().zip(&p).map(|((h, _), p)| *h * (*p)).collect();
        for (k, terms) in seams {
            let gap: DVec3 = terms.iter().map(|&(v, w)| p[v] * w).sum();
            for &(v, w) in terms {
                ap[v] += gap * (*k * w);
            }
        }
        for (k, terms) in rows {
            let value: f64 = terms.iter().map(|&(v, a)| a.dot(p[v])).sum();
            for &(v, a) in terms {
                ap[v] += a * (*k * value);
            }
        }
        let pap: f64 = p.iter().zip(&ap).map(|(a, b)| a.dot(*b)).sum();
        if !pap.is_finite() || pap <= 0. || !rz.is_finite() || rz <= 0. {
            break;
        }
        let alpha = rz / pap;
        for i in 0..x.len() {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        if r.iter().map(|v| v.length_squared()).sum::<f64>() <= 1e-12 * norm2 {
            break;
        }
        for i in 0..x.len() {
            z[i] = inverse[i] * r[i];
        }
        let next: f64 = r.iter().zip(&z).map(|(a, b)| a.dot(*b)).sum();
        let beta = next / rz;
        for i in 0..x.len() {
            p[i] = z[i] + beta * p[i];
        }
        rz = next;
    }
    x.iter().all(|d| d.is_finite()).then_some(x)
}

/// Largest nonnegative step along one direction staying in the ORIGINAL ball.
/// A shared step size keeps the coupled direction intact. No projection follows.
fn ball_step(offset: DVec3, d: DVec3, radius: f64) -> f64 {
    let aa = d.length_squared();
    if aa == 0. {
        return 1.;
    }
    let remaining = (radius * radius - offset.length_squared()).max(0.);
    let b = offset.dot(d);
    let root = (b * b + aa * remaining).sqrt();
    let alpha = if b > 0. {
        remaining / (root + b)
    } else {
        (root - b) / aa
    };
    if alpha < 1. {
        (alpha * (1. - 32. * f64::EPSILON)).max(0.)
    } else {
        1.
    }
}

impl VbdSolver {
    /// Research opt-in, not serialized in garment documents. Zero preserves the
    /// normal path; this method changes iteration work, never material settings.
    pub fn set_seam_block_sweeps(&mut self, sweeps: usize) -> Result<(), String> {
        if sweeps > 16 {
            return Err("At most 16 experimental seam-block sweeps".into());
        }
        if sweeps > 0 && self.seam_components.is_empty() {
            self.seam_components = blocks(self);
        }
        self.seam_block_sweeps = sweeps;
        Ok(())
    }

    pub(crate) fn refine_seams(
        &self,
        x: &mut [DVec3],
        state: &State,
        contacts: &ContactSet,
        settings: &VbdSettings,
        grab: Option<&Grab>,
    ) -> SeamBlockMetrics {
        if self.seam_block_sweeps == 0 || self.seam_components.is_empty() {
            return SeamBlockMetrics::default();
        }
        let start = Instant::now();
        let mut level = vec![usize::MAX; self.graph.len()];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for (index, block) in self.seam_components.iter().enumerate() {
            let mut current = 0;
            for &v in &block.vertices {
                super::coloring::neighbors(v, &self.graph, contacts, grab, |i| {
                    if i < level.len() && level[i] != usize::MAX {
                        current = current.max(level[i] + 1);
                    }
                });
            }
            groups.resize_with(groups.len().max(current + 1), Vec::new);
            groups[current].push(index);
            for &v in &block.vertices {
                level[v] = current;
            }
        }
        let mut metrics = SeamBlockMetrics {
            blocks: self.seam_components.len(),
            ..Default::default()
        };
        for _ in 0..self.seam_block_sweeps {
            for group in &groups {
                let problem = Problem {
                    solver: self,
                    x,
                    contacts,
                    settings,
                    grab,
                    state,
                };
                let updates: Vec<_> = group
                    .par_iter()
                    .map(|&i| (i, problem.step(&self.seam_components[i])))
                    .collect();
                for (i, (positions, limited, improvement)) in updates {
                    metrics.attempts += 1;
                    metrics.bound_limited += usize::from(limited);
                    metrics.accepted += usize::from(improvement > 0.);
                    metrics.objective_decrease_j += improvement;
                    for (v, p) in self.seam_components[i]
                        .vertices
                        .iter()
                        .copied()
                        .zip(positions)
                    {
                        x[v] = p;
                    }
                }
            }
        }
        metrics.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anisotropic_rank_rows_solve_a_known_coupled_system() {
        let exact = [DVec3::new(0.2, -0.1, 0.4), DVec3::ONE];
        let rows = vec![(
            13.,
            vec![(0, DVec3::new(1., 2., -1.)), (1, DVec3::new(-2., 0.5, 1.))],
        )];
        let mut blocks = vec![(DMat3::IDENTITY, DVec3::ZERO); 2];
        for (i, b) in blocks.iter_mut().enumerate() {
            b.1 = -exact[i];
        }
        for (k, terms) in &rows {
            let value: f64 = terms.iter().map(|&(i, a)| a.dot(exact[i])).sum();
            for &(i, a) in terms {
                blocks[i].1 -= a * (*k * value);
            }
        }
        let d = coupled_direction(&blocks, &[], &rows).unwrap();
        assert!(d.iter().zip(exact).all(|(a, b)| a.distance(b) < 1e-8));
    }

    #[test]
    fn membrane_search_model_matches_exact_diagonal_and_rigid_rotation() {
        let g = crate::garment::GarmentTemplate::two_panel_demo()
            .compile()
            .unwrap();
        let t = &g.mesh.triangles[0];
        let mut m = g.materials[t.material].clone();
        m.grain_degrees = 27.;
        let rotation = DMat3::from_rotation_x(0.6) * DMat3::from_rotation_z(-0.4);
        for scale in [0.8, 1., 1.2] {
            let x = t.vertices.map(|i| {
                let p = g.initial_state.x[i];
                DVec3::new(p.x * scale + p.y * 0.1, p.y, p.z + p.x * 0.2)
            });
            let (diag, rows) = membrane_model(x, t, &m);
            let jet = energy::membrane(
                std::array::from_fn(|i| {
                    std::array::from_fn(|k| Jet::<9>::variable(x[i][k], 3 * i + k))
                }),
                t,
                &m,
            );
            let (rotated, rotated_rows) =
                membrane_model(x.map(|p| rotation * p + DVec3::ONE), t, &m);
            for i in 0..3 {
                for a in 0..3 {
                    for b in 0..3 {
                        let expected = jet.h[3 * i + a][3 * i + b];
                        assert!(
                            (diag[i].col(b)[a] - expected).abs() < 1e-10 * (1. + expected.abs())
                        );
                    }
                }
                let expected = rotation * diag[i] * rotation.transpose();
                assert!(
                    (rotated[i] - expected)
                        .to_cols_array()
                        .iter()
                        .all(|x| x.abs() < 1e-8)
                );
            }
            for ((k, a), (kr, ar)) in rows.into_iter().zip(rotated_rows) {
                assert!((k - kr).abs() < 1e-9 * (1. + k.abs()));
                for (v, vr) in a.into_iter().zip(ar) {
                    assert!((rotation * v - vr).length() < 1e-10);
                }
            }
        }
    }

    #[test]
    fn nonmatching_couplings_solve_the_known_spd_system() {
        // One-to-many interpolation and two linked stitches; known displacement
        // contains both common and differential motion in all three dimensions.
        let exact = [DVec3::new(0.2, -0.1, 0.4), DVec3::ONE, -DVec3::Y];
        let seams = vec![
            (2e4, vec![(0, 1.), (1, -0.25), (2, -0.75)]),
            (7e3, vec![(1, 1.), (2, -1.)]),
        ];
        let mut blocks = vec![(DMat3::from_diagonal(DVec3::new(2., 3., 4.)), DVec3::ZERO); 3];
        for (i, (h, g)) in blocks.iter_mut().enumerate() {
            *g = -(*h * exact[i]);
        }
        for (k, terms) in &seams {
            let gap: DVec3 = terms.iter().map(|&(v, w)| exact[v] * w).sum();
            for &(v, w) in terms {
                blocks[v].1 -= gap * (*k * w);
            }
        }
        let d = coupled_direction(&blocks, &seams, &[]).unwrap();
        assert!(d.iter().zip(exact).all(|(a, b)| a.distance(b) < 1e-7));
    }

    #[test]
    fn topology_covers_every_free_stitch_term_and_preserves_pins() {
        let mut t = crate::garment::GarmentTemplate::two_panel_demo();
        for (i, p) in t.panels.iter_mut().enumerate() {
            p.pattern.nx = 2;
            p.pattern.ny = 2 + i;
        }
        let g = Arc::new(t.compile().unwrap());
        let mut s = g.initial_state.clone();
        let mut solver = VbdSolver::new(g.clone(), None, &s).unwrap();
        assert!(solver.seam_components.is_empty()); // no default setup overhead
        solver.set_seam_block_sweeps(1).unwrap();
        let mut owner = vec![None; s.x.len()];
        for (i, b) in solver.seam_components.iter().enumerate() {
            for &v in &b.vertices {
                assert!(g.mesh.pins[v].is_none());
                assert!(owner[v].replace(i).is_none());
            }
        }
        for stitch in &solver.stitches {
            let free: Vec<_> = stitch
                .terms
                .iter()
                .filter(|&&(v, _)| g.mesh.pins[v].is_none())
                .collect();
            if free.len() > 1 {
                let block = owner[free[0].0].unwrap();
                assert!(free.iter().all(|&&(v, _)| owner[v] == Some(block)));
            }
        }
        let settings = VbdSettings {
            iterations: 2,
            ..Default::default()
        };
        for _ in 0..3 {
            solver.step(&mut s, &settings, None).unwrap();
            for (i, pin) in g.mesh.pins.iter().enumerate() {
                if let Some(p) = pin {
                    assert_eq!(s.x[i], *p);
                    assert_eq!(s.v[i], DVec3::ZERO);
                }
            }
        }
    }

    #[test]
    fn disabled_refinement_and_seamless_pull_release_are_exact_noops() {
        for seamless in [false, true] {
            let mut t = crate::garment::GarmentTemplate::two_panel_demo();
            for p in &mut t.panels {
                p.pattern.nx = 2;
                p.pattern.ny = 2;
            }
            if seamless {
                t.seams.clear();
                t.panels.truncate(1);
                t.panels[0].pins.clear();
            }
            let g = Arc::new(t.compile().unwrap());
            let mut a = g.initial_state.clone();
            let mut b = a.clone();
            let mut baseline = VbdSolver::new(g.clone(), None, &a).unwrap();
            let mut opt = VbdSolver::new(g.clone(), None, &a).unwrap();
            opt.set_seam_block_sweeps(1).unwrap();
            if !seamless {
                opt.set_seam_block_sweeps(0).unwrap();
            }
            let vertices = g.mesh.triangles[0].vertices;
            let grab = Grab {
                patch: Vec::new(),
                vertices,
                weights: [1. / 3.; 3],
                target: vertices.iter().map(|&v| a.x[v] / 3.).sum::<DVec3>() + DVec3::Z * 0.01,
                stiffness: 100.,
            };
            let settings = VbdSettings {
                iterations: 2,
                ..Default::default()
            };
            for step in 0..12 {
                let control = (step < 6).then_some(&grab);
                baseline.step(&mut a, &settings, control).unwrap();
                let m = opt.step(&mut b, &settings, control).unwrap();
                assert_eq!(m.seam_blocks.attempts, 0);
                assert_eq!(a.x, b.x);
                assert_eq!(a.v, b.v);
                assert_eq!(a.time, b.time);
            }
        }
    }

    #[test]
    fn coupled_direction_resolves_stiff_common_motion_and_ball_limits() {
        let d = coupled_direction(
            &[(DMat3::IDENTITY, -DVec3::ONE); 2],
            &[(1e8, vec![(0, 1.), (1, -1.)])],
            &[],
        )
        .unwrap();
        assert!(d.iter().all(|q| (*q - DVec3::ONE).length() < 1e-7));
        for o in [DVec3::ZERO, DVec3::X * 0.5, DVec3::X] {
            for d in [
                DVec3::X,
                -DVec3::X,
                DVec3::Y,
                DVec3::ZERO,
                DVec3::new(-1., 2., 3.),
            ] {
                let a = ball_step(o, d, 1.);
                assert!((0.0..=1.).contains(&a));
                assert!((o + a * d).length() <= 1. + 1e-15);
            }
        }
    }

    #[test]
    fn block_derivatives_and_acceptance_match_complete_objective() {
        let mut t = crate::garment::GarmentTemplate::two_panel_demo();
        for (i, p) in t.panels.iter_mut().enumerate() {
            p.pattern.nx = 2;
            p.pattern.ny = 2;
            p.pattern.width = 0.15;
            p.pattern.height = 0.3;
            p.placement.rotation_degrees = DVec3::new(90., 0., 0.);
            p.placement.translation = DVec3::new(if i == 0 { -0.16 } else { 0.01 }, 0.0029, 0.3);
            p.pins.clear();
        }
        let g = Arc::new(t.compile().unwrap());
        let state = g.initial_state.clone();
        let floor = SurfaceMesh {
            vertices: vec![
                DVec3::new(-2., 0., -2.),
                DVec3::new(2., 0., -2.),
                DVec3::new(0., 0., 2.),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let mut solver = VbdSolver::new(g, Some(&floor), &state).unwrap();
        let settings = VbdSettings::default();
        let mut x = solver.world.positions(&state.x);
        let contacts = solver.world.detect(&x, 0.012, &settings.contact).unwrap();
        assert!(contacts.active_contacts > 0);
        // Include adjacent vertices sharing triangles/hinges, not just disjoint
        // panels: their common elements must not be counted twice.
        let a = 0;
        let b = 1;
        let block = Block {
            vertices: vec![a, b],
            triangles: union(&solver.triangles[a], &solver.triangles[b]),
            hinges: union(&solver.hinges[a], &solver.hinges[b]),
            stitches: union(&solver.vertex_stitches[a], &solver.vertex_stitches[b]),
        };
        let grab = Grab {
            patch: Vec::new(),
            vertices: [a, b, 2],
            weights: [0.2, 0.3, 0.5],
            target: state.x[0] + DVec3::Z * 0.01,
            stiffness: 100.,
        };
        let problem = Problem {
            solver: &solver,
            x: &x,
            contacts: &contacts,
            settings: &settings,
            grab: Some(&grab),
            state: &state,
        };
        let cs = union(&contacts.incident[a], &contacts.incident[b]);
        let points = [a, b].map(|v| x[v].to_array());
        let jet = problem.energy(
            &block,
            &cs,
            &std::array::from_fn::<_, 2, _>(|i| {
                std::array::from_fn(|k| Jet::<6>::variable(points[i][k], 3 * i + k))
            }),
        );
        let global = |p: [[f64; 3]; 2]| {
            let mut s = state.clone();
            for (k, position) in p.iter().enumerate() {
                s.x[block.vertices[k]] = DVec3::from_array(*position);
            }
            let e = solver
                .audit_energy(&s, &settings, &contacts, Some(&grab))
                .unwrap();
            let dt = settings.dt;
            let damp = 1. + settings.damping * dt;
            e.potential_j() - e.gravity_j
                + s.x
                    .iter()
                    .enumerate()
                    .map(|(v, &q)| {
                        let target =
                            state.x[v] + (state.v[v] * dt + settings.gravity * dt * dt) / damp;
                        0.5 * solver.garment.mesh.masses[v] * damp / (dt * dt)
                            * q.distance_squared(target)
                    })
                    .sum::<f64>()
        };
        for i in 0..6 {
            let mut plus = points;
            let mut minus = points;
            plus[i / 3][i % 3] += 1e-6;
            minus[i / 3][i % 3] -= 1e-6;
            let numerical = (global(plus) - global(minus)) / 2e-6;
            assert!((numerical - jet.g[i]).abs() < 1e-5 * (1. + numerical.abs()));
            let gradient = |p: [[f64; 3]; 2]| {
                problem
                    .energy(
                        &block,
                        &cs,
                        &std::array::from_fn::<_, 2, _>(|j| {
                            std::array::from_fn(|k| Jet::<6>::variable(p[j][k], 3 * j + k))
                        }),
                    )
                    .g
            };
            let gp = gradient(plus);
            let gm = gradient(minus);
            for j in 0..6 {
                let expected = (gp[j] - gm[j]) / 2e-6;
                assert!((expected - jet.h[j][i]).abs() < 1e-4 * (1. + expected.abs()));
            }
        }
        let before = global(points);
        let (next, _, decrease) = problem.step(&block);
        assert!(decrease > 0.);
        assert!(
            (before - global([next[0].to_array(), next[1].to_array()]) - decrease).abs() < 1e-9
        );
        for (v, p) in block.vertices.into_iter().zip(next) {
            assert!(p.distance(contacts.centers[v]) <= contacts.radii[v]);
            x[v] = p;
        }
    }
}
