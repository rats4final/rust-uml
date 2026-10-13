# Development

## Build and run

```sh
cargo run -p desktop -- path/to/model.mdj         # desktop app (Linux, Windows, macOS)
cargo run -p cli -- info path/to/model.mdj        # CLI
cargo run -p cli -- image model.mdj -f svg -o out # export every diagram
cargo run -p cli -- commands                      # list every command and its params
cargo run -p cli -- run model.mdj node.create '{"kind":"Class","x":40,"y":40}' -o model.mdj
cargo run -p cli -- run --new --script examples/scripts/shop.json -o shop.mdj -o shop.png
cargo run -p cli -- help                          # every subcommand; `help run` explains one
cargo run -p cli -- mcp --root .                  # MCP server for AI agents (docs/agents.md)
```

Linux file dialogs need an XDG desktop portal or `zenity`. Where neither exists (plain WSL), use
**File ▸ Open Path…** / **Save to Path…**, or pass the file on the command line.

## Web

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cd apps/web
trunk serve                  # http://127.0.0.1:8080
trunk build --release        # static site in apps/web/dist
```

In the browser, Open uses the browser's file picker and Save downloads the file.

## Checks

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo xtask layers     # dependency layering
cargo xtask wasm       # everything browser-bound compiles for wasm32
cargo xtask targets    # desktop + CLI type-check for Windows and macOS
```

`cargo xtask targets` needs `rustup target add x86_64-pc-windows-gnu aarch64-apple-darwin`.

## Real-file tests

`crates/mdj/tests/staruml_files.rs` loads every model file shipped inside StarUML's `app.asar` and
checks that saving reproduces it exactly. It looks in the usual install folders, or set
`STARUML_ASAR=/path/to/StarUML/resources/app.asar`. Without an install it is skipped, with a note.

## Looking at the UI without a person

```sh
cargo run -p desktop -- model.mdj --screenshot /tmp/window.png
```

The window opens, settles, writes the PNG and closes. The UI tests in `crates/ui/src/app.rs`
drive the real app with simulated mouse and keyboard (`egui_kittest`) and need no display.

## Releasing

Push a tag: `git tag v0.1.1 && git push origin v0.1.1`. `.github/workflows/release.yml` builds Linux,
Windows, macOS (Apple Silicon and Intel) and web archives and publishes them as a GitHub Release
(notes from `.github/release-notes.md`). Pushing to `main` also redeploys the web app to GitHub Pages.
