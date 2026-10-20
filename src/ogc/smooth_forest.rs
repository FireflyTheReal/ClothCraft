//! CPU research kernel: average barrier-preserving rooted forests.
//! Not an installed force law. See docs/smooth-forest-contact-proposal.md.
//! All matrix operations are differentiated; probabilities are never frozen.
use super::{patch_contact, potential::activation, primitive_distance};
use crate::math::Scalar;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct Link<S> {
    pub nodes: [usize; 2],
    pub barrier: S,
}
pub struct Evaluation<S> {
    pub energy: S,
    pub inclusion_probabilities: Vec<S>,
}

struct Component {
    nodes: Vec<usize>,
    links: Vec<usize>,
}

// Structural components, INCLUDING zero-weight links. Dropping a zero
// weight before AD can erase its derivatives. Only absent links separate
// independent factors of the rooted-forest partition function.
fn components<S>(n: usize, links: &[Link<S>]) -> Vec<Component> {
    fn root(parent: &[usize], mut i: usize) -> usize {
        while parent[i] != i {
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<_> = (0..n).collect();
    for link in links {
        let a = root(&parent, link.nodes[0]);
        let b = root(&parent, link.nodes[1]);
        parent[a.max(b)] = a.min(b);
    }
    let mut groups = BTreeMap::new();
    for i in 0..n {
        groups
            .entry(root(&parent, i))
            .or_insert_with(|| Component {
                nodes: Vec::new(),
                links: Vec::new(),
            })
            .nodes
            .push(i);
    }
    for (i, link) in links.iter().enumerate() {
        groups
            .get_mut(&root(&parent, link.nodes[0]))
            .unwrap()
            .links
            .push(i);
    }
    groups.into_values().collect()
}

// Orthonormal Helmert basis for the subspace perpendicular to the constant
// vector. The constant eigenvalue of I+L is exactly one and does not contribute
// to any incidence quadratic form. Forming the reduced matrix from incidence
// outer products avoids subtracting large Laplacian entries to recover it.
fn basis(row: usize, col: usize) -> f64 {
    let scale = (((col + 1) * (col + 2)) as f64).sqrt();
    if row <= col {
        1. / scale
    } else if row == col + 1 {
        -(col as f64 + 1.) / scale
    } else {
        0.
    }
}

/// Node and link barriers are in joules; tau is a positive energy scale.
/// Domain: 0 <= link barrier <= either endpoint's node barrier.
/// Independent structural components are factored separately. Each dense
/// component is limited to 128 nodes; no links, including zeros, are dropped.
/// Reports factorization failures without an
/// SPD shift, probability clamp, or change to the requested energy scale.
pub fn evaluate<S: Scalar>(
    nodes: &[S],
    links: &[Link<S>],
    tau: f64,
) -> Result<Evaluation<S>, String> {
    validate(nodes, links, tau)?;
    assemble(nodes, links, tau, |_, energy| energy)
}

/// Dense unfactored research control for equivalence and timing tests.
/// Same law and derivatives, limited to 128 total nodes.
pub fn evaluate_monolithic<S: Scalar>(
    nodes: &[S],
    links: &[Link<S>],
    tau: f64,
) -> Result<Evaluation<S>, String> {
    validate(nodes, links, tau)?;
    if nodes.len() > 128 {
        return Err("Monolithic smooth forest exceeds 128 nodes".into());
    }
    dense(nodes, links, tau)
}

fn validate<S: Scalar>(nodes: &[S], links: &[Link<S>], tau: f64) -> Result<(), String> {
    if !tau.is_finite() || tau <= 0. {
        return Err("Smooth forest requires positive finite tau".into());
    }
    if nodes
        .iter()
        .any(|a| !a.value().is_finite() || a.value() < 0.)
    {
        return Err("Smooth forest requires finite nonnegative primitive barriers".into());
    }
    for link in links {
        let [i, j] = link.nodes;
        if i >= nodes.len() || j >= nodes.len() || i == j {
            return Err("Invalid forest link endpoints".into());
        }
        let c = link.barrier.value();
        let bound = nodes[i].value().min(nodes[j].value());
        if !c.is_finite() || c < 0. || c > bound + 64. * f64::EPSILON * bound.max(1.) {
            return Err("Forest link barrier exceeds a primitive barrier".into());
        }
    }
    Ok(())
}

fn assemble<S: Scalar>(
    nodes: &[S],
    links: &[Link<S>],
    tau: f64,
    reduced_energy: impl Fn(&Component, S) -> S,
) -> Result<Evaluation<S>, String> {
    let mut result = Evaluation {
        energy: S::c(0.),
        inclusion_probabilities: vec![S::c(0.); links.len()],
    };
    let mut local_index = vec![0; nodes.len()];
    for component in components(nodes.len(), links) {
        if component.nodes.len() > 128 {
            return Err("Smooth forest dense component exceeds 128 nodes".into());
        }
        for (local, &global) in component.nodes.iter().enumerate() {
            local_index[global] = local;
        }
        let local_nodes: Vec<_> = component.nodes.iter().map(|i| nodes[*i]).collect();
        let local_links: Vec<_> = component
            .links
            .iter()
            .map(|i| Link {
                nodes: links[*i].nodes.map(|j| local_index[j]),
                barrier: links[*i].barrier,
            })
            .collect();
        let evaluation = dense(&local_nodes, &local_links, tau)?;
        result.energy = result.energy + reduced_energy(&component, evaluation.energy);
        for (&global, p) in component
            .links
            .iter()
            .zip(evaluation.inclusion_probabilities)
        {
            result.inclusion_probabilities[global] = p;
        }
    }
    Ok(result)
}

fn dense<S: Scalar>(nodes: &[S], links: &[Link<S>], tau: f64) -> Result<Evaluation<S>, String> {
    let sum = nodes.iter().copied().fold(S::c(0.), |a, b| a + b);
    if links.is_empty() {
        return Ok(Evaluation {
            energy: sum,
            inclusion_probabilities: Vec::new(),
        });
    }
    let n = nodes.len() - 1;
    let incidence: Vec<Vec<f64>> = links
        .iter()
        .map(|l| {
            (0..n)
                .map(|k| basis(l.nodes[0], k) - basis(l.nodes[1], k))
                .collect()
        })
        .collect();
    let mut matrix = vec![vec![S::c(0.); n]; n];
    for (i, row) in matrix.iter_mut().enumerate() {
        row[i] = S::c(1.);
    }
    for (link, d) in links.iter().zip(&incidence) {
        let weight = link.barrier / S::c(tau);
        for i in 0..n {
            for j in 0..=i {
                matrix[i][j] = matrix[i][j] + weight * S::c(d[i] * d[j]);
            }
        }
    }
    // Cholesky in place; only the lower triangle is used.
    for i in 0..n {
        for j in 0..=i {
            let mut a = matrix[i][j];
            for (left, right) in matrix[i].iter().zip(&matrix[j]).take(j) {
                a = a - *left * *right;
            }
            matrix[i][j] = if i == j {
                if !a.value().is_finite() || a.value() <= 0. {
                    return Err("Smooth forest lost positive definiteness numerically".into());
                }
                a.sqrt()
            } else {
                a / matrix[j][j]
            };
        }
    }
    let mut probabilities = Vec::with_capacity(links.len());
    let mut correction = S::c(0.);
    for (link, d) in links.iter().zip(&incidence) {
        let mut y = vec![S::c(0.); n];
        for i in 0..n {
            let mut rhs = S::c(d[i]);
            for (k, value) in y.iter().enumerate().take(i) {
                rhs = rhs - matrix[i][k] * *value;
            }
            y[i] = rhs / matrix[i][i];
        }
        let resistance = y.iter().fold(S::c(0.), |a, y| a + *y * *y);
        let p = link.barrier / S::c(tau) * resistance;
        if !p.value().is_finite() {
            return Err("Nonfinite smooth forest probability".into());
        }
        correction = correction + link.barrier * p;
        probabilities.push(p);
    }
    Ok(Evaluation {
        energy: sum - correction,
        inclusion_probabilities: probabilities,
    })
}

/// Fixed source edge and caller-supplied target edge graph. The caller owns
/// seam exclusions and a complete force halo. All common-vertex clique links,
/// including zero-weight links, are included for consistent differentiation.
/// Finite distance singularities at parallel configurations remain unresolved.
pub fn wire<S: Scalar>(
    positions: &[[S; 3]],
    source: [usize; 2],
    targets: &[[usize; 2]],
    radius: f64,
    stiffness: f64,
    tau: f64,
) -> Result<Evaluation<S>, String> {
    if source[0] == source[1]
        || source.iter().any(|i| *i >= positions.len())
        || !radius.is_finite()
        || radius <= 0.
        || !stiffness.is_finite()
        || stiffness <= 0.
    {
        return Err("Invalid smooth wire source or contact parameters".into());
    }
    let [a, b] = source.map(|i| positions[i]);
    let mut nodes = Vec::with_capacity(targets.len());
    let mut incident: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut unique = BTreeSet::new();
    for (i, &[c, d]) in targets.iter().enumerate() {
        if c == d
            || c >= positions.len()
            || d >= positions.len()
            || source.contains(&c)
            || source.contains(&d)
            || !unique.insert([c.min(d), c.max(d)])
        {
            return Err("Invalid, adjacent or duplicate target edge".into());
        }
        let distance = primitive_distance::segment_segment_symmetric_boundary([
            a,
            b,
            positions[c],
            positions[d],
        ]);
        nodes.push(activation(distance, radius, stiffness));
        incident.entry(c).or_default().push(i);
        incident.entry(d).or_default().push(i);
    }
    let mut links = Vec::new();
    for (vertex, edges) in incident {
        let c = activation(
            primitive_distance::point_segment(positions[vertex], a, b),
            radius,
            stiffness,
        );
        for i in 0..edges.len() {
            for j in i + 1..edges.len() {
                links.push(Link {
                    nodes: [edges[i], edges[j]],
                    barrier: c,
                });
            }
        }
    }
    validate(&nodes, &links, tau)?;
    assemble(&nodes, &links, tau, |component, energy| {
        if component.nodes.len() != 2 || component.links.len() != 1 {
            return energy;
        }
        // Exact two-node reduction: A+B-2c²/(tau+2c) =
        // (A+B-c) + c*tau/(tau+2c). Cancel the common feature BEFORE
        // differentiation. Independently differentiated endpoint branches
        // otherwise choose incompatible Hessians at a flat shared vertex.
        let pair = [targets[component.nodes[0]], targets[component.nodes[1]]];
        let common = pair[0]
            .iter()
            .copied()
            .find(|i| pair[1].contains(i))
            .expect("A graph link requires a common target vertex");
        let left = pair[0].iter().copied().find(|i| *i != common).unwrap();
        let right = pair[1].iter().copied().find(|i| *i != common).unwrap();
        let c = links[component.links[0]].barrier;
        patch_contact::wire_pair(
            [a, b, positions[left], positions[common], positions[right]],
            radius,
            stiffness,
        ) + c * S::c(tau) / (S::c(tau) + S::c(2.) * c)
    })
}
