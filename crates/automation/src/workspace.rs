//! File access confined to one folder, for agents.
//!
//! Paths are relative to the root (or absolute paths inside it). Symlinks
//! are resolved before the check, so a link can't lead outside. Only model
//! and image files can be written.

use std::path::{Component, Path, PathBuf};

/// Extensions an agent may write.
pub const WRITABLE: &[&str] = &["mdj", "mfj", "svg", "png"];

/// Largest file an agent may open (StarUML's own biggest sample is ~1 MB).
pub const MAX_READ: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// A workspace rooted at `root`, which must be an existing folder.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let root = root
            .canonicalize()
            .map_err(|e| format!("workspace {}: {e}", root.display()))?;
        if !root.is_dir() {
            return Err(format!("workspace {} is not a folder", root.display()));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// An existing file inside the workspace.
    pub fn existing(&self, path: &str) -> Result<PathBuf, String> {
        let full = self.join(path)?;
        let real = full.canonicalize().map_err(|e| format!("{path}: {e}"))?;
        self.inside(&real, path)?;
        if !real.is_file() {
            return Err(format!("{path} is not a file"));
        }
        Ok(real)
    }

    /// Read a file inside the workspace as text.
    pub fn read(&self, path: &str) -> Result<String, String> {
        let real = self.existing(path)?;
        let len = std::fs::metadata(&real)
            .map_err(|e| format!("{path}: {e}"))?
            .len();
        if len > MAX_READ {
            return Err(format!("{path} is larger than {} MB", MAX_READ >> 20));
        }
        std::fs::read_to_string(&real).map_err(|e| format!("{path}: {e}"))
    }

    /// Where `path` would be written: its folder must exist inside the
    /// workspace and its extension must be one of [`WRITABLE`].
    pub fn writable(&self, path: &str) -> Result<PathBuf, String> {
        let full = self.join(path)?;
        let ext = full
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if !WRITABLE.contains(&ext.as_str()) {
            return Err(format!(
                "{path}: only {} files can be written",
                WRITABLE.join(", ")
            ));
        }
        let name = full
            .file_name()
            .ok_or_else(|| format!("{path} has no file name"))?
            .to_owned();
        let dir = full.parent().unwrap_or(&self.root);
        let real_dir = dir
            .canonicalize()
            .map_err(|e| format!("{path}: folder {}: {e}", dir.display()))?;
        self.inside(&real_dir, path)?;
        let target = real_dir.join(name);
        // Writing through a link could land anywhere, even where nothing
        // exists yet, so links are never written to.
        if target
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(format!("{path} is a link; write to the file itself"));
        }
        Ok(target)
    }

    /// Write `bytes` to `path` inside the workspace.
    pub fn write(&self, path: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let target = self.writable(path)?;
        std::fs::write(&target, bytes).map_err(|e| format!("{path}: {e}"))?;
        Ok(target)
    }

    fn join(&self, path: &str) -> Result<PathBuf, String> {
        if path.is_empty() {
            return Err("empty path".into());
        }
        let p = Path::new(path);
        if p.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(format!("{path}: `..` is not allowed"));
        }
        Ok(if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.root.join(p)
        })
    }

    fn inside(&self, real: &Path, shown: &str) -> Result<(), String> {
        if real.starts_with(&self.root) {
            Ok(())
        } else {
            Err(format!(
                "{shown} is outside the workspace {}",
                self.root.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Clocks may tick in microseconds (macOS): tests started together
        // would share a folder without the counter.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("rust-uml-ws-{}-{n}-{k}", std::process::id()));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        d
    }

    #[test]
    fn stays_inside_the_root() {
        let dir = temp();
        let ws = Workspace::new(&dir).unwrap();
        ws.write("a.mdj", b"{}").unwrap();
        ws.write("sub/b.png", b"x").unwrap();
        assert_eq!(ws.read("a.mdj").unwrap(), "{}");
        assert!(ws.read(dir.join("a.mdj").to_str().unwrap()).is_ok());

        assert!(ws.write("../escape.mdj", b"").is_err());
        assert!(ws.write("/tmp/escape.mdj", b"").is_err());
        assert!(ws.write("script.sh", b"").is_err());
        assert!(ws.write("nofolder/x.mdj", b"").is_err());
        assert!(ws.read("missing.mdj").is_err());
        assert!(ws.read("sub").is_err());
        assert!(ws.read("").is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", dir.join("link")).unwrap();
            assert!(ws.read("link/hostname").is_err());
            assert!(ws.write("link/x.mdj", b"").is_err());
            std::os::unix::fs::symlink("/tmp/outside.mdj", dir.join("out.mdj")).unwrap();
            assert!(ws.write("out.mdj", b"").is_err());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
