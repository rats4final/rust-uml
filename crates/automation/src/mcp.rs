//! A Model Context Protocol server, so AI agents can build and read models.
//!
//! The transport is newline-delimited JSON-RPC 2.0 (MCP's stdio transport):
//! feed each line to [`Server::handle_line`] and write back what it returns.
//!
//! The tools run in one of two places:
//! - [`Server::new`]: an in-process [`Session`] nobody sees; files are only
//!   read and written inside its [`Workspace`].
//! - [`Server::bridge`]: a running `rust-uml --control` window, over its
//!   control channel ([`crate::control`]), so the user watches the agent work.
//!   The app runs the very same tools ([`call_tool`]) on the model it shows.
//!
//! Every tool is a thin wrapper over the command registry, so an agent can do
//! exactly what the UI does: `command_run` and `command_batch` reach every
//! command, the rest are conveniences for opening, saving, looking and
//! finding out what can be drawn.

use base64::Engine as _;
use engine::{Registry, Session};
use serde_json::{Map, Value, json};

use crate::bridge::BridgeClient;
use crate::script;
use crate::workspace::Workspace;

/// Protocol versions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Longest side of a preview image, in pixels.
pub const MAX_PREVIEW: f64 = 2048.0;

const INSTRUCTIONS: &str = "\
rust-uml edits UML models stored as StarUML-compatible .mdj files. Every edit is a command \
(see command_list); command_run runs one, command_batch runs many at once and can feed ids from \
one step into the next (\"$name.field\").

To build a diagram: diagram.add {type, name} opens a new diagram; node.create {kind, x, y, name} \
adds a box and returns {model, view}; edge.create {kind, tail, head} joins two box *views*; \
feature.add {classifier: model id, kind: attribute|operation|literal, name} adds rows; \
element.set {id, values: {...}} sets fields such as type, visibility, multiplicity. \
toolbox lists the kinds each diagram type offers. Lay boxes out on a grid with room for lines \
(classes are about 150×80; leave 100+ px between them).

From Mermaid: mermaid_import turns classDiagram, sequenceDiagram, stateDiagram, flowchart or \
erDiagram text into a laid-out diagram in one step; often the quickest way to draw a whole diagram.

To look: model_info lists diagrams, diagram_inspect lists boxes and lines with their ids and \
positions, render_preview shows the diagram as an image. Check your work with render_preview.

Files: model_open / model_save read and write inside the workspace folder only. Unsaved changes \
are lost unless you call model_save. edit.undo undoes one command.";

/// Added to the instructions when the tools drive a running window.
const LIVE_INSTRUCTIONS: &str = "\n\n\
You are connected to a running rust-uml window: the user sees every change as you make it. \
ui_inspect tells you what they have open and selected; ui_screenshot shows you the whole window. \
model_new and model_open replace what the user is looking at, so ask before using them. Files are \
confined to the folder the app was started with (--control-root).";

/// One MCP server session.
pub struct Server {
    reg: Registry,
    backend: Backend,
}

/// Where the tools run.
enum Backend {
    /// An in-process model nobody sees.
    Local {
        session: Box<Session>,
        workspace: Option<Workspace>,
    },
    /// A running app, over its control channel.
    Bridge(BridgeClient),
}

/// JSON-RPC error codes.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

impl Server {
    /// A server with an empty project. Without a workspace, no file can be
    /// opened or saved.
    pub fn new(workspace: Option<Workspace>) -> Self {
        Self {
            reg: Registry::default(),
            backend: Backend::Local {
                session: Box::default(),
                workspace,
            },
        }
    }

    /// A server whose tools drive a running `rust-uml --control` window.
    pub fn bridge(client: BridgeClient) -> Self {
        Self {
            reg: Registry::default(),
            backend: Backend::Bridge(client),
        }
    }

    fn is_bridge(&self) -> bool {
        matches!(self.backend, Backend::Bridge(_))
    }

    /// Handle one line of input; returns the line to send back, if any
    /// (notifications get no reply).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Err(e) => Some(error_reply(
                Value::Null,
                PARSE_ERROR,
                &format!("parse error: {e}"),
            )),
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(msg) => self.handle(&msg),
        };
        reply.map(|r| r.to_string())
    }

    fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // A response to something we never sent, or junk.
            return id.map(|id| error_reply(id, INVALID_REQUEST, "missing `method`"));
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        // Notifications (no id) never get a reply.
        let id = id?;
        let result = match method {
            "initialize" => Ok(initialize(&params, self.is_bridge())),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_list(self.is_bridge()) })),
            "tools/call" => self.call(&params),
            "resources/list" => Ok(json!({ "resources": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            _ => Err((METHOD_NOT_FOUND, format!("unknown method `{method}`"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => error_reply(id, code, &message),
        })
    }

    fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((INVALID_PARAMS, "missing tool `name`".to_owned()))?;
        let arguments = params.get("arguments").unwrap_or(&Value::Null);
        match &mut self.backend {
            Backend::Local { session, workspace } => {
                let mut target = Target::new(&self.reg, session, workspace.as_ref());
                call_tool(&mut target, name, arguments).map_err(|e| (INVALID_PARAMS, e))
            }
            Backend::Bridge(client) => bridge_call(client, name, arguments),
        }
    }
}

/// What a tool works on: the open model, and the folder files may be read
/// from and written to. The headless server and the running app each provide
/// one, so the tools behave the same in both.
pub struct Target<'a> {
    pub reg: &'a Registry,
    pub session: &'a mut Session,
    pub workspace: Option<&'a Workspace>,
    /// Set when a tool replaced the model (new, open), so a window showing it
    /// can reset its view.
    pub replaced: bool,
}

impl<'a> Target<'a> {
    pub fn new(
        reg: &'a Registry,
        session: &'a mut Session,
        workspace: Option<&'a Workspace>,
    ) -> Self {
        Self {
            reg,
            session,
            workspace,
            replaced: false,
        }
    }

    fn workspace(&self) -> Result<&'a Workspace, String> {
        self.workspace.ok_or_else(|| {
            "no workspace folder: start the server with --root <folder> (the app: --control-root)"
                .into()
        })
    }

    fn diagram_arg(&mut self, args: &Map<String, Value>) -> Result<Option<String>, String> {
        match args.get("diagram") {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(d)) => crate::find_diagram(self.reg, self.session, d).map(Some),
            Some(_) => Err("`diagram` must be a diagram id or name".into()),
        }
    }

    fn guard_unsaved(&self, args: &Map<String, Value>) -> Result<(), String> {
        let discard = args
            .get("discard")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if self.session.doc.is_modified() && !discard {
            Err(
                "the open model has unsaved changes: model_save first, or pass discard: true"
                    .into(),
            )
        } else {
            Ok(())
        }
    }

    fn info(&mut self) -> Result<Value, String> {
        let diagrams = self
            .reg
            .run(self.session, "model.diagrams", &json!({}))
            .map_err(|e| e.to_string())?;
        Ok(json!({
            "file": self.session.file_name,
            "path": self.session.path,
            "unsavedChanges": self.session.doc.is_modified(),
            "elements": self.session.doc.len(),
            "activeDiagram": self.session.active_diagram,
            "diagrams": diagrams,
            "workspace": self.workspace.map(|w| w.root().display().to_string()),
        }))
    }
}

/// Run the tool `name` on `target`. The value is a `tools/call` result
/// (content blocks and `isError`); `Err` means there is no such tool.
pub fn call_tool(target: &mut Target<'_>, name: &str, arguments: &Value) -> Result<Value, String> {
    let Some(tool) = TOOLS.iter().find(|t| t.name == name) else {
        return Err(format!("unknown tool `{name}`"));
    };
    let empty = Map::new();
    let args = match arguments {
        Value::Null => &empty,
        Value::Object(a) => a,
        _ => return Ok(tool_error("`arguments` must be an object")),
    };
    if let Some(msg) = unknown_argument(tool, args) {
        return Ok(tool_error(&msg));
    }
    Ok(match (tool.run)(target, args) {
        Ok(Reply::Json(v)) => json!({
            "content": [{ "type": "text", "text": pretty(&v) }],
            "isError": false,
        }),
        Ok(Reply::Image { png, note }) => image_result(
            &base64::engine::general_purpose::STANDARD.encode(png),
            &note,
        ),
        Err(e) => tool_error(&e),
    })
}

fn unknown_argument(tool: &Tool, args: &Map<String, Value>) -> Option<String> {
    let k = args
        .keys()
        .find(|k| !tool.args.iter().any(|a| a.name == k.as_str()))?;
    Some(format!(
        "unknown argument `{k}` for {} (accepts: {})",
        tool.name,
        tool.args
            .iter()
            .map(|a| a.name)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// A tool call in bridge mode: the window's own tools go to their control
/// method, the rest run in the app as they would here.
fn bridge_call(
    client: &mut BridgeClient,
    name: &str,
    arguments: &Value,
) -> Result<Value, (i64, String)> {
    if let Some(tool) = LIVE_TOOLS.iter().find(|t| t.name == name) {
        let empty = Map::new();
        let args = match arguments {
            Value::Null => &empty,
            Value::Object(a) => a,
            _ => return Ok(tool_error("`arguments` must be an object")),
        };
        if let Some(msg) = unknown_argument(tool, args) {
            return Ok(tool_error(&msg));
        }
        let screenshot = name == "ui_screenshot";
        let method = if screenshot {
            "ui.screenshot"
        } else {
            "ui.inspect"
        };
        return Ok(match client.call(method, &json!({}), true) {
            Ok(v) if screenshot => image_result(
                v["png_base64"].as_str().unwrap_or_default(),
                &format!("the rust-uml window, {}×{} pixels", v["width"], v["height"]),
            ),
            Ok(v) => json!({
                "content": [{ "type": "text", "text": pretty(&v) }],
                "isError": false,
            }),
            Err(e) => tool_error(&e),
        });
    }
    let Some(tool) = TOOLS.iter().find(|t| t.name == name) else {
        return Err((INVALID_PARAMS, format!("unknown tool `{name}`")));
    };
    let params = json!({ "name": name, "arguments": arguments });
    Ok(match client.call("tool.call", &params, tool.read_only) {
        Ok(result) => result,
        Err(e) => tool_error(&e),
    })
}

enum Reply {
    Json(Value),
    Image { png: Vec<u8>, note: String },
}

type ToolFn = fn(&mut Target<'_>, &Map<String, Value>) -> Result<Reply, String>;

struct Arg {
    name: &'static str,
    schema: &'static str,
    required: bool,
}

const fn arg(name: &'static str, schema: &'static str) -> Arg {
    Arg {
        name,
        schema,
        required: false,
    }
}
const fn req(name: &'static str, schema: &'static str) -> Arg {
    Arg {
        name,
        schema,
        required: true,
    }
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    args: &'static [Arg],
    read_only: bool,
    destructive: bool,
    run: ToolFn,
}

const DIAGRAM: Arg = arg(
    "diagram",
    r#"{"type": "string", "description": "Diagram id or name; default: the open diagram"}"#,
);
const DISCARD: Arg = arg(
    "discard",
    r#"{"type": "boolean", "description": "Throw away unsaved changes in the open model"}"#,
);

const TOOLS: &[Tool] = &[
    Tool {
        name: "model_info",
        title: "Model info",
        description: "The open model: file, unsaved changes, its diagrams (id, name, type, views) and which one is open.",
        args: &[],
        read_only: true,
        destructive: false,
        run: |s, _| s.info().map(Reply::Json),
    },
    Tool {
        name: "model_new",
        title: "New model",
        description: "Start a new, empty model (replaces the open one).",
        args: &[DISCARD],
        read_only: false,
        destructive: true,
        run: |s, a| {
            s.guard_unsaved(a)?;
            *s.session = Session::default();
            s.replaced = true;
            s.info().map(Reply::Json)
        },
    },
    Tool {
        name: "model_open",
        title: "Open model",
        description: "Open a .mdj (or .mfj) file from the workspace folder (replaces the open model).",
        args: &[
            req(
                "path",
                r#"{"type": "string", "description": "Path relative to the workspace folder"}"#,
            ),
            DISCARD,
        ],
        read_only: false,
        destructive: true,
        run: |s, a| {
            s.guard_unsaved(a)?;
            let path = str_arg(a, "path")?;
            let ws = s.workspace()?;
            let text = ws.read(path)?;
            let real = ws.existing(path)?;
            let name = real
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            *s.session = Session::open(&text, name, Some(real.display().to_string()))
                .map_err(|e| format!("{path}: {e}"))?;
            s.replaced = true;
            s.info().map(Reply::Json)
        },
    },
    Tool {
        name: "model_save",
        title: "Save model",
        description: "Save the open model as .mdj inside the workspace folder. Without a path it saves where it was opened from.",
        args: &[arg(
            "path",
            r#"{"type": "string", "description": "Path relative to the workspace folder (.mdj or .mfj)"}"#,
        )],
        read_only: false,
        destructive: true,
        run: |s, a| {
            let path = match a.get("path") {
                None | Some(Value::Null) => s
                    .session
                    .path
                    .clone()
                    .ok_or("the model has never been saved: give a path")?,
                Some(Value::String(p)) => p.clone(),
                Some(_) => return Err("`path` must be text".into()),
            };
            if !(path.ends_with(".mdj") || path.ends_with(".mfj")) {
                return Err("models are saved as .mdj (or .mfj)".into());
            }
            let text = s.session.to_json().map_err(|e| e.to_string())?;
            let written = s.workspace()?.write(&path, text.as_bytes())?;
            s.session.mark_saved();
            s.session.path = Some(written.display().to_string());
            if let Some(n) = written.file_name() {
                s.session.file_name = n.to_string_lossy().into_owned();
            }
            Ok(Reply::Json(
                json!({ "saved": written.display().to_string() }),
            ))
        },
    },
    Tool {
        name: "toolbox",
        title: "Toolbox",
        description: "Without diagram_type: every diagram type diagram.add accepts. With it: the kinds node.create (boxes) and edge.create (lines) accept on that diagram type.",
        args: &[arg(
            "diagram_type",
            r#"{"type": "string", "description": "e.g. UMLClassDiagram or \"Sequence Diagram\""}"#,
        )],
        read_only: true,
        destructive: false,
        run: |_, a| {
            let t = match a.get("diagram_type") {
                None | Some(Value::Null) => None,
                Some(Value::String(t)) => Some(t.as_str()),
                Some(_) => return Err("`diagram_type` must be text".into()),
            };
            crate::toolbox(t).map(Reply::Json)
        },
    },
    Tool {
        name: "diagram_inspect",
        title: "Inspect diagram",
        description: "Boxes (view id, model id, type, name, x, y, width, height, container) and lines (view, model, type, name, tail, head) on a diagram. Use view ids for edge.create and view.move; model ids for element.set and feature.add.",
        args: &[DIAGRAM],
        read_only: true,
        destructive: false,
        run: |s, a| {
            let d = s.diagram_arg(a)?;
            let p = json!({ "diagram": d });
            s.reg
                .run(s.session, "diagram.inspect", &p)
                .map(Reply::Json)
                .map_err(|e| e.to_string())
        },
    },
    Tool {
        name: "render_preview",
        title: "Render preview",
        description: "A PNG image of a diagram, to check layout and look.",
        args: &[
            DIAGRAM,
            arg(
                "max_side",
                r#"{"type": "number", "description": "Longest side in pixels, 64–2048 (default 1024)"}"#,
            ),
        ],
        read_only: true,
        destructive: false,
        run: |s, a| {
            let max_side = match a.get("max_side") {
                None | Some(Value::Null) => 1024.0,
                Some(v) => v
                    .as_f64()
                    .filter(|v| (64.0..=MAX_PREVIEW).contains(v))
                    .ok_or("`max_side` must be 64–2048")?,
            };
            let d = match s.diagram_arg(a)? {
                Some(d) => d,
                None => s.session.active_diagram.clone().ok_or("no open diagram")?,
            };
            let scene = render::build_scene(&s.session.doc, &d, s.session.fonts());
            let side = scene.bounds.w.max(scene.bounds.h).max(1.0);
            let scale = (max_side / side).min(2.0);
            let png = render::to_png(&scene, scale as f32)?;
            Ok(Reply::Image {
                png,
                note: format!(
                    "diagram {d}: {:.0}×{:.0} at scale {scale:.2}",
                    scene.bounds.w, scene.bounds.h
                ),
            })
        },
    },
    Tool {
        name: "diagram_export",
        title: "Export diagram",
        description: "Write a diagram to an .svg or .png file inside the workspace folder (no watermark).",
        args: &[
            req(
                "path",
                r#"{"type": "string", "description": "Path relative to the workspace folder, ending in .svg or .png"}"#,
            ),
            DIAGRAM,
            arg(
                "scale",
                r#"{"type": "number", "description": "PNG scale 0.1–8 (default 2)"}"#,
            ),
        ],
        read_only: false,
        destructive: true,
        run: |s, a| {
            let path = str_arg(a, "path")?.to_owned();
            let d = s.diagram_arg(a)?;
            let svg = path.to_ascii_lowercase().ends_with(".svg");
            let mut p = json!({ "diagram": d });
            if let Some(scale) = a.get("scale") {
                p["scale"] = scale.clone();
            }
            let bytes = if svg {
                let r = s
                    .reg
                    .run(s.session, "export.svg", &p)
                    .map_err(|e| e.to_string())?;
                r["svg"].as_str().unwrap_or_default().as_bytes().to_vec()
            } else {
                let r = s
                    .reg
                    .run(s.session, "export.png", &p)
                    .map_err(|e| e.to_string())?;
                base64::engine::general_purpose::STANDARD
                    .decode(r["png_base64"].as_str().unwrap_or_default())
                    .map_err(|e| e.to_string())?
            };
            let written = s.workspace()?.write(&path, &bytes)?;
            Ok(Reply::Json(
                json!({ "written": written.display().to_string() }),
            ))
        },
    },
    Tool {
        name: "mermaid_import",
        title: "Import Mermaid",
        description: "Add a diagram written in Mermaid (classDiagram, sequenceDiagram, stateDiagram, flowchart or erDiagram) to the open model, laid out automatically. Returns the new diagram's id and any lines that were skipped. One undo step.",
        args: &[
            req(
                "text",
                r#"{"type": "string", "description": "Mermaid text, starting with classDiagram, sequenceDiagram, stateDiagram, flowchart or erDiagram"}"#,
            ),
            arg(
                "name",
                r#"{"type": "string", "description": "Diagram name (default: the Mermaid title)"}"#,
            ),
        ],
        read_only: false,
        destructive: false,
        run: |s, a| {
            let mut p = json!({ "text": str_arg(a, "text")? });
            if let Some(n) = a.get("name").filter(|n| !n.is_null()) {
                p["name"] = n.clone();
            }
            s.reg
                .run(s.session, "import.mermaid", &p)
                .map(Reply::Json)
                .map_err(|e| e.to_string())
        },
    },
    Tool {
        name: "command_list",
        title: "List commands",
        description: "Every command with its id, label, parameter description and whether it can run now.",
        args: &[
            arg(
                "filter",
                r#"{"type": "string", "description": "Only ids or labels containing this text"}"#,
            ),
            arg("enabled_only", r#"{"type": "boolean"}"#),
        ],
        read_only: true,
        destructive: false,
        run: |s, a| {
            let filter = a
                .get("filter")
                .and_then(Value::as_str)
                .map(str::to_lowercase);
            let enabled_only = a
                .get("enabled_only")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let list: Vec<Value> = s
                .reg
                .specs()
                .iter()
                .map(|c| (c, (c.enabled)(s.session)))
                .filter(|(c, on)| {
                    (!enabled_only || *on)
                        && filter.as_ref().is_none_or(|f| {
                            c.id.contains(f.as_str()) || c.label.to_lowercase().contains(f.as_str())
                        })
                })
                .map(|(c, on)| json!({ "id": c.id, "label": c.label, "params": c.params, "enabled": on }))
                .collect();
            Ok(Reply::Json(Value::Array(list)))
        },
    },
    Tool {
        name: "command_run",
        title: "Run command",
        description: "Run one command by id with JSON params (see command_list). Edits are one undo step each.",
        args: &[
            req(
                "id",
                r#"{"type": "string", "description": "Command id, e.g. node.create"}"#,
            ),
            arg(
                "params",
                r#"{"type": "object", "description": "The command's parameters"}"#,
            ),
        ],
        read_only: false,
        destructive: true,
        run: |s, a| {
            let id = str_arg(a, "id")?;
            let params = match a.get("params") {
                None | Some(Value::Null) => json!({}),
                Some(p @ Value::Object(_)) => p.clone(),
                Some(_) => return Err("`params` must be an object".into()),
            };
            s.reg
                .run(s.session, id, &params)
                .map(Reply::Json)
                .map_err(|e| e.to_string())
        },
    },
    Tool {
        name: "command_batch",
        title: "Run commands",
        description: "Run steps in order: [{\"cmd\": id, \"params\": {...}, \"as\": name}]. A later step's params can use an earlier result: \"$name.view\", \"$name.model\", \"$prev.id\". By default it is all-or-nothing: if a step fails, nothing changes.",
        args: &[
            req(
                "steps",
                r#"{"type": "array", "items": {"type": "object", "properties": {"cmd": {"type": "string"}, "params": {"type": "object"}, "as": {"type": "string"}}, "required": ["cmd"]}}"#,
            ),
            arg(
                "atomic",
                r#"{"type": "boolean", "description": "Undo every step if one fails (default true)"}"#,
            ),
        ],
        read_only: false,
        destructive: true,
        run: |s, a| {
            let steps = script::parse_steps(a.get("steps").unwrap_or(&Value::Null))?;
            let atomic = a.get("atomic").and_then(Value::as_bool).unwrap_or(true);
            match script::run(s.reg, s.session, &steps, atomic) {
                Ok(results) => Ok(Reply::Json(json!({ "results": results }))),
                Err(f) => Err(f.to_string()),
            }
        },
    },
];

fn str_arg<'a>(a: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    a.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{key}` must be text"))
}

/// Tools that only exist when driving a running window (bridge mode).
const LIVE_TOOLS: &[Tool] = &[
    Tool {
        name: "ui_inspect",
        title: "Inspect window",
        description: "What the user has open in the rust-uml window: file, unsaved changes, open diagram, selection, chosen tool, zoom, status line and open dialogs.",
        args: &[],
        read_only: true,
        destructive: false,
        run: |_, _| Err("ui_inspect needs a running window (rust-uml-cli mcp --bridge)".into()),
    },
    Tool {
        name: "ui_screenshot",
        title: "Screenshot window",
        description: "A PNG of the whole rust-uml window as the user sees it, panels included. To see just a diagram, use render_preview.",
        args: &[],
        read_only: true,
        destructive: false,
        run: |_, _| Err("ui_screenshot needs a running window (rust-uml-cli mcp --bridge)".into()),
    },
];

fn initialize(params: &Value, bridge: bool) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .and_then(|v| PROTOCOL_VERSIONS.iter().find(|p| **p == v))
        .copied()
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "rust-uml",
            "title": "rust-uml",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": if bridge {
            format!("{INSTRUCTIONS}{LIVE_INSTRUCTIONS}")
        } else {
            INSTRUCTIONS.to_owned()
        },
    })
}

fn tool_list(bridge: bool) -> Vec<Value> {
    let live: &[Tool] = if bridge { LIVE_TOOLS } else { &[] };
    TOOLS
        .iter()
        .chain(live)
        .map(|t| {
            let mut props = Map::new();
            for a in t.args {
                let schema = serde_json::from_str(a.schema).unwrap_or_else(|_| json!({}));
                props.insert(a.name.to_owned(), schema);
            }
            let required: Vec<&str> = t
                .args
                .iter()
                .filter(|a| a.required)
                .map(|a| a.name)
                .collect();
            json!({
                "name": t.name,
                "title": t.title,
                "description": t.description,
                "inputSchema": {
                    "type": "object",
                    "properties": props,
                    "required": required,
                    "additionalProperties": false,
                },
                "annotations": {
                    "title": t.title,
                    "readOnlyHint": t.read_only,
                    "destructiveHint": t.destructive,
                    "idempotentHint": t.read_only,
                    "openWorldHint": false,
                },
            })
        })
        .collect()
}

fn image_result(png_base64: &str, note: &str) -> Value {
    json!({
        "content": [
            { "type": "image", "data": png_base64, "mimeType": "image/png" },
            { "type": "text", "text": note },
        ],
        "isError": false,
    })
}

fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn error_reply(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
