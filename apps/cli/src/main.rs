//! Headless command line, mirroring StarUML's CLI where it overlaps.
//!
//! ```text
//! rust-uml-cli new      <model.mdj>
//! rust-uml-cli info     <model.mdj>
//! rust-uml-cli image    <model.mdj> [-f svg|png] [-o out-dir] [-s scale] [-d diagram-name]
//! rust-uml-cli commands
//! rust-uml-cli tools    [diagram-type]
//! rust-uml-cli run      <model.mdj> <command-id> [json-params] [-o out.mdj] [-d diagram]
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use engine::{Registry, Session};
use serde_json::{Value, json};

fn main() -> std::process::ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage:
  rust-uml-cli new      <model.mdj>
  rust-uml-cli info     <model.mdj>
  rust-uml-cli image    <model.mdj> [-f svg|png] [-o out-dir] [-s scale] [-d diagram-name]
  rust-uml-cli commands
  rust-uml-cli tools    [diagram-type]
  rust-uml-cli run      <model.mdj> <command-id> [json-params] [-o out.mdj] [-d diagram]";

/// Positional arguments and `-x value` options.
type Args<'a> = (Vec<&'a str>, Vec<(&'a str, &'a str)>);

/// Splits `args` into positional arguments and `-x value` options.
fn split(args: &[String]) -> anyhow::Result<Args<'_>> {
    let mut pos = Vec::new();
    let mut opts = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(flag) = a.strip_prefix('-').filter(|f| f.len() == 1) {
            let v = it.next().ok_or_else(|| anyhow!("-{flag} needs a value"))?;
            opts.push((flag, v.as_str()));
        } else {
            pos.push(a.as_str());
        }
    }
    Ok((pos, opts))
}

fn opt<'a>(opts: &[(&str, &'a str)], flag: &str) -> Option<&'a str> {
    opts.iter().rev().find(|(f, _)| *f == flag).map(|(_, v)| *v)
}

fn load(path: &str) -> anyhow::Result<Session> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Session::open(&text, name, Some(path.to_owned())).with_context(|| format!("loading {path}"))
}

fn run(args: Vec<String>) -> anyhow::Result<()> {
    let (pos, opts) = split(&args)?;
    let reg = Registry::default();
    match pos.as_slice() {
        ["new", file] => {
            if Path::new(file).exists() {
                bail!("{file} already exists");
            }
            std::fs::write(file, Session::default().to_json()?)?;
            println!("created {file}");
            Ok(())
        }
        ["info", file] => {
            let mut s = load(file)?;
            let diagrams = reg.run(&mut s, "model.diagrams", &json!({}))?;
            println!("{}: {} elements", file, s.doc.len());
            for d in diagrams.as_array().into_iter().flatten() {
                println!(
                    "  {:<28} {:<32} {} views",
                    d["type"].as_str().unwrap_or(""),
                    d["name"].as_str().unwrap_or(""),
                    d["views"]
                );
            }
            Ok(())
        }
        ["image", file] => {
            let mut s = load(file)?;
            let format = opt(&opts, "f").unwrap_or("png");
            if !["png", "svg"].contains(&format) {
                bail!("-f must be png or svg");
            }
            let out = PathBuf::from(opt(&opts, "o").unwrap_or("."));
            std::fs::create_dir_all(&out)?;
            let only = opt(&opts, "d");
            let scale: f64 = opt(&opts, "s").map(str::parse).transpose()?.unwrap_or(2.0);
            let diagrams = reg.run(&mut s, "model.diagrams", &json!({}))?;
            let mut written = 0;
            for d in diagrams.as_array().into_iter().flatten() {
                let name = d["name"].as_str().unwrap_or("diagram");
                if only.is_some_and(|o| o != name) || d["views"] == 0 {
                    continue;
                }
                let params = json!({ "diagram": d["id"], "scale": scale });
                let path = out.join(format!("{}.{format}", safe_name(name)));
                if format == "svg" {
                    let r = reg.run(&mut s, "export.svg", &params)?;
                    std::fs::write(&path, r["svg"].as_str().unwrap_or(""))?;
                } else {
                    use base64::Engine as _;
                    let r = reg.run(&mut s, "export.png", &params)?;
                    let b64 = r["png_base64"].as_str().unwrap_or("");
                    std::fs::write(
                        &path,
                        base64::engine::general_purpose::STANDARD.decode(b64)?,
                    )?;
                }
                println!("{}", path.display());
                written += 1;
            }
            if written == 0 {
                bail!("no matching diagram with content");
            }
            Ok(())
        }
        ["tools", rest @ ..] => {
            // What node.create / edge.create accept, per diagram type.
            let types: Vec<&str> = match rest {
                [] => uml::DIAGRAMS.iter().map(|(t, _, _)| *t).collect(),
                [t] => vec![*t],
                _ => bail!("{USAGE}"),
            };
            for t in types {
                println!("{t}");
                for g in uml::tool_groups(t) {
                    let tools: Vec<String> = g
                        .tools
                        .iter()
                        .filter_map(|id| uml::tool(id))
                        .map(|tool| match tool {
                            uml::Tool::Node(n) => format!("{} (box)", n.id),
                            uml::Tool::Edge(e) => format!("{} (line)", e.id),
                        })
                        .collect();
                    println!("  {:<20} {}", g.label, tools.join(", "));
                }
            }
            Ok(())
        }
        ["commands"] => {
            for spec in reg.specs() {
                println!(
                    "{:<26} {}\n{:<26} params: {}",
                    spec.id, spec.label, "", spec.params
                );
            }
            Ok(())
        }
        ["run", file, command, rest @ ..] => {
            let mut s = load(file)?;
            let params: Value = match rest {
                [] => json!({}),
                [p] => serde_json::from_str(p).context("params must be JSON")?,
                _ => bail!("{USAGE}"),
            };
            // `-d`: the diagram to work on (id or name); otherwise the file's default.
            if let Some(d) = opt(&opts, "d") {
                let id = find_diagram(&reg, &mut s, d)?;
                reg.run(&mut s, "diagram.open", &json!({ "id": id }))?;
            }
            let result = reg.run(&mut s, command, &params)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            if let Some(out) = opt(&opts, "o") {
                std::fs::write(out, s.to_json()?).with_context(|| format!("writing {out}"))?;
                eprintln!("saved {out}");
            }
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}

/// A diagram's id, given its id or its (unique) name.
fn find_diagram(reg: &Registry, s: &mut Session, wanted: &str) -> anyhow::Result<String> {
    let diagrams = reg.run(s, "model.diagrams", &json!({}))?;
    let matches: Vec<&Value> = diagrams
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["id"] == wanted || d["name"] == wanted)
        .collect();
    match matches.as_slice() {
        [d] => Ok(d["id"].as_str().unwrap_or_default().to_owned()),
        [] => bail!("no diagram `{wanted}`"),
        _ => bail!("several diagrams are named `{wanted}`; use its id"),
    }
}

fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "diagram".into() } else { s }
}
