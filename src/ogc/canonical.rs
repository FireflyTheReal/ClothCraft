//! Sort compact integer keys instead of moving full contact records repeatedly.
//! Larger worlds keep the unrestricted comparison path; no ID is truncated.
use super::potential::Contact;

pub(super) fn canonicalize(mut found: Vec<Contact>, vertices: usize) -> Vec<Contact> {
    if vertices > 1 << 15 {
        found.sort_unstable_by_key(|c| (c.kind, c.vertices));
        found.dedup_by_key(|c| (c.kind, c.vertices));
        return found;
    }
    let bits = usize::BITS - vertices.saturating_sub(1).leading_zeros();
    let mut keys: Vec<_> = found
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut key = c.kind as u64;
            for v in c.vertices {
                key = (key << bits) | v as u64;
            }
            (key, i)
        })
        .collect();
    keys.sort_unstable_by_key(|k| k.0);
    keys.dedup_by_key(|k| k.0);
    keys.into_iter().map(|(_, i)| found[i].clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ogc::potential::Kind;

    #[test]
    fn packed_keys_preserve_full_canonical_order_at_capacity_boundary() {
        for vertices in [1, 17, 32768, 32769, 1 << 20] {
            let mut found: Vec<_> = (0..20000)
                .map(|i| {
                    let kind = [
                        Kind::PointPoint,
                        Kind::PointEdge,
                        Kind::PointFace,
                        Kind::EdgeEdge,
                        Kind::EdgeEdgeInterior,
                        Kind::PointTriangle,
                        Kind::SegmentSegment,
                    ][i % 7];
                    let count = [2, 3, 4, 4, 4, 4, 4][i % 7];
                    let mut ids = [0; 4];
                    for (j, v) in ids[..count].iter_mut().enumerate() {
                        *v = (i * (31 + j * 17)) % vertices;
                    }
                    Contact {
                        kind,
                        count,
                        vertices: ids,
                        mollifier_epsilon: 0.,
                    }
                })
                .collect();
            found.extend(found[..1000].to_vec());
            let expected = {
                let mut a = found.clone();
                a.sort_unstable_by_key(|c| (c.kind, c.vertices));
                a.dedup_by_key(|c| (c.kind, c.vertices));
                a
            };
            let actual = canonicalize(found, vertices);
            let keys = |cs: Vec<Contact>| {
                cs.into_iter()
                    .map(|c| (c.kind, c.vertices, c.count))
                    .collect::<Vec<_>>()
            };
            assert_eq!(keys(actual), keys(expected));
        }
    }
}
