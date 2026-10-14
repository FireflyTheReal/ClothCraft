use std::collections::{BTreeMap, BTreeSet};

/// Stable feature IDs; oriented faces and undirected edges. No geometry caches.
#[derive(Clone, Debug)]
pub struct Topology {
    pub triangles: Vec<[usize; 3]>,
    pub edges: Vec<[usize; 2]>,
    pub triangle_edges: Vec<[usize; 3]>,
    pub neighbors: Vec<Vec<usize>>,
    pub opposites: Vec<Vec<usize>>,
}
impl Topology {
    pub fn new(n: usize, triangles: Vec<[usize; 3]>) -> Result<Self, String> {
        let mut map = BTreeMap::new();
        let mut edges = Vec::new();
        let mut opposites: Vec<Vec<usize>> = Vec::new();
        let mut neighbors = vec![BTreeSet::new(); n];
        let mut triangle_edges = Vec::new();
        for t in &triangles {
            if t.iter().any(|i| *i >= n) || t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                return Err("OGC: invalid triangle indices".into());
            }
            let mut te = [0; 3];
            for k in 0..3 {
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                let key = [a.min(b), a.max(b)];
                let e = *map.entry(key).or_insert_with(|| {
                    edges.push(key);
                    opposites.push(Vec::new());
                    edges.len() - 1
                });
                opposites[e].push(c);
                neighbors[a].insert(b);
                neighbors[b].insert(a);
                te[k] = e;
            }
            triangle_edges.push(te);
        }
        if opposites.iter().any(|v| v.len() > 2) {
            return Err("OGC requires manifold triangle edges (at most two incident faces)".into());
        }
        Ok(Self {
            triangles,
            edges,
            triangle_edges,
            opposites,
            neighbors: neighbors
                .into_iter()
                .map(|s| s.into_iter().collect())
                .collect(),
        })
    }
}
