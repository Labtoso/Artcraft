//! The command table. Ids are `area.action`; labels follow Fusion's wording.

use crate::params::{
    bool_or, expr, expr_or, get, index_list, num, num_or, str_opt, str_req, u32_or, u64_req, vec2, vec3_or,
};
use crate::{CmdResult, Engine, EngineError};
use fusioncraft_doc::{
    Axis, Document, Entity, Expr, ExtentDirection, Feature, FeatureKind, Id, Operation, Parameter, PlaneRef, Sketch,
    SketchPlane,
};
use fusioncraft_doc::feature::WorldAxis;
use fusioncraft_geom::{Plane, Vec3};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Menu or toolbar group, e.g. "File", "Create", "Modify".
    pub group: &'static str,
    pub shortcut: Option<&'static str>,
    /// Short description of the JSON parameters.
    pub params: &'static str,
    /// Changes the document (recorded for undo).
    pub undoable: bool,
    pub run: fn(&mut Engine, &Value) -> CmdResult,
}

macro_rules! cmd {
    ($id:literal, $label:literal, $group:literal, $short:expr, $params:literal, $undo:literal, $run:expr) => {
        CommandSpec { id: $id, label: $label, group: $group, shortcut: $short, params: $params, undoable: $undo, run: $run }
    };
}

pub static COMMANDS: &[CommandSpec] = &[
    cmd!("file.new", "New Design", "File", Some("Ctrl+N"), "{name?}", false, file_new),
    cmd!("file.open", "Open", "File", Some("Ctrl+O"), "{path}", false, file_open),
    cmd!("file.save", "Save", "File", Some("Ctrl+S"), "{path?}", false, file_save),
    cmd!("file.export", "Export", "File", Some("Ctrl+E"), "{path, format?: stl|stl_ascii|obj}", false, file_export),
    cmd!("edit.undo", "Undo", "Edit", Some("Ctrl+Z"), "{}", false, |e, _| e.undo()),
    cmd!("edit.redo", "Redo", "Edit", Some("Ctrl+Y"), "{}", false, |e, _| e.redo()),
    cmd!("doc.rename", "Rename Design", "File", None, "{name}", true, doc_rename),
    cmd!("sketch.create", "Create Sketch", "Create", Some("S"), "{plane: xy|xz|yz} or {origin, normal, x_axis}", true, sketch_create),
    cmd!("sketch.line", "Line", "Sketch", Some("L"), "{sketch?, a:[x,y], b:[x,y], construction?}", true, sketch_line),
    cmd!("sketch.rect", "2-Point Rectangle", "Sketch", Some("R"), "{sketch?, a, b}", true, sketch_rect),
    cmd!("sketch.circle", "Center Diameter Circle", "Sketch", Some("C"), "{sketch?, center, radius | diameter}", true, sketch_circle),
    cmd!("sketch.polygon", "Polygon", "Sketch", None, "{sketch?, center, radius, sides?, rotation?}", true, sketch_polygon),
    cmd!("sketch.arc", "3-Point Arc", "Sketch", None, "{sketch?, start, mid, end}", true, sketch_arc),
    cmd!("sketch.point", "Point", "Sketch", None, "{sketch?, p}", true, sketch_point),
    cmd!("sketch.delete_entity", "Delete Sketch Curve", "Sketch", Some("Delete"), "{sketch?, index}", true, sketch_delete_entity),
    cmd!("sketch.edit_entity", "Edit Sketch Curve", "Sketch", None, "{sketch?, index, entity}", true, sketch_edit_entity),
    cmd!("sketch.show", "Show/Hide Sketch", "Browser", None, "{sketch, visible?}", true, sketch_show),
    cmd!("feature.extrude", "Extrude", "Create", Some("E"), "{sketch?, profiles?:[i], distance, symmetric?, operation?: join|cut|intersect|new_body}", true, feature_extrude),
    cmd!("feature.revolve", "Revolve", "Create", None, "{sketch?, profiles?, axis?: x|y|{line:i}, angle?, operation?}", true, feature_revolve),
    cmd!("feature.box", "Box", "Create", None, "{width, depth, height, base?:[x,y,z], operation?}", true, feature_box),
    cmd!("feature.cylinder", "Cylinder", "Create", None, "{radius | diameter, height, base?, operation?}", true, feature_cylinder),
    cmd!("feature.sphere", "Sphere", "Create", None, "{radius | diameter, center?, operation?}", true, feature_sphere),
    cmd!("feature.torus", "Torus", "Create", None, "{major, minor, center?, operation?}", true, feature_torus),
    cmd!("feature.cone", "Cone", "Create", None, "{radius, top_radius?, height, base?, operation?}", true, feature_cone),
    cmd!("feature.mirror", "Mirror", "Create", None, "{plane: xy|xz|yz}", true, feature_mirror),
    cmd!("feature.pattern_rect", "Rectangular Pattern", "Create", None, "{count_x, count_y?, spacing_x, spacing_y?}", true, feature_pattern_rect),
    cmd!("feature.pattern_circ", "Circular Pattern", "Create", None, "{count, angle?, axis?: x|y|z}", true, feature_pattern_circ),
    cmd!("feature.move", "Move/Copy", "Modify", Some("M"), "{dx?, dy?, dz?}", true, feature_move),
    cmd!("feature.shell", "Shell", "Modify", None, "{thickness}", true, feature_shell),
    cmd!("feature.add", "Add Feature", "Create", None, "{kind: {type, ...}}", true, feature_add),
    cmd!("feature.edit", "Edit Feature", "Timeline", None, "{id, set: {field: value}} or {id, kind}", true, feature_edit),
    cmd!("feature.delete", "Delete", "Timeline", None, "{id}", true, feature_delete),
    cmd!("feature.suppress", "Suppress Features", "Timeline", None, "{id, suppressed?}", true, feature_suppress),
    cmd!("feature.rename", "Rename", "Timeline", None, "{id, name}", true, feature_rename),
    cmd!("timeline.marker", "Move to Position", "Timeline", None, "{position: n | \"end\"}", true, timeline_marker),
    cmd!("param.set", "Change Parameters", "Modify", None, "{name, expr, comment?}", true, param_set),
    cmd!("param.delete", "Delete Parameter", "Modify", None, "{name}", true, param_delete),
    cmd!("model.info", "Properties", "Inspect", None, "{}", false, model_info),
    cmd!("sketch.profiles", "List Profiles", "Inspect", None, "{sketch?}", false, sketch_profiles),
    cmd!("help.commands", "Commands", "Help", None, "{}", false, help_commands),
];

fn failed(msg: impl Into<String>) -> EngineError {
    EngineError::Failed(msg.into())
}

// ---------------------------------------------------------------- file

fn file_new(e: &mut Engine, p: &Value) -> CmdResult {
    e.reset(Document::new(str_opt(p, "name").unwrap_or("Untitled")), None);
    Ok(json!({}))
}

fn file_open(e: &mut Engine, p: &Value) -> CmdResult {
    let path = PathBuf::from(str_req(p, "path")?);
    let doc = fusioncraft_io::open(&path).map_err(|err| failed(format!("Could not open {}: {err}", path.display())))?;
    e.reset(doc, Some(path));
    Ok(json!({ "bodies": e.model.bodies.len() }))
}

fn file_save(e: &mut Engine, p: &Value) -> CmdResult {
    let path = match str_opt(p, "path") {
        Some(s) => PathBuf::from(s),
        None => e.path.clone().ok_or_else(|| EngineError::BadParams("missing 'path' (the design was never saved)".into()))?,
    };
    let path = if path.extension().is_none() { path.with_extension(fusioncraft_io::EXTENSION) } else { path };
    if e.doc.name == "Untitled"
        && let Some(stem) = path.file_stem() {
            e.doc.name = stem.to_string_lossy().into_owned();
        }
    fusioncraft_io::save(&e.doc, &path).map_err(|err| failed(format!("Could not save: {err}")))?;
    e.path = Some(path.clone());
    e.dirty = false;
    Ok(json!({ "path": path.display().to_string() }))
}

fn file_export(e: &mut Engine, p: &Value) -> CmdResult {
    let path = PathBuf::from(str_req(p, "path")?);
    let format = match str_opt(p, "format") {
        Some("stl") => fusioncraft_io::ExportFormat::StlBinary,
        Some("stl_ascii") => fusioncraft_io::ExportFormat::StlAscii,
        Some("obj") => fusioncraft_io::ExportFormat::Obj,
        Some(other) => return Err(EngineError::BadParams(format!("unknown format '{other}'"))),
        None => fusioncraft_io::ExportFormat::from_path(&path),
    };
    fusioncraft_io::export(&e.model, &path, format, &e.doc.name).map_err(|err| failed(format!("Could not export: {err}")))?;
    Ok(json!({ "path": path.display().to_string(), "triangles": e.model.bodies.iter().map(|b| b.mesh.tris.len()).sum::<usize>() }))
}

fn doc_rename(e: &mut Engine, p: &Value) -> CmdResult {
    let name = str_req(p, "name")?.trim();
    if name.is_empty() {
        return Err(EngineError::BadParams("the name is empty".into()));
    }
    e.doc.name = name.to_owned();
    Ok(json!({}))
}

// ---------------------------------------------------------------- sketches

fn sketch_create(e: &mut Engine, p: &Value) -> CmdResult {
    let plane = if let Some(name) = str_opt(p, "plane") {
        SketchPlane::Origin { plane: PlaneRef::parse(name).ok_or_else(|| EngineError::BadParams(format!("unknown plane '{name}'")))? }
    } else if get(p, "normal").is_some() {
        let origin = vec3_or(p, "origin", Vec3::ZERO)?;
        let normal = vec3_or(p, "normal", Vec3::Z)?;
        let x = vec3_or(p, "x_axis", normal.any_perpendicular())?;
        let plane = Plane::new(origin, normal, x);
        if !plane.is_valid() || normal.length() < 1e-9 {
            return Err(EngineError::BadParams("the plane normal is zero".into()));
        }
        SketchPlane::Custom { plane }
    } else {
        SketchPlane::Origin { plane: PlaneRef::Xy }
    };
    let sid = e.doc.alloc_id();
    let name = e.doc.next_name("Sketch");
    e.doc.sketches.push(Sketch::new(sid, name.clone(), plane));
    let fid = e.doc.alloc_id();
    e.doc.insert_feature(Feature { id: fid, name, suppressed: false, kind: FeatureKind::Sketch { sketch: sid } });
    e.active_sketch = Some(sid);
    Ok(json!({ "sketch": sid, "feature": fid }))
}

fn sketch_id(e: &Engine, p: &Value) -> Result<Id, EngineError> {
    let id = match get(p, "sketch") {
        Some(_) => u64_req(p, "sketch")?,
        None => e
            .active_sketch
            .or_else(|| e.doc.sketches.last().map(|s| s.id))
            .ok_or_else(|| EngineError::BadParams("there is no sketch; run sketch.create first".into()))?,
    };
    if e.doc.sketch(id).is_none() {
        return Err(EngineError::BadParams(format!("there is no sketch {id}")));
    }
    Ok(id)
}

const MAX_ENTITIES: usize = 50_000;

fn add_entity(e: &mut Engine, p: &Value, ent: Entity) -> CmdResult {
    if !ent.is_finite() {
        return Err(EngineError::BadParams("the curve has invalid coordinates".into()));
    }
    let id = sketch_id(e, p)?;
    let s = e.doc.sketch_mut(id).ok_or_else(|| failed("the sketch is missing"))?;
    if s.entities.len() >= MAX_ENTITIES {
        return Err(failed("the sketch has too many curves"));
    }
    s.entities.push(ent);
    Ok(json!({ "sketch": id, "index": s.entities.len() - 1 }))
}

fn positive(v: f64, what: &str) -> Result<f64, EngineError> {
    if v > 1e-9 { Ok(v) } else { Err(EngineError::BadParams(format!("{what} must be greater than 0"))) }
}

fn sketch_line(e: &mut Engine, p: &Value) -> CmdResult {
    let (a, b) = (vec2(p, "a")?, vec2(p, "b")?);
    if a.distance(b) < 1e-9 {
        return Err(EngineError::BadParams("the line has no length".into()));
    }
    add_entity(e, p, Entity::Line { a, b, construction: bool_or(p, "construction", false) })
}

fn sketch_rect(e: &mut Engine, p: &Value) -> CmdResult {
    let (a, b) = (vec2(p, "a")?, vec2(p, "b")?);
    if (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9 {
        return Err(EngineError::BadParams("the rectangle has no area".into()));
    }
    add_entity(e, p, Entity::Rect { a, b })
}

/// A number, or an expression evaluated now with the design's parameters.
fn value(e: &Engine, p: &Value, key: &str) -> Result<f64, EngineError> {
    match get(p, key) {
        Some(Value::String(s)) => e.doc.params().eval(&Expr::new(s.clone())).map_err(|err| EngineError::BadParams(format!("'{key}': {err}"))),
        _ => num(p, key),
    }
}

fn radius_param(e: &Engine, p: &Value) -> Result<f64, EngineError> {
    let r = if get(p, "diameter").is_some() { value(e, p, "diameter")? / 2.0 } else { value(e, p, "radius")? };
    positive(r, "radius")
}

fn sketch_circle(e: &mut Engine, p: &Value) -> CmdResult {
    let center = vec2(p, "center")?;
    let radius = radius_param(e, p)?;
    add_entity(e, p, Entity::Circle { center, radius })
}

fn sketch_polygon(e: &mut Engine, p: &Value) -> CmdResult {
    let center = vec2(p, "center")?;
    let radius = positive(value(e, p, "radius")?, "radius")?;
    let sides = u32_or(p, "sides", 6, 1024)?.max(3);
    add_entity(e, p, Entity::Polygon { center, radius, sides, rotation: num_or(p, "rotation", 0.0)? })
}

fn sketch_arc(e: &mut Engine, p: &Value) -> CmdResult {
    add_entity(e, p, Entity::Arc { start: vec2(p, "start")?, mid: vec2(p, "mid")?, end: vec2(p, "end")? })
}

fn sketch_point(e: &mut Engine, p: &Value) -> CmdResult {
    add_entity(e, p, Entity::Point { p: vec2(p, "p")? })
}

fn entity_index(e: &Engine, p: &Value, sketch: Id) -> Result<usize, EngineError> {
    let i = usize::try_from(u64_req(p, "index")?).map_err(|_| EngineError::BadParams("bad index".into()))?;
    let n = e.doc.sketch(sketch).map_or(0, |s| s.entities.len());
    if i >= n {
        return Err(EngineError::BadParams(format!("there is no curve {i}")));
    }
    Ok(i)
}

fn sketch_delete_entity(e: &mut Engine, p: &Value) -> CmdResult {
    let id = sketch_id(e, p)?;
    let i = entity_index(e, p, id)?;
    if let Some(s) = e.doc.sketch_mut(id) {
        s.entities.remove(i);
    }
    Ok(json!({}))
}

fn sketch_edit_entity(e: &mut Engine, p: &Value) -> CmdResult {
    let id = sketch_id(e, p)?;
    let i = entity_index(e, p, id)?;
    let ent: Entity = serde_json::from_value(get(p, "entity").cloned().unwrap_or(Value::Null))
        .map_err(|err| EngineError::BadParams(format!("'entity': {err}")))?;
    if !ent.is_finite() {
        return Err(EngineError::BadParams("the curve has invalid coordinates".into()));
    }
    if let Some(slot) = e.doc.sketch_mut(id).and_then(|s| s.entities.get_mut(i)) {
        *slot = ent;
    }
    Ok(json!({}))
}

fn sketch_show(e: &mut Engine, p: &Value) -> CmdResult {
    let id = u64_req(p, "sketch")?;
    let s = e.doc.sketch_mut(id).ok_or_else(|| EngineError::BadParams(format!("there is no sketch {id}")))?;
    s.visible = bool_or(p, "visible", !s.visible);
    Ok(json!({ "visible": s.visible }))
}

fn sketch_profiles(e: &mut Engine, p: &Value) -> CmdResult {
    let id = sketch_id(e, p)?;
    let profs = fusioncraft_model::sketch_profiles(&e.doc, id);
    Ok(json!({ "profiles": profs.iter().enumerate().map(|(i, pr)| json!({"index": i, "area": pr.area(), "holes": pr.holes.len()})).collect::<Vec<_>>() }))
}

// ---------------------------------------------------------------- features

fn operation(p: &Value, default: Operation) -> Result<Operation, EngineError> {
    match str_opt(p, "operation") {
        None => Ok(default),
        Some(s) => match s.to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
            "join" => Ok(Operation::Join),
            "cut" => Ok(Operation::Cut),
            "intersect" => Ok(Operation::Intersect),
            "new_body" | "new" => Ok(Operation::NewBody),
            other => Err(EngineError::BadParams(format!("unknown operation '{other}'"))),
        },
    }
}

fn add_feature(e: &mut Engine, kind: FeatureKind) -> CmdResult {
    let id = e.doc.alloc_id();
    let name = e.doc.next_name(kind.label());
    e.doc.insert_feature(Feature { id, name: name.clone(), suppressed: false, kind });
    Ok(json!({ "feature": id, "name": name }))
}

/// After a feature command, report whether it evaluated cleanly.
fn checked(e: &mut Engine, r: CmdResult) -> CmdResult {
    let v = r?;
    let id = v.get("feature").and_then(Value::as_u64);
    let model = fusioncraft_model::evaluate(&e.doc);
    let err = id.and_then(|id| model.errors.get(&id).cloned());
    e.model = model;
    match err {
        Some(msg) => Err(failed(msg)),
        None => Ok(v),
    }
}

fn feature_extrude(e: &mut Engine, p: &Value) -> CmdResult {
    let sketch = sketch_id(e, p)?;
    let kind = FeatureKind::Extrude {
        sketch,
        profiles: index_list(p, "profiles")?,
        distance: expr(p, "distance")?,
        direction: if bool_or(p, "symmetric", false) { ExtentDirection::Symmetric } else { ExtentDirection::OneSide },
        operation: operation(p, Operation::Join)?,
    };
    let r = add_feature(e, kind);
    if r.is_ok() {
        e.active_sketch = None;
    }
    checked(e, r)
}

fn feature_revolve(e: &mut Engine, p: &Value) -> CmdResult {
    let sketch = sketch_id(e, p)?;
    let axis = match get(p, "axis") {
        None => Axis::SketchY,
        Some(Value::String(s)) if s.eq_ignore_ascii_case("x") => Axis::SketchX,
        Some(Value::String(s)) if s.eq_ignore_ascii_case("y") => Axis::SketchY,
        Some(v) => Axis::Line {
            index: v.get("line").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).ok_or_else(|| EngineError::BadParams("'axis' must be x, y or {line: i}".into()))?,
        },
    };
    let kind = FeatureKind::Revolve {
        sketch,
        profiles: index_list(p, "profiles")?,
        axis,
        angle: expr_or(p, "angle", "360 deg")?,
        operation: operation(p, Operation::Join)?,
    };
    let r = add_feature(e, kind);
    if r.is_ok() {
        e.active_sketch = None;
    }
    checked(e, r)
}

fn radius_expr(p: &Value) -> Result<Expr, EngineError> {
    if get(p, "diameter").is_some() {
        let d = expr(p, "diameter")?;
        Ok(Expr::new(format!("({}) / 2", d.as_str())))
    } else {
        expr(p, "radius")
    }
}

fn feature_box(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::Box {
        base: vec3_or(p, "base", Vec3::ZERO)?,
        width: expr(p, "width")?,
        depth: expr(p, "depth")?,
        height: expr(p, "height")?,
        operation: operation(p, Operation::NewBody)?,
    };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_cylinder(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::Cylinder { base: vec3_or(p, "base", Vec3::ZERO)?, radius: radius_expr(p)?, height: expr(p, "height")?, operation: operation(p, Operation::NewBody)? };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_sphere(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::Sphere { center: vec3_or(p, "center", Vec3::ZERO)?, radius: radius_expr(p)?, operation: operation(p, Operation::NewBody)? };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_torus(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::Torus { center: vec3_or(p, "center", Vec3::ZERO)?, major: expr(p, "major")?, minor: expr(p, "minor")?, operation: operation(p, Operation::NewBody)? };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_cone(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::Cone {
        base: vec3_or(p, "base", Vec3::ZERO)?,
        radius: radius_expr(p)?,
        top_radius: expr_or(p, "top_radius", "0")?,
        height: expr(p, "height")?,
        operation: operation(p, Operation::NewBody)?,
    };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_mirror(e: &mut Engine, p: &Value) -> CmdResult {
    let name = str_opt(p, "plane").unwrap_or("yz");
    let plane = PlaneRef::parse(name).ok_or_else(|| EngineError::BadParams(format!("unknown plane '{name}'")))?;
    let r = add_feature(e, FeatureKind::Mirror { plane });
    checked(e, r)
}

fn feature_pattern_rect(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::PatternRect {
        count_x: u32_or(p, "count_x", 2, fusioncraft_model::MAX_COPIES)?,
        count_y: u32_or(p, "count_y", 1, fusioncraft_model::MAX_COPIES)?,
        spacing_x: expr_or(p, "spacing_x", "10")?,
        spacing_y: expr_or(p, "spacing_y", "10")?,
    };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_pattern_circ(e: &mut Engine, p: &Value) -> CmdResult {
    let axis = match str_opt(p, "axis").map(str::to_ascii_lowercase).as_deref() {
        Some("x") => WorldAxis::X,
        Some("y") => WorldAxis::Y,
        None | Some("z") => WorldAxis::Z,
        Some(other) => return Err(EngineError::BadParams(format!("unknown axis '{other}'"))),
    };
    let kind = FeatureKind::PatternCirc { count: u32_or(p, "count", 4, fusioncraft_model::MAX_COPIES)?, angle: expr_or(p, "angle", "360 deg")?, axis };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_move(e: &mut Engine, p: &Value) -> CmdResult {
    let kind = FeatureKind::Move { dx: expr_or(p, "dx", "0")?, dy: expr_or(p, "dy", "0")?, dz: expr_or(p, "dz", "0")? };
    let r = add_feature(e, kind);
    checked(e, r)
}

fn feature_shell(e: &mut Engine, p: &Value) -> CmdResult {
    let r = add_feature(e, FeatureKind::Shell { thickness: expr(p, "thickness")? });
    checked(e, r)
}

fn feature_id(e: &Engine, p: &Value) -> Result<Id, EngineError> {
    let id = u64_req(p, "id")?;
    if e.doc.feature(id).is_none() {
        return Err(EngineError::BadParams(format!("there is no feature {id}")));
    }
    Ok(id)
}

/// Add any feature from its full JSON description (used by the feature dialogs).
fn feature_add(e: &mut Engine, p: &Value) -> CmdResult {
    let kind: FeatureKind = serde_json::from_value(get(p, "kind").cloned().unwrap_or(Value::Null))
        .map_err(|err| EngineError::BadParams(format!("'kind': {err}")))?;
    if let Some(s) = kind.sketch_id()
        && e.doc.sketch(s).is_none() {
            return Err(EngineError::BadParams(format!("there is no sketch {s}")));
        }
    if matches!(kind, FeatureKind::Sketch { .. }) {
        return Err(EngineError::BadParams("use sketch.create for sketches".into()));
    }
    let r = add_feature(e, kind);
    checked(e, r)
}

/// Merge `set` into the feature's JSON and read it back, so any field can be edited and the
/// result is type-checked.
fn feature_edit(e: &mut Engine, p: &Value) -> CmdResult {
    let id = feature_id(e, p)?;
    if let Some(k) = get(p, "kind") {
        let kind: FeatureKind = serde_json::from_value(k.clone()).map_err(|err| EngineError::BadParams(format!("'kind': {err}")))?;
        let f = e.doc.feature_mut(id).ok_or_else(|| failed("the feature is missing"))?;
        if std::mem::discriminant(&f.kind) != std::mem::discriminant(&kind) {
            return Err(EngineError::BadParams("the feature type cannot change".into()));
        }
        f.kind = kind;
        return report(e, id);
    }
    let set = get(p, "set").and_then(Value::as_object).ok_or_else(|| EngineError::BadParams("'set' must be an object".into()))?;
    let f = e.doc.feature_mut(id).ok_or_else(|| failed("the feature is missing"))?;
    let mut v = serde_json::to_value(&f.kind).map_err(|err| failed(err.to_string()))?;
    if let Some(obj) = v.as_object_mut() {
        for (k, val) in set {
            if k == "type" {
                continue;
            }
            obj.insert(k.clone(), val.clone());
        }
    }
    let kind: FeatureKind = serde_json::from_value(v).map_err(|err| EngineError::BadParams(format!("{err}")))?;
    f.kind = kind;
    report(e, id)
}

fn report(e: &mut Engine, id: Id) -> CmdResult {
    let model = fusioncraft_model::evaluate(&e.doc);
    let err = model.errors.get(&id).cloned();
    e.model = model;
    match err {
        Some(msg) => Err(failed(msg)),
        None => Ok(json!({ "feature": id })),
    }
}

fn feature_delete(e: &mut Engine, p: &Value) -> CmdResult {
    let id = feature_id(e, p)?;
    let idx = e.doc.feature_index(id).ok_or_else(|| failed("the feature is missing"))?;
    let removed = e.doc.timeline.remove(idx);
    if let FeatureKind::Sketch { sketch } = removed.kind {
        e.doc.sketches.retain(|s| s.id != sketch);
        if e.active_sketch == Some(sketch) {
            e.active_sketch = None;
        }
    }
    if let Some(m) = e.doc.marker {
        e.doc.marker = if idx < m { Some(m - 1) } else { Some(m) };
        if e.doc.marker.is_some_and(|m| m >= e.doc.timeline.len()) {
            e.doc.marker = None;
        }
    }
    Ok(json!({ "deleted": id }))
}

fn feature_suppress(e: &mut Engine, p: &Value) -> CmdResult {
    let id = feature_id(e, p)?;
    let f = e.doc.feature_mut(id).ok_or_else(|| failed("the feature is missing"))?;
    f.suppressed = bool_or(p, "suppressed", !f.suppressed);
    Ok(json!({ "suppressed": f.suppressed }))
}

fn feature_rename(e: &mut Engine, p: &Value) -> CmdResult {
    let id = feature_id(e, p)?;
    let name = str_req(p, "name")?.trim().to_owned();
    if name.is_empty() {
        return Err(EngineError::BadParams("the name is empty".into()));
    }
    let sketch = e.doc.feature(id).and_then(|f| match f.kind {
        FeatureKind::Sketch { sketch } => Some(sketch),
        _ => None,
    });
    if let Some(s) = sketch.and_then(|s| e.doc.sketch_mut(s)) {
        s.name = name.clone();
    }
    if let Some(f) = e.doc.feature_mut(id) {
        f.name = name;
    }
    Ok(json!({}))
}

fn timeline_marker(e: &mut Engine, p: &Value) -> CmdResult {
    let n = e.doc.timeline.len();
    e.doc.marker = match get(p, "position") {
        None => None,
        Some(Value::String(s)) if s == "end" => None,
        Some(v) => {
            let m = v.as_u64().and_then(|m| usize::try_from(m).ok()).ok_or_else(|| EngineError::BadParams("'position' must be a number or \"end\"".into()))?;
            if m >= n { None } else { Some(m) }
        }
    };
    Ok(json!({ "position": e.doc.active_len() }))
}

fn valid_name(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c.is_alphabetic() || c == '_') && cs.all(|c| c.is_alphanumeric() || c == '_') && s.len() <= 64
}

fn param_set(e: &mut Engine, p: &Value) -> CmdResult {
    let name = str_req(p, "name")?.trim().to_owned();
    if !valid_name(&name) || ["mm", "cm", "m", "in", "ft", "deg", "rad", "pi", "PI"].contains(&name.as_str()) {
        return Err(EngineError::BadParams(format!("'{name}' is not a valid parameter name")));
    }
    let ex = expr(p, "expr")?;
    let comment = str_opt(p, "comment").unwrap_or_default().to_owned();
    match e.doc.parameters.iter_mut().find(|q| q.name == name) {
        Some(q) => {
            q.expr = ex;
            if get(p, "comment").is_some() {
                q.comment = comment;
            }
        }
        None => e.doc.parameters.push(Parameter { name: name.clone(), expr: ex, comment }),
    }
    let value = e.doc.params().eval(&Expr::new(name.clone())).map_err(|err| failed(err.to_string()))?;
    Ok(json!({ "name": name, "value": value }))
}

fn param_delete(e: &mut Engine, p: &Value) -> CmdResult {
    let name = str_req(p, "name")?;
    let before = e.doc.parameters.len();
    e.doc.parameters.retain(|q| q.name != name);
    if e.doc.parameters.len() == before {
        return Err(EngineError::BadParams(format!("there is no parameter '{name}'")));
    }
    Ok(json!({}))
}

// ---------------------------------------------------------------- inspect

fn model_info(e: &mut Engine, _p: &Value) -> CmdResult {
    let bodies: Vec<Value> = e
        .model
        .bodies
        .iter()
        .map(|b| {
            let bb = b.mesh.aabb();
            json!({
                "name": b.name,
                "volume": b.mesh.volume(),
                "area": b.mesh.area(),
                "triangles": b.mesh.tris.len(),
                "min": bb.min.to_array(),
                "max": bb.max.to_array(),
            })
        })
        .collect();
    let errors: Vec<Value> = e
        .doc
        .timeline
        .iter()
        .filter_map(|f| e.model.errors.get(&f.id).map(|m| json!({ "feature": f.name, "error": m })))
        .collect();
    Ok(json!({ "name": e.doc.name, "bodies": bodies, "volume": e.model.volume(), "errors": errors }))
}

fn help_commands(_e: &mut Engine, _p: &Value) -> CmdResult {
    Ok(Value::Array(
        COMMANDS
            .iter()
            .map(|c| json!({ "id": c.id, "label": c.label, "group": c.group, "shortcut": c.shortcut, "params": c.params }))
            .collect(),
    ))
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    fn run(e: &mut Engine, id: &str, p: Value) -> Value {
        match e.execute(id, &p) {
            Ok(v) => v,
            Err(err) => panic!("{id} failed: {err}"),
        }
    }

    #[test]
    fn bracket_workflow_with_undo() {
        let mut e = Engine::new();
        run(&mut e, "param.set", json!({"name": "thick", "expr": "4 mm"}));
        run(&mut e, "sketch.create", json!({"plane": "xy"}));
        run(&mut e, "sketch.rect", json!({"a": [0, 0], "b": [40, 20]}));
        run(&mut e, "sketch.circle", json!({"center": [30, 10], "diameter": 8}));
        let prof = run(&mut e, "sketch.profiles", json!({}));
        assert_eq!(prof["profiles"].as_array().map(Vec::len), Some(2));
        run(&mut e, "feature.extrude", json!({"profiles": [0], "distance": "thick"}));
        assert_eq!(e.model.bodies.len(), 1);
        let v1 = e.model.volume();
        assert!((v1 - (800.0 - std::f64::consts::PI * 16.0) * 4.0).abs() < 2.0, "{v1}");
        run(&mut e, "param.set", json!({"name": "thick", "expr": "8"}));
        assert!((e.model.volume() - 2.0 * v1).abs() < 1.0);
        run(&mut e, "edit.undo", json!({}));
        assert!((e.model.volume() - v1).abs() < 1e-6);
        run(&mut e, "edit.redo", json!({}));
        assert!((e.model.volume() - 2.0 * v1).abs() < 1.0);
        // A failing command changes nothing and is not undoable.
        let before = e.doc.clone();
        assert!(e.execute("feature.extrude", &json!({"distance": "nope"})).is_err());
        assert_eq!(e.doc, before);
        assert!(e.execute("feature.cylinder", &json!({"radius": 2, "height": 30, "base": [5, 5, -5], "operation": "cut"})).is_ok());
        assert!(e.model.volume() < 2.0 * v1 - 50.0);
        // Edit the cut.
        let cut = e.doc.timeline.last().map(|f| f.id).unwrap_or(0);
        run(&mut e, "feature.edit", json!({"id": cut, "set": {"radius": "1"}}));
        run(&mut e, "feature.suppress", json!({"id": cut}));
        assert!((e.model.volume() - 2.0 * v1).abs() < 1.0);
        run(&mut e, "timeline.marker", json!({"position": 1}));
        assert!(e.model.bodies.is_empty());
        run(&mut e, "timeline.marker", json!({"position": "end"}));
        assert_eq!(e.model.bodies.len(), 1);
    }

    #[test]
    fn hostile_params_never_panic() {
        let mut e = Engine::new();
        let junk = [
            json!(null),
            json!({}),
            json!({"a": "x", "b": [1]}),
            json!({"a": [1e308, 1], "b": [0, f64::MAX]}),
            json!({"plane": 7}),
            json!({"id": -1}),
            json!({"index": 99999999999u64}),
            json!({"distance": [1, 2]}),
            json!({"count_x": 1000000000u64, "spacing_x": "1"}),
            json!({"set": {"type": "box", "width": {"x": 1}}, "id": 1}),
            json!({"name": "", "expr": "((((("}),
            json!({"path": "/nonexistent/dir/x.fcraft"}),
            json!({"position": "middle"}),
        ];
        for c in COMMANDS {
            if c.id == "file.new" {
                continue;
            }
            for p in &junk {
                let _ = e.execute(c.id, p);
            }
        }
        assert!(e.execute("no.such", &json!({})).is_err());
    }
}
