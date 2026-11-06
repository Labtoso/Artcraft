//! `fusioncraft-cli`: the same commands as the app, from a terminal or a script.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use clap::{Parser, Subcommand};
use fusioncraft_engine::Engine;
use fusioncraft_render::{png, render, Camera, DisplayMode, Scene, StdView, Style};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "fusioncraft-cli", version, about = "FusionCraft on the command line")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a JSON script: a list of {"cmd": "<id>", ...params}. Optionally start from a design.
    Run {
        script: PathBuf,
        #[arg(long)]
        open: Option<PathBuf>,
        /// Save the design (.fcraft) afterwards.
        #[arg(long)]
        save: Option<PathBuf>,
        /// Export the bodies afterwards (.stl or .obj).
        #[arg(long, short)]
        export: Option<PathBuf>,
        /// Render a PNG afterwards.
        #[arg(long)]
        png: Option<PathBuf>,
    },
    /// Run one command, e.g. `exec model.info --open part.fcraft`.
    Exec {
        id: String,
        #[arg(default_value = "{}")]
        params: String,
        #[arg(long)]
        open: Option<PathBuf>,
        #[arg(long)]
        save: Option<PathBuf>,
    },
    /// Export a design to STL or OBJ.
    Export { input: PathBuf, output: PathBuf },
    /// Print bodies, volumes and feature errors.
    Info { input: PathBuf },
    /// Render a design to PNG.
    Render {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value_t = 1280)]
        width: usize,
        #[arg(long, default_value_t = 800)]
        height: usize,
        /// home, top, front, right, ...
        #[arg(long, default_value = "home")]
        view: String,
    },
    /// List every command with its parameters.
    Commands,
}

fn exec(e: &mut Engine, id: &str, p: &Value) -> Result<Value, String> {
    e.execute(id, p).map_err(|err| format!("{id}: {err}"))
}

fn open(e: &mut Engine, path: &Option<PathBuf>) -> Result<(), String> {
    if let Some(p) = path {
        exec(e, "file.open", &json!({ "path": p.display().to_string() }))?;
    }
    Ok(())
}

fn view_of(s: &str) -> StdView {
    match s.to_ascii_lowercase().as_str() {
        "top" => StdView::Top,
        "bottom" => StdView::Bottom,
        "front" => StdView::Front,
        "back" => StdView::Back,
        "left" => StdView::Left,
        "right" => StdView::Right,
        _ => StdView::Home,
    }
}

fn write_png(e: &Engine, path: &Path, w: usize, h: usize, view: StdView) -> Result<(), String> {
    let mut cam = Camera::default();
    cam.set_view(view);
    cam.fit(&e.model.aabb());
    let scene = Scene { model: &e.model, camera: &cam, style: Style::default(), mode: DisplayMode::ShadedWithEdges, selected: &[], hover: None, hidden: &[], grid: true, lines: &[] };
    let f = render(&scene, w.clamp(16, 8192), h.clamp(16, 8192), 2);
    std::fs::write(path, png::encode_rgba(f.width, f.height, &f.rgba)).map_err(|err| format!("{}: {err}", path.display()))
}

fn run(cli: Cli) -> Result<(), String> {
    let mut e = Engine::new();
    match cli.cmd {
        Cmd::Run { script, open: input, save, export, png } => {
            open(&mut e, &input)?;
            let text = std::fs::read_to_string(&script).map_err(|err| format!("{}: {err}", script.display()))?;
            let steps: Vec<Value> = serde_json::from_str(&text).map_err(|err| format!("{}: {err}", script.display()))?;
            for (i, step) in steps.iter().enumerate() {
                let id = step.get("cmd").and_then(Value::as_str).ok_or_else(|| format!("step {}: missing \"cmd\"", i + 1))?;
                let r = exec(&mut e, id, step).map_err(|err| format!("step {}: {err}", i + 1))?;
                if !r.as_object().is_some_and(serde_json::Map::is_empty) {
                    println!("{id}: {r}");
                }
            }
            if let Some(p) = save {
                exec(&mut e, "file.save", &json!({ "path": p.display().to_string() }))?;
            }
            if let Some(p) = export {
                exec(&mut e, "file.export", &json!({ "path": p.display().to_string() }))?;
            }
            if let Some(p) = png {
                write_png(&e, &p, 1280, 800, StdView::Home)?;
            }
            println!("{}", exec(&mut e, "model.info", &json!({}))?);
        }
        Cmd::Exec { id, params, open: input, save } => {
            open(&mut e, &input)?;
            let p: Value = serde_json::from_str(&params).map_err(|err| format!("params: {err}"))?;
            println!("{}", exec(&mut e, &id, &p)?);
            if let Some(path) = save {
                exec(&mut e, "file.save", &json!({ "path": path.display().to_string() }))?;
            }
        }
        Cmd::Export { input, output } => {
            open(&mut e, &Some(input))?;
            println!("{}", exec(&mut e, "file.export", &json!({ "path": output.display().to_string() }))?);
        }
        Cmd::Info { input } => {
            open(&mut e, &Some(input))?;
            let v = exec(&mut e, "model.info", &json!({}))?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        Cmd::Render { input, output, width, height, view } => {
            open(&mut e, &Some(input))?;
            write_png(&e, &output, width, height, view_of(&view))?;
        }
        Cmd::Commands => {
            for c in fusioncraft_engine::COMMANDS {
                println!("{:<22} {:<26} {}", c.id, c.label, c.params);
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
