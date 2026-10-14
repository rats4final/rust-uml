//! Desktop app for Linux, Windows and macOS.
//!
//! Usage: `rust-uml [model.mdj] [--screenshot out.png] [--azure-icons <dir>]
//!         [--control <port> --control-token-file <path> [--control-root <dir>]]`
//!
//! `--screenshot` captures the window once it has settled, writes a PNG and
//! exits: a way to check the UI without someone watching the screen.
//!
//! `--azure-icons` points at Microsoft's Azure icon set, downloaded by the
//! user (rust-uml doesn't ship it); else `RUST_UML_AZURE_ICONS`, else the
//! folder last chosen with View ▸ Azure Icons Folder….
//!
//! `--control` lets agents drive the window over a loopback channel guarded by
//! the token in `--control-token-file` (created if missing); agent file access
//! is confined to `--control-root`. See `docs/agents.md`.

// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod control;
mod settings;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail};
use control::{Control, ControlArgs};
use eframe::egui;
use ui::{FileEvent, FileKind, FileService, UmlApp};

const USAGE: &str = "usage: rust-uml [model.mdj] [--screenshot out.png] [--azure-icons <dir>] \
[--control <port> --control-token-file <path> [--control-root <dir>]]";

#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    initial: Option<String>,
    screenshot: Option<String>,
    azure_icons: Option<PathBuf>,
    control: Option<ControlArgs>,
}

fn parse_args(words: impl IntoIterator<Item = String>) -> anyhow::Result<Args> {
    let mut args = Args::default();
    let (mut port, mut token_file, mut root) = (None, None, None);
    let mut words = words.into_iter();
    while let Some(w) = words.next() {
        let mut value = || {
            words
                .next()
                .ok_or_else(|| anyhow!("{w} needs a value\n{USAGE}"))
        };
        match w.as_str() {
            "--screenshot" => args.screenshot = Some(value()?),
            "--azure-icons" => args.azure_icons = Some(PathBuf::from(value()?)),
            "--control" => {
                let v = value()?;
                port = Some(
                    v.parse::<u16>()
                        .map_err(|_| anyhow!("--control takes a port number, not `{v}`"))?,
                );
            }
            "--control-token-file" => token_file = Some(PathBuf::from(value()?)),
            "--control-root" => root = Some(PathBuf::from(value()?)),
            "-h" | "--help" => bail!("{USAGE}"),
            flag if flag.starts_with("--") => bail!("unknown option {flag}\n{USAGE}"),
            _ if args.initial.is_some() => bail!("open one model at a time\n{USAGE}"),
            _ => args.initial = Some(w),
        }
    }
    args.control = match (port, token_file) {
        (Some(port), Some(token_file)) => Some(ControlArgs {
            port,
            token_file,
            root,
        }),
        (Some(_), None) => bail!(
            "--control needs --control-token-file <path> (created with a new token if missing)"
        ),
        (None, _) if root.is_some() => bail!("--control-root only applies with --control"),
        (None, Some(_)) => bail!("--control-token-file only applies with --control"),
        (None, None) => None,
    };
    Ok(args)
}

fn main() -> anyhow::Result<()> {
    let Args {
        initial,
        screenshot,
        azure_icons,
        control,
    } = parse_args(std::env::args().skip(1))?;
    // The flag, else the environment, else the folder chosen last time.
    let icons = azure_icons
        .or_else(|| std::env::var_os(automation::icons::ICONS_ENV).map(PathBuf::from))
        .or_else(settings::azure_icons);
    if let Some(dir) = icons
        && let Err(e) = automation::icons::use_folder(&dir)
    {
        eprintln!("Azure icons not loaded: {e}");
    }
    let control = control.as_ref().map(Control::bind).transpose()?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title(ui::APP_NAME),
        ..Default::default()
    };
    eframe::run_native(
        ui::APP_NAME,
        options,
        Box::new(move |cc| {
            let mut files = NativeFiles::default();
            if let Some(path) = initial {
                files.open_path(&path);
            }
            let mut app = UmlApp::new(cc, Box::new(files));
            if let Some(control) = control {
                app = app.with_remote(control.serve(cc.egui_ctx.clone())?);
            }
            if let Some(out) = screenshot {
                app = app.screenshot_and_exit(10, move |image| {
                    let [w, h] = image.size;
                    let rgba: Vec<u8> = image
                        .pixels
                        .iter()
                        .flat_map(|c| c.to_srgba_unmultiplied())
                        .collect();
                    match render::encode_rgba_png(w as u32, h as u32, &rgba)
                        .map_err(|e| e.to_string())
                        .and_then(|png| std::fs::write(&out, png).map_err(|e| e.to_string()))
                    {
                        Ok(()) => eprintln!("screenshot written to {out}"),
                        Err(e) => eprintln!("screenshot failed: {e}"),
                    }
                });
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow!("{e}"))
}

/// Native dialogs (rfd) plus direct disk access. Dialogs block, so results
/// are ready on the next poll.
#[derive(Default)]
struct NativeFiles {
    events: Vec<FileEvent>,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl FileService for NativeFiles {
    fn open_model(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("StarUML model", &["mdj", "mfj"])
            .add_filter("All files", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.open_path(&path.to_string_lossy());
        }
    }

    fn save(&mut self, kind: FileKind, suggested_name: &str, path: Option<&str>, bytes: Vec<u8>) {
        let target = match path {
            Some(p) => Some(Path::new(p).to_path_buf()),
            None => rfd::FileDialog::new()
                .add_filter(kind.description(), &[kind.extension()])
                .set_file_name(suggested_name)
                .save_file(),
        };
        let Some(target) = target else { return };
        self.events.push(match std::fs::write(&target, bytes) {
            Ok(()) => FileEvent::Saved {
                kind,
                name: file_name(&target),
                path: Some(target.to_string_lossy().into_owned()),
            },
            Err(e) => FileEvent::Failed(format!("Could not write {}: {e}", target.display())),
        });
    }

    fn poll(&mut self) -> Vec<FileEvent> {
        std::mem::take(&mut self.events)
    }

    fn has_paths(&self) -> bool {
        true
    }

    fn has_icon_folder(&self) -> bool {
        true
    }

    fn choose_icon_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Folder with Microsoft's Azure icons")
            .pick_folder()
        else {
            return;
        };
        self.events.push(match automation::icons::use_folder(&dir) {
            Ok(0) => FileEvent::Failed(format!("No .svg icons in {}", dir.display())),
            Ok(n) => {
                settings::set_azure_icons(&dir);
                FileEvent::IconsChanged(format!("Using {n} Azure icons from {}", dir.display()))
            }
            Err(e) => FileEvent::Failed(format!("Could not read the icons: {e}")),
        });
    }

    fn open_path(&mut self, path: &str) {
        let p = Path::new(path);
        self.events.push(match std::fs::read_to_string(p) {
            Ok(text) => FileEvent::Opened {
                name: file_name(p),
                path: Some(path.to_owned()),
                text,
            },
            Err(e) => FileEvent::Failed(format!("Could not read {path}: {e}")),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(words: &[&str]) -> anyhow::Result<Args> {
        parse_args(words.iter().map(|w| (*w).to_owned()))
    }

    #[test]
    fn arguments() {
        assert_eq!(parse(&[]).unwrap(), Args::default());
        let a = parse(&["m.mdj", "--screenshot", "o.png"]).unwrap();
        assert_eq!(a.initial.as_deref(), Some("m.mdj"));
        assert_eq!(a.screenshot.as_deref(), Some("o.png"));
        let a = parse(&["--azure-icons", "icons"]).unwrap();
        assert_eq!(a.azure_icons, Some(PathBuf::from("icons")));

        let a = parse(&[
            "--control",
            "7878",
            "--control-token-file",
            "t",
            "--control-root",
            "r",
        ])
        .unwrap();
        assert_eq!(
            a.control,
            Some(ControlArgs {
                port: 7878,
                token_file: "t".into(),
                root: Some("r".into()),
            })
        );

        for bad in [
            &["--control", "7878"][..],
            &["--control", "port", "--control-token-file", "t"],
            &["--control-token-file", "t"],
            &["--control-root", "r"],
            &["--screenshot"],
            &["--azure-icons"],
            &["--bogus"],
            &["a.mdj", "b.mdj"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should be refused");
        }
    }
}
