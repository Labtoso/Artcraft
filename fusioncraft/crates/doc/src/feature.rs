//! Timeline features.

use crate::expr::Expr;
use crate::Id;
use fusioncraft_geom::{Plane, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaneRef {
    Xy,
    Xz,
    Yz,
}

impl PlaneRef {
    pub fn plane(self) -> Plane {
        match self {
            PlaneRef::Xy => Plane::xy(),
            PlaneRef::Xz => Plane::xz(),
            PlaneRef::Yz => Plane::yz(),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PlaneRef::Xy => "XY",
            PlaneRef::Xz => "XZ",
            PlaneRef::Yz => "YZ",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "xy" | "top" => Some(PlaneRef::Xy),
            "xz" | "front" => Some(PlaneRef::Xz),
            "yz" | "right" => Some(PlaneRef::Yz),
            _ => None,
        }
    }
}

/// How a feature's new geometry combines with existing bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Join with the bodies it touches, or a new body if it touches none.
    #[default]
    Join,
    Cut,
    Intersect,
    NewBody,
}

impl Operation {
    pub const ALL: [Operation; 4] = [Operation::Join, Operation::Cut, Operation::Intersect, Operation::NewBody];
    pub fn label(self) -> &'static str {
        match self {
            Operation::Join => "Join",
            Operation::Cut => "Cut",
            Operation::Intersect => "Intersect",
            Operation::NewBody => "New Body",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtentDirection {
    /// From the sketch plane by the distance (negative = the other way).
    #[default]
    OneSide,
    /// The distance on both sides of the plane.
    Symmetric,
}

/// Revolve axis in sketch coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Axis {
    SketchX,
    #[default]
    SketchY,
    /// A line entity of the sketch (usually a construction line).
    Line { index: usize },
}

/// A world axis through the origin, for circular patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorldAxis {
    X,
    Y,
    #[default]
    Z,
}

impl WorldAxis {
    pub fn vector(self) -> Vec3 {
        match self {
            WorldAxis::X => Vec3::X,
            WorldAxis::Y => Vec3::Y,
            WorldAxis::Z => Vec3::Z,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FeatureKind {
    Sketch {
        sketch: Id,
    },
    Extrude {
        sketch: Id,
        /// Indices into the sketch's profiles; empty = all.
        #[serde(default)]
        profiles: Vec<usize>,
        distance: Expr,
        #[serde(default)]
        direction: ExtentDirection,
        #[serde(default)]
        operation: Operation,
    },
    Revolve {
        sketch: Id,
        #[serde(default)]
        profiles: Vec<usize>,
        #[serde(default)]
        axis: Axis,
        angle: Expr,
        #[serde(default)]
        operation: Operation,
    },
    Box {
        /// Centre of the bottom face.
        #[serde(default)]
        base: Vec3,
        width: Expr,
        depth: Expr,
        height: Expr,
        #[serde(default)]
        operation: Operation,
    },
    Cylinder {
        #[serde(default)]
        base: Vec3,
        radius: Expr,
        height: Expr,
        #[serde(default)]
        operation: Operation,
    },
    Sphere {
        #[serde(default)]
        center: Vec3,
        radius: Expr,
        #[serde(default)]
        operation: Operation,
    },
    Torus {
        #[serde(default)]
        center: Vec3,
        major: Expr,
        minor: Expr,
        #[serde(default)]
        operation: Operation,
    },
    Cone {
        #[serde(default)]
        base: Vec3,
        radius: Expr,
        #[serde(default = "zero")]
        top_radius: Expr,
        height: Expr,
        #[serde(default)]
        operation: Operation,
    },
    /// Mirror every body across an origin plane and join the copies.
    Mirror {
        plane: PlaneRef,
    },
    /// Copies of every body along X and Y, joined.
    PatternRect {
        count_x: u32,
        count_y: u32,
        spacing_x: Expr,
        spacing_y: Expr,
    },
    /// Copies of every body around a world axis, joined.
    PatternCirc {
        count: u32,
        #[serde(default = "full_turn")]
        angle: Expr,
        #[serde(default)]
        axis: WorldAxis,
    },
    /// Move every body.
    Move {
        dx: Expr,
        dy: Expr,
        dz: Expr,
    },
    /// Hollow every body to a wall thickness (inward offset, approximated by scaling about the
    /// body's centre; exact for boxes and spheres).
    Shell {
        thickness: Expr,
    },
}

fn zero() -> Expr {
    Expr::new("0")
}
fn full_turn() -> Expr {
    Expr::new("360 deg")
}

impl FeatureKind {
    pub fn label(&self) -> &'static str {
        match self {
            FeatureKind::Sketch { .. } => "Sketch",
            FeatureKind::Extrude { .. } => "Extrude",
            FeatureKind::Revolve { .. } => "Revolve",
            FeatureKind::Box { .. } => "Box",
            FeatureKind::Cylinder { .. } => "Cylinder",
            FeatureKind::Sphere { .. } => "Sphere",
            FeatureKind::Torus { .. } => "Torus",
            FeatureKind::Cone { .. } => "Cone",
            FeatureKind::Mirror { .. } => "Mirror",
            FeatureKind::PatternRect { .. } => "RectangularPattern",
            FeatureKind::PatternCirc { .. } => "CircularPattern",
            FeatureKind::Move { .. } => "Move",
            FeatureKind::Shell { .. } => "Shell",
        }
    }
    pub fn sketch_id(&self) -> Option<Id> {
        match self {
            FeatureKind::Sketch { sketch } | FeatureKind::Extrude { sketch, .. } | FeatureKind::Revolve { sketch, .. } => Some(*sketch),
            _ => None,
        }
    }
    pub fn operation_mut(&mut self) -> Option<&mut Operation> {
        match self {
            FeatureKind::Extrude { operation, .. }
            | FeatureKind::Revolve { operation, .. }
            | FeatureKind::Box { operation, .. }
            | FeatureKind::Cylinder { operation, .. }
            | FeatureKind::Sphere { operation, .. }
            | FeatureKind::Torus { operation, .. }
            | FeatureKind::Cone { operation, .. } => Some(operation),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    pub kind: FeatureKind,
}
