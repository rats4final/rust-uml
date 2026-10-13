Preview of **rust-uml**: an independent, open-source UML modeler written in Rust that opens and
saves StarUML `.mdj` files. Not affiliated with MKLabs/StarUML. What changed in each version:
`log/devlog.md`.

**New in v0.6.0:** every UML diagram type, and agents that work in the open window.
- **Communication diagrams:** lifelines, connectors, forward/reverse messages numbered `1 : name`
  (a message drawn between lifelines brings its connector; dropped on a connector it rides on it).
- **Timing diagrams:** lifelines with states, time segments drawn as a stepped line, time ticks and
  messages between segments, arranged automatically inside the frame as in StarUML.
- **Interaction overview diagrams:** `ref` frames calling other interactions and inline `sd` frames
  showing their own sequence diagram, joined by the activity control nodes and flows.
- **Information flow diagrams:** information items and «flow» arrows, alongside class and use-case tools.
- **Live app control:** start the app with `rust-uml --control 7878 --control-token-file <file>
  --control-root <folder>` and point `rust-uml-cli mcp --bridge 127.0.0.1:7878 --control-token-file
  <file>` at it: Claude (or any MCP client) then edits the model you have open while you watch, and
  can look at the window (`ui_screenshot`). Loopback only, token-protected; see `docs/agents.md`.
- New examples: "Checkout (communication)", "Door alarm", "Shopping overview" in `behaviour.mdj`,
  "Payment information" in `structures.mdj`.

**What works:** every UML diagram type StarUML offers (class, the six structural ones, use case,
statechart, activity, sequence, communication, timing, interaction overview, information flow)
with StarUML's tool sets, properties panel, undo/redo, lossless save (verified both ways with
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

Each archive also contains `rust-uml-cli`, three example models: `shapes.mdj`,
`structures.mdj` and `behaviour.mdj` (try `rust-uml behaviour.mdj`, then pick diagrams in the
Model Explorer), and Mermaid and script samples under `examples/`.

**macOS note:** the binaries are not signed or notarised. After unpacking, run
`xattr -d com.apple.quarantine rust-uml rust-uml-cli` once, then `./rust-uml`.

**Linux note:** needs a desktop with X11 or Wayland and OpenGL. File dialogs use the XDG desktop
portal or `zenity`; without them use **File ▸ Open Path…**.
