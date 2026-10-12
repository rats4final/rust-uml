//! `cargo xtask <task>`: repository checks that plain cargo can't express.
//!
//! - `layers`  every crate depends only on crates in lower layers
//! - `wasm`    the browser-capable crates and the web app build for wasm32
//! - `targets` the desktop app type-checks for Windows and macOS
//! - `ci`      all of the above plus fmt, clippy and tests

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Internal crates by layer. A crate may depend only on crates in lower layers.
/// Apps sit above every crate.
const LAYERS: &[(&str, u8)] = &[
    ("mdj", 0),
    ("uml", 1),
    ("render", 2),
    ("engine", 3),
    ("ui", 4),
];

/// Crates nothing below `ui` may use: they pull in windowing or native dialogs.
const UI_ONLY: &[&str] = &["egui", "eframe", "winit", "rfd"];

/// Everything that must compile to WebAssembly.
const WASM_PACKAGES: &[&str] = &["mdj", "uml", "render", "engine", "ui", "web"];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let root = workspace_root();
    let result = match task.as_str() {
        "layers" => layers(&root),
        "wasm" => wasm(&root),
        "targets" => targets(&root),
        "ci" => ci(&root),
        _ => Err("usage: cargo xtask <layers|wasm|targets|ci>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask {task}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The workspace root, found at run time (cargo sets `CARGO_MANIFEST_DIR`
/// for `cargo run`), so a moved checkout still works.
fn workspace_root() -> PathBuf {
    let here = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    here.parent().map(Path::to_path_buf).unwrap_or(here)
}

fn layers(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    for (name, layer) in LAYERS {
        let manifest = root.join("crates").join(name).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
        for dep in dependency_names(&text) {
            if let Some((_, dep_layer)) = LAYERS.iter().find(|(n, _)| *n == dep)
                && dep_layer >= layer
            {
                problems.push(format!("{name} (L{layer}) depends on {dep} (L{dep_layer})"));
            }
            if *layer < 4 && UI_ONLY.contains(&dep.as_str()) {
                problems.push(format!("{name} (L{layer}) depends on UI-only crate {dep}"));
            }
        }
    }
    let registered: Vec<_> = LAYERS.iter().map(|(n, _)| *n).collect();
    for entry in std::fs::read_dir(root.join("crates")).map_err(|e| e.to_string())? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        if !registered.contains(&name.as_ref()) {
            problems.push(format!("crates/{name} is not registered in xtask LAYERS"));
        }
    }
    if problems.is_empty() {
        println!("layers ok");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Names under `[dependencies]` (not dev-dependencies) of a manifest.
fn dependency_names(manifest: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut names = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]"
                || line.ends_with(".dependencies]") && !line.contains("dev-");
            continue;
        }
        if in_deps
            && let Some(name) = line
                .split(['=', '.', ' '])
                .next()
                .filter(|n| !n.is_empty() && !n.starts_with('#'))
        {
            names.push(name.to_owned());
        }
    }
    names
}

fn wasm(_root: &Path) -> Result<(), String> {
    let mut args = vec!["check", "--target", "wasm32-unknown-unknown"];
    for p in WASM_PACKAGES {
        args.extend(["-p", p]);
    }
    cargo(&args)
}

fn targets(_root: &Path) -> Result<(), String> {
    for target in ["x86_64-pc-windows-gnu", "aarch64-apple-darwin"] {
        cargo(&["check", "--target", target, "-p", "desktop", "-p", "cli"])?;
    }
    Ok(())
}

fn ci(root: &Path) -> Result<(), String> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&["clippy", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test"])?;
    layers(root)?;
    wasm(root)?;
    targets(root)
}

fn cargo(args: &[&str]) -> Result<(), String> {
    println!("$ cargo {}", args.join(" "));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}
