# Attribution

Third-party assets shipped in this repository. Every row keeps its license file next to the asset.

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| `crates/render/assets/fonts/LiberationSans-{Regular,Bold,Italic,BoldItalic}.ttf` | Liberation Sans 2.1.5 | Red Hat, Inc. (based on Liberation by Ascender) | Ubuntu package `fonts-liberation` 1:2.1.5-3build1 | SIL Open Font License 1.1 (`crates/render/assets/fonts/LICENSE-LiberationSans.txt`) |

Liberation Sans is metric-compatible with Arial, the font StarUML uses, so diagrams lay out the same.

## Not included

No StarUML code, icons, stylesheets, templates or profiles are part of this repository. StarUML's
shipped model files are read from a local install by the opt-in test `crates/mdj/tests/staruml_files.rs`
and are never copied here.
