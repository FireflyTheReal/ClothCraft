//! Block-Jacobi preconditioned conjugate gradient for the assembled operator.
//!
//! # Reverse-engineering basis
//!
//! `Marvelous::CG` in the shipped `libCloSimulateSolvers.dylib` (evidence
//! `ev_ac2b9b3f6fc794c7a713da497c77a2862424b08dd64ae4bebcaa141d30de97b4`) is a
//! preconditioned conjugate gradient over the mixed block-CSR operator, taking
//! an explicit `symMat3 const*` preconditioner buffer that
//! `Marvelous::ComputeInverseAii` filled with the inverted nodal diagonal
//! blocks.
//!
//! The decompiled control flow shows the textbook Preconditioned CG structure:
//! `r = b - A x`; `rho = r . r`; `eps = max(1e-15, rho * 1e-15)`; then, while
//! `iter < maxIter` and `eps <= rho`, `Ap = A p`; `alpha = rho / (p . Ap)`;
//! `x += alpha p`; `r -= alpha Ap`; `z = M^-1 r`; `rho' = r . z`;
//! `beta = rho' / rho`; `p = z + beta p`; `rho = rho'`. The neighbour
//! `ConjugateGradient_CPU` is the OpenMP-dispatched driver around the same
//! iteration (`__kmpc_fork_call` instantiations for the signed and unsigned
//! index overloads).
//!
//! This module implements that algorithm from the published method, with this
//! project's own storage, convergence reporting and failure modes. It is not a
//! transcription of the decompiled bodies.
//!
//! Two details of the shipped loop remain only partially resolved and are
//! deliberately **not** reproduced: the exact pre-loop initialisation of `p`
//! and `z` (the decompiler shows the vectors arriving partially primed by the
//! OpenMP driver), and the exact meaning of the shipped `1e-15`-scaled
//! absolute epsilon in a double-precision context. The cleanroom solver below
//! uses an unambiguous relative tolerance on the residual.

use super::{csr::MatMixCsr, sym_mat3::SymMat3};

/// Iteration budget and tolerances for [`block_jacobi_pcg`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockPcgSettings {
    /// Maximum number of CG iterations before the direction is reported as
    /// exhausted. `0` returns immediately without touching `x`.
    pub max_iterations: usize,
    /// Convergence threshold expressed against the initial residual norm.
    pub relative_tolerance: f64,
    /// Relative tolerance used when inverting nodal diagonal blocks. A block
    /// whose `|det|` falls below `this * max|a_ij|^3` is singular.
    pub inverse_relative_tolerance: f64,
}

impl Default for BlockPcgSettings {
    fn default() -> Self {
        Self {
            max_iterations: 250,
            relative_tolerance: 1e-5,
            inverse_relative_tolerance: 1e-14,
        }
    }
}

/// Why [`block_jacobi_pcg`] stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockPcgStatus {
    /// The residual norm fell to the requested tolerance.
    Converged,
    /// The iteration budget ran out with a finite, well-defined residual.
    Exhausted,
    /// Curvature `p . A p`, or the preconditioned residual norm, stopped being
    /// finite and positive. The operator is not positive definite for this
    /// preconditioner, so the direction must not be trusted.
    NotPositiveDefinite,
    /// A nodal diagonal block could not be inverted, so no block-Jacobi
    /// preconditioner exists for this operator.
    SingularBlock(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockPcgReport {
    pub status: BlockPcgStatus,
    pub iterations: usize,
    /// Residual norm `||b - A x||` at the reported iterate.
    pub residual_norm: f64,
    /// Residual norm of the incoming `x`, before any iteration.
    pub initial_residual_norm: f64,
}

impl BlockPcgReport {
    /// Whether the returned `x` is a usable descent-quality iterate.
    pub fn is_usable(&self) -> bool {
        matches!(
            self.status,
            BlockPcgStatus::Converged | BlockPcgStatus::Exhausted
        )
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

fn apply_preconditioner(minv: &[SymMat3], r: &[f64], z: &mut [f64]) {
    for (n, block) in minv.iter().enumerate() {
        let out = block.mul_vec([r[3 * n], r[3 * n + 1], r[3 * n + 2]]);
        z[3 * n] = out[0];
        z[3 * n + 1] = out[1];
        z[3 * n + 2] = out[2];
    }
}

/// Solve `A x = b` in place, starting from the incoming `x`, using Preconditioned
/// Conjugate Gradient with the supplied nodal block-Jacobi preconditioner.
///
/// `minv` holds the inverted nodal diagonal blocks in node order. `a` is
/// expected to be symmetric positive definite on the free dofs; callers that
/// pin dofs should call [`MatMixCsr::apply_dirichlet`] before building the
/// preconditioner so that both stages see the same operator.
pub fn block_jacobi_pcg(
    a: &MatMixCsr,
    minv: &[SymMat3],
    b: &[f64],
    x: &mut [f64],
    settings: &BlockPcgSettings,
) -> BlockPcgReport {
    let n = 3 * a.nodes();
    assert_eq!(b.len(), n, "right-hand side must hold 3 dofs per node");
    assert_eq!(x.len(), n, "solution must hold 3 dofs per node");
    assert_eq!(minv.len(), a.nodes(), "one preconditioner block per node");

    let mut r = vec![0.0; n];
    let mut ax = vec![0.0; n];
    a.spmv(x, &mut ax);
    for i in 0..n {
        r[i] = b[i] - ax[i];
    }

    let initial = norm(&r);
    if !initial.is_finite() {
        return BlockPcgReport {
            status: BlockPcgStatus::NotPositiveDefinite,
            iterations: 0,
            residual_norm: f64::INFINITY,
            initial_residual_norm: initial,
        };
    }
    let tolerance = settings.relative_tolerance * initial;
    if initial == 0.0 || (tolerance > 0.0 && initial <= tolerance) {
        return BlockPcgReport {
            status: BlockPcgStatus::Converged,
            iterations: 0,
            residual_norm: initial,
            initial_residual_norm: initial,
        };
    }

    let mut z = vec![0.0; n];
    let mut p = vec![0.0; n];
    apply_preconditioner(minv, &r, &mut z);
    p.copy_from_slice(&z);
    let mut rho = dot(&r, &z);
    if !rho.is_finite() || rho <= 0.0 {
        return BlockPcgReport {
            status: BlockPcgStatus::NotPositiveDefinite,
            iterations: 0,
            residual_norm: initial,
            initial_residual_norm: initial,
        };
    }

    let mut ap = vec![0.0; n];
    for iteration in 0..settings.max_iterations {
        a.spmv(&p, &mut ap);
        let curvature = dot(&p, &ap);
        if !curvature.is_finite() || curvature <= 0.0 {
            return BlockPcgReport {
                status: BlockPcgStatus::NotPositiveDefinite,
                iterations: iteration,
                residual_norm: norm(&r),
                initial_residual_norm: initial,
            };
        }

        let alpha = rho / curvature;
        if !alpha.is_finite() {
            return BlockPcgReport {
                status: BlockPcgStatus::NotPositiveDefinite,
                iterations: iteration,
                residual_norm: norm(&r),
                initial_residual_norm: initial,
            };
        }
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }

        let residual = norm(&r);
        if residual <= tolerance {
            return BlockPcgReport {
                status: BlockPcgStatus::Converged,
                iterations: iteration + 1,
                residual_norm: residual,
                initial_residual_norm: initial,
            };
        }

        apply_preconditioner(minv, &r, &mut z);
        let rho_next = dot(&r, &z);
        if !rho_next.is_finite() || rho_next <= 0.0 {
            return BlockPcgReport {
                status: BlockPcgStatus::NotPositiveDefinite,
                iterations: iteration + 1,
                residual_norm: residual,
                initial_residual_norm: initial,
            };
        }
        let beta = rho_next / rho;
        if !beta.is_finite() {
            return BlockPcgReport {
                status: BlockPcgStatus::NotPositiveDefinite,
                iterations: iteration + 1,
                residual_norm: residual,
                initial_residual_norm: initial,
            };
        }
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
        rho = rho_next;
    }

    BlockPcgReport {
        status: BlockPcgStatus::Exhausted,
        iterations: settings.max_iterations,
        residual_norm: norm(&r),
        initial_residual_norm: initial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocksolve::csr::MatMixCsrBuilder;

    /// Dense Gaussian elimination with partial pivoting, for reference tests only.
    fn dense_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
        let n = b.len();
        for col in 0..n {
            let pivot = (col..n)
                .max_by(|&x, &y| a[x][col].abs().partial_cmp(&a[y][col].abs()).unwrap())
                .unwrap();
            a.swap(col, pivot);
            b.swap(col, pivot);
            let d = a[col][col];
            for value in a[col].iter_mut().skip(col) {
                *value /= d;
            }
            b[col] /= d;
            // Re-read the normalised pivot row so the elimination loop below can
            // borrow its own row mutably.
            let normalized = a[col].clone();
            for row in 0..n {
                if row == col {
                    continue;
                }
                let factor = a[row][col];
                if factor == 0.0 {
                    continue;
                }
                for (j, value) in a[row].iter_mut().enumerate().skip(col) {
                    *value -= factor * normalized[j];
                }
                b[row] -= factor * b[col];
            }
        }
        b
    }

    /// A well-conditioned SPD chain of 3-dof nodes.
    fn chain(nodes: usize) -> MatMixCsr {
        let mut builder = MatMixCsrBuilder::new(nodes);
        for n in 0..nodes {
            builder.add_block(n, n, [[4.0, 0.2, 0.0], [0.2, 5.0, -0.3], [0.0, -0.3, 6.0]]);
            if n + 1 < nodes {
                builder.add_block(
                    n,
                    n + 1,
                    [[-1.0, 0.1, 0.0], [0.0, -1.0, 0.2], [0.05, 0.0, -1.0]],
                );
            }
        }
        builder.build()
    }

    #[test]
    fn converged_solution_matches_dense_reference() {
        let a = chain(6);
        let n = 3 * a.nodes();
        let b: Vec<f64> = (0..n).map(|i| 0.5 + i as f64 * 0.25).collect();
        let minv = a.block_diagonal_inverse(1e-14).expect("invertible");
        let mut x = vec![0.0; n];
        // A tight CG tolerance is required before comparing against a direct
        // solve: the default 1e-5 target bounds the *residual*, not the solution
        // error, so it cannot support a 1e-7 solution comparison.
        let settings = BlockPcgSettings {
            relative_tolerance: 1e-12,
            ..BlockPcgSettings::default()
        };
        let report = block_jacobi_pcg(&a, &minv, &b, &mut x, &settings);

        assert_eq!(report.status, BlockPcgStatus::Converged, "{report:?}");
        assert!(report.iterations > 0);
        assert!(report.residual_norm <= report.initial_residual_norm * 1e-12);

        let reference = dense_solve(a.to_dense(), b.clone());
        for i in 0..n {
            assert!(
                (x[i] - reference[i]).abs() < 1e-7,
                "x[{i}] = {} reference {}",
                x[i],
                reference[i]
            );
        }
    }

    #[test]
    fn residual_decreases_from_the_initial_guess() {
        let a = chain(4);
        let n = 3 * a.nodes();
        let b = vec![1.0; n];
        let minv = a.block_diagonal_inverse(1e-14).unwrap();
        let mut x = vec![0.0; n];
        let report = block_jacobi_pcg(&a, &minv, &b, &mut x, &BlockPcgSettings::default());
        assert!(report.residual_norm < report.initial_residual_norm);
        assert!(report.residual_norm.is_finite());
    }

    #[test]
    fn exhausted_budget_reports_remaining_residual() {
        let a = chain(8);
        let n = 3 * a.nodes();
        let b = vec![1.0; n];
        let minv = a.block_diagonal_inverse(1e-14).unwrap();
        let mut x = vec![0.0; n];
        let settings = BlockPcgSettings {
            max_iterations: 1,
            relative_tolerance: 1e-14,
            ..BlockPcgSettings::default()
        };
        let report = block_jacobi_pcg(&a, &minv, &b, &mut x, &settings);
        assert_eq!(report.status, BlockPcgStatus::Exhausted);
        assert_eq!(report.iterations, 1);
        assert!(report.residual_norm > 0.0);
        assert!(report.is_usable());
    }

    #[test]
    fn zero_right_hand_side_converges_without_iterating() {
        let a = chain(3);
        let n = 3 * a.nodes();
        let minv = a.block_diagonal_inverse(1e-14).unwrap();
        let mut x = vec![0.0; n];
        let report = block_jacobi_pcg(
            &a,
            &minv,
            &vec![0.0; n],
            &mut x,
            &BlockPcgSettings::default(),
        );
        assert_eq!(report.status, BlockPcgStatus::Converged);
        assert_eq!(report.iterations, 0);
        assert_eq!(x, vec![0.0; n]);
    }

    #[test]
    fn zero_budget_leaves_the_guess_untouched() {
        let a = chain(3);
        let n = 3 * a.nodes();
        let minv = a.block_diagonal_inverse(1e-14).unwrap();
        let mut x = vec![0.5; n];
        let before = x.clone();
        let settings = BlockPcgSettings {
            max_iterations: 0,
            ..BlockPcgSettings::default()
        };
        let report = block_jacobi_pcg(&a, &minv, &vec![1.0; n], &mut x, &settings);
        assert_eq!(report.status, BlockPcgStatus::Exhausted);
        assert_eq!(report.iterations, 0);
        assert_eq!(x, before);
    }

    #[test]
    fn indefinite_operator_is_rejected_rather_than_returned() {
        // Negative diagonal blocks make the operator indefinite.
        let mut builder = MatMixCsrBuilder::new(2);
        builder.add_block(0, 0, [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]);
        builder.add_block(1, 1, [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]);
        let a = builder.build();
        let minv = a.block_diagonal_inverse(1e-14).unwrap();
        let mut x = vec![0.0; 6];
        let report = block_jacobi_pcg(&a, &minv, &[1.0; 6], &mut x, &BlockPcgSettings::default());
        assert_eq!(report.status, BlockPcgStatus::NotPositiveDefinite);
        assert!(!report.is_usable());
    }

    #[test]
    fn dirichlet_operator_solves_pinned_dofs_exactly() {
        let mut a = chain(4);
        let fixed = [
            false, true, false, false, false, false, false, false, false, false, false, false,
        ];
        a.apply_dirichlet(&fixed);
        let minv = a.block_diagonal_inverse(1e-14).unwrap();
        let n = 3 * a.nodes();
        // The Newton right-hand side is zero on pinned dofs, exactly as `System`
        // builds it before calling the linear solve.
        let mut b = vec![1.0; n];
        b[1] = 0.0;
        let mut x = vec![0.0; n];
        let settings = BlockPcgSettings {
            relative_tolerance: 1e-12,
            ..BlockPcgSettings::default()
        };
        let report = block_jacobi_pcg(&a, &minv, &b, &mut x, &settings);
        assert!(report.is_usable(), "{report:?}");
        // A pinned dof keeps its prescribed (zero) increment exactly.
        assert_eq!(x[1], 0.0);
        let reference = dense_solve(a.to_dense(), b.clone());
        for i in 0..n {
            assert!(
                (x[i] - reference[i]).abs() < 1e-6,
                "x[{i}] = {} reference {}",
                x[i],
                reference[i]
            );
        }
    }
}
