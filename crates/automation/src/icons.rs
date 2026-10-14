//! Icons from a folder on disk: the official Azure icon set the user
//! downloaded from Microsoft (rust-uml ships none; see ATTRIBUTION.md).
//! The folder is scanned once; files are read when first drawn.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use render::IconSource;

/// How deep and how many files a scan goes, and the largest icon read, so a
/// wrong folder (a whole disk) can't stall the app.
const MAX_DEPTH: usize = 6;
const MAX_FILES: usize = 20_000;
const MAX_ICON_BYTES: u64 = 2 << 20;

/// The Azure icons in a folder, found by the names models store
/// (`App-Services.svg`) whatever the files are called on disk
/// (`10035-icon-service-App-Services.svg`).
pub struct FolderIcons {
    dir: PathBuf,
    files: HashMap<String, PathBuf>,
    names: Vec<String>,
    read: Mutex<HashMap<String, Option<Arc<str>>>>,
}

impl FolderIcons {
    /// Index the `.svg` files under `dir` (subfolders included).
    pub fn scan(dir: &Path) -> io::Result<Self> {
        let mut files = HashMap::new();
        let mut names = Vec::new();
        let mut stack = vec![(dir.to_path_buf(), 0)];
        let mut seen = 0usize;
        while let Some((folder, depth)) = stack.pop() {
            let entries = match std::fs::read_dir(&folder) {
                Ok(e) => e,
                // The folder itself must be readable; a subfolder may not be.
                Err(e) if depth == 0 => return Err(e),
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                seen += 1;
                if seen > MAX_FILES {
                    break;
                }
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    if depth < MAX_DEPTH {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }
                let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
                    continue;
                };
                if !file.to_ascii_lowercase().ends_with(".svg") {
                    continue;
                }
                // The first file with a name wins (some icons sit in two categories).
                if let std::collections::hash_map::Entry::Vacant(slot) =
                    files.entry(render::icon_key(file))
                {
                    names.push(render::icon_file_name(file).to_owned());
                    slot.insert(path);
                }
            }
        }
        names.sort_by_key(|n| n.to_lowercase());
        Ok(Self {
            dir: dir.to_path_buf(),
            files,
            names,
            read: Mutex::new(HashMap::new()),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// How many icons the folder has.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    fn load(path: &Path) -> Option<Arc<str>> {
        let size = std::fs::metadata(path).ok()?.len();
        if size > MAX_ICON_BYTES {
            return None;
        }
        std::fs::read_to_string(path).ok().map(Arc::from)
    }
}

impl IconSource for FolderIcons {
    fn svg(&self, family: &str, name: &str) -> Option<Arc<str>> {
        if family != "azure" || name.is_empty() {
            return None;
        }
        let key = render::icon_key(name);
        let mut read = self.read.lock().ok()?;
        if let Some(found) = read.get(&key) {
            return found.clone();
        }
        let found = self.files.get(&key).and_then(|p| Self::load(p));
        read.insert(key, found.clone());
        found
    }

    fn names(&self, family: &str) -> Vec<String> {
        if family == "azure" {
            self.names.clone()
        } else {
            Vec::new()
        }
    }
}

/// The environment variable naming the Azure icon folder, for the CLI, the
/// MCP server and the desktop app alike.
pub const ICONS_ENV: &str = "RUST_UML_AZURE_ICONS";

/// Use the folder in [`ICONS_ENV`] (if set) for every session from now on.
/// Returns how many icons it found, or why it couldn't be read.
pub fn icons_from_env() -> Option<Result<usize, String>> {
    let dir = std::env::var_os(ICONS_ENV)?;
    Some(use_folder(Path::new(&dir)))
}

/// Use the icons in `dir` for every session from now on.
pub fn use_folder(dir: &Path) -> Result<usize, String> {
    let icons = FolderIcons::scan(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let n = icons.len();
    engine::set_default_icons(Arc::new(icons));
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_icons_by_model_name_whatever_the_file_is_called() {
        let dir = std::env::temp_dir().join(format!("rust-uml-icons-{}", std::process::id()));
        let sub = dir.join("app services");
        std::fs::create_dir_all(&sub).unwrap();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#;
        std::fs::write(sub.join("10035-icon-service-App-Services.svg"), svg).unwrap();
        std::fs::write(dir.join("Functions.svg"), svg).unwrap();
        std::fs::write(dir.join("readme.txt"), "not an icon").unwrap();

        let icons = FolderIcons::scan(&dir).unwrap();
        assert_eq!(icons.len(), 2);
        assert_eq!(icons.names("azure"), ["App-Services.svg", "Functions.svg"]);
        assert!(icons.names("gcp").is_empty());
        assert_eq!(icons.svg("azure", "App-Services.svg").as_deref(), Some(svg));
        assert_eq!(icons.svg("azure", "app-services.SVG").as_deref(), Some(svg));
        assert!(icons.svg("azure", "Missing.svg").is_none());
        assert!(icons.svg("gcp", "Functions.svg").is_none());
        assert!(FolderIcons::scan(&dir.join("nowhere")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
