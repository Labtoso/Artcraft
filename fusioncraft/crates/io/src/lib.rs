//! Files (layer L3): FusionCraft documents (`.fcraft`, JSON) and mesh export (STL, OBJ).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use fusioncraft_doc::{DocError, Document};
use fusioncraft_geom::Mesh;
use fusioncraft_model::Model;
use std::io::Write;
use std::path::Path;

pub const EXTENSION: &str = "fcraft";
/// Refuse documents larger than this when opening.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Doc(#[from] DocError),
    #[error("could not write the document: {0}")]
    Serialize(String),
    #[error("the file is too large")]
    TooLarge,
    #[error("there is nothing to export")]
    Empty,
}

pub fn open(path: &Path) -> Result<Document, IoError> {
    if std::fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(IoError::TooLarge);
    }
    let text = std::fs::read_to_string(path)?;
    Ok(Document::from_json(&text)?)
}

/// Write via a temporary file in the same folder and rename, so a failed save never destroys
/// the previous version.
pub fn save(doc: &Document, path: &Path) -> Result<(), IoError> {
    let text = doc.to_json().map_err(|e| IoError::Serialize(e.to_string()))?;
    write_atomic(path, text.as_bytes())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), IoError> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
    let tmp = dir.join(format!(".{name}.tmp{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

fn all_bodies(model: &Model) -> Mesh {
    let mut m = Mesh::default();
    for b in &model.bodies {
        m.append(&b.mesh);
    }
    m
}

/// Binary STL of every body.
pub fn stl_binary(mesh: &Mesh, name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(84 + mesh.tris.len() * 50);
    let mut header = [0u8; 80];
    let title = format!("FusionCraft {name}");
    for (h, b) in header.iter_mut().zip(title.bytes()) {
        *h = b;
    }
    out.extend_from_slice(&header);
    out.extend_from_slice(&u32::try_from(mesh.tris.len()).unwrap_or(u32::MAX).to_le_bytes());
    for t in &mesh.tris {
        let n = t.normal();
        for v in [n, t.v[0], t.v[1], t.v[2]] {
            for c in v.to_array() {
                out.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

pub fn stl_ascii(mesh: &Mesh, name: &str) -> String {
    let mut s = String::with_capacity(mesh.tris.len() * 256);
    let name: String = name.chars().filter(|c| c.is_ascii_graphic()).collect();
    s.push_str(&format!("solid {name}\n"));
    for t in &mesh.tris {
        let n = t.normal();
        s.push_str(&format!("  facet normal {:e} {:e} {:e}\n    outer loop\n", n.x, n.y, n.z));
        for v in t.v {
            s.push_str(&format!("      vertex {:e} {:e} {:e}\n", v.x, v.y, v.z));
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    s.push_str(&format!("endsolid {name}\n"));
    s
}

/// Wavefront OBJ with one object per body and shared vertices.
pub fn obj(model: &Model) -> String {
    let mut s = String::from("# FusionCraft export, units: mm\n");
    let mut base = 1usize;
    for b in &model.bodies {
        s.push_str(&format!("o {}\n", b.name.replace(char::is_whitespace, "_")));
        let mut index = std::collections::HashMap::new();
        let mut verts = Vec::new();
        let mut faces = Vec::new();
        for t in &b.mesh.tris {
            let mut f = [0usize; 3];
            for (k, v) in t.v.iter().enumerate() {
                let key = (v.x.to_bits(), v.y.to_bits(), v.z.to_bits());
                let i = *index.entry(key).or_insert_with(|| {
                    verts.push(*v);
                    verts.len() - 1
                });
                if let Some(slot) = f.get_mut(k) {
                    *slot = i;
                }
            }
            faces.push(f);
        }
        for v in &verts {
            s.push_str(&format!("v {} {} {}\n", v.x, v.y, v.z));
        }
        for f in faces {
            s.push_str(&format!("f {} {} {}\n", f[0] + base, f[1] + base, f[2] + base));
        }
        base += verts.len();
    }
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    StlBinary,
    StlAscii,
    Obj,
}

impl ExportFormat {
    pub fn from_path(path: &Path) -> Self {
        match path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
            Some("obj") => ExportFormat::Obj,
            _ => ExportFormat::StlBinary,
        }
    }
}

pub fn export(model: &Model, path: &Path, format: ExportFormat, name: &str) -> Result<(), IoError> {
    if model.bodies.is_empty() {
        return Err(IoError::Empty);
    }
    let bytes = match format {
        ExportFormat::StlBinary => stl_binary(&all_bodies(model), name),
        ExportFormat::StlAscii => stl_ascii(&all_bodies(model), name).into_bytes(),
        ExportFormat::Obj => obj(model).into_bytes(),
    };
    write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusioncraft_geom::{solid, Vec3};

    #[test]
    fn stl_sizes_and_roundtrip() {
        let m = solid::cuboid(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(stl_binary(&m, "x").len(), 84 + 12 * 50);
        assert_eq!(stl_ascii(&m, "x").matches("facet normal").count(), 12);
        let dir = std::env::temp_dir().join(format!("fcraft-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("a.fcraft");
        let d = Document::new("A");
        assert!(save(&d, &p).is_ok());
        assert_eq!(open(&p).ok(), Some(d));
        let _ = std::fs::write(dir.join("bad.fcraft"), "{ not json");
        assert!(open(&dir.join("bad.fcraft")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
