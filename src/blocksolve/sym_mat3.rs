//! Symmetric 3x3 nodal block: the atomic unit of the assembled cloth operator.
//!
//! # Reverse-engineering basis
//!
//! `Marvelous::ComputeAD` in the shipped `libCloSimulateSolvers.dylib`
//! (CLO 2026.1.310, arm64 slice SHA-256
//! `3050227736cfcd66db262f552660551e735909900e1f8632a301b7c564143446`,
//! evidence `ev_ac2b9b3f6fc794c7a713da497c77a2862424b08dd64ae4bebcaa141d30de97b4`)
//! reads each nodal *diagonal* block as six `float`s at a 24-byte stride and
//! contracts it as a symmetric matrix, while off-diagonal blocks are read as
//! nine `float`s at a 36-byte stride. The timestep operator is therefore
//! block-sparse over **3x3 nodal blocks**, with the nodal diagonal stored
//! symmetrically and the off-diagonal entries stored in full.
//!
//! This module implements that *structure* independently. The field order below
//! is this project's own choice and is not a transcription of the observed byte
//! order.

/// A symmetric 3x3 matrix stored as its upper triangle in row-major order:
/// `[m00, m01, m02, m11, m12, m22]`.
///
/// Every operation preserves symmetry exactly; nothing here ever produces an
/// asymmetric result from symmetric input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymMat3 {
    /// Upper triangle in row-major order.
    pub m: [f64; 6],
}

impl SymMat3 {
    pub const ZERO: Self = Self { m: [0.0; 6] };
    pub const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 1.0, 0.0, 1.0],
    };

    pub const fn new(m00: f64, m01: f64, m02: f64, m11: f64, m12: f64, m22: f64) -> Self {
        Self {
            m: [m00, m01, m02, m11, m12, m22],
        }
    }

    /// Symmetrised block from a dense row-major matrix, using
    /// `(a_ij + a_ji) / 2` so that assembly from element Hessians cannot inject
    /// an asymmetric part through floating-point round-off.
    pub fn from_dense(dense: [[f64; 3]; 3]) -> Self {
        let s = |i: usize, j: usize| 0.5 * (dense[i][j] + dense[j][i]);
        Self::new(s(0, 0), s(0, 1), s(0, 2), s(1, 1), s(1, 2), s(2, 2))
    }

    pub fn to_dense(self) -> [[f64; 3]; 3] {
        let [a, b, c, d, e, f] = self.m;
        [[a, b, c], [b, d, e], [c, e, f]]
    }

    pub fn add_assign(&mut self, other: Self) {
        for (x, y) in self.m.iter_mut().zip(other.m) {
            *x += y;
        }
    }

    pub fn scaled(self, s: f64) -> Self {
        Self {
            m: self.m.map(|v| v * s),
        }
    }

    pub fn add_scaled(&mut self, other: Self, s: f64) {
        for (x, y) in self.m.iter_mut().zip(other.m) {
            *x += y * s;
        }
    }

    pub fn mul_vec(self, v: [f64; 3]) -> [f64; 3] {
        let [a, b, c, d, e, f] = self.m;
        [
            a * v[0] + b * v[1] + c * v[2],
            b * v[0] + d * v[1] + e * v[2],
            c * v[0] + e * v[1] + f * v[2],
        ]
    }

    pub fn determinant(self) -> f64 {
        let [a, b, c, d, e, f] = self.m;
        a * (d * f - e * e) - b * (b * f - e * c) + c * (b * e - d * c)
    }

    pub fn max_abs(self) -> f64 {
        self.m.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()))
    }

    pub fn is_finite(self) -> bool {
        self.m.iter().all(|v| v.is_finite())
    }

    /// `v^T A v`. Used to check that a nodal block is positive definite before
    /// it is trusted as a preconditioner.
    pub fn quadratic_form(self, v: [f64; 3]) -> f64 {
        let av = self.mul_vec(v);
        av[0] * v[0] + av[1] * v[1] + av[2] * v[2]
    }

    /// Inverse of a symmetric 3x3 block via its adjugate.
    ///
    /// Returns `None` when the block is numerically singular: `|det(A)|` must
    /// exceed `relative_tolerance * max|a_ij|^3`. A block-Jacobi preconditioner
    /// must never divide by a non-positive-definite block silently, so the
    /// caller decides how to regularise.
    pub fn inverse(self, relative_tolerance: f64) -> Option<Self> {
        let det = self.determinant();
        let scale = self.max_abs();
        let threshold = relative_tolerance * scale.powi(3);
        if !det.is_finite() || det.abs() <= threshold {
            return None;
        }
        let d = self.to_dense();
        let [[a, b, c], [_, dd, e], [_, _, f]] = d;
        // The adjugate of a symmetric matrix is symmetric.
        let adj = Self::new(
            dd * f - e * e,
            c * e - b * f,
            b * e - c * dd,
            a * f - c * c,
            b * c - a * e,
            a * dd - b * b,
        );
        let inv = adj.scaled(1.0 / det);
        // The symmetrisation in `from_dense` is a no-op here; `is_finite` is the
        // real guard against overflow for extreme blocks.
        inv.is_finite().then_some(inv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spd() -> SymMat3 {
        // 4x4-style SPD-ish block with distinct entries.
        SymMat3::new(4.0, 1.0, 0.5, 3.0, -0.25, 2.5)
    }

    #[test]
    fn dense_round_trip_is_symmetric() {
        let a = spd();
        let d = a.to_dense();
        for (i, row) in d.iter().enumerate() {
            for (j, value) in row.iter().enumerate() {
                assert_eq!(*value, d[j][i]);
            }
        }
        assert_eq!(SymMat3::from_dense(d), a);
    }

    #[test]
    fn inverse_satisfies_a_times_inverse_is_identity() {
        let a = spd();
        let inv = a.inverse(1e-14).expect("block is well conditioned");
        let dense_inv = inv.to_dense();
        let dense_a = a.to_dense();
        // Columns of the inverse, so the product can be taken with iterators.
        let inv_cols = [
            [dense_inv[0][0], dense_inv[1][0], dense_inv[2][0]],
            [dense_inv[0][1], dense_inv[1][1], dense_inv[2][1]],
            [dense_inv[0][2], dense_inv[1][2], dense_inv[2][2]],
        ];
        for (i, row_a) in dense_a.iter().enumerate() {
            for (j, col_inv) in inv_cols.iter().enumerate() {
                let acc: f64 = row_a.iter().zip(col_inv).map(|(a, b)| a * b).sum();
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (acc - expected).abs() < 1e-12,
                    "A A^-1 [{i}][{j}] = {acc}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn singular_block_has_no_inverse() {
        // Rank one: second row is twice the first.
        let a = SymMat3::new(1.0, 2.0, 0.0, 4.0, 0.0, 0.0);
        assert_eq!(a.determinant(), 0.0);
        assert!(a.inverse(1e-14).is_none());
    }

    #[test]
    fn quadratic_form_is_positive_for_a_positive_definite_block() {
        let a = spd();
        for v in [[1.0, 0.0, 0.0], [0.3, -0.7, 0.2], [-1.0, 1.0, 0.5]] {
            assert!(a.quadratic_form(v) > 0.0);
        }
    }

    #[test]
    fn matrix_vector_product_matches_dense() {
        let a = spd();
        let d = a.to_dense();
        let v = [0.4, -1.2, 0.9];
        let got = a.mul_vec(v);
        for i in 0..3 {
            let expect = (0..3).map(|k| d[i][k] * v[k]).sum::<f64>();
            assert!((got[i] - expect).abs() < 1e-15);
        }
    }
}
