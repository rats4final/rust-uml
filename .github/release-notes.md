Preview of **rust-uml**: an independent, open-source UML modeler written in Rust that opens and
saves StarUML `.mdj` files. Not affiliated with MKLabs/StarUML. What changed in each version:
`log/devlog.md`.

**New in v0.2.0:** all six UML structural diagram types — package, object, component,
deployment, composite structure and profile — with StarUML's tool sets (Model ▸ Add Diagram).
Boxes can sit inside boxes (classes in packages, artifacts in nodes, parts and ports in classes and
components), ports snap to borders, objects show slots, and notes are available everywhere.
Try `structures.mdj` (one diagram per type).

**What works:** class and structural diagrams end to end — 23 element and 17 relationship tools,
properties panel, undo/redo, lossless save (verified both ways with StarUML 7.1.1 for class diagrams),
SVG/PNG export with no watermarks. Behavioural diagrams open and are drawn in a simplified way.
Full checklist: `docs/parity.md`.

## Downloads

| Platform | File | Run |
|---|---|---|
| Windows 10/11 (x64) | `…-windows-x86_64.zip` | Unzip, double-click `rust-uml.exe`. SmartScreen may warn (unsigned): **More info → Run anyway**. |
| macOS Apple Silicon | `…-macos-arm64.tar.gz` | See macOS note below. |
| macOS Intel | `…-macos-x86_64.tar.gz` | See macOS note below. |
| Linux (x64) | `…-linux-x86_64.tar.gz` | `tar xzf …` then `./rust-uml`. |
| Browser | `…-web.zip`, or the hosted copy on GitHub Pages | Serve the folder with any static web server. |

Each archive also contains `rust-uml-cli` and two example models, `shapes.mdj` and
`structures.mdj` (try `rust-uml structures.mdj`, then pick diagrams in the Model Explorer).

**macOS note:** the binaries are not signed or notarised. After unpacking, run
`xattr -d com.apple.quarantine rust-uml rust-uml-cli` once, then `./rust-uml`.

**Linux note:** needs a desktop with X11 or Wayland and OpenGL. File dialogs use the XDG desktop
portal or `zenity`; without them use **File ▸ Open Path…**.
