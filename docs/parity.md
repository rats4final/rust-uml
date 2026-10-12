# Parity with StarUML 7.1.1

Status line: **milestone 1 (core) — class diagrams.** Updated 2026-10-12.

Each row comes from the evidence in [`evidence.md`](evidence.md). "Paid" marks features StarUML
locks behind a licence (see evidence §4: only exports are locked, by watermark).

Legend: ✅ done · 🟡 partial · ⬜ not started

## Files

| Feature | Status | Notes |
|---|---|---|
| Open `.mdj` | ✅ | Schema-free loader; unknown element types survive untouched |
| Save `.mdj` | ✅ | Same content and key order as StarUML's own files (14/14 shipped files) |
| Open/save `.mfj` profile fragments | ✅ | Root `_parent` outside the file is preserved |
| Files saved here open in StarUML | ✅ | `examples/shapes.mdj` opened in StarUML 7.1.1 (2026-10-12): all boxes, rows, keywords, arrows and labels correct |
| StarUML-edited files reopen here | ⬜ | Next check: edit + save in StarUML, reopen in rust-uml |
| StarUML v1 (`.uml`) import | ⬜ | `extensions/default/staruml-v1` |
| Mermaid import | ⬜ | `extensions/default/mermaid`; also behind the local API |

## Export (paid in StarUML: watermarks)

| Feature | Status | Notes |
|---|---|---|
| SVG | ✅ | No watermark for any diagram type |
| PNG | ✅ | Bundled fonts, identical on every platform |
| JPEG | ⬜ | |
| PDF | ⬜ | StarUML uses pdfkit |
| HTML docs | ⬜ | `extensions/default/html-export` |
| Markdown docs | ⬜ | `extensions/default/markdown` |

## Diagram families

| Family | Diagram types | Paid in StarUML | Status |
|---|---|---|---|
| UML class | Class | | ✅ Class, Interface, Enumeration, DataType, Package; 7 relationship kinds |
| UML structure | Package, Object, Component, Deployment, Composite Structure, Profile | | 🟡 drawn generically (boxes, lines, stereotypes, notation by edge type); no dedicated tools |
| UML behaviour | Use Case, Activity, Statechart, Sequence, Communication, Timing, Interaction Overview, Information Flow | | ⬜ (generic drawing only) |
| SysML | Requirement, Block Definition, Internal Block, Parametric | ✔ | ⬜ |
| BPMN | BPMN | ✔ | ⬜ |
| Wireframe | Wireframe | ✔ | ⬜ |
| AWS | AWS | ✔ | ⬜ |
| GCP | GCP | ✔ | ⬜ |
| Azure | Azure | | ⬜ |
| C4 | C4 | | ⬜ |
| ERD | ERD | | ⬜ |
| DFD | DFD | | ⬜ |
| Flowchart | Flowchart | | ⬜ |
| Mind map | Mind map | | ⬜ |

## Editing

| Feature | Status | Notes |
|---|---|---|
| Create elements from toolbox | ✅ | Class diagram set |
| Draw relationships by dragging | ✅ | |
| Move / resize | ✅ | Attached edges re-route; sub-view positions shift with their box |
| Undo / redo | ✅ | One step per command; text edits commit on Enter/blur |
| Delete from view / from model | ✅ | Cascades to views, relationships, dangling references |
| Property editor | 🟡 | Name, stereotype, visibility, flags, types, multiplicity, defaults, parameters, association ends, documentation |
| Model explorer | 🟡 | Tree, select, open diagram; no drag-and-drop yet |
| Quick edit (inline text syntax) | ⬜ | StarUML uses PEG grammars in `uml/grammars` |
| Copy / paste | ⬜ | |
| Alignment tools | ⬜ | `extensions/default/alignment` |
| Auto layout | ⬜ | StarUML uses dagre |
| Line styles (rectilinear / oblique / curve / round) | 🟡 | Drawn and routed as straight or right-angle; no UI to switch yet |
| Colours and fonts per view | 🟡 | Read and drawn; no UI to change yet |
| Find | ⬜ | |
| Validation rules | ⬜ | `resources/default/rules.js`, `uml/rules.js` |
| Tags / tagged values | ⬜ | Preserved in files; no editor |
| Profiles / stereotypes as references | 🟡 | Displayed; editing sets text |

## Application

| Feature | Status | Notes |
|---|---|---|
| Desktop: Linux, Windows, macOS | ✅ | One build per OS; macOS type-checked only (no Mac to run on yet) |
| Browser (WebAssembly) | ✅ | trunk build (7.3 MB wasm), runs in Chromium; open = file picker, save = download |
| CLI: image export | ✅ | `rust-uml-cli image` (PNG/SVG) |
| CLI: run any command | ✅ | `rust-uml-cli run <file> <command> <json>` |
| CLI: ejs code generation, html, pdf | ⬜ | |
| Local HTTP API (`/generate_diagram`, …) | ⬜ | Same command registry would back it |
| Code generation / reverse engineering | ⬜ | Separate StarUML extensions, not in the installed app |
| Minimap, thumbnails, relationship view | ⬜ | |
| Themes (light/dark) | 🟡 | Follows egui's theme |
| Preferences, keymaps | ⬜ | |
