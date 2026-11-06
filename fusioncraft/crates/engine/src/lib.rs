//! The engine (layer L4). **Everything is a command**: the toolbar, menus, shortcuts, the
//! command-line tool and scripts all call [`Engine::execute`] with a command id and JSON
//! parameters. Commands that change the document are undoable; a failing or panicking command
//! leaves the document exactly as it was.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

mod commands;
pub mod params;

pub use commands::{CommandSpec, COMMANDS};

use fusioncraft_doc::{Document, Id};
use fusioncraft_model::Model;
use serde_json::Value;
use std::path::PathBuf;

pub const MAX_UNDO: usize = 200;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum EngineError {
    #[error("unknown command '{0}'")]
    UnknownCommand(String),
    #[error("{0}")]
    BadParams(String),
    #[error("{0}")]
    Failed(String),
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

pub type CmdResult = Result<Value, EngineError>;

pub struct Engine {
    pub doc: Document,
    pub model: Model,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// The sketch new sketch commands go to when none is named.
    pub active_sketch: Option<Id>,
    undo: Vec<(Document, String)>,
    redo: Vec<(Document, String)>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self::with_document(Document::new("Untitled"))
    }

    pub fn with_document(doc: Document) -> Self {
        let model = fusioncraft_model::evaluate(&doc);
        Self { doc, model, path: None, dirty: false, active_sketch: None, undo: Vec::new(), redo: Vec::new() }
    }

    pub fn spec(id: &str) -> Option<&'static CommandSpec> {
        COMMANDS.iter().find(|c| c.id == id)
    }

    /// Run a command. Changes are recorded for undo and the model is recomputed.
    pub fn execute(&mut self, id: &str, params: &Value) -> CmdResult {
        let spec = Self::spec(id).ok_or_else(|| EngineError::UnknownCommand(id.to_owned()))?;
        let before = self.doc.clone();
        let before_sketch = self.active_sketch;
        let run = spec.run;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(self, params)));
        let result = match outcome {
            Ok(r) => r,
            Err(_) => Err(EngineError::Failed(format!("internal error in '{id}'; nothing was changed"))),
        };
        match result {
            Ok(v) => {
                if spec.undoable && self.doc != before {
                    self.undo.push((before, spec.label.to_owned()));
                    if self.undo.len() > MAX_UNDO {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                    self.dirty = true;
                    self.recompute();
                }
                Ok(v)
            }
            Err(e) => {
                if spec.undoable {
                    self.doc = before;
                    self.active_sketch = before_sketch;
                }
                Err(e)
            }
        }
    }

    pub fn recompute(&mut self) {
        self.model = fusioncraft_model::evaluate(&self.doc);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(_, l)| l.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(_, l)| l.as_str())
    }

    fn undo(&mut self) -> CmdResult {
        let (doc, label) = self.undo.pop().ok_or(EngineError::NothingToUndo)?;
        let current = std::mem::replace(&mut self.doc, doc);
        self.redo.push((current, label.clone()));
        self.after_history();
        Ok(serde_json::json!({ "undone": label }))
    }

    fn redo(&mut self) -> CmdResult {
        let (doc, label) = self.redo.pop().ok_or(EngineError::NothingToRedo)?;
        let current = std::mem::replace(&mut self.doc, doc);
        self.undo.push((current, label.clone()));
        self.after_history();
        Ok(serde_json::json!({ "redone": label }))
    }

    fn after_history(&mut self) {
        if self.active_sketch.is_some_and(|s| self.doc.sketch(s).is_none()) {
            self.active_sketch = None;
        }
        self.dirty = true;
        self.recompute();
    }

    fn reset(&mut self, doc: Document, path: Option<PathBuf>) {
        self.doc = doc;
        self.path = path;
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
        self.active_sketch = None;
        self.recompute();
    }

    /// The display title: document name plus a star when unsaved.
    pub fn title(&self) -> String {
        format!("{}{}", self.doc.name, if self.dirty { " *" } else { "" })
    }
}
