//! The CLI as a user (or a script) runs it.

// Test code: failing loudly is the point.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rust-uml-cli");

fn temp_dir() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Clocks may tick in microseconds (macOS): tests started together
    // would share a folder without the counter.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("rust-uml-cli-{}-{n}-{k}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cli(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = cli(dir, args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn example(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
        .display()
        .to_string()
}

#[test]
fn chained_steps_build_save_and_export() {
    let dir = temp_dir();
    let out = ok(
        &dir,
        &[
            "run",
            "--new",
            "--cmd",
            "diagram.add",
            "--params",
            r#"{"type": "Class Diagram", "name": "Shop"}"#,
            "--cmd",
            "node.create",
            "--params",
            r#"{"kind": "Class", "x": 40, "y": 40, "name": "Cart"}"#,
            "--as",
            "cart",
            "--cmd",
            "node.create",
            "--params",
            r#"{"kind": "Class", "x": 320, "y": 40, "name": "Item"}"#,
            "--as",
            "item",
            "--cmd",
            "edge.create",
            "--params",
            r#"{"kind": "Composition", "tail": "$cart.view", "head": "$item.view"}"#,
            "-o",
            "shop.mdj",
            "-o",
            "shop.svg",
            "-o",
            "shop.png",
        ],
    );
    let results: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(results.as_array().unwrap().len(), 4);
    assert!(
        std::fs::read_to_string(dir.join("shop.svg"))
            .unwrap()
            .contains("Cart")
    );
    assert!(
        std::fs::read(dir.join("shop.png"))
            .unwrap()
            .starts_with(b"\x89PNG")
    );

    // The saved file reopens; the old one-command form still works on it.
    let inspect = ok(&dir, &["run", "shop.mdj", "diagram.inspect", "-d", "Shop"]);
    let inspect: Value = serde_json::from_str(&inspect).unwrap();
    assert_eq!(inspect["boxes"].as_array().unwrap().len(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failing_step_writes_nothing() {
    let dir = temp_dir();
    let out = cli(
        &dir,
        &[
            "run",
            "--new",
            "--cmd",
            "diagram.add",
            "--params",
            r#"{"type": "Class Diagram"}"#,
            "--cmd",
            "node.create",
            "--params",
            r#"{"kind": "Nope", "x": 0, "y": 0}"#,
            "-o",
            "x.mdj",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("step 2"));
    assert!(!dir.join("x.mdj").exists());

    // Bad options are caught before anything runs.
    assert!(
        !cli(&dir, &["run", "--new", "--cmd", "file.new", "-o", "x.txt"])
            .status
            .success()
    );
    assert!(
        !cli(&dir, &["run", "--new", "--params", "{}"])
            .status
            .success()
    );
    assert!(!cli(&dir, &["run", "--new", "--bogus"]).status.success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_example_script_builds_the_shop() {
    let dir = temp_dir();
    ok(
        &dir,
        &[
            "run",
            "--new",
            "--script",
            &example("scripts/shop.json"),
            "-o",
            "shop.mdj",
        ],
    );
    let info: Value = serde_json::from_str(&ok(&dir, &["info", "shop.mdj", "--json"])).unwrap();
    assert!(info["diagrams"].to_string().contains("Shop"));
    let d: Value = serde_json::from_str(&ok(
        &dir,
        &["run", "shop.mdj", "diagram.inspect", "-d", "Shop"],
    ))
    .unwrap();
    assert_eq!(d["boxes"].as_array().unwrap().len(), 5);
    assert_eq!(d["lines"].as_array().unwrap().len(), 4);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn batch_runs_a_script_over_a_folder_and_exports() {
    let dir = temp_dir();
    std::fs::create_dir_all(dir.join("in")).unwrap();
    for f in ["shapes.mdj", "behaviour.mdj"] {
        std::fs::copy(example(f), dir.join("in").join(f)).unwrap();
    }
    std::fs::write(
        dir.join("note.json"),
        json!([
            {"cmd": "diagram.add", "params": {"type": "Class Diagram", "name": "Batch"}},
            {"cmd": "node.create", "params": {"kind": "Class", "x": 0, "y": 0, "name": "Added"}}
        ])
        .to_string(),
    )
    .unwrap();
    let out = ok(
        &dir,
        &[
            "batch",
            "--in",
            "in",
            "--out",
            "out",
            "--script",
            "note.json",
            "--image",
            "svg",
        ],
    );
    assert!(out.contains("shapes.mdj") && out.contains("behaviour-Batch.svg"));
    let d = ok(
        &dir,
        &["run", "out/shapes.mdj", "diagram.inspect", "-d", "Batch"],
    );
    assert!(d.contains("Added"));
    // Originals are kept.
    assert!(
        !cli(
            &dir,
            &[
                "batch",
                "--in",
                "in",
                "--out",
                "in",
                "--script",
                "note.json"
            ]
        )
        .status
        .success()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn import_mermaid_to_a_model_and_an_image() {
    let dir = temp_dir();
    std::fs::write(
        dir.join("zoo.mmd"),
        "classDiagram\nAnimal <|-- Duck\nDuck : +swim()\nweird line here\n",
    )
    .unwrap();
    let out = cli(
        &dir,
        &["import", "zoo.mmd", "-o", "zoo.mdj", "-o", "zoo.svg"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("warning: line 4"));
    assert!(
        std::fs::read_to_string(dir.join("zoo.svg"))
            .unwrap()
            .contains("Duck")
    );
    // Into an existing model, as a second diagram.
    ok(
        &dir,
        &[
            "import", "zoo.mmd", "--into", "zoo.mdj", "--name", "Again", "-o", "zoo.mdj",
        ],
    );
    let info = ok(&dir, &["info", "zoo.mdj"]);
    assert!(info.contains("Again"));
    assert!(!cli(&dir, &["import", "missing.mmd"]).status.success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn help_explains_every_subcommand() {
    let dir = temp_dir();
    let all = ok(&dir, &["help"]);
    for sub in [
        "new", "info", "image", "run", "import", "batch", "commands", "tools", "mcp",
    ] {
        assert!(all.contains(&format!("rust-uml-cli {sub}")), "{sub}");
        assert!(ok(&dir, &[sub, "--help"]).starts_with("usage:"), "{sub}");
    }
    let tools: Value =
        serde_json::from_str(&ok(&dir, &["tools", "Class Diagram", "--json"])).unwrap();
    assert_eq!(tools["type"], "UMLClassDiagram");
    let commands: Value = serde_json::from_str(&ok(&dir, &["commands", "--json"])).unwrap();
    assert!(
        commands
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "diagram.inspect")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mcp_over_stdio() {
    let dir = temp_dir();
    let mut child = Command::new(BIN)
        .args(["mcp", "--root", "."])
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |msg: Value| -> Value {
        writeln!(stdin, "{msg}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let init = ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "rust-uml");
    let script: Value =
        serde_json::from_str(&std::fs::read_to_string(example("scripts/shop.json")).unwrap())
            .unwrap();
    let built = ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "command_batch", "arguments": {"steps": script}}}));
    assert_eq!(built["result"]["isError"], false, "{built}");
    let saved = ask(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "model_save", "arguments": {"path": "shop.mdj"}}}));
    assert_eq!(saved["result"]["isError"], false, "{saved}");
    drop(stdin);
    assert!(child.wait().unwrap().success());
    assert!(dir.join("shop.mdj").exists());
    std::fs::remove_dir_all(dir).unwrap();
}
