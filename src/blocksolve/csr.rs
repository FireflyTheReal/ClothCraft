//! Mixed block-CSR operator: one symmetric 3x3 block per node plus full 3x3
//! off-diagonal blocks.
//!
//! # Reverse-engineering basis
//!
//! `Marvelous::ComputeAD` (evidence `ev_ac2b9b3f6fc794c7a713da497c77a2862424b08dd64ae4bebcaa141d30de97b4`)
//! walks a `matMixCSR` whose `int32` row pointers live at struct offset `+0x10`,
//! column indices at `+0x40`, a **dedicated array of 24-byte symmetric diagonal
//! blocks** at `+0x70`, and a **second array of 36-byte full 3x3 off-diagonal
//! blocks** at `+0x88`. The two distinct block arrays are what make the format
//! "mixed": the diagonal is stored once in six floats per node, off-diagonal
//! entries keep a full nine.
//!
//! `Marvelous::ComputeInverseAii` (same evidence) walks exactly the same
//! structure and calls `invert(symMat3&, symMat3 const&)` on each node whose
//! column index equals its row index, i.e. the preconditioner is precisely the
//! inverted nodal diagonal.
//!
//! This project stores the same mathematical object with its own layout: only
//! the strict upper triangle of the node graph is retained, because the
//! assembled cloth operator is symmetric. That halves off-diagonal storage
//! relative to keeping both triangles, at the cost of requiring symmetric
//! assembly (documented on [`MatMixCsrBuilder::add_block`]).

use super::sym_mat3::SymMat3;

/// A dense 3x3 block in row-major order.
pub type Block3 = [[f64; 3]; 3];

pub const ZERO_BLOCK3: Block3 = [[0.0; 3]; 3];

/// Transpose of a dense 3x3 block.
pub fn transpose(b: Block3) -> Block3 {
    let mut t = ZERO_BLOCK3;
    for r in 0..3 {
        for c in 0..3 {
            t[r][c] = b[c][r];
        }
    }
    t
}

/// A nodal diagonal block that could not be inverted during preconditioner
/// construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SingularBlock {
    pub node: usize,
}

/// Assembled symmetric operator over 3-DOF nodes, in mixed block-CSR form.
#[derive(Clone, Debug, Default)]
pub struct MatMixCsr {
    nodes: usize,
    row_start: Vec<usize>,
    col: Vec<usize>,
    off: Vec<Block3>,
    diag: Vec<SymMat3>,
}

impl MatMixCsr {
    pub fn nodes(&self) -> usize {
        self.nodes
    }

    pub fn rows(&self) -> usize {
        self.nodes
    }

    /// Number of stored strict-upper off-diagonal blocks.
    pub fn off_diagonal_blocks(&self) -> usize {
        self.off.len()
    }

    pub fn diagonal_blocks(&self) -> &[SymMat3] {
        &self.diag
    }

    /// Column indices of the off-diagonal blocks, grouped by row.
    pub fn columns(&self) -> &[usize] {
        &self.col
    }

    pub fn row_start(&self) -> &[usize] {
        &self.row_start
    }

    pub fn off_diagonal(&self) -> &[Block3] {
        &self.off
    }

    /// `y = A x`, **overwriting** `y`.
    ///
    /// Overwrite semantics mirror the shipped `ComputeAD`, which zeroes its
    /// output before accumulating. Because only the strict upper triangle of the
    /// node graph is stored, each stored block contributes to both of its
    /// endpoints, using the transpose for the lower ordering.
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) {
        assert_eq!(x.len(), 3 * self.nodes, "x must hold 3 dofs per node");
        assert_eq!(y.len(), 3 * self.nodes, "y must hold 3 dofs per node");

        for (n, d) in self.diag.iter().enumerate() {
            let r = d.mul_vec([x[3 * n], x[3 * n + 1], x[3 * n + 2]]);
            y[3 * n] = r[0];
            y[3 * n + 1] = r[1];
            y[3 * n + 2] = r[2];
        }

        for i in 0..self.nodes {
            for k in self.row_start[i]..self.row_start[i + 1] {
                let j = self.col[k];
                let b = &self.off[k];
                let xi = [x[3 * i], x[3 * i + 1], x[3 * i + 2]];
                let xj = [x[3 * j], x[3 * j + 1], x[3 * j + 2]];
                for r in 0..3 {
                    y[3 * i + r] += b[r][0] * xj[0] + b[r][1] * xj[1] + b[r][2] * xj[2];
                    // (B^T x_i)_r, i.e. the lower-triangle counterpart of B.
                    y[3 * j + r] += b[0][r] * xi[0] + b[1][r] * xi[1] + b[2][r] * xi[2];
                }
            }
        }
    }

    /// Inverted nodal diagonal blocks, the block-Jacobi preconditioner.
    ///
    /// This is the cleanroom counterpart of `Marvelous::ComputeInverseAii`.
    pub fn block_diagonal_inverse(
        &self,
        relative_tolerance: f64,
    ) -> Result<Vec<SymMat3>, SingularBlock> {
        self.diag
            .iter()
            .enumerate()
            .map(|(node, block)| {
                block
                    .inverse(relative_tolerance)
                    .ok_or(SingularBlock { node })
            })
            .collect()
    }

    /// Apply homogeneous Dirichlet conditions on `fixed` dofs.
    ///
    /// A fixed dof gets a unit diagonal row and its column is cleared, which is
    /// exactly the convention the matrix-free reference uses: Newton increments
    /// are zero on constrained dofs, so their coupling must not feed free rows.
    /// `fixed` is per dof (length `3 * nodes`), matching `System::fixed`.
    pub fn apply_dirichlet(&mut self, fixed: &[bool]) {
        assert_eq!(fixed.len(), 3 * self.nodes, "fixed must be per dof");

        for i in 0..self.nodes {
            for k in self.row_start[i]..self.row_start[i + 1] {
                let j = self.col[k];
                let b = &mut self.off[k];
                for (r, row) in b.iter_mut().enumerate() {
                    if fixed[3 * i + r] {
                        *row = [0.0; 3];
                    }
                }
                for c in 0..3 {
                    if fixed[3 * j + c] {
                        for row in b.iter_mut() {
                            row[c] = 0.0;
                        }
                    }
                }
            }
        }

        for n in 0..self.nodes {
            let pinned = [0usize, 1, 2].map(|r| fixed[3 * n + r]);
            if !pinned.iter().any(|p| *p) {
                continue;
            }
            let mut d = self.diag[n].to_dense();
            for (r, is_pinned) in pinned.iter().enumerate() {
                if !*is_pinned {
                    continue;
                }
                // Clear the whole column, then the whole row, then restore the unit
                // diagonal. Order matters when two dofs of one node are pinned.
                for row in d.iter_mut() {
                    row[r] = 0.0;
                }
                d[r] = [0.0; 3];
                d[r][r] = 1.0;
            }
            self.diag[n] = SymMat3::from_dense(d);
        }
    }

    /// Dense expansion, for diagnostics and reference tests only.
    pub fn to_dense(&self) -> Vec<Vec<f64>> {
        let n = 3 * self.nodes;
        let mut a = vec![vec![0.0; n]; n];
        for (node, d) in self.diag.iter().enumerate() {
            let dense = d.to_dense();
            for r in 0..3 {
                for c in 0..3 {
                    a[3 * node + r][3 * node + c] += dense[r][c];
                }
            }
        }
        for i in 0..self.nodes {
            for k in self.row_start[i]..self.row_start[i + 1] {
                let j = self.col[k];
                let b = &self.off[k];
                for r in 0..3 {
                    for c in 0..3 {
                        a[3 * i + r][3 * j + c] += b[r][c];
                        a[3 * j + c][3 * i + r] += b[r][c];
                    }
                }
            }
        }
        a
    }
}

/// Incremental assembly for [`MatMixCsr`].
///
/// Blocks accumulate; duplicate `(i, j)` pairs are merged at [`Self::build`].
#[derive(Clone, Debug)]
pub struct MatMixCsrBuilder {
    nodes: usize,
    diag: Vec<SymMat3>,
    rows: Vec<Vec<(usize, Block3)>>,
}

impl MatMixCsrBuilder {
    pub fn new(nodes: usize) -> Self {
        Self {
            nodes,
            diag: vec![SymMat3::ZERO; nodes],
            rows: (0..nodes).map(|_| Vec::new()).collect(),
        }
    }

    pub fn nodes(&self) -> usize {
        self.nodes
    }

    /// Accumulate one 3x3 sub-block of the global operator.
    ///
    /// `i == j` adds the *symmetric part* to the nodal diagonal. For `i != j`
    /// the block is stored once under `min(i, j)`, transposed when the caller
    /// supplied the lower ordering. Storing one triangle is only valid because
    /// the assembled operator is symmetric; every contribution here comes from
    /// the Hessian of a scalar energy, which is symmetric by construction.
    pub fn add_block(&mut self, i: usize, j: usize, block: Block3) {
        assert!(i < self.nodes && j < self.nodes, "node index out of range");
        if i == j {
            self.diag[i].add_assign(SymMat3::from_dense(block));
            return;
        }
        let (lo, hi, block) = if i < j {
            (i, j, block)
        } else {
            (j, i, transpose(block))
        };
        self.rows[lo].push((hi, block));
    }

    pub fn build(mut self) -> MatMixCsr {
        let mut row_start = Vec::with_capacity(self.nodes + 1);
        let mut col = Vec::new();
        let mut off = Vec::new();
        row_start.push(0);

        for row in &mut self.rows {
            row.sort_by_key(|(j, _)| *j);
            let mut write = 0usize;
            for read in 0..row.len() {
                if write > 0 && row[write - 1].0 == row[read].0 {
                    let (_, add) = row[read];
                    let slot = &mut row[write - 1].1;
                    for r in 0..3 {
                        for c in 0..3 {
                            slot[r][c] += add[r][c];
                        }
                    }
                } else {
                    row.swap(write, read);
                    write += 1;
                }
            }
            row.truncate(write);
            for (j, b) in row.iter() {
                col.push(*j);
                off.push(*b);
            }
            row_start.push(col.len());
        }

        MatMixCsr {
            nodes: self.nodes,
            row_start,
            col,
            off,
            diag: self.diag,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MatMixCsr {
        // Two nodes, a symmetric 3x3 coupling and non-trivial diagonals.
        let mut b = MatMixCsrBuilder::new(2);
        b.add_block(0, 0, [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]]);
        b.add_block(1, 1, [[5.0, 0.0, 0.0], [0.0, 6.0, 0.0], [0.0, 0.0, 7.0]]);
        b.add_block(0, 1, [[1.0, 0.5, 0.0], [0.0, 1.5, 0.0], [0.25, 0.0, 2.0]]);
        b.build()
    }

    #[test]
    fn duplicate_blocks_are_merged() {
        let mut b = MatMixCsrBuilder::new(2);
        b.add_block(0, 1, [[1.0; 3]; 3]);
        b.add_block(0, 1, [[2.0; 3]; 3]);
        let m = b.build();
        assert_eq!(m.off_diagonal_blocks(), 1);
        assert_eq!(m.off_diagonal()[0], [[3.0; 3]; 3]);
    }

    #[test]
    fn lower_ordering_is_transposed_into_upper_storage() {
        let upper = {
            let mut b = MatMixCsrBuilder::new(2);
            b.add_block(0, 1, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
            b.build()
        };
        let lower = {
            let mut b = MatMixCsrBuilder::new(2);
            b.add_block(1, 0, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
            b.build()
        };
        // Supplying the same block in the lower ordering stores its transpose.
        assert_eq!(upper.off_diagonal()[0], transpose(lower.off_diagonal()[0]));
    }

    #[test]
    fn spmv_is_symmetric_and_matches_dense() {
        let m = sample();
        let dense = m.to_dense();
        assert_eq!(dense.len(), 6);
        for (i, row) in dense.iter().enumerate() {
            for (j, value) in row.iter().enumerate() {
                assert!(
                    (*value - dense[j][i]).abs() < 1e-15,
                    "assembled operator must be symmetric at ({i},{j})"
                );
            }
        }
        let x = [0.3, -0.7, 1.1, 0.2, 0.9, -0.4];
        let mut y = vec![0.0; 6];
        m.spmv(&x, &mut y);
        for i in 0..6 {
            let expect: f64 = (0..6).map(|k| dense[i][k] * x[k]).sum();
            assert!(
                (y[i] - expect).abs() < 1e-12,
                "spmv[{i}] = {} expected {expect}",
                y[i]
            );
        }
    }

    #[test]
    fn dirichlet_pins_rows_and_columns_to_identity() {
        let mut m = sample();
        // Pin all three dofs of node 1.
        let fixed = [false, false, false, true, true, true];
        m.apply_dirichlet(&fixed);
        let dense = m.to_dense();
        for i in 0..6 {
            for j in 0..6 {
                if fixed[i] || fixed[j] {
                    let expect = if i == j { 1.0 } else { 0.0 };
                    assert!(
                        (dense[i][j] - expect).abs() < 1e-15,
                        "pinned entry ({i},{j}) = {} expected {expect}",
                        dense[i][j]
                    );
                }
            }
        }
        // Free block is untouched.
        assert_eq!(dense[0][0], 2.0);
        assert_eq!(dense[1][1], 3.0);
        assert_eq!(dense[2][2], 4.0);
    }

    #[test]
    fn block_diagonal_inverse_inverts_each_node_block() {
        let m = sample();
        let inv = m.block_diagonal_inverse(1e-14).expect("blocks invertible");
        for (n, block) in inv.iter().enumerate() {
            let product = block.mul_vec(m.diagonal_blocks()[n].mul_vec([1.0, 1.0, 1.0]));
            for (k, v) in product.iter().enumerate() {
                let expect = 1.0;
                assert!(
                    (v - expect).abs() < 1e-12,
                    "node {n} component {k}: M^-1 M 1 = {v}"
                );
            }
        }
    }

    #[test]
    fn singular_diagonal_block_is_reported_with_its_node() {
        let mut b = MatMixCsrBuilder::new(2);
        b.add_block(0, 0, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        b.add_block(1, 1, ZERO_BLOCK3);
        let m = b.build();
        assert_eq!(
            m.block_diagonal_inverse(1e-14),
            Err(SingularBlock { node: 1 })
        );
    }
}
