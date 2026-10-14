//! Icons named by elements (an Azure service's `icon` file). rust-uml ships
//! none of them: the app supplies an [`IconSource`] that finds them, in a
//! folder the user downloaded; without one, a placeholder is drawn.

use std::sync::Arc;

use resvg::{tiny_skia, usvg};

/// Where icon pictures come from.
pub trait IconSource: Send + Sync {
    /// SVG text of the icon `name` (as stored in the model, e.g.
    /// `App-Services.svg`) of a `family` (`azure`), if available.
    fn svg(&self, family: &str, name: &str) -> Option<Arc<str>>;

    /// Every icon name available in `family`, for choosing one.
    fn names(&self, _family: &str) -> Vec<String> {
        Vec::new()
    }
}

/// No icons: everything draws as a placeholder.
pub struct NoIcons;

impl IconSource for NoIcons {
    fn svg(&self, _: &str, _: &str) -> Option<Arc<str>> {
        None
    }
}

/// An icon file's name as models store it: Microsoft's download names
/// files `10035-icon-service-App-Services.svg`, StarUML's models store
/// `App-Services.svg`.
pub fn icon_file_name(file: &str) -> &str {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let digits = base.bytes().take_while(u8::is_ascii_digit).count();
    match base[digits..].strip_prefix("-icon-service-") {
        Some(rest) if digits > 0 => rest,
        _ => base,
    }
}

/// The key an icon is found by: its [`icon_file_name`], ignoring case.
pub fn icon_key(file: &str) -> String {
    icon_file_name(file).to_lowercase()
}

/// The readable name of an icon file: `App-Services.svg` → `App Services`
/// (StarUML names a service after the icon picked for it).
pub fn icon_title(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let stem = base
        .strip_suffix(".svg")
        .or_else(|| base.strip_suffix(".SVG"))
        .unwrap_or(base);
    stem.replace(['-', '_'], " ")
}

/// Up to two capitals for a placeholder: `App-Services.svg` → `AS`.
pub fn initials(file: &str) -> String {
    let title = icon_title(file);
    let letters: String = title
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .filter(|c| c.is_alphanumeric())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if letters.is_empty() {
        "?".to_owned()
    } else {
        letters
    }
}

/// Largest icon drawn as pixels, per side.
const MAX_ICON_SIDE: u32 = 1024;

/// Draw SVG text into `width` × `height` straight RGBA pixels, scaled to fit
/// and centred (icons keep their proportions).
pub fn rasterize_svg(svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > MAX_ICON_SIDE || height > MAX_ICON_SIDE {
        return None;
    }
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = (width as f32 / size.width()).min(height as f32 / size.height());
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let dx = (width as f32 - size.width() * scale) / 2.0;
    let dy = (height as f32 - size.height() * scale) / 2.0;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy),
        &mut pixmap.as_mut(),
    );
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect(),
    )
}

/// Standard base64, for `data:` URIs.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ABC[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_titles_and_initials() {
        assert_eq!(
            icon_key("10035-icon-service-App-Services.svg"),
            "app-services.svg"
        );
        assert_eq!(icon_key("App-Services.svg"), "app-services.svg");
        assert_eq!(
            icon_key("icons/compute/00021-icon-service-Virtual-Machine.svg"),
            "virtual-machine.svg"
        );
        assert_eq!(icon_key("2024-Plan.svg"), "2024-plan.svg");
        assert_eq!(icon_title("SQL-Database.svg"), "SQL Database");
        assert_eq!(initials("App-Services.svg"), "AS");
        assert_eq!(initials("Functions.svg"), "F");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn rasterizes_fitting_and_rejecting_nonsense() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#ff0000"/></svg>"##;
        let px = rasterize_svg(svg, 20, 20).unwrap();
        assert_eq!(px.len(), 20 * 20 * 4);
        let at = |x: usize, y: usize| &px[(y * 20 + x) * 4..(y * 20 + x) * 4 + 4];
        assert_eq!(at(10, 10), [255, 0, 0, 255], "centre is red");
        assert_eq!(at(0, 10)[3], 0, "the sides stay clear: proportions kept");
        assert!(rasterize_svg("not svg", 10, 10).is_none());
        assert!(rasterize_svg(svg, 0, 10).is_none());
        assert!(rasterize_svg(svg, 5000, 10).is_none());
    }
}
