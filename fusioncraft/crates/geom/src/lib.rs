//! Geometry for FusionCraft (layer L0): vectors, planes, polygons, triangle meshes, the solid
//! builders (extrude, revolve, primitives) and mesh booleans. Nothing here knows about documents,
//! files or the UI.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod csg;
pub mod mesh;
pub mod plane;
pub mod poly;
pub mod solid;
pub mod vec;

pub use mesh::{Aabb, Mesh, Triangle};
pub use plane::Plane;
pub use vec::{Vec2, Vec3};

/// Lengths below this are treated as zero (model units are millimetres).
pub const EPS: f64 = 1e-7;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum GeomError {
    #[error("the profile is degenerate (fewer than three distinct points or zero area)")]
    DegenerateProfile,
    #[error("the value {0} is out of range")]
    OutOfRange(String),
    #[error("the boolean operation is too complex (limit {0} reached)")]
    TooComplex(usize),
    #[error("the profile crosses the revolve axis")]
    CrossesAxis,
}
