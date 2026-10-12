# How StarUML 7.1.1 works — evidence

Target: `C:\Program Files\StarUML` (StarUML 7.1.1, `productId` `STARUML.V7`, from `package.json` inside `app.asar`).

Everything below was read from the installed files. Nothing was executed. The REA record for the
whole archive is `evidence/rea-app-asar.json` (Evidence `ev_918ac867…2e09777`, archive SHA-256
`205ef1de…261baca9`). The file excerpts were read straight out of `app.asar` with the small readers in
[`tools/evidence/`](../tools/evidence/), which parse the asar table of contents and print one member file.
The REA record is kept locally and is not committed (it quotes StarUML code).

This document records **what StarUML does and how its files look**. The Rust code in this repo is
written from scratch; no StarUML code is copied into it.

**Confirmed by running the Rust renderer on StarUML's own files:** the profile diagram shipped in
`UMLConventional.mdj` draws with every box, routed line and stereotype in place, and box sizes match
StarUML's stored sizes to within a few pixels (Liberation Sans has Arial's metrics).

---

## 1. What the program is made of

| Observation | Where |
|---|---|
| It is an **Electron** app: a copy of Chrome plus Node.js, running a web app. `StarUML.exe` (177 MB) is the Chrome/Node runtime; `LICENSES.chromium.html`, `libEGL.dll`, `vk_swiftshader.dll` are stock Electron parts. | install folder listing |
| All of StarUML's own logic is JavaScript inside **`resources/app.asar`** (100 MB, ~3,700 JS files, most of them third-party libraries). | REA statistics: 3,721 parsed JS files, 31 parse failures (all in bundled libraries) |
| Two halves: a **main process** (`src/main-process/*`: windows, menus, file dialogs, licensing, a CLI, a local HTTP API) and a **renderer** (the window's web page: `src/core`, `src/engine`, `src/ui`, `src/views`). They talk over **43 named IPC channels** (35 main-side handlers). | REA summary `ipc`; `ipcMain.handle/on` in `src/main-process/application.js` |
| UI is old-school web: jQuery 2.1, Kendo UI widgets, CodeMirror, LESS stylesheets, HTML templates in `src/static/html-contents/`. | `src/static/thirdparty/` |
| Libraries used for real work: `pdfkit` + `svg-to-pdfkit` (PDF export), `svgcanvas` (SVG export), `dagre` (auto-layout), `pegjs` (parsers for the quick-edit text syntax), `ejs`/`mustache` (templates, HTML docs), `express` (local API), `electron-updater`. | `package.json` dependencies |

## 2. The core idea: everything is driven by a metamodel

StarUML doesn't hard-code "a Class has attributes". Each diagram family ships a
**`metamodel.json`** that declares every element type, its parent type, and its fields. The engine
(`src/core/core.js`, `src/core/repository.js`) is generic and just follows those declarations.

Each field has a **kind** that decides how it is stored:

| kind | meaning | count across all metamodels |
|---|---|---|
| `prim` | plain value (string / int / bool / real) | 274 |
| `enum` | one of a fixed word list, e.g. visibility `public/protected/private/package` | 40 |
| `ref` / `refs` | pointer(s) to another element by id | 148 / 17 |
| `obj` / `objs` | owned child element(s), nested inside | 3 / 58 |
| `var` | either a pointer **or** a plain string (e.g. an attribute's `type` can point to a class or just say `"int"`) | 11 |
| `custom` | special text format (`Font`, `Points`) | 2 |

### Element families shipped in `extensions/essential/`

| Family | element types | of which views | diagram types |
|---|---|---|---|
| core (`resources/default`) | 23 | 12 | `Diagram` |
| UML | 284 | 142 | Class, Package, Composite Structure, Object, Component, Deployment, Use Case, Statechart, Activity, Sequence, Communication, Timing, Interaction Overview, Information Flow, Profile |
| SysML | 68 | 42 | Requirement, Block Definition, Internal Block, Parametric |
| BPMN | 102 | 30 | BPMN |
| Flowchart | 61 | 30 | Flowchart |
| Wireframe | 42 | 20 | Wireframe |
| AWS / Azure / GCP | 24 / 14 / 15 | 12 / 7 / 7 | one each |
| C4 / ERD / DFD / Mind map | 16 / 11 / 13 / 7 | 8 / 4 / 6 / 2 | one each |
| **Total** | **680** | | **~28** |

Every family is laid out the same way: `metamodel.json` (types), `elements.js` (how each type is drawn),
`*-factory.js` (how new elements are created), `*-commands.js` (menu actions), plus `toolbox/`,
`menus/`, `quickedits/`, `preferences/` JSON.

Bundled tools in `extensions/default/`: alignment, auto-layout (`diagram-layout`), diagram generator,
find, HTML export, Markdown docs, **Mermaid import**, minimap, diagram thumbnails, relationship view,
open-recent, Robustness toolbox, a StarUML v1 file importer.

## 3. The `.mdj` file format

An `.mdj` file is one JSON tree (shape observed in StarUML's default template and in the
`Reader`/`Writer` classes of `src/core/repository.js`). This example was written by rust-uml
(`rust-uml-cli new`); ids are shortened:

Described in our words (StarUML's code is not reproduced here):

- a list of eight diagram type names counts as "Pro": `SysMLRequirementDiagram`,
  `SysMLBlockDefinitionDiagram`, `SysMLInternalBlockDiagram`, `SysMLParametricDiagram`,
  `BPMNDiagram`, `WFWireframeDiagram`, `AWSDiagram`, `GCPDiagram`;
- before drawing an export, if the licence status is *trial*, an "UNREGISTERED" watermark is drawn;
  otherwise, if the edition is not "PRO" and the diagram's type is in that list, a "PRO ONLY"
  watermark is drawn.

In plain words:

- **Unregistered / trial**: every exported image or PDF gets an "UNREGISTERED" watermark.
- **Standard licence**: clean exports, *except* the 8 "Pro" diagram types (SysML ×4, BPMN, Wireframe,
  AWS, GCP), which get "PRO ONLY" stamped on export.
- **Pro licence**: everything clean.
- A search for `"PRO"`, `edition`, `getLicenseStatus`, `UNREGISTERED`, and `trialDaysLeft` across all
  non-`node_modules` code found **no other feature checks**. Editing is never locked, only exports.
  The title bar and the activation dialog only *display* trial status. (This is a bounded search over
  string matches, so a check built from computed strings would not show up.)

So in a from-scratch rebuild, "the paid features" means: the SysML, BPMN, Wireframe, AWS and GCP
diagram families, plus clean SVG/PNG/PDF export. There is no licensing in this rebuild at all.

Not in this app: the code generators/reverse-engineers (Java, C#, C++, Python…) are separate
open-source extensions downloaded from `staruml.io/api/extensions/registry.json`, so they're out of
scope for "the installed app".

## 5. Other observed features worth matching later

- **CLI** (`src/main-process/cli.js`): `ejs` (code generation with templates), `image` (export
  diagram images), `html` (HTML docs), `pdf`, `exec` (run a command).
- **Local HTTP API** (`src/main-process/api-server.js`, Express): `GET /`, `POST /generate_diagram`
  (takes Mermaid code), `/get_all_diagrams_info`, `/get_current_diagram_info`,
  `/get_diagram_image_by_id`. This looks like an AI/MCP integration hook.
- **Undo/redo** (`OperationBuilder` in `repository.js`): every edit is recorded as a list of small
  reversible operations: insert, remove, field assign, field insert/remove/reorder, relocate.
- Light/dark themes, a keymap, menus, and preferences are all JSON-driven
  (`resources/default/{menus,keymaps,preferences}`).

## 6. Unknowns / limits of this evidence

- Everything here is static reading. **No runtime behaviour was observed.** Things like "StarUML
  re-lays out a class box when it opens a file" are inferred from the code shape, not watched.
- REA's analysis of the whole archive hit its size budget on 3,676 bundled-library files (Mermaid
  parser, Kendo cultures); their static facts are kept on file nodes, but cross-file relationships
  there are incomplete. None of these are StarUML's own `src/` or diagram-family code.
- The exact drawing rules (fonts, padding, arrow sizes) were **not** copied. The Rust renderer uses
  UML-standard notation with its own measurements, so pixel output will differ.
- The real proof of file compatibility is opening a file saved by the Rust app in StarUML, and
  the other way round. That needs a sample `.mdj` saved from StarUML with a real class diagram.
