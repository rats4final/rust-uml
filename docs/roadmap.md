# Roadmap and handoff

**Read this to pick up the work.** Status as of 2026-10-12, after v0.1.0.
Order of reading for a new agent/session: `AGENTS.md` → this file → `docs/parity.md` → `log/devlog.md`.

## 1. Where things stand

- **Released:** v0.1.0 (pre-release) — https://github.com/rats4final/rust-uml/releases/tag/v0.1.0
  Linux x64, Windows x64, macOS arm64 + x64, web zip. Web app live at
  https://rats4final.github.io/rust-uml/ (redeploys on every push to `main`).
- **Works:** class diagrams end to end (5 element kinds, 7 relationship kinds, properties panel,
  undo/redo, lossless `.mdj` save, SVG/PNG export without watermarks). Any StarUML file opens;
  non-class diagrams draw in a simplified generic way and cannot be edited with tools yet.
- **Verified:** 33 tests (incl. mouse-driven UI tests and a real-file round trip of all 14 model files
  shipped with StarUML); CI green on Linux, Windows, macOS; Windows exe launched natively; web build
  screenshotted in Chromium. **Not verified:** macOS app actually launching (no Mac); a file saved by
  rust-uml opened in real StarUML.

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

### M1.5 — Prove StarUML interoperability (small, do first)

1. Open `examples/shapes.mdj` (made only by rust-uml) in real StarUML 7.1.1. Expected risk: StarUML
   may need sub-view geometry (`left/top/width/height` of compartments and labels) or fields we omit.
   If it fails or looks wrong, compare with a file StarUML saves for the same diagram:
   `rust-uml-cli run x.mdj model.get '{"id":"…"}'` vs the StarUML file, field by field.
2. The reverse: build a class diagram in StarUML, save, open in rust-uml, edit, save, reopen in StarUML.
3. Turn both into a test fixture **we** author (never commit StarUML-made files: they contain
   StarUML's generated content; describe differences in tests instead).

### M2 — UML structural diagrams (tools, not just generic drawing)

Package, Object, Component, Deployment, Composite Structure, Profile diagrams. For each:
`tools/evidence/mm_fields.py` on the element and view types (e.g. `UMLComponent,UMLComponentView,
UMLNode,UMLNodeView,UMLPort,UMLPortView`) to get the exact file shape; add factories in `crates/uml`;
notation in `crates/render` (component icon, node 3D box, port squares, object underlined name);
toolbox sets per diagram type in `crates/ui` (the toolbox should switch by active diagram type);
`diagram.add-*` commands. Include interface "lollipop/socket" (`stereotypeDisplay: icon`).

### M3 — UML behavioural diagrams

Use Case (actors, use cases, system boundary, include/extend), Activity (actions, decisions, forks,
swimlanes, object flows), Statechart (states, regions, pseudostates, transitions), Sequence
(lifelines, messages, activations, combined fragments — the hardest: layout is vertical-time based),
Communication, Timing, Interaction Overview, Information Flow.

### M4 — The families StarUML sells ("Pro")

SysML (Requirement, Block Definition, Internal Block, Parametric), BPMN, Wireframe, AWS, GCP.
These are the paid features; here they are ordinary features with watermark-free export.
Evidence per family: `extensions/essential/<family>/metamodel.json` (types/fields) and behaviour in
`elements.js`. **Icons:** AWS/GCP/Azure diagrams need provider icons. Do not copy StarUML's. Use the
providers' official icon sets only if their licenses allow redistribution (check each; record in
`ATTRIBUTION.md`), or draw original simplified icons.

### M5 — Remaining families

Azure, C4, ERD, DFD, Flowchart, Mind map.

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
  `/get_current_diagram_info`, `/get_diagram_image_by_id`) on top of the command registry; desktop only.
- MCP server exposing the commands (the registry already has ids, labels and param docs).
- Mermaid import (StarUML: `extensions/default/mermaid`), StarUML v1 `.uml` import.
- CLI parity: `ejs`-style code generation (use a Rust template engine such as minijinja), `html`, `pdf`.

### M9 — Packaging

App icon; macOS `.app` bundle + signing/notarisation; Windows icon, installer (MSI/MSIX) and signing;
Linux AppImage/Flatpak; auto-update; smaller wasm (7.3 MB now; fonts are embedded twice, once in
`render` and once registered with egui — share one copy).

## 5. Known gaps and debt (fix opportunistically)

| Where | Gap |
|---|---|
| `render` | Edges whose end is another edge (association classes, constraints on links) are skipped. |
| `render` | Interface lollipop/socket notation not drawn; interfaces we create pin `stereotypeDisplay: label`. |
| `render`/`uml` | Sub-view geometry (compartment/label `left/top/width/height`) is never recomputed; only shifted on move. StarUML may expect it (see M1.5). |
| `render` | Curve and rounded line styles draw as straight/right-angle. |
| `mdj` | Undoing removal of a single-child (`obj`) field re-inserts the key at the end (key order changes). |
| `ui` | Toolbox is class-diagram only regardless of diagram type. No marquee select, no clipboard. |
| `ui` | Linux file dialogs need an XDG portal or `zenity`; fallback is File ▸ Open Path… |
| `ui` | Properties text buffers are never pruned (harmless, grows per edited field). |
| `.github` | Actions warn about Node 20 deprecation: bump `actions/checkout`, `upload-artifact`, `download-artifact` majors. |
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
