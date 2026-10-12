First preview of **rust-uml**: an independent, open-source UML modeler written in Rust that opens and
saves StarUML `.mdj` files. Not affiliated with MKLabs/StarUML.

**What works:** class diagrams end to end — classes, interfaces, enumerations, data types, packages,
7 relationship kinds, properties panel, undo/redo, lossless save, SVG/PNG export with no watermarks.
Other StarUML diagram types open and are drawn in a simplified way. Full checklist: `docs/parity.md`.

## Downloads

| Platform | File | Run |
|---|---|---|
| Windows 10/11 (x64) | `…-windows-x86_64.zip` | Unzip, double-click `rust-uml.exe`. SmartScreen may warn (unsigned): **More info → Run anyway**. |
| macOS Apple Silicon | `…-macos-arm64.tar.gz` | See macOS note below. |
| macOS Intel | `…-macos-x86_64.tar.gz` | See macOS note below. |
| Linux (x64) | `…-linux-x86_64.tar.gz` | `tar xzf …` then `./rust-uml`. |
| Browser | `…-web.zip`, or the hosted copy on GitHub Pages | Serve the folder with any static web server. |

Each archive also contains `rust-uml-cli` and an example model, `shapes.mdj`
(try `rust-uml shapes.mdj`).

**macOS note:** the binaries are not signed or notarised. After unpacking, run
`xattr -d com.apple.quarantine rust-uml rust-uml-cli` once, then `./rust-uml`.

**Linux note:** needs a desktop with X11 or Wayland and OpenGL. File dialogs use the XDG desktop
portal or `zenity`; without them use **File ▸ Open Path…**.
