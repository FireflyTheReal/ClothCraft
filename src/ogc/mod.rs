//! Offset Geometric Contact (Chen et al., SIGGRAPH 2025), independent Rust/f64
//! implementation. See docs/ogc.md for the precise scope and paper errata.
pub mod bvh;
mod candidates;
mod canonical;
pub mod diagnostics;
mod directional;
pub use directional::{DirectionalBounds, HalfSpace};
pub mod feature_hierarchy;
pub mod feature_mollifier;
pub mod filter;
pub mod geometry;
mod incidence;
pub mod local_surface;
pub mod patch_contact;
pub mod potential;
mod primitive_distance;
mod simd;
pub mod smooth_forest;
pub mod topology;
pub mod two_sided_contact;
pub mod wire_quadrature;
mod world;
pub use incidence::ContactIncidence;
pub use simd::CpuContactKernel;
pub use world::{ContactSet, DetectionProfile, OgcSettings, OgcWorld};
