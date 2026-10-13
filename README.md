# rust-uml

An independent, open-source UML modeler written from scratch in Rust. It opens and saves
StarUML `.mdj` files, and runs on Linux, Windows and macOS and in the browser (WebAssembly).

> Not affiliated with or endorsed by MKLabs. "StarUML" is their trademark. No StarUML code or
> assets are included; see [`ATTRIBUTION.md`](ATTRIBUTION.md) and [`docs/evidence.md`](docs/evidence.md).

## Status

Class diagrams, all UML structural diagrams (package, object, component, deployment, composite
structure, profile) and use case, statechart, activity and sequence diagrams, with StarUML's tool
sets. Any StarUML file opens (communication, timing and interaction-overview diagrams are drawn in a
simplified way for now), saves losslessly, and exports to
SVG/PNG with no watermarks. Files round-trip with StarUML 7.1.1. The full checklist against StarUML
is in [`docs/parity.md`](docs/parity.md); what's next is in [`docs/roadmap.md`](docs/roadmap.md).

## Try it

```sh
cargo run -p desktop -- your-model.mdj
cargo run -p cli -- image your-model.mdj -f png -o out/
```

More in [`docs/development.md`](docs/development.md). Contributors and AI agents: start with
[`AGENTS.md`](AGENTS.md).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
Bundled fonts keep their own license (SIL Open Font License 1.1; see [`ATTRIBUTION.md`](ATTRIBUTION.md)).

Unless you explicitly state otherwise, any contribution you submit for inclusion in this project, as
defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms.
