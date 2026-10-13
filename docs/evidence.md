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

```json
{ "_type": "Project", "_id": "P1", "name": "Untitled",
  "ownedElements": [
    { "_type": "UMLModel", "_id": "M1", "_parent": { "$ref": "P1" }, "name": "Model",
      "ownedElements": [
        { "_type": "UMLClassDiagram", "_id": "D1", "_parent": { "$ref": "M1" },
          "name": "Main", "defaultDiagram": true } ] } ] }
```

Rules, all observed:

- Every element has `_type` (its metamodel type name) and `_id`.
- **Owned children are nested inline**, and each child points back with `"_parent": {"$ref": id}`.
- **Pointers are `{"$ref": "<id>"}`** objects; pointer lists are arrays of those.
- **Missing field = metamodel default.** When loading, StarUML gives a field that is absent from
  the file the `default` declared in `metamodel.json`, *not* the value its own code would set for a
  new element. When saving, it leaves out every field equal to that default
  (`Element.load`/`Element.save` in `src/core/core.js`). Consequences, confirmed by opening
  rust-uml files in StarUML 7.1.1 on 2026-10-12:
  - attribute/operation/literal rows are created *left*-aligned by StarUML's code, but the metamodel
    default for `horizontalAlignment` is centre, so a row without the field shows centred. StarUML's
    own files store `"horizontalAlignment": 0`; rust-uml now does too;
  - `edgePosition` 0 (label follows the *head* end) and `lineStyle` 0 (rectilinear) are the
    defaults, so StarUML omits them; a reader must not assume "middle" or "oblique".
  rust-uml keeps these defaults in `mdj::values::absent`.
- **Diagrams own their views.** A diagram's `ownedViews` holds view elements. A view says which model
  element it shows (`model: {$ref}`), plus its position and size (`left`, `top`, `width`, `height`) for
  boxes, or its end views (`head`, `tail`) and bend points for lines.
- **Sub-view geometry is recomputed by StarUML.** Files rust-uml writes without positions for
  compartments and labels open fine; StarUML fills in `left/top/width/height` for every compartment
  and label when it saves (observed 2026-10-12).
- `custom` text formats (from `src/core/graphics.js`):
  - `Points`: `"x1:y1;x2:y2;…"`, running from the tail end to the head end.
  - `Font`: `"face;size;style"` (style bits: 1 bold, 2 italic), where `?` in a slot means "inherit".
- **Label placement** (`EdgeParasiticView`): `edgePosition` picks the head end (0), middle (1) or
  tail end (2); the label sits `distance` away at angle `alpha` from the line's direction, with
  `alpha` measured opposite to screen angles (confirmed against StarUML's drawing).
- **IDs** (`IdGenerator` in `src/core/core.js`): hex of `timestamp(16 hex digits) + counter(4) + random(4)`,
  converted to bytes and then base64. So it's 12 bytes, 16 base64 characters. Older files have 20-character
  ids with an extra prefix. Any unique string loads fine, because ids are only compared, never decoded.
- Profiles use the same format with the `.mfj` extension (`resources/profiles/UMLStandardProfile.mfj`).
  Their root keeps a `_parent` pointing at the project they were cut from.

### Class-diagram view structure (from `extensions/essential/uml/metamodel.json`)

A class box is not one element. It is a small tree of views:

```
UMLClassView                 (left/top/width/height, model → UMLClass)
├─ UMLNameCompartmentView    (referenced by nameCompartment)
│   ├─ LabelView  stereotypeLabel
│   ├─ LabelView  nameLabel
│   ├─ LabelView  namespaceLabel
│   └─ LabelView  propertyLabel
├─ UMLAttributeCompartmentView   (attributeCompartment) → one UMLAttributeView per attribute
├─ UMLOperationCompartmentView   (operationCompartment) → one UMLOperationView per operation
├─ UMLReceptionCompartmentView   (receptionCompartment)
└─ UMLTemplateParameterCompartmentView
```

A relationship line (`UMLAssociationView`, `UMLGeneralizationView`, …) is an `EdgeView` with
`head`/`tail` → the two box views, `points`, `lineStyle`, and **EdgeLabelView** children for
name/stereotype/property and, for associations, role-name/multiplicity/property labels at each end.
Each label points back with `hostEdge` and to the relationship with `model`.

### Structural diagrams (observed for M2, 2026-10-12)

From `extensions/essential/uml/{metamodel.json,uml-factory.js,toolbox/uml.json,preferences/uml.json}`
and `src/core/core.js`, described in our words:

- **Boxes inside boxes stay flat.** Every view of a diagram is in its `ownedViews`, however deeply
  it is nested on screen. Nesting is two references: the inner view's `containerView` and the outer
  view's `containedViews` list.
- **Parts and ports are attributes.** A part is a `UMLAttribute` (aggregation `composite`) and a
  port a `UMLPort`, both in the owning classifier's `attributes`; their views (`UMLPartView`,
  `UMLPortView`) are contained by the classifier's view. A part dropped on a part view goes to that
  part's *type*.
- **Ports** are 14 × 14 squares on the owner's border, with *floating* labels (`NodeLabelView`,
  `hostNode`) placed by `alpha`/`distance` from the port's centre (name: 20 px at 3π/4).
- **The containment line** (`UMLContainmentView`) has no model element: drawing it moves the tail
  element into the head element's `ownedElements`.
- **Objects and instances** (`UMLObject`, `UMLNodeInstance`, …) have a `classifier` (reference or
  text) and `slots` (`UMLSlot`, with `value` and optionally `definingFeature`).
- **New elements take their look from preferences** (`preferences/uml.json`), not the metamodel:
  components, artifacts, nodes, enumerations, interfaces, use cases and actors start with their
  attribute/operation compartments hidden; artifacts and interfaces start as icons
  (`stereotypeDisplay: "icon"`). Per the defaults rule above, rust-uml writes those values.
- **Toolbox groups per diagram type** come from `toolbox/uml.json` (`diagram-types` lists).
  rust-uml mirrors them in `crates/uml/src/catalog.rs`.

### Behavioural diagrams (observed for M3a, 2026-10-12)

From the same files (`uml-factory.js` diagram functions, `metamodel.json`, `toolbox/uml.json`):

- **Some diagrams come wrapped.** A statechart diagram lives inside a `UMLStateMachine` (created
  with one `UMLRegion` in `regions`); an activity diagram inside a `UMLActivity`; a profile diagram
  inside a `UMLProfile`. StarUML creates the wrapper unless the diagram is added to one already.
- **State machines:** states, final states and pseudostates go in a region's `vertices`;
  transitions in the region's `transitions`. A composite state has its own `regions` (orthogonal:
  several). Pseudostates are one type with a `kind` (initial, choice, fork, join, junction,
  shallow/deep history, entry/exit point, terminate). A state view's `decompositionCompartment`
  holds one `UMLRegionView` per region.
- **Activities:** actions and control nodes go in the activity's `nodes`, flows in its `edges`,
  swimlanes (`UMLActivityPartition`) in its `groups`. All control nodes share one view type
  (`UMLControlNodeView`) and are drawn by model type. Swimlane views have `isVertical` (default
  true) and a `nameLabel`.
- **Actors are always stick figures** (`UMLActorView` draws its iconic form even for the "label"
  display); use cases are ovals with an extension-point compartment.
- **Toolbox ids that aren't model types** carry their model in the tool's arguments: e.g.
  `UMLInitialState` = `UMLPseudostate` kind `initial`; `UMLSelfTransition` = a `UMLTransition` from a
  state to itself; `UMLSwimlaneHorz` = a partition whose view has `isVertical: false`.

### Sequence diagrams (observed for M3b, 2026-10-12)

From `sequenceDiagramFn`, `lifelineFn`, `messageFn` (`uml-factory.js`) and `UMLSeqLifelineView`
(`elements.js`), described in our words:

- **Doubly wrapped.** A new sequence diagram goes in a `UMLInteraction`, which goes in a new
  `UMLCollaboration` (unless added to a classifier: then only the interaction is created; or to an
  interaction: then nothing). A `UMLFrameView` whose `model` is the diagram itself is added at
  (8, 8), 700 × 600.
- **Lifelines** go in the interaction's `participants`. Creating one also creates a role attribute
  `RoleN` in the collaboration's `attributes`; the lifeline's `represent` points at it.
- **Lifeline view** (`UMLSeqLifelineView`): a head box 40 px tall (`SEQ_OBJECT_MINHEIGHT`) at the top
  of the view (default top 40, `LIFELINE_TOP_POSITION`), and a `linePart` sub-view
  (`UMLLinePartView`, 1 px wide) from just below the head to the view's bottom, at its centre x.
- **Messages** go in the interaction's `messages`, with `source`/`target` lifelines and a
  `messageSort` (synchCall default; asynchCall, asynchSignal, createMessage, deleteMessage, reply).
  Their views (`UMLSeqMessageView`) connect the two lifelines' **line parts** (`tail`/`head`), not
  the lifeline views. A message view may own an `activation` (`UMLActivationView`, ≥ 14 × 25).
- **Combined fragments** (`UMLCombinedFragment`, `interactionOperator` alt/opt/loop/par/…) go in the
  interaction's `fragments`, with `operands` (`UMLInteractionOperand`, `guard`).

### Communication diagrams (observed for M3b part 2, 2026-10-13)

From `communicationDiagramFn`, `lifelineFn`, `messageFn` (`uml-factory.js`), `UMLCommMessageView`
and `UMLMessage.getString` (`elements.js`), and the `uml.communication` toolbox group, described in
our words:

- **Wrapped like a sequence diagram:** collaboration → interaction → `UMLCommunicationDiagram`,
  plus the same 700 × 600 frame. The diagram's `showSequenceNumber` defaults to true,
  `sequenceNumbering` to `auto`.
- **Lifelines** are the same `UMLLifeline` (same toolbox id, `participants`, role attribute) with a
  `UMLCommLifelineView`: a plain named box, at least 100 × 30, no line part.
- **Connectors** (`UMLConnector`, view `UMLConnectorView` between two lifeline views) are owned by
  the *tail lifeline's role* (`represent`) in its `ownedElements`, and their ends (`end1`, `end2`,
  `UMLConnectorEnd`) refer to the two roles, not the lifelines. "Self Connector" is a connector
  from a lifeline to itself.
- **Forward/Reverse Message** tools: drawn between two lifelines they create a connector *and* a
  message in one step; drawn onto a connector they add a message to it. The `UMLMessage` goes in
  the interaction's `messages` with `connector`; forward runs tail → head of the connector,
  reverse head → tail. Its view is a `UMLCommMessageView` (an edge-parasitic view: `hostEdge` =
  the connector view, `edgePosition` 1 = middle, `alpha` π/2, `distance` 10) with three
  `NodeLabelView`s (`nameLabel` at 10, `stereotypeLabel` at 25, `propertyLabel` at 10, all π/2).
  `edgePosition` differs from its metamodel default (0) and `alpha`/`distance` have none: write
  them.
- **Drawing:** a 40 px arrow parallel to the connector's middle segment, offset by
  `alpha`/`distance`, its head toward the target. The label is `number [guard] : target =
  name(arguments)`: `number` is the message's position in the interaction's `messages` (auto) or
  its `sequenceNumber` (custom), omitted when `showSequenceNumber` is off.

### Interaction overview and information flow diagrams (observed 2026-10-13)

From `interactionOverviewDiagramFn`, `interactionUseInOverviewFn`, `interactionInOverviewFn`,
`UMLInteractionUseView`, `UMLInteractionInlineView`, `UMLInformationItemView`,
`UMLInformationFlowView`, the frame-type mapping in `elements.js`, and the toolbox, in our words:

- **Interaction overview:** collaboration → interaction → **activity** → diagram (+ frame); added
  to an activity, no wrappers; to an interaction, only the activity. Its toolbox reuses the
  activity control nodes and Control Flow (they go in the activity as on activity diagrams).
- **Interaction Use** tool: a `UMLAction` (`kind: callBehavior`) in the activity's `nodes`, whose
  `target` is a new `UMLInteractionUse` in the outer interaction's `fragments`, whose `refersTo`
  is a new `UMLInteraction` created beside the outer one (in the collaboration). View
  `UMLInteractionUseView`: `nameLabel` and `frameTypeLabel`; drawn as a frame with a `ref` tag
  and `recipient = Called(arguments): returnValue` beside the tag.
- **Interaction (Inline)** tool: the same kind of action, whose `target` is a new
  `UMLSequenceDiagram` inside a new interaction (no frame is added to it). View
  `UMLInteractionInlineView` (a `UMLFrameView`): an `sd` frame that draws that diagram inside,
  scaled to fit (10 px from the sides, 30 from the top, never enlarged).
- **Information flow:** an ordinary diagram in the model. Toolbox: Information Item and
  Information Flow, plus the use-case and class groups. `UMLInformationItem` is a classifier
  (`represented` refs) drawn with «information»; `UMLInformationFlow` is a directed relationship
  owned by its source (`conveyed`, `realizing…` refs), drawn dotted with a stick arrow and «flow»;
  its name label shows the conveyed classifiers' names when there are any.
- **Frame keywords** (the tag of a frame showing a diagram or element): `class`, `cmp`, `dep`,
  `sd` (interaction, sequence, communication, timing, interaction overview), `stm`, `act`, `uc`,
  `pkg`, `req`, `bdd`, `par`, `ibd`; anything else has none.

### Timing diagrams (observed 2026-10-13)

From `timingDiagramFn`, `_addTimingFrame`, `lifelineFn`, `timingStateFn`, `timeSegmentFn`,
`timeTickFn`, the timing branch of `messageFn`, and the `UMLTiming…`/`UMLTime…` views in
`elements.js`, in our words:

- **Wrapped like a sequence diagram** (collaboration → interaction), with a `UMLTimingFrameView`
  (model = the diagram) initialised from (5, 5) to (700, 600).
- **Lifeline**: the same `UMLLifeline` (participants, role); its `UMLTimingLifelineView` must sit
  in the frame (`containerView`, frame `containedViews`), at least 200 wide, with a `nameLabel`.
- **State/Condition**: a `UMLConstraint` named `State-ConditionN` in the *lifeline's*
  `ownedElements`; `UMLTimingStateView` (with `nameLabel`) inside the lifeline view. Dropped on a
  state, it goes to that state's lifeline.
- **Time Segment**: a `UMLStateInvariant` named `Time SegmentN` in the interaction's `fragments`,
  `covered` = the lifeline, `invariant` = the state's constraint; `UMLTimeSegmentView` inside the
  state view.
- **Time Tick**: a `UMLTimeTickView` only (no model), inside the frame, with a `text`.
- **Message**: between two time segments; a `UMLMessage` in `messages` whose source/target are the
  lifelines the segments cover; view `UMLTimingMessageView` (`tail`/`head` = the segment views),
  label as `UMLMessage.getString` (timing diagrams have no sequence numbers).
- **The frame arranges on every draw** (stored positions only give order): lifelines stacked
  below the frame's tag, full frame width, each at least `25 + 25 × states + 25` tall; the
  lifeline name turned vertical at the left; states one 25 px row each from 25 px down, names
  right-aligned up to the time axis start (right of the widest state name); segments of a
  lifeline (sorted by left) placed one after another from that start, each on its state's row,
  with a vertical step from the previous one; ticks along the frame's bottom edge (5 px above it)
  from the same start. Ticks are created one pixel narrower than segments (grid × 3 − 1).
- Time constraints (`{min..max}` label + link to a message or segment) and duration constraints
  (`{min..max}`, placed in the frame) exist too.

## 4. Free vs. paid — exactly what is locked

The only place the license is checked for features is **`src/engine/diagram-export.js`**, in the
three export paths (PNG/JPEG, SVG, PDF). Described in our words (StarUML's code is not reproduced here):

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

### Mermaid import (observed for milestone A, 2026-10-12)

`extensions/default/mermaid` (behaviour only; our parser is written from Mermaid's public docs):

- Dialog with a text editor and examples; command "[Mermaid] Generate Diagram". The kind is read
  from the first word: `classDiagram`, `sequenceDiagram`, `stateDiagram`, `flowchart`, `erDiagram`,
  `mindmap`, `requirementDiagram` each go to their own family's diagram.
- After building, the diagram is laid out with the app's dagre layout in the Mermaid `direction`.
- **Class:** `<<interface>>` → interface, `<<enumeration>>` → enumeration (members become
  literals), `<<abstract>>` → abstract class, any other annotation → stereotype; label → name.
  Solid lines: triangle → generalization, lollipop → interface realization, otherwise an
  association with `*` → composite and `o` → shared aggregation on that end, arrow → navigable,
  quoted cardinalities → end multiplicities. Dotted lines: triangle → interface realization,
  anything else → dependency. `note for X` → note with a note link. Namespaces become package
  models (not drawn).
- **Sequence:** collaboration → interaction → diagram, as for a new sequence diagram. Messages are
  synchronous calls except: the message right after `create participant` → create message, after
  `destroy` → delete message, `-->>` → **asynchronous signal**, `-)` → asynchronous call,
  `--)` → **reply**. `loop/alt/opt/par/critical/break` → combined fragments with an operand per
  section. Notes left of / right of / over. The diagram's `showActivation` is set to whether the
  text used activations and `showSequenceNumber` to whether it had `autonumber`; both default to
  **true** in the metamodel, so they are written as `false` otherwise.
- **State:** one state machine with one region; all states flattened into it (composites are not
  nested); `<<choice>>`/`<<fork>>`/`<<join>>` pseudostates; transition label → name; notes linked.

rust-uml follows these mappings, except that composite states keep their nesting and namespaces
are drawn as package boxes. Flowchart, ER, mind map and requirement imports wait for those families.

## 6. Unknowns / limits of this evidence

- Most of this is static reading. Runtime behaviour observed so far comes only from the owner
  opening rust-uml files in StarUML 7.1.1 and saving them (2026-10-12), noted where it applies.
- REA's analysis of the whole archive hit its size budget on 3,676 bundled-library files (Mermaid
  parser, Kendo cultures); their static facts are kept on file nodes, but cross-file relationships
  there are incomplete. None of these are StarUML's own `src/` or diagram-family code.
- The exact drawing rules (fonts, padding, arrow sizes) were **not** copied. The Rust renderer uses
  UML-standard notation with its own measurements, so pixel output will differ.
- File compatibility was proven both ways on 2026-10-12 for class diagrams: StarUML 7.1.1 opened
  `examples/shapes.mdj`; the owner edited and saved it there (added an abstract, active class,
  renamed a literal) and rust-uml reopened it and saved it back unchanged. Other diagram families
  are untested.
