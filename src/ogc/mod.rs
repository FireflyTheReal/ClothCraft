//! Offset Geometric Contact (Chen et al., SIGGRAPH 2025), independent Rust/f64
//! implementation. See docs/ogc.md for the precise scope and paper errata.
pub mod bvh;
mod candidates;
pub mod geometry;
pub mod potential;
pub mod topology;
mod world;
pub use world::{ContactSet, DetectionProfile, OgcSettings, OgcWorld};
