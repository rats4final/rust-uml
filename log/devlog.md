# Dev log

Newest first. Terse: what landed, numbers, what's still open.

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
