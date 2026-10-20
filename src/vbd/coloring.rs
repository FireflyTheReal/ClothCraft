//! Deterministic greedy coloring. Reuse contact incidence instead of expanding
//! every contact into another heap-allocated per-vertex adjacency graph.
use super::{ContactSet, Grab};

pub(super) fn colors(
    graph: &[Vec<usize>],
    contact: &ContactSet,
    grab: Option<&Grab>,
) -> Vec<Vec<usize>> {
    // Most cloth graphs need far fewer than 64 colors. A register mask avoids
    // repeatedly marking a heap array; retain exactly the same greedy order.
    let mut assigned = vec![0; graph.len()];
    let mut result: Vec<Vec<usize>> = Vec::new();
    for v in 0..graph.len() {
        let mut blocked = 0u64;
        neighbors(v, graph, contact, grab, |i| {
            if i < v {
                blocked |= 1 << assigned[i];
            }
        });
        let color = blocked.trailing_ones() as usize;
        if color == 64 {
            return colors_unrestricted(graph, contact, grab);
        }
        if color == result.len() {
            result.push(Vec::new());
        }
        result[color].push(v);
        assigned[v] = color;
    }
    result
}

#[inline]
pub(super) fn neighbors(
    v: usize,
    graph: &[Vec<usize>],
    contact: &ContactSet,
    grab: Option<&Grab>,
    mut visit: impl FnMut(usize),
) {
    for &i in &graph[v] {
        visit(i);
    }
    for &index in &contact.incident[v] {
        let c = &contact.contacts[index];
        for &i in &c.vertices[..c.count] {
            visit(i);
        }
    }
    if let Some(g) = grab.filter(|g| g.vertices.contains(&v)) {
        for i in g.vertices {
            visit(i);
        }
    }
}

fn colors_unrestricted(
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

/// Parallel levels equivalent to visiting `vertices` sequentially. Every shared
/// energy/contact/grab orders the later vertex after the earlier one, so only
/// independent minimizations can share a level. This is not Jacobi iteration.
#[cfg(feature = "desktop")]
pub(super) fn ordered_groups(
    graph: &[Vec<usize>],
    contact: &ContactSet,
    grab: Option<&Grab>,
    vertices: &[usize],
) -> Vec<Vec<usize>> {
    let mut level = vec![usize::MAX; graph.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &v in vertices {
        let mut current = 0;
        neighbors(v, graph, contact, grab, |i| {
            if i < level.len() && level[i] != usize::MAX {
                current = current.max(level[i] + 1);
            }
        });
        groups.resize_with(groups.len().max(current + 1), Vec::new);
        groups[current].push(v);
        level[v] = current;
    }
    groups
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
            patch: Vec::new(),
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
            #[cfg(feature = "desktop")]
            for vertices in [vec![8, 0, 5, 3, 2, 4], (0..9).collect()] {
                let groups = ordered_groups(&graph, &contact, g, &vertices);
                let mut levels = [usize::MAX; 9];
                for (level, group) in groups.iter().enumerate() {
                    for &v in group {
                        assert_eq!(levels[v], usize::MAX, "Scheduled once");
                        levels[v] = level;
                    }
                }
                for (rank, &v) in vertices.iter().enumerate() {
                    for &earlier in &vertices[..rank] {
                        if full[v].contains(&earlier) {
                            assert!(
                                levels[earlier] < levels[v],
                                "Preserve every sequential dependency"
                            );
                        }
                    }
                }
            }
            for v in 0..9 {
                for &other in &full[v] {
                    assert_ne!(assigned[v], assigned[other]);
                }
            }
        }
        let empty = ContactSet {
            contacts: vec![],
            incident: crate::ogc::ContactIncidence::new(9, &[]),
            ..contact
        };
        assert_eq!(
            colors(&vec![vec![]; 9], &empty, None),
            vec![(0..9).collect::<Vec<_>>()]
        );
        // A dense graph must fall back, never shift by >=64 or merge conflicts.
        let dense: Vec<Vec<_>> = (0..70)
            .map(|v| (0..70).filter(|i| *i != v).collect())
            .collect();
        let empty = ContactSet {
            incident: crate::ogc::ContactIncidence::new(70, &[]),
            ..empty
        };
        assert_eq!(
            colors(&dense, &empty, None),
            colors_unrestricted(&dense, &empty, None)
        );
    }
}
