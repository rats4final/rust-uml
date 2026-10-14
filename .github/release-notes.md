Preview of **rust-uml**: an independent, open-source UML modeler written in Rust that opens and
saves StarUML `.mdj` files. Not affiliated with MKLabs/StarUML. What changed in each version:
`log/devlog.md`.

**New in v0.7.0:** flowcharts, ER diagrams and mind maps, each with Mermaid import.
- **Flowcharts:** all 27 ISO flowchart symbols StarUML offers (process, decision, terminator,
  data, document, database, manual input, display, connectors, …) and labelled flows.
- **ER diagrams:** entities with a keys / name / type column table, one-to-one, one-to-many and
  many-to-many relationships with crow's-foot ends, dotted when non-identifying. Edit column
  types, lengths and keys and each end's cardinality in the properties panel.
- **Mind maps:** rounded nodes whose names wrap, joined by plain lines.
- **Mermaid import** now also reads `flowchart`/`graph`, `erDiagram` and `mindmap` (File ▸ Import
  Mermaid…, `rust-uml-cli import`, MCP `mermaid_import`). Flowchart shapes land on the matching
  symbols, including Mermaid's newer `A@{ shape: doc }` form; ER relationships keep their
  cardinalities and identifying/non-identifying lines; mind maps are laid out with the root in
  the middle and branches to both sides. What can't be kept (subgraphs, link styles, mind map
  node shapes) is listed as a warning.
- Help ▸ About and the MCP server now report the real version (they said 0.1.0 before).
- New examples: `flowchart.mdj`, `erd.mdj`, `mindmap.mdj`, and Mermaid samples `login.mmd`,
  `shop.mmd`, `trip.mmd`.
- Known gap: right-angle lines leaving the same side of a box start from the same point, so on
  busy ER diagrams their end marks can overlap.

**What works:** every UML diagram type StarUML offers (class, the six structural ones, use case,
statechart, activity, sequence, communication, timing, interaction overview, information flow),
plus flowcharts, ER diagrams and mind maps, with StarUML's tool sets, properties panel, undo/redo, lossless save (verified both ways with
StarUML 7.1.1 for class diagrams), SVG/PNG export with no watermarks, scripting, Mermaid import
and an MCP server. Full checklist: `docs/parity.md`.

## Downloads

| Platform | File | Run |
|---|---|---|
| Windows 10/11 (x64) | `…-windows-x86_64.zip` | Unzip, double-click `rust-uml.exe`. SmartScreen may warn (unsigned): **More info → Run anyway**. |
| macOS Apple Silicon | `…-macos-arm64.tar.gz` | See macOS note below. |
| macOS Intel | `…-macos-x86_64.tar.gz` | See macOS note below. |
| Linux (x64) | `…-linux-x86_64.tar.gz` | `tar xzf …` then `./rust-uml`. |
| Browser | `…-web.zip`, or the hosted copy on GitHub Pages | Serve the folder with any static web server. |

Each archive also contains `rust-uml-cli`, six example models: `shapes.mdj`,
`structures.mdj`, `behaviour.mdj`, `flowchart.mdj`, `erd.mdj` and `mindmap.mdj` (try
`rust-uml behaviour.mdj`, then pick diagrams in the Model Explorer), and Mermaid and script
samples under `examples/`.

**macOS note:** the binaries are not signed or notarised. After unpacking, run
`xattr -d com.apple.quarantine rust-uml rust-uml-cli` once, then `./rust-uml`.

**Linux note:** needs a desktop with X11 or Wayland and OpenGL. File dialogs use the XDG desktop
portal or `zenity`; without them use **File ▸ Open Path…**.
