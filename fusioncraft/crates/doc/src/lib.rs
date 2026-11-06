//! The FusionCraft document (layer L1): user parameters, sketches and the parametric feature
//! timeline. A document is plain data; turning it into bodies is the job of `fusioncraft-model`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

pub mod expr;
pub mod feature;
pub mod profile;
pub mod sketch;

pub use expr::{Expr, ExprError, Params};
pub use feature::{Axis, ExtentDirection, Feature, FeatureKind, Operation, PlaneRef};
pub use profile::Profile;
pub use sketch::{Entity, Sketch, SketchPlane};

use serde::{Deserialize, Serialize};

pub type Id = u64;

/// File format version written into every saved document.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub expr: Expr,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "default_format")]
    pub format: u32,
    pub name: String,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    #[serde(default)]
    pub sketches: Vec<Sketch>,
    #[serde(default)]
    pub timeline: Vec<Feature>,
    /// Features at index `>= marker` are rolled back (not evaluated). `None` = end.
    #[serde(default)]
    pub marker: Option<usize>,
    #[serde(default = "first_id")]
    pub next_id: Id,
}

fn default_format() -> u32 {
    FORMAT_VERSION
}
fn first_id() -> Id {
    1
}

impl Default for Document {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl Document {
    pub fn new(name: &str) -> Self {
        Self {
            format: FORMAT_VERSION,
            name: name.to_owned(),
            parameters: Vec::new(),
            sketches: Vec::new(),
            timeline: Vec::new(),
            marker: None,
            next_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn sketch(&self, id: Id) -> Option<&Sketch> {
        self.sketches.iter().find(|s| s.id == id)
    }
    pub fn sketch_mut(&mut self, id: Id) -> Option<&mut Sketch> {
        self.sketches.iter_mut().find(|s| s.id == id)
    }
    pub fn feature(&self, id: Id) -> Option<&Feature> {
        self.timeline.iter().find(|f| f.id == id)
    }
    pub fn feature_mut(&mut self, id: Id) -> Option<&mut Feature> {
        self.timeline.iter_mut().find(|f| f.id == id)
    }
    pub fn feature_index(&self, id: Id) -> Option<usize> {
        self.timeline.iter().position(|f| f.id == id)
    }
    /// Number of active timeline entries (before the marker).
    pub fn active_len(&self) -> usize {
        self.marker.map_or(self.timeline.len(), |m| m.min(self.timeline.len()))
    }

    /// Next default name such as "Extrude3" for a kind label.
    pub fn next_name(&self, base: &str) -> String {
        let used = |n: &str| self.timeline.iter().any(|f| f.name == n) || self.sketches.iter().any(|s| s.name == n);
        (1..).map(|i| format!("{base}{i}")).find(|n| !used(n)).unwrap_or_else(|| base.to_owned())
    }

    /// Insert a feature at the marker position (like Fusion: new features go where the marker
    /// is) and advance the marker past it.
    pub fn insert_feature(&mut self, f: Feature) -> Id {
        let id = f.id;
        let at = self.active_len();
        self.timeline.insert(at, f);
        self.marker = match self.marker {
            Some(m) if m + 1 < self.timeline.len() => Some(m + 1),
            _ => None,
        };
        id
    }

    /// The evaluated parameter table.
    pub fn params(&self) -> Params {
        let mut p = Params::default();
        for param in &self.parameters {
            p.define(&param.name, param.expr.clone());
        }
        p
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(text: &str) -> Result<Self, DocError> {
        let doc: Self = serde_json::from_str(text).map_err(|e| DocError::Parse(e.to_string()))?;
        if doc.format > FORMAT_VERSION {
            return Err(DocError::NewerFormat(doc.format));
        }
        doc.validate()?;
        Ok(doc)
    }

    /// Hostile-input checks after loading.
    pub fn validate(&self) -> Result<(), DocError> {
        const MAX_ITEMS: usize = 100_000;
        if self.sketches.len() > MAX_ITEMS || self.timeline.len() > MAX_ITEMS || self.parameters.len() > MAX_ITEMS {
            return Err(DocError::TooLarge);
        }
        if self.sketches.iter().any(|s| s.entities.len() > MAX_ITEMS) {
            return Err(DocError::TooLarge);
        }
        let max_id = self
            .sketches
            .iter()
            .map(|s| s.id)
            .chain(self.timeline.iter().map(|f| f.id))
            .max()
            .unwrap_or(0);
        if max_id >= self.next_id {
            return Err(DocError::Parse("next_id is not above every id".into()));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum DocError {
    #[error("the file is not a FusionCraft document: {0}")]
    Parse(String),
    #[error("the file was written by a newer FusionCraft (format {0})")]
    NewerFormat(u32),
    #[error("the document is too large")]
    TooLarge,
}
