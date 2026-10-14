# Parity with StarUML 7.1.1

Status line: **v0.6.0: every UML diagram type. M5 started: flowcharts (with Mermaid import).** Updated 2026-10-14.

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
| StarUML-edited files reopen here | ✅ | Owner edited `shapes.mdj` in StarUML (new abstract/active class, renamed literal); reopens and round-trips unchanged |
| StarUML v1 (`.uml`) import | ⬜ | `extensions/default/staruml-v1` |
| Mermaid import | 🟡 | ✅ class, sequence, state and flowchart (File ▸ Import Mermaid…, `rust-uml-cli import`, MCP `mermaid_import`), laid out automatically, one undo step. ⬜ ER, mind map, requirement (need those families). Composite states stay nested (StarUML flattens them); flowcharts also read Mermaid's named shapes (`A@{ shape: doc }`), which StarUML doesn't |

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
| UML structure | Package, Object, Component, Deployment, Composite Structure, Profile | | ✅ all six: StarUML's toolbox groups, nesting, ports, parts, objects/slots, 3-D nodes, icons, containment ⊕, «deploy». Not yet: n-ary associations, association classes, template bindings, frames, role bindings, ball-and-socket |
| UML behaviour (M3a) | Use Case, Statechart, Activity | | ✅ StarUML's tool sets: actors, use cases, subjects, include/extend; states, composite/orthogonal states, all pseudostate kinds, transitions with `trigger [guard] / effect`; actions, control nodes, swimlanes, object nodes, guarded flows. Not yet: pins, expansion regions, interruptible regions, signals/events as actions, internal activities (entry/do/exit), submachine states, frames |
| UML sequence | Sequence | | ✅ frame, lifelines (with role attributes), all 7 message kinds, activations (hidden when the diagram's `showActivation` is off), combined fragments with operands/guards; messages keep their height when lifelines move. Not yet: automatic sequence numbers (`showSequenceNumber`), found/lost messages, gates, endpoints, state invariants, continuations, interaction uses, time/duration constraints |
| UML communication | Communication | | ✅ StarUML's tool set: lifelines, connectors (between the lifelines' roles), self connectors, forward/reverse messages (drawn between lifelines they bring the connector; dropped on a connector they ride on it), numbered `1 : name` labels. Not yet: frames, moving a message along its connector, sequence ↔ communication conversion |
| UML interaction overview | Interaction Overview | | ✅ Interaction Use (`ref` frame naming the called interaction), Interaction (Inline) (an `sd` frame drawing its own sequence diagram inside), control nodes and flows. Not yet: choosing which existing interaction a use refers to (each creates a new one) |
| UML information flow | Information Flow | | ✅ Information items, information flows (dotted, «flow»), plus the class and use-case tools. Not yet: editing the conveyed items (shown when a file has them) |
| UML timing | Timing | | ✅ Lifelines in the timing frame, states/conditions, time segments (stepped state line), time ticks, messages between segments; the frame arranges them as StarUML does, lifeline names turned. Not yet: time constraints, duration constraints, dragging a segment to another state |
| SysML | Requirement, Block Definition, Internal Block, Parametric | ✔ | ⬜ |
| BPMN | BPMN | ✔ | ⬜ |
| Wireframe | Wireframe | ✔ | ⬜ |
| AWS | AWS | ✔ | ⬜ |
| GCP | GCP | ✔ | ⬜ |
| Azure | Azure | | ⬜ |
| C4 | C4 | | ⬜ |
| ERD | ERD | | ⬜ |
| DFD | DFD | | ⬜ |
| Flowchart | Flowchart | | ✅ All 27 of StarUML's symbols and labelled flows; lines end on diamond and circle outlines; Mermaid `flowchart`/`graph` import |
| Mind map | Mind map | | ⬜ |

## Editing

| Feature | Status | Notes |
|---|---|---|
| Create elements from toolbox | ✅ | Tools follow the diagram type (StarUML's groups); click on a box to create inside it |
| Boxes inside boxes | 🟡 | Create inside, move and delete together; dragging an existing box into/out of a container ⬜ |
| Notes, text, note links | ✅ | Rectangles/ellipses/images/hyperlinks from StarUML's annotation set ⬜ |
| Draw relationships by dragging | ✅ | |
| Move / resize | ✅ | Attached edges re-route; sub-view positions shift with their box |
| Undo / redo | ✅ | One step per command; text edits commit on Enter/blur |
| Delete from view / from model | ✅ | Cascades to views, relationships, dangling references |
| Property editor | 🟡 | Name, stereotype, visibility, flags, types, multiplicity, defaults, parameters, association ends, documentation |
| Model explorer | 🟡 | Tree, select, open diagram; no drag-and-drop yet |
| Quick edit (inline text syntax) | ⬜ | StarUML uses PEG grammars in `uml/grammars` |
| Copy / paste | ⬜ | |
| Alignment tools | ⬜ | `extensions/default/alignment` |
| Auto layout | 🟡 | Layered layout (`render::autolayout`) used by Mermaid import, nested containers included; no menu command for existing diagrams yet |
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
| CLI: scripts, chained steps, batch over a folder | ✅ | Beyond StarUML. `run --cmd … --as …`, `--script`, `batch`; all-or-nothing; `docs/agents.md` |
| CLI: per-subcommand help, JSON output | ✅ | `help <sub>`, `--help`, `info/commands/tools --json` |
| MCP server for AI agents | ✅ | `rust-uml-cli mcp --root <dir>`: 11 tools incl. image previews; StarUML reaches MCP through its HTTP API instead |
| Drive the running desktop app (control channel, MCP bridge) | ✅ | Beyond StarUML. `rust-uml --control`, token file, agent files confined to `--control-root`; `mcp --bridge` adds `ui_inspect`, `ui_screenshot`. Pointer/key driving ⬜ |
| CLI: ejs code generation, html, pdf | ⬜ | |
| Local HTTP API (`/generate_diagram`, …) | ⬜ | Same command registry would back it |
| Code generation / reverse engineering | ⬜ | Separate StarUML extensions, not in the installed app |
| Minimap, thumbnails, relationship view | ⬜ | |
| Themes (light/dark) | 🟡 | Follows egui's theme |
| Preferences, keymaps | ⬜ | |
