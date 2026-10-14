# Scripts, batch runs and AI agents

Every action in rust-uml is a command with an id and JSON parameters (`rust-uml-cli commands`).
The desktop app, the web app, the CLI and the MCP server all run the same commands through the same
registry, so anything you can click, a script or an AI agent can do too. The shared layer is
`crates/automation`.

## Scripts

A script is a JSON list of steps. A step's parameters can use the result of an earlier step: a
string that is exactly `$name` or `$name.path` is replaced by that part of the result of the step
saved `as` that name. `$prev` is the step just before. Path parts that are numbers index into lists.
Any other string is left alone; start one with `$$` for a literal `$`.

```json
[
  {"cmd": "diagram.add", "params": {"type": "Class Diagram", "name": "Shop"}},
  {"cmd": "node.create", "params": {"kind": "Class", "x": 40, "y": 40, "name": "Cart"}, "as": "cart"},
  {"cmd": "node.create", "params": {"kind": "Class", "x": 320, "y": 40, "name": "Item"}, "as": "item"},
  {"cmd": "edge.create", "params": {"kind": "Composition", "tail": "$cart.view", "head": "$item.view"}},
  {"cmd": "feature.add", "params": {"classifier": "$cart.model", "kind": "attribute", "name": "total"}},
  {"cmd": "element.set", "params": {"id": "$prev.id", "values": {"type": "Money", "visibility": "private"}}}
]
```

A script is all-or-nothing: if a step fails, the model is left exactly as it was and nothing is
written. A script that succeeds is one undo step. A full example is `examples/scripts/shop.json`.

Useful commands for building diagrams:

| Command | Does | Returns |
|---|---|---|
| `diagram.add {type, name?}` | adds a diagram and opens it | `{id}` |
| `node.create {kind, x, y, name?, container?}` | adds a box | `{model, view}` |
| `edge.create {kind, tail, head, name?}` | joins two box **views** | `{model, view}` |
| `feature.add {classifier, kind, name?}` | adds an attribute, operation, literal, slot or operand | `{id}` |
| `parameter.add {operation, direction?}` | adds a parameter (`return` sets the return type) | `{id}` |
| `element.set {id, values: {…}}` | sets fields, e.g. `type`, `visibility`, `multiplicity` | |
| `diagram.inspect {diagram?}` | the boxes and lines on a diagram, with ids and positions | |
| `model.tree {id?, depth?}` | the model's elements as a tree | |

`rust-uml-cli tools [diagram-type]` lists the `kind`s each diagram type accepts.

## The CLI

```sh
# Several steps at once, then save and export (-o can repeat; the extension decides)
rust-uml-cli run --new \
  --cmd diagram.add --params '{"type": "Class Diagram", "name": "Shop"}' \
  --cmd node.create --params '{"kind": "Class", "x": 40, "y": 40, "name": "Cart"}' --as cart \
  --cmd node.create --params '{"kind": "Class", "x": 320, "y": 40, "name": "Item"}' --as item \
  --cmd edge.create --params '{"kind": "Composition", "tail": "$cart.view", "head": "$item.view"}' \
  -o shop.mdj -o shop.png

# A script file (or - for stdin) on an existing model, on a chosen diagram
rust-uml-cli run model.mdj --script steps.json -d "Main" -o model.mdj

# A script over every .mdj in a folder, plus an image of every diagram
rust-uml-cli batch --in models --out updated --script steps.json --image png

# Machine-readable listings
rust-uml-cli info model.mdj --json
rust-uml-cli commands --json
rust-uml-cli tools "Sequence Diagram" --json

# Mermaid text → a laid-out diagram (or add it to an existing model with --into)
rust-uml-cli import examples/mermaid/checkout.mmd -o checkout.mdj -o checkout.png

# Every subcommand explains itself
rust-uml-cli help run
```

## MCP: letting an AI agent drive rust-uml

`rust-uml-cli mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server on
stdin/stdout. It holds one open model. Files are read and written only inside `--root` (default:
the folder it starts in); `..` is refused, links are never written through, and only `.mdj`, `.mfj`,
`.svg` and `.png` files can be written.

**Claude Code:**

```sh
claude mcp add rust-uml -- rust-uml-cli mcp --root /path/to/models
```

**Claude Desktop** and other clients that take a JSON config:

```json
{
  "mcpServers": {
    "rust-uml": {
      "command": "/path/to/rust-uml-cli",
      "args": ["mcp", "--root", "/path/to/models"]
    }
  }
}
```

Then ask, for example: *"Open shop.mdj and add a Payment class linked to Order, then show me the
diagram."*

| Tool | Does |
|---|---|
| `model_info` | the open model: file, unsaved changes, diagrams |
| `model_new`, `model_open {path}`, `model_save {path?}` | start, open, save (refuse to drop unsaved changes unless `discard: true`) |
| `toolbox {diagram_type?}` | the diagram types, or the box and line kinds of one |
| `diagram_inspect {diagram?}` | boxes and lines with view ids, model ids, names and positions |
| `render_preview {diagram?, max_side?}` | the diagram as a PNG image, so the agent can look at its work |
| `diagram_export {path, diagram?, scale?}` | write `.svg`/`.png` into the workspace |
| `mermaid_import {text, name?}` | a Mermaid class, sequence, state or flowchart diagram → a laid-out diagram |
| `command_list {filter?, enabled_only?}` | every command and its parameters |
| `command_run {id, params?}` | run one command |
| `command_batch {steps, atomic?}` | run a script (all-or-nothing by default) |

The MCP tools accept a diagram's id or its name (raw commands take ids). Bad arguments come back as tool errors the agent
can read and fix, not as protocol failures.

## Live: an agent drives the open window

Start the app with a control channel, and point the MCP server at it with `--bridge`. Every tool
above then works on the model **shown in the window**, so you watch the agent build the diagram,
and two more tools let it see what you see:

```sh
rust-uml --control 7878 --control-token-file ~/.rust-uml-token --control-root ~/models
claude mcp add rust-uml-live -- rust-uml-cli mcp --bridge 127.0.0.1:7878 \
  --control-token-file ~/.rust-uml-token
```

| Extra tool (bridge only) | Does |
|---|---|
| `ui_inspect` | file, unsaved changes, open diagram, selection, chosen tool, zoom, status line, open dialogs |
| `ui_screenshot` | a PNG of the whole window, panels included |

**Safety.** The app listens on `127.0.0.1` only. The token file is created on first use with a new
256-bit token, readable by you alone (on Unix an existing file that others can read is refused);
the bridge must present it before anything runs, and a wrong token closes the connection. Agent
file access (`model_open`, `model_save`, `diagram_export`) is confined to `--control-root` with the
same rules as `--root`; without `--control-root` agents can't touch files at all. Your own File
menu is unaffected. `file.new` and `model_new` refuse to throw away unsaved work unless asked to
(`discard: true`).

### The control protocol

For writing your own client. One JSON object per line, both ways. The first line on a connection:

```json
{"id": 0, "method": "auth", "params": {"token": "<64 hex digits>"}}
```

Then each request `{"id", "method", "params"}` gets `{"id", "ok": true, "result"}` or
`{"id", "ok": false, "error"}`:

| Method | Params | Result |
|---|---|---|
| `ping` | | `{}` |
| `engine.execute` | `{command, params?}` | the command's result (see `rust-uml-cli commands`) |
| `engine.commands` | | every command with `enabled` for the current state |
| `tool.call` | `{name, arguments?}` | an MCP tool from the table above, run on the open model (an MCP `tools/call` result) |
| `ui.inspect` | | as `ui_inspect` |
| `ui.screenshot` | | `{png_base64, width, height}` |

Requests run on the UI thread between frames. A request still queued after 60 s is dropped
unrun and says `timeout: not run` (safe to resend); `timeout: no reply` means it started and may
still apply. A minimised window may not draw frames, so requests to it time out. The bridge
never resends an edit after a broken connection; it reports that the edit may have applied.

## Not yet

- StarUML's local HTTP API (`/generate_diagram`, …), on the same control server.
- Driving the UI by pointer and keys (clicks, drags) over the control channel.

See `docs/roadmap.md`.
