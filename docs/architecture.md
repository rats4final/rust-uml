# Architecture

## The idea in one paragraph

A StarUML project is one JSON tree. Model elements (classes, attributes, packages) own each other
through nested lists. Diagrams own *views*, small trees of boxes, labels and lines that point at the
model element they show. Everything else follows from that: `mdj` holds the tree, `uml` knows which
view trees StarUML expects, `render` turns a diagram's views into drawing instructions, `engine`
changes the tree only through named commands, and `ui` shows it all and turns clicks into commands.

## How a click becomes a saved file

```text
mouse drag on canvas            (ui::canvas)        → ("view.move", {"views": [...], "dx": 40, "dy": 0})
Registry::run                   (engine)            → validates params, checks `enabled`
Session::edit                   (engine)            → one undo step; on error everything rolls back
  Editor::set left/top          (mdj)               → each change records its own inverse
  render::reflow                (render)            → grow boxes that no longer fit, re-route attached edges
Document revision changes       (mdj)               → ui rebuilds the Scene once, draws it
File ▸ Save                     (ui → FileService)  → Document::to_json_string → bytes → disk / download
```

## Crates

| Crate | Layer | Knows about | Doesn't know about |
|---|---|---|---|
| `mdj` | L0 | JSON shape of `.mdj`: `_type`, `_id`, `_parent`, `{"$ref"}`, nested children; `Points`/`Font` encodings; undo | UML, drawing, files on disk |
| `mermaid` | L0 | Mermaid class, sequence and state syntax | UML, the model |
| `uml` | L1 | Which element and view types make a class diagram, and how StarUML nests them | Pixels |
| `render` | L2 | Text metrics (Liberation Sans = Arial metrics), box layout, edge routing, SVG/PNG | Windows, input |
| `engine` | L3 | Sessions, selection, the command list, parameter validation | egui |
| `ui` | L4 | egui widgets, mouse and keyboard | Operating systems (dialogs come in through `FileService`) |
| `automation` | L4 | Scripts of commands with `$name.field` references, file access confined to one folder, the MCP server, the running app's control channel and its bridge client | egui, the UI |

## Design choices and why

- **Schema-free loader.** Children and references are recognised by shape, not by a type table, so a
  file containing BPMN, SysML or a third-party extension's elements still loads and saves exactly.
  Type knowledge is added only where we create or draw things.
- **Undo as reversible operations**, not snapshots: three operation kinds (set field, insert subtree,
  remove subtree), each returning its inverse. Cheap for big projects, and the same design StarUML
  uses (`OperationBuilder`).
- **Stored geometry stays truthful.** After each command, `render::reflow` writes grown box sizes and
  re-routed edge points into the file, so StarUML (and the next load) sees what we drew.
- **Display list (`Scene`).** Layout happens once and every backend (egui canvas, SVG, PNG) draws the
  same primitives, so the export matches the screen.
- **Bundled metric-compatible font.** StarUML measures with Arial. Liberation Sans has identical
  advance widths and is freely licensed, so box sizes match across apps and platforms.
- **Commands with JSON params.** One code path for UI, CLI and automation; parameters are treated as
  hostile input.
