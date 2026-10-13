# Dev log

Newest first. Terse: what landed, numbers, what's still open.

## 2026-10-13 — M3b part 2: interaction overview and information flow

- `create_diagram` builds a chain of wrappers; interaction overview diagrams live in an activity in
  an interaction. Interaction Use / Interaction (Inline) are call-behaviour actions targeting a new
  interaction use or a new sequence diagram; activity control nodes and flows work unchanged.
- Renderer: `ref` frames name the called interaction; inline frames draw their sequence diagram
  scaled inside (nesting capped); overview boxes are clickable everywhere. Frame tags now use
  StarUML's keywords (class diagrams say `class`, not `pkg`; unknown types say nothing).
- Information items («information», classifier compartments) and information flows (dotted,
  «flow», conveyed names); the class and use-case tools are offered on these diagrams too.
- Examples "Shopping overview" and "Payment information". 132 tests.
- Open: timing diagrams; pick an existing interaction for a use; edit conveyed items; check in
  StarUML.

## 2026-10-13 — M3b part 2: communication diagrams

- Catalog rows may be limited to diagram types; the engine and toolbox pick by the open diagram
  (`node_spec_on`, `edge_spec_on`, `tool_on`). Communication: lifeline box, connector (ends and
  owner via the lifelines' roles), self connector, forward/reverse message.
- A message drawn between lifelines brings its connector in the same undo step; dropped on a
  connector (canvas or `edge.create` head) it rides on it. `diagram.inspect` lists such messages
  under `lines` with `on`.
- Renderer: `UMLCommMessageView` as a 40 px arrow beside the connector, label `1 : name` numbered
  by message order (`uml::numbered_message_text`); extra messages on a connector stacked 35 px out.
- Fixed a flaky CLI test on macOS CI (temp folders named by a microsecond clock collided).
- 127 tests. Open: check in StarUML; timing, interaction overview, information flow.

## 2026-10-13 — agent milestone, part 4: live app control

- `rust-uml --control <port> --control-token-file <path> [--control-root <dir>]`: loopback-only
  JSON lines; 256-bit token (`getrandom`), file created 0600, a group/world-readable one refused,
  compared in constant time; auth must be the first line; max 8 connections, 64 MB lines.
- Methods `ping`, `engine.execute`, `engine.commands`, `tool.call`, `ui.inspect`, `ui.screenshot`.
  `tool.call` runs the MCP tools on the window's model: the tools now take a `mcp::Target`, shared
  by the headless server and the app. `file.new` over the channel refuses to drop unsaved work.
- Requests run between frames (`ui::RemoteJob`); a request that times out unrun is never run
  later ("timeout: not run" vs "timeout: no reply").
- `rust-uml-cli mcp --bridge 127.0.0.1:<port> --control-token-file <path>` forwards every tool and
  adds `ui_inspect`/`ui_screenshot`; edits are never resent, reads retried once.
- Desktop flags parsed strictly (unknown options are errors now). 121 tests; drove the real window
  under WSLg via the bridge (Mermaid import, inspect, screenshot, save inside/outside the root).
- Open: try from Claude Code itself; pointer/key driving; StarUML's HTTP API on this server.

## 2026-10-13 — v0.5.0: agent milestone parts 1–3 released

- Release notes for the scriptable CLI, MCP server and Mermaid import.
- Release archives now include `examples/mermaid/` and `examples/scripts/`, so the README's
  commands work from an unpacked download.
- Open: agent milestone part 4 (live app control); check imported models in StarUML.

## 2026-10-12 — agent milestone, part 3: Mermaid import

- New crate `mermaid` (L0): class, sequence and state diagram text → data; styling skipped,
  unknown lines become warnings, unclosed blocks are errors. 12 parser tests.
- `render::autolayout`: layered layout (cycle breaking, longest-path ranks, barycentre ordering,
  neighbour pull), 4 directions. The importer nests it for packages and composite states.
- `engine` command `import.mermaid`: builds from ordinary commands, lines drawn after layout
  (drawn first they kept detours around the piled-up boxes), one undo step via
  `mdj::Document::squash_undo_since`; failure restores everything. StarUML's arrow mapping
  (`-->>` async signal, `--)` reply) and `showActivation`/`showSequenceNumber` written as false.
- Renderer honours `showActivation`. `command_batch`/scripts are now one undo step when atomic.
- File ▸ Import Mermaid… (examples, inline errors; UI test drives it), `rust-uml-cli import`
  (`--into`, `--name`, `-o`), MCP `mermaid_import`. `examples/mermaid/*.mmd`. 109 tests.
- Open: check an imported model in StarUML; flowchart/ER/mindmap/requirement need their families;
  sequence numbers aren't drawn; Auto Layout menu command.

## 2026-10-12 — agent milestone, parts 1–2: scriptable CLI and MCP server

- Owner chose agent features (PhotoCraft-style) before M3b part 2; no in-app AI panel.
- `engine`: `name` on `diagram.add`/`node.create`/`edge.create`/`feature.add`; `element.set
  {id, values}` (one undo step, all checked first); `diagram.inspect`; `model.tree`. 24 commands.
- New crate `automation` (L4): scripts with `$name.path` references, all-or-nothing by default;
  `Workspace` confines files to one folder (no `..`, no writing through links, model/image
  extensions only); MCP server (JSON-RPC over stdio, 11 tools, PNG previews).
- `rust-uml-cli`: `run` chains `--cmd/--params/--as`, `--script`, `--new`, repeatable `-o`;
  `batch`; `help <sub>`; `--json`; `mcp --root`. Old `run <file> <cmd> <json>` form unchanged.
- `examples/scripts/shop.json`; `docs/agents.md`. 87 tests; wasm/Windows/macOS checks clean.
- Open: try the MCP server from a real client; Mermaid import; live-app control channel.

## 2026-10-12 — v0.4.0: sequence diagrams

- Sequence diagrams as StarUML builds them: collaboration → interaction → diagram + frame;
  lifelines with role attributes and line parts; 7 message kinds attached to line parts at the
  drag height; activations; combined fragments with operands and guards.
- Reflow keeps message heights and moves activations when lifelines move.
- Message labels always above the arrow (the generic angle rule put right-to-left ones below).
- Found while doing it: an M3a edit (region views as boxes, not rows) had silently not applied;
  its test only checked the view type. Fixed, and the test now checks the field too.
- 70 tests; 54 box / 30 line tools. Open: communication, timing, interaction overview, info flow.

## 2026-10-12 — v0.3.0: M3a use case, statechart, activity

- Catalog now offers 52 box and 23 line tools across all toolbox groups (counted with `rust-uml-cli tools`).
- Placement in regions/activities; transitions/flows owned by region/activity.
- `uml::create_diagram` adds StarUML's wrappers (state machine + region, activity, profile) —
  also fixes M2 profile diagrams. Engine `diagram.add` uses it.
- Render: actors, use cases, subjects, round rects, `Symbol`s by model, region dividers, swimlanes,
  `trigger [guard] / effect` and `[guard]` labels, «include»/«extend».
- Process lesson: a text edit made after `cargo fmt` can match nothing; edit scripts now assert
  their anchors (one silent miss was caught by a failing test).
- 67 tests. Example `behaviour.mdj`. Open: check examples in StarUML; M3b (sequence et al.).

## 2026-10-12 — v0.2.0: M2 UML structural diagrams

- `uml` rebuilt around a catalog (`catalog.rs`): 23 box + 17 line tools mirroring StarUML's toolbox
  ids and per-diagram groups; generic factories; nesting (`containerView`/`containedViews`); parts
  and ports as attributes; containment line relocates the model; slots on instances.
- `render`: package/model/subsystem folders, 3-D nodes, component/artifact icons, underlined
  instances with slots, parts (dashed when shared), ports with floating names, dashed collaboration
  ellipses, notes, text, lollipops; ⊕ containment end; «deploy». Containers paint first.
- `engine`: `diagram.add` (7 types), `node.create` with `container`, contents move with their box.
- `ui`: toolbox follows the diagram type; Model ▸ Add Diagram; properties for the new kinds.
- `cli`: `tools [type]`, `run … -d <diagram>` (the example script found each run reset to Main).
- `mdj`: `Editor::relocate`. 59 tests; wasm/Windows/macOS checks clean.
- Open: check `examples/structures.mdj` in StarUML; drag-into-container; see roadmap M2 list.

## 2026-10-12 — v0.1.1: interop proven both ways

- Owner edited `shapes.mdj` in StarUML and reopened it here: works; round-trips unchanged.
- Found the defaults rule: StarUML loads missing fields as metamodel defaults and omits fields equal
  to them. Fixed: rows now store left alignment; `edgePosition`/`lineStyle`/`alpha`/`distance`
  fallbacks now match StarUML (`mdj::values::absent`). Active classes drawn.
- Restored `docs/evidence.md` §3–§4: the publishing-day cleanup script had deleted the file-format
  rules (its "```js" search matched "```json").
- `examples/build-shapes.sh` rebuilds the example from CLI commands. 37 tests.

## 2026-10-12 — StarUML opens our files

- Owner opened `examples/shapes.mdj` in StarUML 7.1.1: loads cleanly, all elements drawn.
- Fixed: association end labels were mirrored vs StarUML (alpha sign). Regression test added.
- Open: StarUML centred the rows of our class boxes; check whether that's its default.
- Next: edit + save in StarUML, reopen in rust-uml.

## 2026-10-12 — handoff

- `docs/roadmap.md`: setup, milestones M1.5–M9 in order, known gaps, verification recipes, decisions.
- Original machine now has Ghidra 12.1.4 (`~/opt`, own JDK 21) for any native-code evidence work.

## 2026-10-12 — rust-uml 0.1.0: public release

Landed
- Renamed to rust-uml (binaries `rust-uml`, `rust-uml-cli`, `rust-uml-web`). "StarUML" is a trademark.
- Dual licensed MIT OR Apache-2.0. Evidence doc now paraphrases StarUML's code instead of quoting it.
- GitHub Actions: CI (Linux checks + Windows/macOS build and test), Release on `v*` tags (Linux x64,
  Windows x64, macOS arm64 + x64, web zip), Pages deploy of the web app.
- The view fits the whole diagram when a file or diagram opens; View ▸ Fit Diagram / Actual Size.
- Release binaries: desktop 17 MB, CLI 5.8 MB (Linux x64).

Still open
- macOS and Windows binaries are unsigned (Gatekeeper/SmartScreen warnings).

## 2026-10-12 — milestone 1: core

Landed
- Evidence: StarUML 7.1.1 is Electron; all logic in `app.asar`. Paid features = export watermarks only
  (`src/engine/diagram-export.js`): "UNREGISTERED" on trial, "PRO ONLY" on 8 diagram types for
  Standard. Editing is never locked. See `docs/evidence.md`.
- `mdj`: schema-free `.mdj`/`.mfj` load/save, lossless. All 14 model files shipped with StarUML
  round-trip with identical content and key order (incl. 726-element UML profile). Undo/redo
  as reversible ops; failed edits roll back.
- `uml`: class-diagram factories in StarUML's exact view-tree shape; cascading delete.
- `render`: layout + display list; SVG and PNG export (no watermarks). Liberation Sans gives
  Arial metrics: StarUML's own profile diagram renders with boxes within a few px of StarUML's sizes.
- `engine`: 22 commands; fuzz test proves no command panics on hostile params.
- `ui` + apps: desktop (Linux/Windows/macOS), CLI, web. Mouse-driven UI tests via egui_kittest.
- Web: `trunk build --release` → 7.3 MB wasm; screenshot in headless Chromium shows the full UI.
- Desktop screenshot via `--screenshot` on WSLg shows a demo model built entirely through CLI commands.
- Checks: 33 tests; clippy `-D warnings` with no-panic lints; `xtask layers`; wasm32, Windows
  and macOS type-check.

Still open
- Not yet verified: a file saved here opened in real StarUML. Try `examples/shapes.mdj` (built only through CLI commands).
- Every non-class diagram family (see `docs/parity.md`).
