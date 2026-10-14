//! The few things the desktop app remembers between runs, in
//! `rust-uml/settings.json` under the platform's configuration folder.
//! Unreadable or missing settings are simply not used.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The configuration folder: `%APPDATA%` on Windows, `~/Library/Application
/// Support` on macOS, `$XDG_CONFIG_HOME` or `~/.config` elsewhere.
fn config_dir() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))
    }
}

fn file() -> Option<PathBuf> {
    Some(config_dir()?.join("rust-uml").join("settings.json"))
}

fn read() -> Value {
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// The Azure icon folder chosen last time.
pub fn azure_icons() -> Option<PathBuf> {
    read()
        .get("azureIcons")
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

/// Remember the Azure icon folder. Failing to save is not worth an error:
/// the folder is still used for this run.
pub fn set_azure_icons(dir: &Path) {
    let Some(path) = file() else { return };
    let mut settings = read();
    settings["azureIcons"] = json!(dir.to_string_lossy());
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(&settings) {
        let _ = std::fs::write(path, text);
    }
}
