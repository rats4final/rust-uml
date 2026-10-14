//! Headless command line: StarUML's CLI where it overlaps, plus scripting
//! and an MCP server for AI agents. `rust-uml-cli help` lists everything;
//! `rust-uml-cli help <subcommand>` explains one.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use automation::script::{self, Step};
use automation::workspace::Workspace;
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

/// name, usage, explanation.
const SUBCOMMANDS: &[(&str, &str, &str)] = &[
    (
        "new",
        "new <model.mdj>",
        "Create an empty model file.",
    ),
    (
        "info",
        "info <model.mdj> [--json]",
        "List the diagrams in a model.",
    ),
    (
        "image",
        "image <model.mdj> [-f svg|png] [-o out-dir] [-s scale] [-d diagram]",
        "Export every diagram (or just -d, by id or name) as PNG or SVG, without watermarks.",
    ),
    (
        "run",
        "run (<model.mdj> | --new) [<command-id> [json-params]]
      [--cmd <id> [--params <json>] [--as <name>]]... [--script <steps.json>|-]
      [-d diagram] [-o out.mdj|out.svg|out.png]...",
        "Run commands on a model, in order, then write the outputs (-o can repeat: .mdj saves the
model, .svg/.png export the open diagram). Nothing is written if a step fails.

Steps come from the positional command, from --cmd/--params/--as, and from a script file: a
JSON list of {\"cmd\": id, \"params\": {...}, \"as\": name}. A later step can use an earlier
result: \"$name.view\", \"$name.model\", \"$prev.id\" (start a string with $$ for a literal $).

Example:
  rust-uml-cli run --new \\
    --cmd diagram.add --params '{\"type\": \"Class Diagram\", \"name\": \"Shop\"}' \\
    --cmd node.create --params '{\"kind\": \"Class\", \"x\": 40, \"y\": 40, \"name\": \"Cart\"}' --as cart \\
    --cmd node.create --params '{\"kind\": \"Class\", \"x\": 320, \"y\": 40, \"name\": \"Item\"}' --as item \\
    --cmd edge.create --params '{\"kind\": \"Composition\", \"tail\": \"$cart.view\", \"head\": \"$item.view\"}' \\
    -o shop.mdj -o shop.png",
    ),
    (
        "import",
        "import <diagram.mmd>|- [--into model.mdj] [--name name] [-o out.mdj|out.svg|out.png]...",
        "Turn Mermaid text (classDiagram, sequenceDiagram, stateDiagram, flowchart, erDiagram, mindmap)
into a new diagram, laid out automatically, in a new model or added to --into. Lines that can't be imported are listed as
warnings. -o works as in `run`; without -o the result is printed.

Example:
  rust-uml-cli import shop.mmd -o shop.mdj -o shop.png",
    ),
    (
        "batch",
        "batch --in <dir> --out <dir> [--script <steps.json>] [--image png|svg] [-s scale]",
        "Apply a script to every .mdj file in a folder and save the results in another folder,
and/or export every diagram of every file (--image). A file whose script fails is skipped
and reported; the others still run.",
    ),
    (
        "commands",
        "commands [filter] [--json]",
        "List every command with its parameters.",
    ),
    (
        "tools",
        "tools [diagram-type] [--json]",
        "List the diagram types, or the kinds node.create and edge.create accept on one.",
    ),
    (
        "mcp",
        "mcp [--root <dir>] | mcp --bridge 127.0.0.1:<port> --control-token-file <path>",
        "Serve the Model Context Protocol on stdin/stdout so AI agents (Claude Code, Claude Desktop,
...) can build and read models. Files are read and written only inside --root (default: the
current folder). Example for Claude Code:
  claude mcp add rust-uml -- rust-uml-cli mcp --root /path/to/models

With --bridge the tools drive a running window instead, so you watch the agent work. Start the
app with the same port and token file (the app creates the file if it is missing; agent file
access is confined to --control-root):
  rust-uml --control 7878 --control-token-file ~/.rust-uml-token --control-root ~/models
  claude mcp add rust-uml-live -- rust-uml-cli mcp --bridge 127.0.0.1:7878 \\
    --control-token-file ~/.rust-uml-token",
    ),
    (
        "help",
        "help [subcommand]",
        "Show this list, or explain one subcommand.",
    ),
];

fn usage() -> String {
    let mut s = String::from("rust-uml-cli: UML models from the command line\n\nusage:\n");
    for (_, u, _) in SUBCOMMANDS {
        let first = u.lines().next().unwrap_or_default();
        s.push_str(&format!("  rust-uml-cli {first}\n"));
    }
    s.push_str("\n`rust-uml-cli help <subcommand>` explains one.");
    s
}

fn help(name: &str) -> anyhow::Result<String> {
    let (_, u, about) = SUBCOMMANDS
        .iter()
        .find(|(n, _, _)| *n == name)
        .ok_or_else(|| anyhow!("no subcommand `{name}`\n\n{}", usage()))?;
    Ok(format!("usage: rust-uml-cli {u}\n\n{about}"))
}

/// Flags that take a value.
const VALUE_FLAGS: &[&str] = &[
    "-o",
    "-f",
    "-s",
    "-d",
    "--cmd",
    "--params",
    "--as",
    "--script",
    "--in",
    "--out",
    "--image",
    "--root",
    "--into",
    "--name",
    "--bridge",
    "--control-token-file",
];
/// Flags that stand alone.
const SWITCHES: &[&str] = &["--new", "--json", "--help", "-h"];

/// Command-line words split into positional arguments and flags, in order.
struct Args {
    pos: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Args {
    fn parse(words: &[String]) -> anyhow::Result<Self> {
        let mut pos = Vec::new();
        let mut flags = Vec::new();
        let mut it = words.iter();
        while let Some(w) = it.next() {
            if VALUE_FLAGS.contains(&w.as_str()) {
                let v = it.next().ok_or_else(|| anyhow!("{w} needs a value"))?;
                flags.push((w.clone(), v.clone()));
            } else if SWITCHES.contains(&w.as_str()) {
                flags.push((w.clone(), String::new()));
            } else if w.starts_with('-') && w.len() > 1 && !looks_like_json(w) {
                bail!("unknown option {w}");
            } else {
                pos.push(w.clone());
            }
        }
        Ok(Self { pos, flags })
    }

    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|(f, _)| f == flag)
    }

    /// The last value given for `flag`.
    fn value(&self, flag: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(f, _)| f == flag)
            .map(|(_, v)| v.as_str())
    }

    fn values(&self, flag: &str) -> impl Iterator<Item = &str> {
        self.flags
            .iter()
            .filter(move |(f, _)| f == flag)
            .map(|(_, v)| v.as_str())
    }
}

/// Negative numbers and JSON can start with `-`; nothing else here does.
fn looks_like_json(w: &str) -> bool {
    w[1..].starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

fn load(path: &str) -> anyhow::Result<Session> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Session::open(&text, name, Some(path.to_owned())).with_context(|| format!("loading {path}"))
}

fn print_json(v: &Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn run(words: Vec<String>) -> anyhow::Result<()> {
    let args = Args::parse(&words)?;
    let Some(sub) = args.pos.first().map(String::as_str) else {
        if args.has("--help") || args.has("-h") {
            println!("{}", usage());
            return Ok(());
        }
        bail!("{}", usage());
    };
    if args.has("--help") || args.has("-h") {
        println!("{}", help(sub)?);
        return Ok(());
    }
    let rest: Vec<&str> = args.pos[1..].iter().map(String::as_str).collect();
    let reg = Registry::default();
    match (sub, rest.as_slice()) {
        ("help", []) => {
            println!("{}", usage());
            Ok(())
        }
        ("help", [name]) => {
            println!("{}", help(name)?);
            Ok(())
        }
        ("new", [file]) => {
            if Path::new(file).exists() {
                bail!("{file} already exists");
            }
            std::fs::write(file, Session::default().to_json()?)?;
            println!("created {file}");
            Ok(())
        }
        ("info", [file]) => info(&reg, file, args.has("--json")),
        ("image", [file]) => {
            let mut s = load(file)?;
            let format = args.value("-f").unwrap_or("png");
            let out = PathBuf::from(args.value("-o").unwrap_or("."));
            let scale = scale(&args)?;
            let written = export_all(&reg, &mut s, &out, None, format, scale, args.value("-d"))?;
            for p in &written {
                println!("{}", p.display());
            }
            if written.is_empty() {
                bail!("no matching diagram with content");
            }
            Ok(())
        }
        ("tools", rest) if rest.len() <= 1 => tools(rest.first().copied(), args.has("--json")),
        ("commands", rest) if rest.len() <= 1 => {
            let filter = rest.first().map(|f| f.to_lowercase());
            let specs = reg.specs().iter().filter(|c| {
                filter.as_ref().is_none_or(|f| {
                    c.id.contains(f.as_str()) || c.label.to_lowercase().contains(f.as_str())
                })
            });
            if args.has("--json") {
                let list: Vec<Value> = specs
                    .map(|c| {
                        json!({
                            "id": c.id, "label": c.label, "params": c.params,
                            "menu": c.menu, "shortcut": c.shortcut,
                        })
                    })
                    .collect();
                return print_json(&Value::Array(list));
            }
            for c in specs {
                println!("{:<26} {}\n{:<26} params: {}", c.id, c.label, "", c.params);
            }
            Ok(())
        }
        ("run", rest) => run_steps(&reg, &args, rest),
        ("batch", []) => batch(&reg, &args),
        ("import", [source]) => import(&reg, &args, source),
        ("mcp", []) => mcp(&args),
        (sub, _) if SUBCOMMANDS.iter().any(|(n, _, _)| *n == sub) => {
            bail!("{}", help(sub)?)
        }
        _ => bail!("{}", usage()),
    }
}

fn scale(args: &Args) -> anyhow::Result<f64> {
    let s: f64 = args
        .value("-s")
        .map(str::parse)
        .transpose()
        .context("-s must be a number")?
        .unwrap_or(2.0);
    if !(0.1..=8.0).contains(&s) {
        bail!("-s must be 0.1–8");
    }
    Ok(s)
}

fn info(reg: &Registry, file: &str, as_json: bool) -> anyhow::Result<()> {
    let mut s = load(file)?;
    let diagrams = reg.run(&mut s, "model.diagrams", &json!({}))?;
    if as_json {
        return print_json(&json!({ "file": file, "elements": s.doc.len(), "diagrams": diagrams }));
    }
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

fn tools(diagram_type: Option<&str>, as_json: bool) -> anyhow::Result<()> {
    if as_json {
        return print_json(&automation::toolbox(diagram_type).map_err(|e| anyhow!(e))?);
    }
    let types: Vec<String> = match diagram_type {
        None => uml::DIAGRAMS
            .iter()
            .map(|(t, _, _)| (*t).to_owned())
            .collect(),
        Some(t) => {
            let found = automation::toolbox(Some(t)).map_err(|e| anyhow!(e))?;
            vec![found["type"].as_str().unwrap_or(t).to_owned()]
        }
    };
    for t in types {
        let tb = automation::toolbox(Some(&t)).map_err(|e| anyhow!(e))?;
        println!("{t}");
        for g in tb["groups"].as_array().into_iter().flatten() {
            let tools: Vec<String> = g["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|tool| {
                    let kind = tool["kind"].as_str().unwrap_or_default();
                    match tool["shape"].as_str() {
                        Some("line") => format!("{kind} (line)"),
                        _ => format!("{kind} (box)"),
                    }
                })
                .collect();
            println!(
                "  {:<20} {}",
                g["group"].as_str().unwrap_or(""),
                tools.join(", ")
            );
        }
    }
    Ok(())
}

/// The steps given on the command line, in order.
fn collect_steps(args: &Args, positional: &[&str]) -> anyhow::Result<Vec<Step>> {
    let mut steps = Vec::new();
    match positional {
        [] => {}
        [cmd] => steps.push(Step::new(*cmd, json!({}))),
        [cmd, params] => {
            let p: Value = serde_json::from_str(params).context("params must be JSON")?;
            steps.push(Step::new(*cmd, p));
        }
        _ => bail!("{}", help("run")?),
    }
    for (flag, value) in &args.flags {
        match flag.as_str() {
            "--cmd" => steps.push(Step::new(value.clone(), json!({}))),
            "--params" | "--as" => {
                let Some(last) = steps.last_mut() else {
                    bail!("{flag} must follow a --cmd");
                };
                if flag == "--params" {
                    last.params = serde_json::from_str(value)
                        .with_context(|| format!("--params for {} must be JSON", last.cmd))?;
                } else {
                    // Validate the name the same way script files are.
                    let parsed = script::parse_steps(&json!([{ "cmd": last.cmd, "as": value }]))
                        .map_err(|e| anyhow!("--as {value}: {e}"))?;
                    last.name = parsed.into_iter().next().and_then(|s| s.name);
                }
            }
            "--script" => {
                let text = if value == "-" {
                    std::io::read_to_string(std::io::stdin()).context("reading the script")?
                } else {
                    std::fs::read_to_string(value).with_context(|| format!("reading {value}"))?
                };
                let v: Value =
                    serde_json::from_str(&text).with_context(|| format!("{value} is not JSON"))?;
                steps.extend(script::parse_steps(&v).map_err(|e| anyhow!("{value}: {e}"))?);
            }
            _ => {}
        }
    }
    Ok(steps)
}

fn run_steps(reg: &Registry, args: &Args, rest: &[&str]) -> anyhow::Result<()> {
    let (mut s, commands) = if args.has("--new") {
        (Session::default(), rest)
    } else {
        let [file, commands @ ..] = rest else {
            bail!("{}", help("run")?);
        };
        (load(file)?, commands)
    };
    let steps = collect_steps(args, commands)?;
    let outputs: Vec<&str> = args.values("-o").collect();
    if steps.is_empty() && outputs.is_empty() {
        bail!(
            "nothing to do: give a command, --cmd, --script or -o\n\n{}",
            help("run")?
        );
    }
    // Check outputs before running anything, so a typo doesn't waste a run.
    for out in &outputs {
        output_kind(out)?;
    }
    if let Some(d) = args.value("-d") {
        let id = automation::find_diagram(reg, &mut s, d).map_err(|e| anyhow!(e))?;
        reg.run(&mut s, "diagram.open", &json!({ "id": id }))?;
    }
    let results = script::run(reg, &mut s, &steps, true).map_err(|f| anyhow!("{f}"))?;
    match results.as_slice() {
        [] => {}
        [one] => print_json(one)?,
        many => print_json(&Value::Array(many.to_vec()))?,
    }
    for out in outputs {
        let bytes = match output_kind(out)? {
            "model" => s.to_json()?.into_bytes(),
            format => diagram_bytes(reg, &mut s, None, format, 2.0)?,
        };
        std::fs::write(out, bytes).with_context(|| format!("writing {out}"))?;
        eprintln!("wrote {out}");
    }
    Ok(())
}

/// What `-o path` writes, from its extension.
fn output_kind(path: &str) -> anyhow::Result<&'static str> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "mdj" | "mfj" => Ok("model"),
        "svg" => Ok("svg"),
        "png" => Ok("png"),
        _ => bail!("-o {path}: the extension must be .mdj, .mfj, .svg or .png"),
    }
}

/// One diagram (default: the open one) as SVG or PNG bytes.
fn diagram_bytes(
    reg: &Registry,
    s: &mut Session,
    diagram: Option<&str>,
    format: &str,
    scale: f64,
) -> anyhow::Result<Vec<u8>> {
    let params = json!({ "diagram": diagram, "scale": scale });
    if format == "svg" {
        let r = reg.run(s, "export.svg", &params)?;
        Ok(r["svg"].as_str().unwrap_or_default().as_bytes().to_vec())
    } else {
        use base64::Engine as _;
        let r = reg.run(s, "export.png", &params)?;
        let b64 = r["png_base64"].as_str().unwrap_or_default();
        Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
    }
}

/// Export each diagram with content to `out`, named `<prefix>-<name>.<format>`
/// (or just `<name>.<format>` without a prefix).
fn export_all(
    reg: &Registry,
    s: &mut Session,
    out: &Path,
    prefix: Option<&str>,
    format: &str,
    scale: f64,
    only: Option<&str>,
) -> anyhow::Result<Vec<PathBuf>> {
    if !["png", "svg"].contains(&format) {
        bail!("the image format must be png or svg");
    }
    std::fs::create_dir_all(out)?;
    let diagrams = reg.run(s, "model.diagrams", &json!({}))?;
    let mut written = Vec::new();
    for d in diagrams.as_array().into_iter().flatten() {
        let name = d["name"].as_str().unwrap_or("diagram");
        if only.is_some_and(|o| o != name && d["id"] != o) || d["views"] == 0 {
            continue;
        }
        let file = match prefix {
            Some(p) => format!("{}-{}.{format}", safe_name(p), safe_name(name)),
            None => format!("{}.{format}", safe_name(name)),
        };
        let bytes = diagram_bytes(reg, s, d["id"].as_str(), format, scale)?;
        let path = out.join(file);
        std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

fn import(reg: &Registry, args: &Args, source: &str) -> anyhow::Result<()> {
    let text = if source == "-" {
        std::io::read_to_string(std::io::stdin()).context("reading stdin")?
    } else {
        std::fs::read_to_string(source).with_context(|| format!("reading {source}"))?
    };
    let outputs: Vec<&str> = args.values("-o").collect();
    for out in &outputs {
        output_kind(out)?;
    }
    let mut s = match args.value("--into") {
        Some(model) => load(model)?,
        None => Session::default(),
    };
    let mut params = json!({ "text": text });
    if let Some(n) = args.value("--name") {
        params["name"] = json!(n);
    }
    let result = reg.run(&mut s, "import.mermaid", &params)?;
    for w in result["warnings"].as_array().into_iter().flatten() {
        eprintln!("warning: {}", w.as_str().unwrap_or_default());
    }
    if outputs.is_empty() {
        return print_json(&result);
    }
    for out in outputs {
        let bytes = match output_kind(out)? {
            "model" => s.to_json()?.into_bytes(),
            format => diagram_bytes(reg, &mut s, None, format, 2.0)?,
        };
        std::fs::write(out, bytes).with_context(|| format!("writing {out}"))?;
        eprintln!("wrote {out}");
    }
    Ok(())
}

fn batch(reg: &Registry, args: &Args) -> anyhow::Result<()> {
    let (Some(input), Some(output)) = (args.value("--in"), args.value("--out")) else {
        bail!("{}", help("batch")?);
    };
    let image = args.value("--image");
    let steps = match args.value("--script") {
        Some(_) => Some(collect_steps(args, &[])?),
        None => None,
    };
    if steps.is_none() && image.is_none() {
        bail!(
            "nothing to do: give --script and/or --image\n\n{}",
            help("batch")?
        );
    }
    let scale = scale(args)?;
    std::fs::create_dir_all(output)?;
    if steps.is_some() && Path::new(input).canonicalize()? == Path::new(output).canonicalize()? {
        bail!("--out must differ from --in, so the originals are kept");
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(input)
        .with_context(|| format!("reading {input}"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mdj")))
        .collect();
    files.sort();
    if files.is_empty() {
        bail!("no .mdj files in {input}");
    }
    let mut failed = 0;
    for file in &files {
        let shown = file.display().to_string();
        let outcome = (|| -> anyhow::Result<()> {
            let mut s = load(&shown)?;
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
            if let Some(steps) = &steps {
                script::run(reg, &mut s, steps, true).map_err(|f| anyhow!("{f}"))?;
                let target = Path::new(output).join(name.unwrap_or_default());
                std::fs::write(&target, s.to_json()?)?;
                println!("{}", target.display());
            }
            if let Some(format) = image {
                let stem = file.file_stem().map(|n| n.to_string_lossy().into_owned());
                let written = export_all(
                    reg,
                    &mut s,
                    Path::new(output),
                    stem.as_deref(),
                    format,
                    scale,
                    None,
                )?;
                for p in written {
                    println!("{}", p.display());
                }
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            eprintln!("{shown}: {e:#}");
            failed += 1;
        }
    }
    if failed > 0 {
        bail!("{failed} of {} files failed", files.len());
    }
    Ok(())
}

fn mcp(args: &Args) -> anyhow::Result<()> {
    let mut server = match args.value("--bridge") {
        Some(addr) => {
            if args.has("--root") {
                bail!("--root does not apply with --bridge: the app's --control-root decides");
            }
            let addr: std::net::SocketAddr = addr
                .parse()
                .map_err(|_| anyhow!("--bridge takes 127.0.0.1:<port>, not `{addr}`"))?;
            let path = args
                .value("--control-token-file")
                .context("--bridge needs --control-token-file <path> (the app's token file)")?;
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
            let token =
                automation::control::Token::parse(&text).map_err(|e| anyhow!("{path}: {e}"))?;
            let client =
                automation::bridge::BridgeClient::new(addr, token).map_err(|e| anyhow!(e))?;
            eprintln!("rust-uml MCP server; driving the window at {addr}");
            automation::mcp::Server::bridge(client)
        }
        None => {
            if args.has("--control-token-file") {
                bail!("--control-token-file only applies with --bridge");
            }
            let root = args.value("--root").unwrap_or(".");
            let ws = Workspace::new(root).map_err(|e| anyhow!(e))?;
            eprintln!("rust-uml MCP server; workspace {}", ws.root().display());
            automation::mcp::Server::new(Some(ws))
        }
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("reading stdin")?;
        if let Some(reply) = server.handle_line(&line) {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
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
