# Roadmap and handoff

**Read this to pick up the work.** Status as of 2026-10-14: v0.7.0 (flowcharts, ER diagrams and mind maps, each with its Mermaid import); M5 continues with DFD, C4 and Azure.
Order of reading for a new agent/session: `AGENTS.md` → this file → `docs/parity.md` → `log/devlog.md`.

## 1. Where things stand

- **Released:** v0.7.0 (pre-release) — https://github.com/rats4final/rust-uml/releases
  Linux x64, Windows x64, macOS arm64 + x64, web zip. Web app live at
  https://rats4final.github.io/rust-uml/ (redeploys on every push to `main`).
- **Works:** class diagrams, all six UML structural diagram types, and use case, statechart,
  activity, sequence, communication, timing, interaction overview and information flow diagrams
  (every UML diagram type), flowcharts, ER diagrams and mind maps, with StarUML's toolbox groups (see `rust-uml-cli tools`; rows in
  `crates/uml/src/catalog.rs`). Properties panel, undo/redo, lossless `.mdj` save, SVG/PNG export
  without watermarks.
- **Verified:** 161 tests (incl. mouse-driven UI tests and a real-file round trip of all 14 model files
  shipped with StarUML); CI green on Linux, Windows, macOS; Windows exe launched natively; web build
  screenshotted in Chromium; StarUML interop proven both ways for class diagrams (M1.5).
  **Not verified:** macOS app actually launching (no Mac); other diagram families with StarUML.

## 2. Setting up a machine to continue

```sh
# Rust (stable) + targets used by the checks
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
rustup target add wasm32-unknown-unknown x86_64-pc-windows-gnu aarch64-apple-darwin
cargo install trunk --locked          # only for building the web app locally
git clone https://github.com/rats4final/rust-uml && cd rust-uml
cargo test && cargo xtask layers && cargo xtask wasm
```

Reference material (optional, for evidence gathering):

- StarUML 7.1.1 installed on Windows at `C:\Program Files\StarUML` (WSL: `/mnt/c/Program Files/StarUML`).
  `tools/evidence/*.py` read its `resources/app.asar` without extracting it.
- The REA MCP server (`analyze_javascript_application` on the asar). Its full analysis was exported to
  `evidence/rea-app-asar.json` on the original machine only (git-ignored, 187 MB, quotes StarUML code).
- Ghidra 12.1.4 is installed on the original WSL at `~/opt/ghidra_12.1.4_PUBLIC` (`ghidra`,
  `ghidra-headless` on PATH, `GHIDRA_INSTALL_DIR` set). StarUML is JavaScript, so Ghidra is only
  needed for native code (REA found **no** native addons in StarUML 7.1.1).

## 3. Rules that don't change

Clean-room (behaviour and file format only; never copy StarUML code/assets/icons; never touch
licensing), never crash, everything is a command, files stay lossless, never break wasm, verify UI by
driving it. Details in `AGENTS.md`. The product name is **rust-uml**; never call it StarUML.

## 4. What's next, in order

Each milestone ends with: tests, `docs/parity.md` rows updated, a `log/devlog.md` entry, a tag for a
release when it's user-visible.

**Order (owner, 2026-10-13):** the paid families (M4) come last. Next up is M5, then M6 onward; the
milestone numbers are kept so older notes still point at the right sections.

### M1.5 — Prove StarUML interoperability ✅ done 2026-10-12

- rust-uml → StarUML: the owner opened `examples/shapes.mdj` in StarUML 7.1.1; no errors.
- StarUML → rust-uml: the owner edited it in StarUML (added an abstract, active class; renamed a
  literal) and saved; rust-uml reopens it and saves it back with identical content and key order.
- Bugs it exposed, all fixed with tests: end labels mirrored (alpha sign); rows centred in StarUML
  (missing `horizontalAlignment` = metamodel default centre → now written as left); labels without
  `edgePosition` drawn mid-line and lines without `lineStyle` assumed oblique (now metamodel
  defaults, `mdj::values::absent`); active classes drawn (double side borders).
- Lesson for every later family: see `AGENTS.md` rule 5 (defaults) and `docs/evidence.md` §3.
- `examples/build-shapes.sh` regenerates the example from CLI commands only.

### M2 — UML structural diagrams ✅ done 2026-10-12 (v0.2.0)

- Tools are data: `crates/uml/src/catalog.rs` (one row per StarUML toolbox tool, plus which groups
  show on which diagram type). **Adding a tool for a later milestone = a catalog row** + notation in
  `crates/render/src/layout.rs` (`shape_of`, `heading`) / `scene.rs` (`edge_style`) if it looks new.
- Nesting: `uml::container_owner` decides what a box can hold; `view.move` carries contents;
  deleting a box deletes what's inside (`VIEW_DEPENDENCIES` in `crates/uml/src/delete.rs`).
- Example built from CLI commands only: `examples/build-structures.sh` → `examples/structures.mdj`
  (one diagram per type). `rust-uml-cli run … -d <diagram>` picks the diagram to work on;
  `rust-uml-cli tools [type]` lists tools.
- Left for later (each small, in `docs/parity.md`): drag a box into/out of a container; ports
  sliding along the border; n-ary association, association class, template binding, frame, role
  binding; ball-and-socket (required interfaces); interface `decoration` display; StarUML's other
  annotations (rect, ellipse, image, hyperlink).
- **Not yet checked in StarUML:** open `examples/structures.mdj` in StarUML 7.1.1 and compare each
  diagram (as M1.5 did for class diagrams). Most likely differences: port label positions,
  icon/keyword choices, nested view geometry.

### M3a — Use case, statechart, activity ✅ done 2026-10-12 (v0.3.0)

- Catalog rows for StarUML's `uml.usecase`, `uml.statechart(-adv)`, `uml.activity(-adv)` groups.
- New placement rules (`catalog::Slot`): region `vertices`, activity `nodes`/`groups`; edges owned by
  the source's region (`transitions`) or activity (`edges`) (`catalog::EdgeOwner`).
- `uml::create_diagram` adds the wrappers StarUML adds (state machine + region, activity, profile).
  This also fixed M2's profile diagrams, which previously sat directly in the model.
- Notation: stick-figure actors, use-case ovals, subjects, rounded states/actions, symbols by model
  (`layout::Symbol`), composite-state region dividers, swimlanes, transition/flow labels.
- Example: `examples/build-behaviour.sh` → `examples/behaviour.mdj`.
- Left (see parity): pins, expansion/interruptible regions, internal activities, submachines,
  triggers/effects editing in the properties panel (labels already show them), frames.
- **Not yet checked in StarUML:** open `examples/behaviour.mdj` and `structures.mdj` in StarUML.

### M3b part 1 — Sequence diagrams ✅ done 2026-10-12 (v0.4.0)

- `create_diagram` wraps a sequence diagram as StarUML does (collaboration → interaction →
  diagram) and adds the "sd" frame; lifelines create a role attribute (`represent`).
- Lifeline views get a `linePart`; messages attach to line parts (`create_edge_at` takes the
  drag's height); calls get activation bars; `render::reflow` keeps message heights and moves
  activations when lifelines move. Frames and fragments are picked by their tag only.
- Message notation by `messageSort`; combined fragments with operands (`feature.add` kind
  `operand`) and guards. Example: the "Checkout" diagram in `examples/behaviour.mdj`.
- Left: found/lost messages, gates/endpoints, state invariants, continuations, interaction uses,
  time/duration constraints; dragging a message vertically (today: delete and redraw).

### A — Agent-ready, PhotoCraft-style ✅ (parts 1–3 in v0.5.0, part 4 after; the owner put it before M3b part 2)

The owner asked for PhotoCraft-style CLI and AI integrations ahead of more diagram families, and
said **no built-in AI assistant panel**: AI stays outside the app, through the CLI and MCP (no API
keys inside rust-uml). Guide for users: `docs/agents.md`.

1. ✅ **CLI:** `run` chains steps (`--cmd/--params/--as`, `--script file|-`, `--new`, repeatable
   `-o` by extension), all-or-nothing; `batch --in --out [--script] [--image]`; `help <sub>`;
   `--json` listings. Engine additions for agents: `name` on create commands, `element.set
   {values}`, `diagram.inspect`, `model.tree`. Shared layer: `crates/automation` (`script`,
   `workspace`, `mcp`).
2. ✅ **MCP server:** `rust-uml-cli mcp --root <dir>`, hand-rolled JSON-RPC over stdio (no SDK
   dependency), 11 tools incl. `render_preview` (PNG image content) and `command_batch`. Tested by
   an in-process conversation and by spawning the binary. **Not yet tried from a real client** —
   do `claude mcp add rust-uml -- rust-uml-cli mcp --root <dir>` and ask it to build a diagram.
3. ✅ **Mermaid import:** crate `mermaid` (L0, no dependencies) parses class, sequence and state
   text; `render::autolayout` is a layered layout (cycles, nesting, 4 directions); command
   `import.mermaid {text, name?, parent?}` builds the diagram from ordinary commands, draws the
   lines only after layout, and squashes it into one undo step (`Document::squash_undo_since`;
   `command_batch` uses it too). File ▸ Import Mermaid…, `rust-uml-cli import`, MCP
   `mermaid_import`. Mappings follow StarUML's (`docs/evidence.md`, Mermaid section). Samples in
   `examples/mermaid/`. Flowcharts, ER and mind maps added in M5. Left: requirement (needs SysML),
   automatic sequence numbers, Auto Layout as a menu command (the layout exists).
   **Not yet checked in StarUML:** open an imported model in StarUML 7.1.1.
4. ✅ **Live app control:** `rust-uml --control <port> --control-token-file <path>
   [--control-root <dir>]`, a loopback JSON-lines channel (PhotoCraft's design: token on the first
   line, 60 s reply deadline that says whether the request ran). Methods `engine.execute`,
   `engine.commands`, `tool.call`, `ui.inspect`, `ui.screenshot`. `tool.call` runs the MCP tools
   themselves (`automation::mcp::call_tool`) on the window's model, so `rust-uml-cli mcp --bridge`
   just forwards them and adds `ui_inspect`/`ui_screenshot`. Transport and dispatch in
   `automation::control` (no egui; tested over real sockets with a stand-in window), client in
   `automation::bridge`, UI side `ui::RemoteJob` (jobs run between frames), glue in
   `apps/desktop/src/control.rs`. Verified by driving the real window under WSLg through the bridge.
   Left: pointer/key driving (`ui.pointer`-style), StarUML's HTTP API on the same server (M8),
   **trying it from a real MCP client** (Claude Code) — the end-to-end run used a scripted client.

### M3b part 2 — Communication, timing, interaction overview, information flow ✅ done 2026-10-13

- ✅ **Communication** (2026-10-13): catalog rows can now be limited to diagram types
  (`NodeSpec::diagrams`, `EdgeSpec::diagrams`; look-ups `node_spec_on`/`edge_spec_on`/`tool_on`),
  because StarUML reuses tool ids (`UMLLifeline`) with different views. Connectors between lifelines
  join their roles (`EdgeOwner::TailRole`); Forward/Reverse Message create the connector too, or
  ride on one (`UMLCommMessageView`, drawn after edges in `render::scene`). Extra messages on one
  connector are stacked further out. Example: "Checkout (communication)" in `behaviour.mdj`.
  Evidence: `docs/evidence.md`, communication section. **Not yet checked in StarUML.**
- ✅ **Interaction overview** and **information flow** (2026-10-13). `create_diagram` takes a chain
  of wrappers (overview: interaction → activity). The overview's two frame tools are actions whose
  `target` is created by `uml::factory::add_overview_target`; the inline frame draws its sequence
  diagram scaled inside (`render::scene::draw_nested`, nesting capped at 2). Frame tags use
  StarUML's keywords (`frame_kind`). Examples: "Shopping overview" (`behaviour.mdj`), "Payment
  information" (`structures.mdj`). **Not yet checked in StarUML.**
- ✅ **Timing** (2026-10-13): slots `Timing{Lifeline,State,Segment,Tick}` put each piece in its
  container (lifelines/ticks default to the frame); `uml::factory::place_timing_view` stores a
  starting geometry, but the renderer arranges from structure every draw (`render::timing`,
  StarUML's rules), messages attach to arranged segments, and `node_layout` never grows these
  views. Text can be turned (`TextStyle::vertical`: SVG rotate, egui text angle). Example "Door
  alarm" in `behaviour.mdj`. Left: time/duration constraints. **Not yet checked in StarUML.**

Evidence pointers: `communicationDiagramFn`, `timingDiagramFn`, `interactionOverviewDiagramFn` and
their `viewFor…DiagramFn` in `uml-factory.js` (communication messages create a connector between
lifelines plus a message; see `messageFn`). Toolbox groups `uml.communication`, `uml.timing`,
`uml.interaction-overview`, `uml.information-flow` in `toolbox/uml.json`.

### M5 — Remaining families (in progress)

Order chosen 2026-10-13: Flowchart, ERD, Mind map (each unlocks a Mermaid import), then DFD, C4,
and Azure last (provider icons: check their licence first).

- ✅ **Flowchart** (2026-10-13): 27 symbol rows (`catalog::flowchart`), `FCFlow` with a name label
  only (`EdgeLabels::Name`), `FCFlowchart` wrapper. `render::layout::Flow` lists the symbols:
  minimum sizes and label bands per symbol; `scene::draw_flowchart` draws them, curves sampled
  (`cubic`, `quadratic`). Lines now end on diamond and oval outlines (`clip_to_outlines`), which
  helps UML use cases and decision nodes too. Look-ups match ids before labels, and a label
  shared by several families resolves by the open diagram's toolbox. Example
  `examples/flowchart.mdj` from `examples/scripts/login-flowchart.json`.
- ✅ **Mermaid flowchart import** (2026-10-14): `mermaid::flowchart` reads `flowchart`/`graph`
  (bracket shapes, `A@{ shape: …, label: … }`, chains, `&` groups, every link form, link ids,
  entity codes); `engine::import` maps shapes to symbols (`symbol`), StarUML's mapping for the
  bracket shapes, extended to the named ones. Subgraphs are flattened and link styles dropped,
  each with a warning; `~~~` links only steer the layout. Sample `examples/mermaid/login.mmd`.
  **Not yet checked in StarUML.**
- ✅ **ERD** (2026-10-14): `ERDEntity` row with a `columnCompartment` (columns are features:
  `feature.add` kind `column`); three relationship rows sharing `ERDRelationship`, told apart by
  end cardinality (`EdgeSpec::end_fields`), drawn with rounded right angles as StarUML's preference
  says (`EdgeSpec::line_style`) and only between entities. `create_diagram` wraps the diagram in a
  `Data Model`. `render::layout::entity_layout` lays out the keys | name | type table as StarUML
  does; `scene::draw_crow` draws the crow's feet. Mermaid `erDiagram` import (`mermaid::er`,
  `engine::import::er`): symbols and word forms, aliases, PK/FK/UK, comments, `varchar(255)` →
  type + length; `..` stays non-identifying (StarUML's importer loses that). Example
  `examples/erd.mdj` from `examples/scripts/shop-erd.json`, sample `examples/mermaid/shop.mmd`.
  Left: Suppress Columns menu item, ERD validation rules, column quick-edit syntax.
  **Not yet checked in StarUML.**
- ✅ **Mind map** (2026-10-14): `MMNode` (rounded box, `layout` wraps the name to the box) and
  `MMEdge` (plain line, no labels, oblique, nodes only); `create_diagram` wraps the diagram in a
  `Mindmap`. Mermaid `mindmap` import (`mermaid::mindmap`; `mermaid::Line` now keeps each line's
  indentation): one node per line, a line from each child to its parent as StarUML draws them,
  laid out by `engine::import::mind_layout` (root in the middle, branches split left/right by
  height, iterative). Shapes become rounded boxes, with one warning. Example `examples/mindmap.mdj`
  from `examples/scripts/trip-mindmap.json`, sample `examples/mermaid/trip.mmd`.
  Left: Add Node Left/Right quick edits. **Not yet checked in StarUML.**
- ✅ **DFD** (2026-10-14): `catalog::dfd` rows for external entity, process and data store
  (`nameLabel` + `idLabel`; only the name label gets the name), `DFDDataFlow` (named, filled
  arrowhead, rounded right angles), only between DFD elements. `create_diagram` wraps the diagram
  in a `DataFlowModel` unless the parent is already a DFD element, so a process holds its lower
  level. `render::layout::dfd_layout` places id and name with StarUML's padding; `Shape::DataStore`
  is the box open on the right. No Mermaid syntax exists for DFDs. Example `examples/dfd.mdj` from
  `examples/scripts/order-dfd.json`. **Not yet checked in StarUML.**
- ✅ **C4** (2026-10-14): `catalog::c4` rows (person, software system, component, element) and
  `c4_container` rows (plain, database, web, desktop and mobile app tools, all `C4Container` with a
  `kind`), each view with name, technology and description labels and StarUML's preference fill
  colour stored. `C4Relationship` (dotted, filled arrowhead, `EdgeLabels::DescriptionAndTechnology`).
  `create_diagram` adds a `C4Model` only under the project, as StarUML; a diagram inside a C4
  element holds its contents. `render::layout::c4_layout` stacks the labels with StarUML's padding
  per shape; an element whose diagram lies inside it is drawn as that diagram's boundary
  (`layout::encloses`). Relationship descriptions wrap. New command `view.add {model, x, y}`.
  Example `examples/c4.mdj` from `examples/scripts/bank-c4.json`. **Not yet checked in StarUML.**
- ✅ **Azure** (2026-10-14): group, service, callout and connector rows; groups hold Azure
  elements (`container_owner`); `NodeSpec::floating_labels` is now data (a service's name 42 px
  below it). **Icons:** Microsoft's terms only allow distribution "for the permitted use", so the
  owner chose (2026-10-14) not to ship them: the user downloads the set and points rust-uml at it.
  `render::Prim::Icon` names the icon and carries a placeholder; `render::IconSource` supplies
  pictures (`to_svg_with`/`to_png_with` embed them as data URIs, the canvas rasterises them with
  `render::rasterize_svg`); `automation::icons::FolderIcons` indexes a folder by StarUML's names
  (`render::icon_key`: `10035-icon-service-App-Services.svg` = `App-Services.svg`; 596 of
  StarUML's 605 names match Microsoft's set of July 2026). Every session starts with the
  process-wide default (`engine::set_default_icons`), set from `--azure-icons`,
  `RUST_UML_AZURE_ICONS` or the desktop's remembered folder (`apps/desktop/src/settings.rs`).
  Example `examples/azure.mdj` from `examples/scripts/webapp-azure.json`.
  **Not yet checked in StarUML.** Left: icons in the web app (no folder access in browsers).

**M5 is complete**: every family StarUML offers for free is in.

### M6 — Editing depth

Copy/paste (StarUML: `clipboard-manager.js`), marquee multi-select, keyboard nudge, alignment and
distribution (`extensions/default/alignment`), auto layout (StarUML uses dagre; pick a Rust layered
layout or write one), line style switching UI (rectilinear/oblique/rounded/curve; codes in
`mdj::values::line_style`), colour/font UI (view `fillColor`, `lineColor`, `fontColor`, `font`),
tags and tagged-value editor, find, validation rules (`resources/default/rules.js`,
`uml/rules.js`, `uml-rules.md`), quick edit (StarUML's inline text syntax, `uml/grammars/*.pegjs`),
model explorer drag-and-drop and context menus, relationship view, minimap, diagram thumbnails.

### M7 — Export

PDF (vector; StarUML uses pdfkit — consider `printpdf`/`krilla` from the Scene), JPEG, HTML docs
(`extensions/default/html-export`), Markdown docs. All without watermarks.

### M8 — Integration

- Local HTTP API mirroring StarUML's (`/generate_diagram`, `/get_all_diagrams_info`,
  `/get_current_diagram_info`, `/get_diagram_image_by_id`) on top of the command registry; desktop
  only. (Moved into milestone A part 4.)
- ~~MCP server~~ done in milestone A; ~~Mermaid import~~ moved to milestone A part 3.
- StarUML v1 `.uml` import.
- CLI parity: `ejs`-style code generation (use a Rust template engine such as minijinja), `html`, `pdf`.

### M9 — Packaging

App icon; macOS `.app` bundle + signing/notarisation; Windows icon, installer (MSI/MSIX) and signing;
Linux AppImage/Flatpak; auto-update; smaller wasm (7.3 MB now; fonts are embedded twice, once in
`render` and once registered with egui — share one copy).

### M4 — The families StarUML sells ("Pro") — last, by the owner's choice (2026-10-13)

SysML (Requirement, Block Definition, Internal Block, Parametric), BPMN, Wireframe, AWS, GCP.
These are the paid features; here they are ordinary features with watermark-free export.
Evidence per family: `extensions/essential/<family>/metamodel.json` (types/fields) and behaviour in
`elements.js`. **Icons:** AWS/GCP/Azure diagrams need provider icons. Do not copy StarUML's. Use the
providers' official icon sets only if their licenses allow redistribution (check each; record in
`ATTRIBUTION.md`), or draw original simplified icons.

## 5. Known gaps and debt (fix opportunistically)

| Where | Gap |
|---|---|
| `render` | Edges whose end is another edge (association classes, constraints on links) are skipped. |
| `render` | Interface lollipop/socket notation not drawn; interfaces we create pin `stereotypeDisplay: label`. |
| `render`/`uml` | Sub-view geometry (compartment/label `left/top/width/height`) is never recomputed; only shifted on move. StarUML recomputes it, so this is cosmetic in the file only. |
| `uml` | Audit the other view types we create for fields whose intended value ≠ metamodel default (only rows and `propertyLabel` were checked). |
| `mdj` | Undoing removal of a single-child (`obj`) field re-inserts the key at the end (key order changes). |
| `ui` | No marquee select, no clipboard. |
| `ui` | Linux file dialogs need an XDG portal or `zenity`; fallback is File ▸ Open Path… |
| `render` | Right-angle lines start apart only when routed (new lines, moved boxes); a straight line between two boxes can't move when they face each other along less than one spacing. Oblique lines aren't spread. |
| `engine` | Renames refresh name/id label text; other derived label texts (stereotype, namespace, edge labels) are still not stored. StarUML recomputes them when drawing. |
| release | Binaries unsigned; macOS ships a bare binary, not an `.app`. |

## 6. How to verify work (recipes)

- Everything: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test && cargo xtask layers && cargo xtask wasm && cargo xtask targets`.
- Real StarUML files: `crates/mdj/tests/staruml_files.rs` (auto-finds the install; `STARUML_ASAR` to override).
- Look at a diagram: `cargo run -p render --example render_file -- model.mdj /tmp/out` (SVG + PNG).
- Look at the app: `cargo run -p desktop -- model.mdj --screenshot /tmp/w.png`.
- Drive the UI in tests: see `crates/ui/src/app.rs` tests (`egui_kittest`).
- Web: `cd apps/web && trunk serve`; headless screenshot with Chromium
  `--screenshot --virtual-time-budget=8000`.
- Release: push a `vX.Y.Z` tag; `.github/workflows/release.yml` builds and publishes.

## 7. Decisions already made (don't relitigate)

| Decision | Why |
|---|---|
| Clean-room rebuild, not a port | StarUML is proprietary; copying its code is infringement. |
| Name `rust-uml` | "StarUML" is MKLabs' trademark. Owner said "for now". |
| MIT OR Apache-2.0 | Rust standard; permissive with a patent grant. |
| egui/eframe | One codebase for desktop and wasm. |
| Liberation Sans bundled | Arial metrics → same box sizes as StarUML; OFL licensed. |
| Schema-free `.mdj` loader | Unknown element types survive round trips. |
| Layout like PhotoCraft | Owner asked to follow github.com/storytold/photocraft's organisation. |
