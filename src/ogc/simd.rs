//! Four independent edge pairs in f64. Runtime AVX dispatch, scalar oracle and
//! portable fallback. No approximate reciprocals, FMA, or changed tolerances.
use glam::DVec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CpuContactKernel {
    #[default]
    Auto,
    Scalar,
}
impl CpuContactKernel {
    pub fn uses_avx(self) -> bool {
        #[cfg(target_arch = "x86_64")]
        if self == Self::Auto {
            return std::is_x86_feature_detected!("avx");
        }
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EdgeClosest {
    pub s: f64,
    pub t: f64,
    pub p: DVec3,
    pub q: DVec3,
    pub distance: f64,
}
#[inline]
pub(crate) fn edge_scalar([a, b, c, d]: [DVec3; 4]) -> EdgeClosest {
    let (s, t) = super::geometry::closest_segments(a, b, c, d);
    let p = a + (b - a) * s;
    let q = c + (d - c) * t;
    EdgeClosest {
        s,
        t,
        p,
        q,
        distance: p.distance(q),
    }
}

/// The feature check is outside the primitive loop. Partial batches use the
/// scalar routine; no dummy pair can change contact or distance accumulation.
pub(crate) struct EdgeKernel {
    avx: bool,
}
impl EdgeKernel {
    pub fn new(mode: CpuContactKernel) -> Self {
        Self {
            avx: mode.uses_avx(),
        }
    }
    pub fn is_wide(&self) -> bool {
        self.avx
    }
    pub fn batch(&self, pairs: [[DVec3; 4]; 4]) -> [EdgeClosest; 4] {
        #[cfg(target_arch = "x86_64")]
        if self.avx {
            // SAFETY: private flag is set only by runtime detection, including
            // OS support for saving AVX registers. Stores below are bounded.
            return unsafe { edges_avx(pairs) };
        }
        pairs.map(edge_scalar)
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx")]
unsafe fn edges_avx(pairs: [[DVec3; 4]; 4]) -> [EdgeClosest; 4] {
    use std::arch::x86_64::*;
    let points: [[__m256d; 3]; 4] = std::array::from_fn(|p| {
        std::array::from_fn(|axis| {
            _mm256_setr_pd(
                pairs[0][p][axis],
                pairs[1][p][axis],
                pairs[2][p][axis],
                pairs[3][p][axis],
            )
        })
    });
    macro_rules! sub3 {
        ($a:expr, $b:expr) => {
            std::array::from_fn::<_, 3, _>(|k| _mm256_sub_pd($a[k], $b[k]))
        };
    }
    macro_rules! dot3 {
        ($a:expr, $b:expr) => {
            _mm256_add_pd(
                _mm256_add_pd(_mm256_mul_pd($a[0], $b[0]), _mm256_mul_pd($a[1], $b[1])),
                _mm256_mul_pd($a[2], $b[2]),
            )
        };
    }
    let zero = _mm256_setzero_pd();
    let one = _mm256_set1_pd(1.);
    macro_rules! clamp {
        ($v:expr) => {{
            // Comparisons/blends preserve signed zero, unlike min/max.
            let v = $v;
            let v = _mm256_blendv_pd(v, zero, _mm256_cmp_pd::<_CMP_LT_OQ>(v, zero));
            _mm256_blendv_pd(v, one, _mm256_cmp_pd::<_CMP_GT_OQ>(v, one))
        }};
    }
    let [a, b, c, d] = points;
    let u = sub3!(b, a);
    let v = sub3!(d, c);
    let w = sub3!(a, c);
    let aa = dot3!(u, u);
    let ee = dot3!(v, v);
    let bb = dot3!(u, v);
    let cc = dot3!(u, w);
    let ff = dot3!(v, w);
    let den = _mm256_sub_pd(_mm256_mul_pd(aa, ee), _mm256_mul_pd(bb, bb));
    let stable = _mm256_cmp_pd::<_CMP_GT_OQ>(
        den,
        _mm256_mul_pd(_mm256_mul_pd(_mm256_set1_pd(1e-14), aa), ee),
    );
    let s0 = clamp!(_mm256_div_pd(
        _mm256_sub_pd(_mm256_mul_pd(bb, ff), _mm256_mul_pd(cc, ee)),
        den
    ));
    let mut s = _mm256_blendv_pd(zero, s0, stable);
    let mut t = _mm256_div_pd(_mm256_add_pd(_mm256_mul_pd(bb, s), ff), ee);
    let low = _mm256_cmp_pd::<_CMP_LT_OQ>(t, zero);
    let high = _mm256_cmp_pd::<_CMP_GT_OQ>(t, one);
    let negative_cc = _mm256_xor_pd(cc, _mm256_set1_pd(-0.));
    s = _mm256_blendv_pd(s, clamp!(_mm256_div_pd(negative_cc, aa)), low);
    s = _mm256_blendv_pd(s, clamp!(_mm256_div_pd(_mm256_sub_pd(bb, cc), aa)), high);
    t = _mm256_blendv_pd(t, zero, low);
    t = _mm256_blendv_pd(t, one, high);
    let degenerate = _mm256_or_pd(
        _mm256_cmp_pd::<_CMP_LT_OQ>(aa, _mm256_set1_pd(1e-24)),
        _mm256_cmp_pd::<_CMP_LT_OQ>(ee, _mm256_set1_pd(1e-24)),
    );
    s = _mm256_blendv_pd(s, zero, degenerate);
    t = _mm256_blendv_pd(t, zero, degenerate);
    let p: [_; 3] = std::array::from_fn(|k| _mm256_add_pd(a[k], _mm256_mul_pd(u[k], s)));
    let q: [_; 3] = std::array::from_fn(|k| _mm256_add_pd(c[k], _mm256_mul_pd(v[k], t)));
    let delta = sub3!(p, q);
    let distance = _mm256_sqrt_pd(dot3!(delta, delta));
    let packed = [s, t, p[0], p[1], p[2], q[0], q[1], q[2], distance];
    let mut rows = [[0.; 4]; 9];
    for (row, value) in rows.iter_mut().zip(packed) {
        // SAFETY: row contains exactly four writable f64s; no alignment needed.
        unsafe {
            _mm256_storeu_pd(row.as_mut_ptr(), value);
        }
    }
    std::array::from_fn(|i| EdgeClosest {
        s: rows[0][i],
        t: rows[1][i],
        p: DVec3::new(rows[2][i], rows[3][i], rows[4][i]),
        q: DVec3::new(rows[5][i], rows[6][i], rows[7][i]),
        distance: rows[8][i],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edge_batches_match_scalar_including_degenerate_and_boundary_cases() {
        let mut seed = 0x12345678_u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 11) as f64 / ((1_u64 << 53) as f64) - 0.5) * 2.
        };
        for batch in 0..10_000 {
            let mut pairs = std::array::from_fn(|_| {
                std::array::from_fn(|_| DVec3::new(next(), next(), next()))
            });
            match batch % 7 {
                0 => pairs[0][1] = pairs[0][0],
                1 => pairs[1][3] = pairs[1][2],
                2 => pairs[2][3] = pairs[2][2] + pairs[2][1] - pairs[2][0],
                3 => pairs[3] = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::X + DVec3::Y],
                4 => pairs[0][2] = pairs[0][1],
                5 => pairs[1] = pairs[1].map(|p| p * 1e-13),
                _ => {}
            }
            let oracle = pairs.map(edge_scalar);
            for mode in [CpuContactKernel::Scalar, CpuContactKernel::Auto] {
                let actual = EdgeKernel::new(mode).batch(pairs);
                for (a, b) in actual.iter().zip(oracle) {
                    let bits = |r: &EdgeClosest| {
                        [
                            r.s, r.t, r.p.x, r.p.y, r.p.z, r.q.x, r.q.y, r.q.z, r.distance,
                        ]
                        .map(f64::to_bits)
                    };
                    assert_eq!(bits(a), bits(&b), "batch {batch}, {mode:?}");
                }
            }
        }
    }
}
