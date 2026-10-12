# Dev log

Newest first. Terse: what landed, numbers, what's still open.

## 2026-10-12 — rust-uml 0.1.0: public release

Landed
- Renamed to rust-uml (binaries `rust-uml`, `rust-uml-cli`, `rust-uml-web`). "StarUML" is a trademark.
- Dual licensed MIT OR Apache-2.0. Evidence doc now paraphrases StarUML's code instead of quoting it.
- GitHub Actions: CI (Linux checks + Windows/macOS build and test), Release on `v*` tags (Linux x64,
  Windows x64, macOS arm64 + x64, web zip), Pages deploy of the web app.
- The view fits the whole diagram when a file or diagram opens; View ▸ Fit Diagram / Actual Size.
- Release binaries: desktop 17 MB, CLI 5.8 MB (Linux x64).

Still open
- macOS and Windows binaries are unsigned (Gatekeeper/SmartScreen warnings).

## 2026-10-12 — milestone 1: core

Landed
- Evidence: StarUML 7.1.1 is Electron; all logic in `app.asar`. Paid features = export watermarks only
  (`src/engine/diagram-export.js`): "UNREGISTERED" on trial, "PRO ONLY" on 8 diagram types for
  Standard. Editing is never locked. See `docs/evidence.md`.
- `mdj`: schema-free `.mdj`/`.mfj` load/save, lossless. All 14 model files shipped with StarUML
  round-trip with identical content and key order (incl. 726-element UML profile). Undo/redo
  as reversible ops; failed edits roll back.
- `uml`: class-diagram factories in StarUML's exact view-tree shape; cascading delete.
- `render`: layout + display list; SVG and PNG export (no watermarks). Liberation Sans gives
  Arial metrics: StarUML's own profile diagram renders with boxes within a few px of StarUML's sizes.
- `engine`: 22 commands; fuzz test proves no command panics on hostile params.
- `ui` + apps: desktop (Linux/Windows/macOS), CLI, web. Mouse-driven UI tests via egui_kittest.
- Web: `trunk build --release` → 7.3 MB wasm; screenshot in headless Chromium shows the full UI.
- Desktop screenshot via `--screenshot` on WSLg shows a demo model built entirely through CLI commands.
- Checks: 33 tests; clippy `-D warnings` with no-panic lints; `xtask layers`; wasm32, Windows
  and macOS type-check.

Still open
- Not yet verified: a file saved here opened in real StarUML. Try `examples/shapes.mdj` (built only through CLI commands).
- Every non-class diagram family (see `docs/parity.md`).
