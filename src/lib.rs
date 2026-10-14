//! SI units throughout: metres, kilograms, seconds, Newtons.
pub mod authoring;
pub mod avatar;
pub mod contact;
pub mod energy;
pub mod fabrics;
pub mod fitting;
pub mod garment;
#[cfg(feature = "desktop")]
pub mod gpu_vbd;
pub mod math;
pub mod model;
pub mod ogc;
pub mod picking;
pub mod project;
/// Shared optional renderer; the numerical modules do not depend on this module.
#[cfg(feature = "desktop")]
pub mod render;
pub mod solver;
pub mod vbd;
