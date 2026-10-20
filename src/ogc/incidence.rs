//! Compact contact incidence in canonical contact order. Two contiguous arrays
//! replace one allocation per moving vertex; fixed vertices have no row.
use super::potential::Contact;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactIncidence {
    offsets: Vec<usize>,
    indices: Vec<usize>,
}
impl ContactIncidence {
    pub fn new(moving: usize, contacts: &[Contact]) -> Self {
        let mut offsets = vec![0; moving + 1];
        for c in contacts {
            for &v in &c.vertices[..c.count] {
                if v < moving {
                    offsets[v + 1] += 1;
                }
            }
        }
        for v in 0..moving {
            offsets[v + 1] += offsets[v];
        }
        let mut indices = vec![0; offsets[moving]];
        let mut cursor = offsets[..moving].to_vec();
        for (i, c) in contacts.iter().enumerate() {
            for &v in &c.vertices[..c.count] {
                if v < moving {
                    indices[cursor[v]] = i;
                    cursor[v] += 1;
                }
            }
        }
        Self { offsets, indices }
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &[usize]> {
        self.offsets.windows(2).map(|r| &self.indices[r[0]..r[1]])
    }
}
impl std::ops::Index<usize> for ContactIncidence {
    type Output = [usize];
    fn index(&self, v: usize) -> &Self::Output {
        &self.indices[self.offsets[v]..self.offsets[v + 1]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ogc::potential::Kind;

    #[test]
    fn compact_incidence_matches_expanded_rows_including_repeated_and_fixed_vertices() {
        let contacts: Vec<_> = (0..123)
            .map(|i| Contact {
                kind: Kind::PointFace,
                vertices: [i % 11, (i * 7) % 13, (i * 3) % 17, i % 11],
                count: 2 + i % 3,
                mollifier_epsilon: 0.,
            })
            .collect();
        for n in [0, 1, 9, 17, 30] {
            for cs in [&contacts[..], &contacts[..0]] {
                let mut reference = vec![Vec::new(); n];
                for (i, c) in cs.iter().enumerate() {
                    for &v in &c.vertices[..c.count] {
                        if v < n {
                            reference[v].push(i);
                        }
                    }
                }
                let compact = ContactIncidence::new(n, cs);
                assert_eq!(compact.iter().len(), reference.len());
                for (row, expected) in compact.iter().zip(&reference) {
                    assert_eq!(row, expected.as_slice());
                }
                for v in 0..n {
                    assert_eq!(compact[v], reference[v]);
                }
            }
        }
    }
}
