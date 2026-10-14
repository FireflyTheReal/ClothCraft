//! Editable garment templates compile into a fixed, CPU-only physical model.
//! No mesh indices are persisted in the authoring format.
pub mod embedding;
pub mod export;
pub mod meshing;
pub mod polygon;
pub mod presets;
pub mod seam;
pub mod template;

pub use template::{CompiledGarment, GarmentTemplate};
