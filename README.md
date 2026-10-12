# rust-uml

An independent, open-source UML modeler written from scratch in Rust. It opens and saves
StarUML `.mdj` files, and runs on Linux, Windows and macOS and in the browser (WebAssembly).

> Not affiliated with or endorsed by MKLabs. "StarUML" is their trademark. No StarUML code or
> assets are included; see [`ATTRIBUTION.md`](ATTRIBUTION.md) and [`docs/evidence.md`](docs/evidence.md).

## Status

Milestone 1 (core): class diagrams end to end. You can open any StarUML file (other diagram types
are drawn in a simplified way), edit class diagrams with undo, save losslessly, and export SVG/PNG with
no watermarks. The full feature checklist against StarUML is in [`docs/parity.md`](docs/parity.md).

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
