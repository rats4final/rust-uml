//! Desktop app for Linux, Windows and macOS.
//!
//! Usage: `rust-uml [model.mdj] [--screenshot out.png]`
//!
//! `--screenshot` captures the window once it has settled, writes a PNG and
//! exits: a way to check the UI without someone watching the screen.

// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::Path;

use anyhow::anyhow;
use eframe::egui;
use ui::{FileEvent, FileKind, FileService, UmlApp};

fn main() -> anyhow::Result<()> {
    let mut initial = None;
    let mut screenshot = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next();
        } else {
            initial = Some(a);
        }
    }
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
