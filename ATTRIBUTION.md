# Attribution

Third-party assets shipped in this repository. Every row keeps its license file next to the asset.

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| `crates/render/assets/fonts/LiberationSans-{Regular,Bold,Italic,BoldItalic}.ttf` | Liberation Sans 2.1.5 | Red Hat, Inc. (based on Liberation by Ascender) | Ubuntu package `fonts-liberation` 1:2.1.5-3build1 | SIL Open Font License 1.1 (`crates/render/assets/fonts/LICENSE-LiberationSans.txt`) |

Liberation Sans is metric-compatible with Arial, the font StarUML uses, so diagrams lay out the same.

## Not included

**Azure icons.** Azure diagrams name each service's icon (`App-Services.svg`), as StarUML's files
do, but rust-uml ships none of Microsoft's icons: Microsoft lets them be copied and distributed
"only for the permitted use" (architectural diagrams, training materials, documentation) and
reserves all other rights, which can't sit inside an MIT/Apache app. Download them yourself from
<https://learn.microsoft.com/azure/architecture/icons/> (accepting Microsoft's terms there), unzip,
and point rust-uml at the folder: View ▸ Azure Icons Folder… in the desktop app, `--azure-icons
<folder>`, or the `RUST_UML_AZURE_ICONS` environment variable (also for `rust-uml-cli` and its MCP
server). Without them, services draw as placeholders with their initials. Diagrams and exported
images you make with the icons fall under Microsoft's terms for icon use.

No StarUML code, icons, stylesheets, templates or profiles are part of this repository. StarUML's
shipped model files are read from a local install by the opt-in test `crates/mdj/tests/staruml_files.rs`
and are never copied here.
