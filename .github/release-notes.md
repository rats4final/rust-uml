Preview of **rust-uml**: an independent, open-source UML modeler written in Rust that opens and
saves StarUML `.mdj` files. Not affiliated with MKLabs/StarUML. What changed in each version:
`log/devlog.md`.

**New in v0.8.0:** data flow, C4 and Azure diagrams (every diagram family StarUML offers for
free is now in), and a polish pass.
- **Data flow diagrams:** external entities, processes with an ID band, data stores open on the
  right, named data flows. A process can hold its lower-level diagram.
- **C4:** people, software systems, containers (database cylinder, web, desktop and mobile app
  looks, λ / `>_` / `#_` badges), components, elements, and dotted relationships with a
  description and `[technology]`, in StarUML's blues. A diagram inside a system shows the system
  as its dashed boundary.
- **Azure:** groups (solid or dashed, square or rounded), services, numbered callouts,
  connectors. **Service icons are not included:** Microsoft only allows its Azure icons to be
  distributed for diagrams and documentation, so download them from
  <https://learn.microsoft.com/azure/architecture/icons/>, unzip, and choose the folder with
  View ▸ Azure Icons Folder… (or `--azure-icons <folder>`, or `RUST_UML_AZURE_ICONS` for the CLI
  and MCP server). Without them, services show their initials. StarUML's Azure files find their
  icons by name.
- **Lines and labels:** right-angle lines leaving one side of a box start apart; rounded and
  curved line styles are drawn as such; labels no longer sit across their own line; C4
  relationship descriptions wrap.
- **Editing:** Format ▸ Suppress Columns (ER), Model ▸ Validate Model (StarUML's ER rules), and
  quick-edit buttons in the properties panel: add a mind map node left or right, add a related
  entity. Renaming keeps the label text saved in the file up to date.
- **For scripts and agents:** `view.add` shows an existing element on a diagram; `element.set`
  accepts `{"$ref": id}` to set a reference.
- New examples: `dfd.mdj`, `c4.mdj`, `azure.mdj`.

**What works:** every UML diagram type StarUML offers (class, the six structural ones, use case,
statechart, activity, sequence, communication, timing, interaction overview, information flow),
plus flowcharts, ER, mind map, data flow, C4 and Azure diagrams, with StarUML's tool sets, properties panel, undo/redo, lossless save (verified both ways with
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

Each archive also contains `rust-uml-cli`, nine example models: `shapes.mdj`,
`structures.mdj`, `behaviour.mdj`, `flowchart.mdj`, `erd.mdj`, `mindmap.mdj`, `dfd.mdj`,
`c4.mdj` and `azure.mdj` (try
`rust-uml behaviour.mdj`, then pick diagrams in the Model Explorer), and Mermaid and script
samples under `examples/`.

**macOS note:** the binaries are not signed or notarised. After unpacking, run
`xattr -d com.apple.quarantine rust-uml rust-uml-cli` once, then `./rust-uml`.

**Linux note:** needs a desktop with X11 or Wayland and OpenGL. File dialogs use the XDG desktop
portal or `zenity`; without them use **File ▸ Open Path…**.
