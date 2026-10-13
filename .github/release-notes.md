Preview of **rust-uml**: an independent, open-source UML modeler written in Rust that opens and
saves StarUML `.mdj` files. Not affiliated with MKLabs/StarUML. What changed in each version:
`log/devlog.md`.

**New in v0.5.0:** built for scripts and AI agents.
- **Mermaid import:** File ▸ Import Mermaid… (or `rust-uml-cli import`) turns Mermaid class,
  sequence and state diagrams into editable UML diagrams, laid out automatically, in one undo step.
- **MCP server:** `rust-uml-cli mcp --root <folder>` lets Claude or any MCP client create, edit,
  inspect and preview models in that folder. Try `claude mcp add rust-uml -- rust-uml-cli mcp --root ~/models`.
- **Scriptable CLI:** `rust-uml-cli run` chains commands or runs a JSON script (all-or-nothing),
  `batch` runs one script over a folder, `help <sub>` and `--json` listings.
- Samples in `examples/mermaid/` and `examples/scripts/`; guide in `docs/agents.md`.

**What works:** class, structural, use case, statechart, activity and sequence diagrams end to
end — 54 element and 30 relationship tools,
properties panel, undo/redo, lossless save (verified both ways with StarUML 7.1.1 for class diagrams),
SVG/PNG export with no watermarks. Communication, timing and interaction-overview diagrams open and are drawn in a simplified way.
Full checklist: `docs/parity.md`.

## Downloads

| Platform | File | Run |
|---|---|---|
| Windows 10/11 (x64) | `…-windows-x86_64.zip` | Unzip, double-click `rust-uml.exe`. SmartScreen may warn (unsigned): **More info → Run anyway**. |
| macOS Apple Silicon | `…-macos-arm64.tar.gz` | See macOS note below. |
| macOS Intel | `…-macos-x86_64.tar.gz` | See macOS note below. |
| Linux (x64) | `…-linux-x86_64.tar.gz` | `tar xzf …` then `./rust-uml`. |
| Browser | `…-web.zip`, or the hosted copy on GitHub Pages | Serve the folder with any static web server. |

Each archive also contains `rust-uml-cli`, three example models: `shapes.mdj`,
`structures.mdj` and `behaviour.mdj` (try `rust-uml behaviour.mdj`, then pick diagrams in the
Model Explorer), and Mermaid and script samples under `examples/`.

**macOS note:** the binaries are not signed or notarised. After unpacking, run
`xattr -d com.apple.quarantine rust-uml rust-uml-cli` once, then `./rust-uml`.

**Linux note:** needs a desktop with X11 or Wayland and OpenGL. File dialogs use the XDG desktop
portal or `zenity`; without them use **File ▸ Open Path…**.
