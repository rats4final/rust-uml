use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

const FACES: [(&str, &[u8]); 4] = [
    ("diagram-regular", render::FONT_REGULAR),
    ("diagram-bold", render::FONT_BOLD),
    ("diagram-italic", render::FONT_ITALIC),
    ("diagram-bold-italic", render::FONT_BOLD_ITALIC),
];

/// Register the diagram fonts (the ones `render` measures with) as named
/// families, keeping egui's own fonts for the rest of the UI.
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    for (name, bytes) in FACES {
        defs.font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        defs.families
            .insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
    }
    ctx.set_fonts(defs);
}

pub fn family(bold: bool, italic: bool) -> FontFamily {
    let (name, _) = FACES[bold as usize | (italic as usize) << 1];
    FontFamily::Name(name.into())
}
