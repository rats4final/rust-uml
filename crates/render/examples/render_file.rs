//! Render every diagram in a model file to SVG:
//! `cargo run -p render --example render_file -- <file.mdj> <out-dir>`

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(out)) = (args.next(), args.next()) else {
        return Err("usage: render_file <file.mdj> <out-dir>".into());
    };
    let doc = mdj::Document::from_json_str(&std::fs::read_to_string(&input)?)?;
    let fonts = render::Fonts::default();
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out)?;
    for el in doc.iter().filter(|e| e.type_name.ends_with("Diagram")) {
        let scene = render::build_scene(&doc, &el.id, &fonts);
        if scene.prims.is_empty() {
            continue;
        }
        let name: String = el
            .name()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let path = out.join(format!("{name}.svg"));
        std::fs::write(&path, render::to_svg(&scene))?;
        std::fs::write(path.with_extension("png"), render::to_png(&scene, 1.5)?)?;
        println!("{} ({} prims)", path.display(), scene.prims.len());
    }
    Ok(())
}
