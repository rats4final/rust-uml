# AGENTS.md: guide for AI agents and contributors

rust-uml is an independent, from-scratch Rust UML modeler that reads and writes StarUML's
`.mdj` files and aims at feature parity with StarUML 7.1.1, including the features StarUML sells.
It runs as a native desktop app (Linux, Windows, macOS) and in the browser (WebAssembly), from one
codebase. Read this file first, then `docs/`.

## 1. Orientation

| Read | Why |
|---|---|
| `docs/roadmap.md` | **Start here when resuming:** state, setup, what's next in order, known gaps |
| `docs/evidence.md` | What StarUML is and how its files look, with the evidence for each claim |
| `docs/parity.md` | Every StarUML feature and where we stand: the work list |
| `docs/architecture.md` | Crate map, layers, how a click becomes a saved file |
| `docs/development.md` | Build, test, run, web build |
| `log/devlog.md` | What landed recently and what's still open |

## 2. Workspace map

```text
crates/
  mdj      L0  .mdj/.mfj documents: schema-free load/save, edits with undo (pure data, no I/O)
  uml      L1  tool catalog (catalog.rs), factories (StarUML's exact view trees), cascading delete, text
  render   L2  layout, display list (Scene), routing, SVG/PNG export, bundled fonts
  engine   L3  Session + command registry: every action is a command with JSON params
  ui       L4  egui shell (thin); platform services injected through FileService
apps/
  desktop  rust-uml       native app (eframe + rfd dialogs)
  cli      rust-uml-cli   headless: new / info / image / commands / run
  web      rust-uml-web   the same UI in a browser (trunk + wasm-bindgen)
xtask/     cargo xtask layers | wasm | targets | ci
```

**Layering is enforced** by `cargo xtask layers`: a crate depends only on lower layers, and nothing
below `ui` may use egui, eframe, winit or rfd. Register new crates in `xtask/src/main.rs`.

## 3. Golden rules

1. **Clean-room.** StarUML is proprietary. We study it for *behaviour, look and file format only*
   (`docs/evidence.md`). Never copy its code, stylesheets, icons, templates or profiles into this
   repo. Never implement or touch its licensing. File-format facts (field names, value encodings)
   are interoperability data and fine to use.
2. **Never crash.** Non-test code has no `unwrap`/`expect`/`panic!`/`unreachable!`; clippy denies
   them (`clippy.toml` allows them in tests). Malformed files and hostile command parameters return
   errors. `engine`'s `no_command_panics_on_hostile_params` test must stay green; extend its input
   list when you add a command.
3. **Tools are data.** A new element or relationship kind is a row in `crates/uml/src/catalog.rs`
   (ids as in StarUML's toolbox), plus notation in `render` if it looks new. Don't hand-code
   per-kind enums.
4. **Everything is a command.** New user-visible behaviour = a `CommandSpec` in
   `crates/engine/src/commands.rs` with id, label, menu, shortcut, params doc, `enabled`, `run`, plus
   tests. The UI, CLI and future HTTP API all dispatch by id.
5. **Files stay lossless.** Loading then saving any StarUML file must not change it. Run the
   real-file test (`crates/mdj/tests/staruml_files.rs`) against a local StarUML install when you
   touch `mdj`. Never commit StarUML's files.
   **Defaults:** StarUML loads a missing field as the *metamodel* default and omits fields equal to
   it when saving. So when writing, store every field whose intended value differs from the
   metamodel default (e.g. row `horizontalAlignment: 0`); when reading, fall back to the metamodel
   default (`mdj::values::absent`), never to "what a new element would get". See `docs/evidence.md` §3.
6. **Never break wasm.** `cargo xtask wasm` must pass. File-system and dialog code lives in
   `apps/*`, behind `ui::FileService`.
7. **Verify the UI by driving it.** Add `egui_kittest` tests in `crates/ui` that click and drag
   (see `app.rs` tests). For a visual check run
   `rust-uml model.mdj --screenshot out.png` and look at the PNG.
8. **Tests are the gate.** Every change comes with tests.

## 4. Before you finish a task

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo xtask layers
cargo xtask wasm       # if you touched crates/ or apps/web
cargo xtask targets    # if you touched apps/desktop or apps/cli
```

Then update `docs/parity.md` if a row moved, and add a terse entry to `log/devlog.md`.

On a memory-constrained machine run heavy steps one at a time, never in parallel.
