//! Deterministic greedy coloring. Reuse contact incidence instead of expanding
//! every contact into another heap-allocated per-vertex adjacency graph.
use super::{ContactSet, Grab};

pub(super) fn colors(
    graph: &[Vec<usize>],
    contact: &ContactSet,
    grab: Option<&Grab>,
) -> Vec<Vec<usize>> {
    let n = graph.len();
    let mut assigned = vec![usize::MAX; n];
    let mut colors: Vec<Vec<usize>> = Vec::new();
    let mut seen = vec![usize::MAX; n];
    for v in 0..n {
        let mut mark = |i: usize| {
            // Fixed avatar vertices never run a local update.
            if i < n && assigned[i] != usize::MAX {
                seen[assigned[i]] = v;
            }
        };
        for i in &graph[v] {
            mark(*i);
        }
        for index in &contact.incident[v] {
            let c = &contact.contacts[*index];
            for i in &c.vertices[..c.count] {
                mark(*i);
            }
        }
        if let Some(g) = grab.filter(|g| g.vertices.contains(&v)) {
            for i in g.vertices {
                mark(i);
            }
        }
        let c = (0..).find(|c| seen[*c] != v).unwrap();
        if c == colors.len() {
            colors.push(Vec::new());
        }
        colors[c].push(v);
        assigned[v] = c;
    }
    colors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ogc::{OgcSettings, OgcWorld};
    use glam::DVec3;

    #[test]
    fn incidence_coloring_matches_expanded_graph_and_has_no_conflicts() {
        let x: Vec<_> = (0..4)
            .flat_map(|i| {
                let z = 0.002 * i as f64;
                [
                    DVec3::new(0., 0., z),
                    DVec3::new(1., 0., z),
                    DVec3::new(0., 1., z),
                ]
            })
            .collect();
        let mut world = OgcWorld::new(
            &x,
            vec![[0, 1, 2], [3, 4, 5], [6, 7, 8], [9, 10, 11]],
            9,
            [],
        )
        .unwrap();
        let settings = OgcSettings::default();
        let contact = world.detect(&x, 0.012, &settings).unwrap();
        assert!(!contact.contacts.is_empty());
        let graph = vec![
            vec![1],
            vec![0, 2],
            vec![1],
            vec![],
            vec![6],
            vec![],
            vec![4],
            vec![],
            vec![],
        ];
        let grab = Grab {
            vertices: [0, 4, 8],
            weights: [0.2, 0.3, 0.5],
            target: DVec3::ZERO,
            stiffness: 100.,
        };
        for g in [None, Some(&grab)] {
            let result = colors(&graph, &contact, g);
            // Independent reference: explicitly build all cliques, then color.
            let mut full = graph.clone();
            let mut connect = |vertices: &[usize]| {
                for &a in vertices {
                    for &b in vertices {
                        if a != b && a < 9 && b < 9 {
                            full[a].push(b);
                        }
                    }
                }
            };
            for c in &contact.contacts {
                connect(&c.vertices[..c.count]);
            }
            if let Some(g) = g {
                connect(&g.vertices);
            }
            let mut assigned = [usize::MAX; 9];
            let mut expected: Vec<Vec<usize>> = Vec::new();
            for v in 0..9 {
                let c = (0..)
                    .find(|c| full[v].iter().all(|i| assigned[*i] != *c))
                    .unwrap();
                if c == expected.len() {
                    expected.push(Vec::new());
                }
                expected[c].push(v);
                assigned[v] = c;
            }
            assert_eq!(result, expected, "Same colors AND vertex order");
            for v in 0..9 {
                for &other in &full[v] {
                    assert_ne!(assigned[v], assigned[other]);
                }
            }
        }
        let empty = ContactSet {
            contacts: vec![],
            incident: vec![vec![]; 9],
            ..contact
        };
        assert_eq!(
            colors(&vec![vec![]; 9], &empty, None),
            vec![(0..9).collect::<Vec<_>>()]
        );
    }
}
