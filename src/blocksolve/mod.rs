//! Assembled nodal block solver for the implicit cloth timestep.
//!
//! ClothCraft's historical Newton reference is *matrix-free*: it stores one
//! dense Hessian per element and applies it without ever assembling a global
//! matrix. Its preconditioner is therefore point-Jacobi, a single scalar per
//! dof. That misses the coupling between the three translational dofs that share
//! a vertex.
//!
//! The shipped CLO 2026.1.310 timestep does the opposite: it assembles a
//! **mixed block-CSR** operator with one symmetric 3x3 block per node and uses
//! the **inverted nodal diagonal** as its preconditioner
//! (`Marvelous::ComputeAD`, `Marvelous::ComputeInverseAii` and
//! `Marvelous::CG` in `libCloSimulateSolvers.dylib`, target SHA-256
//! `3050227736cfcd66db262f552660551e735909900e1f8632a301b7c564143446`, evidence
//! `ev_ac2b9b3f6fc794c7a713da497c77a2862424b08dd64ae4bebcaa141d30de97b4`).
//!
//! This module is a cleanroom implementation of that assembled nodal-block
//! formulation. It is written from the published Preconditioned Conjugate
//! Gradient method and the block-sparse idea; no decompiled body, identifier or
//! memory layout is reproduced. See `docs/clo3d-reverse-engineering.md` for the
//! separation between what was observed and what was implemented.
//!
//! Everything here is pure `f64` standard library code: the module does not
//! depend on `eframe`, `wgpu` or any GPU backend, so it stays usable from the
//! CPU-only build (`--no-default-features`).

pub mod csr;
pub mod pcg;
pub mod sym_mat3;

pub use csr::{Block3, MatMixCsr, MatMixCsrBuilder, SingularBlock, ZERO_BLOCK3, transpose};
pub use pcg::{BlockPcgReport, BlockPcgSettings, BlockPcgStatus, block_jacobi_pcg};
pub use sym_mat3::SymMat3;
