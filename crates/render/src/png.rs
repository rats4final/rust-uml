use std::sync::Arc;

use resvg::{tiny_skia, usvg};

use crate::icons::{IconSource, NoIcons};
use crate::scene::Scene;
use crate::svg::to_svg_with;
use crate::text::{FONT_BOLD, FONT_BOLD_ITALIC, FONT_ITALIC, FONT_REGULAR};

/// Largest side of an exported image, to keep a huge diagram or scale from
/// allocating gigabytes.
const MAX_SIDE: u32 = 16_384;

/// Rasterise a scene to PNG bytes at `scale` (1.0 = 1 px per diagram unit).
/// Uses the bundled fonts only, so output is identical on every platform.
/// Icons draw as placeholders; see [`to_png_with`].
pub fn to_png(scene: &Scene, scale: f32) -> Result<Vec<u8>, String> {
    to_png_with(scene, scale, &NoIcons)
}

/// [`to_png`], drawing the icons `icons` has.
pub fn to_png_with(scene: &Scene, scale: f32, icons: &dyn IconSource) -> Result<Vec<u8>, String> {
    if !(scale.is_finite() && scale > 0.0) {
        return Err(format!("invalid scale {scale}"));
    }
    let mut db = usvg::fontdb::Database::new();
    for face in [FONT_REGULAR, FONT_BOLD, FONT_ITALIC, FONT_BOLD_ITALIC] {
        db.load_font_data(face.to_vec());
    }
    let opts = usvg::Options {
        fontdb: Arc::new(db),
        font_family: "Liberation Sans".into(),
        ..Default::default()
    };
    let tree =
        usvg::Tree::from_str(&to_svg_with(scene, icons), &opts).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (w, h) = (
        (size.width() * scale).ceil(),
        (size.height() * scale).ceil(),
    );
    if w > MAX_SIDE as f32 || h > MAX_SIDE as f32 {
        return Err(format!(
            "image would be {w}×{h} px; the limit is {MAX_SIDE} per side"
        ));
    }
    let mut pixmap = tiny_skia::Pixmap::new(w as u32, h as u32).ok_or("empty image")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

/// Encode straight (unpremultiplied) RGBA pixels as PNG, e.g. a window
/// screenshot.
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let size = tiny_skia::IntSize::from_wh(width, height).ok_or("empty image")?;
    let (pixels, _) = rgba.as_chunks::<4>();
    let pixels: Vec<u8> = pixels
        .iter()
        .flat_map(|&[r, g, b, a]| {
            let pre = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
            [pre(r), pre(g), pre(b), a]
        })
        .collect();
    let pixmap =
        tiny_skia::Pixmap::from_vec(pixels, size).ok_or("pixel buffer has the wrong size")?;
    pixmap.encode_png().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_rgba_and_rejects_wrong_sizes() {
        let png = encode_rgba_png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(encode_rgba_png(3, 1, &[0; 8]).is_err());
        assert!(encode_rgba_png(0, 0, &[]).is_err());
    }
}
