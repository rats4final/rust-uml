use egui::{Key, KeyboardShortcut, Modifiers};
use engine::{Registry, Session};
use render::Scene;
use serde_json::{Value, json};

use crate::canvas::{Canvas, Tool};
use crate::explorer;
use crate::platform::{FileEvent, FileKind, FileService};
use crate::properties::Properties;
use crate::{APP_NAME, fonts};

/// Something that would throw away unsaved work, waiting for confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Discard {
    New,
    Open,
    OpenPath(String),
}

/// A typed-in path being entered (fallback for platforms without dialogs).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PathPrompt {
    Open(String),
    Save(String),
}

/// File ▸ Import Mermaid…: the text being edited and the last error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MermaidPrompt {
    text: String,
    error: Option<String>,
}

/// Examples offered in the Mermaid dialog (our own, one per supported kind).
const MERMAID_EXAMPLES: &[(&str, &str)] = &[
    (
        "Class",
        "classDiagram\n    direction LR\n    class Order {\n        +String id\n        +total() Money\n    }\n    Customer \"1\" --> \"*\" Order : places\n    Order *-- OrderLine\n    OrderLine --> Product\n    <<interface>> Payable\n    Order ..|> Payable",
    ),
    (
        "Sequence",
        "sequenceDiagram\n    actor C as Customer\n    participant S as Shop\n    C->>S: checkout(cart)\n    alt in stock\n        S--)C: confirmation\n    else out of stock\n        S--)C: apology\n    end",
    ),
    (
        "State",
        "stateDiagram-v2\n    [*] --> Draft\n    Draft --> Review : submit\n    Review --> Draft : changes\n    Review --> Published : approve\n    Published --> [*]",
    ),
];

/// Work sent from outside the window (the desktop app's control channel),
/// run on the UI thread at the start of the next frame.
pub type RemoteJob = Box<dyn FnOnce(&mut Remote<'_>) + Send>;

/// Receives the window's pixels once a requested capture is ready.
pub type Capture = Box<dyn FnOnce(&egui::ColorImage) + Send>;

/// The app as a [`RemoteJob`] sees it.
pub struct Remote<'a> {
    app: &'a mut UmlApp,
}

impl Remote<'_> {
    /// The command registry and the open model.
    pub fn parts(&mut self) -> (&Registry, &mut Session) {
        (&self.app.registry, &mut self.app.session)
    }

    /// The open model was replaced: reset the view onto it.
    pub fn session_replaced(&mut self) {
        self.app.reset_view();
    }

    /// What the user sees, for agents.
    pub fn inspect(&self) -> Value {
        let app = &self.app;
        let s = &app.session;
        let diagram = s.active_diagram.as_deref().and_then(|d| s.doc.get(d));
        let tool = match app.tool {
            Tool::Select => "Select",
            Tool::Node(n) => n.id,
            Tool::Edge(e) => e.id,
        };
        let dialogs: Vec<&str> = [
            (app.confirm.is_some(), "discard"),
            (app.path_prompt.is_some(), "path"),
            (app.mermaid.is_some(), "mermaid"),
        ]
        .into_iter()
        .filter_map(|(open, name)| open.then_some(name))
        .collect();
        json!({
            "file": s.file_name,
            "path": s.path,
            "unsavedChanges": s.doc.is_modified(),
            "activeDiagram": diagram.map(|d| json!({
                "id": d.id, "name": d.name(), "type": d.type_name,
            })),
            "selection": s.selection,
            "tool": tool,
            "zoom": app.canvas.zoom(),
            "status": app.status,
            "dialogs": dialogs,
        })
    }

    /// Capture the window as drawn at the end of this frame.
    pub fn screenshot(&mut self, done: Capture) {
        self.app.captures.push(done);
    }
}

/// Developer aid: capture the window after a few frames, then close.
struct Screenshot {
    frames_left: u32,
    requested: bool,
    on_done: Box<dyn FnOnce(&egui::ColorImage)>,
}

pub struct UmlApp {
    session: Session,
    registry: Registry,
    files: Box<dyn FileService>,
    canvas: Canvas,
    properties: Properties,
    tool: Tool,
    status: String,
    confirm: Option<Discard>,
    path_prompt: Option<PathPrompt>,
    mermaid: Option<MermaidPrompt>,
    /// Scene of the active diagram, keyed by document revision.
    scene: Option<(u64, String, Scene)>,
    title: String,
    screenshot: Option<Screenshot>,
    remote: Option<std::sync::mpsc::Receiver<RemoteJob>>,
    /// Waiting for the next window capture.
    captures: Vec<Capture>,
    capture_requested: bool,
}

impl UmlApp {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Box<dyn FileService>) -> Self {
        fonts::install(&cc.egui_ctx);
        Self {
            session: Session::default(),
            registry: Registry::default(),
            files,
            canvas: Canvas::default(),
            properties: Properties::default(),
            tool: Tool::Select,
            status: String::new(),
            confirm: None,
            path_prompt: None,
            mermaid: None,
            scene: None,
            title: String::new(),
            screenshot: None,
            remote: None,
            captures: Vec::new(),
            capture_requested: false,
        }
    }

    /// Run jobs sent on `jobs` (by the control channel). Whoever sends one
    /// should also wake the window (`egui::Context::request_repaint`).
    pub fn with_remote(mut self, jobs: std::sync::mpsc::Receiver<RemoteJob>) -> Self {
        self.remote = Some(jobs);
        self
    }

    fn run_remote_jobs(&mut self) {
        let Some(jobs) = self.remote.take() else {
            return;
        };
        while let Ok(job) = jobs.try_recv() {
            job(&mut Remote { app: self });
        }
        self.remote = Some(jobs);
    }

    fn drive_captures(&mut self, ctx: &egui::Context) {
        if self.captures.is_empty() {
            return;
        }
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            for done in self.captures.drain(..) {
                done(&image);
            }
            self.capture_requested = false;
        } else if !self.capture_requested {
            self.capture_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
        }
    }

    /// Capture the window after `frames` frames, hand the image to
    /// `on_done`, then close. Used to check the UI without a person watching.
    pub fn screenshot_and_exit(
        mut self,
        frames: u32,
        on_done: impl FnOnce(&egui::ColorImage) + 'static,
    ) -> Self {
        self.screenshot = Some(Screenshot {
            frames_left: frames,
            requested: false,
            on_done: Box::new(on_done),
        });
        self
    }

    fn drive_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.screenshot else {
            return;
        };
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            if let Some(shot) = self.screenshot.take() {
                (shot.on_done)(&image);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if shot.frames_left > 0 {
            shot.frames_left -= 1;
            ctx.request_repaint();
        } else if !shot.requested {
            shot.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
        }
    }

    /// Run a command; report failures in the status bar.
    fn exec(&mut self, id: &str, params: Value) {
        match self.registry.run(&mut self.session, id, &params) {
            Ok(_) => self.status.clear(),
            Err(e) => self.status = e.to_string(),
        }
    }

    fn scene(&mut self) -> Option<&Scene> {
        let diagram = self.session.active_diagram.clone()?;
        let rev = self.session.doc.revision();
        let fresh = matches!(&self.scene, Some((r, d, _)) if *r == rev && *d == diagram);
        if !fresh {
            let scene = render::build_scene(&self.session.doc, &diagram, self.session.fonts());
            self.scene = Some((rev, diagram, scene));
        }
        self.scene.as_ref().map(|(_, _, s)| s)
    }

    fn replace_session(&mut self, session: Session) {
        self.session = session;
        self.reset_view();
    }

    fn reset_view(&mut self) {
        self.canvas.reset_view();
        self.properties = Properties::default();
        self.tool = Tool::Select;
        self.scene = None;
    }

    fn request(&mut self, action: Discard) {
        if self.session.doc.is_modified() {
            self.confirm = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: Discard) {
        match action {
            Discard::New => self.replace_session(Session::default()),
            Discard::Open => self.files.open_model(),
            Discard::OpenPath(p) => self.files.open_path(&p),
        }
    }

    fn save(&mut self, save_as: bool) {
        let path = if save_as {
            None
        } else {
            self.session.path.clone()
        };
        self.save_to(path);
    }

    fn save_to(&mut self, path: Option<String>) {
        match self.session.to_json() {
            Ok(text) => {
                let name = self.session.file_name.clone();
                self.files
                    .save(FileKind::Model, &name, path.as_deref(), text.into_bytes());
            }
            Err(e) => self.status = format!("Could not serialise the model: {e}"),
        }
    }

    fn export(&mut self, kind: FileKind) {
        let Some(diagram) = self.session.active_diagram.clone() else {
            self.status = "Open a diagram to export it.".into();
            return;
        };
        let name = self
            .session
            .doc
            .get(&diagram)
            .map(|d| d.name().to_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "diagram".into());
        let scene = render::build_scene(&self.session.doc, &diagram, self.session.fonts());
        let bytes = match kind {
            FileKind::Svg => Ok(render::to_svg(&scene).into_bytes()),
            FileKind::Png => render::to_png(&scene, 2.0),
            FileKind::Model => return,
        };
        match bytes {
            Ok(b) => self
                .files
                .save(kind, &format!("{name}.{}", kind.extension()), None, b),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    fn handle_file_events(&mut self) {
        for event in self.files.poll() {
            match event {
                FileEvent::Opened { name, path, text } => {
                    match Session::open(&text, name.clone(), path) {
                        Ok(s) => {
                            self.replace_session(s);
                            self.status = format!("Opened {name}");
                        }
                        Err(e) => self.status = format!("Could not open {name}: {e}"),
                    }
                }
                FileEvent::Saved { kind, name, path } => {
                    if kind == FileKind::Model {
                        self.session.file_name = name.clone();
                        if path.is_some() {
                            self.session.path = path;
                        }
                        self.session.mark_saved();
                    }
                    self.status = format!("Saved {name}");
                }
                FileEvent::Failed(e) => self.status = e,
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let pressed = |m: Modifiers, k: Key| {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)))
        };
        if pressed(cmd | Modifiers::SHIFT, Key::Z) || pressed(cmd, Key::Y) {
            self.exec("edit.redo", json!({}));
        }
        if pressed(cmd, Key::Z) {
            self.exec("edit.undo", json!({}));
        }
        if pressed(cmd | Modifiers::SHIFT, Key::S) {
            self.save(true);
        }
        if pressed(cmd, Key::S) {
            self.save(false);
        }
        if pressed(cmd, Key::O) {
            self.request(Discard::Open);
        }
        if pressed(cmd, Key::N) {
            self.request(Discard::New);
        }
        if pressed(Modifiers::NONE, Key::Escape) {
            self.tool = Tool::Select;
        }
        // Delete keys only act on the diagram, not while typing in a field.
        if !ctx.egui_wants_keyboard_input() {
            if pressed(cmd, Key::Delete) {
                self.exec("edit.delete-from-model", json!({}));
            }
            if pressed(Modifiers::NONE, Key::Delete) || pressed(Modifiers::NONE, Key::Backspace) {
                self.exec("edit.delete", json!({}));
            }
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() {
                    self.request(Discard::New);
                }
                if ui.button("Open…").clicked() {
                    self.request(Discard::Open);
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    self.save(false);
                }
                if ui.button("Save As…").clicked() {
                    self.save(true);
                }
                if self.files.has_paths() {
                    ui.separator();
                    if ui.button("Open Path…").clicked() {
                        self.path_prompt = Some(PathPrompt::Open(String::new()));
                    }
                    if ui.button("Save to Path…").clicked() {
                        let current = self.session.path.clone().unwrap_or_default();
                        self.path_prompt = Some(PathPrompt::Save(current));
                    }
                }
                ui.separator();
                if ui.button("Import Mermaid…").clicked() {
                    self.mermaid = Some(MermaidPrompt::default());
                }
                ui.separator();
                if ui.button("Export Diagram as SVG…").clicked() {
                    self.export(FileKind::Svg);
                }
                if ui.button("Export Diagram as PNG…").clicked() {
                    self.export(FileKind::Png);
                }
            });
            ui.menu_button("Edit", |ui| {
                for id in [
                    "edit.undo",
                    "edit.redo",
                    "edit.delete",
                    "edit.delete-from-model",
                ] {
                    self.command_button(ui, id);
                }
            });
            ui.menu_button("Model", |ui| {
                ui.menu_button("Add Diagram", |ui| {
                    for (ty, label, _) in uml::DIAGRAMS {
                        if ui.button(*label).clicked() {
                            self.exec("diagram.add", json!({ "type": ty }));
                        }
                    }
                });
            });
            ui.menu_button("View", |ui| {
                if ui.button("Zoom In").clicked() {
                    self.canvas.set_zoom(self.canvas.zoom() * 1.25);
                }
                if ui.button("Zoom Out").clicked() {
                    self.canvas.set_zoom(self.canvas.zoom() / 1.25);
                }
                if ui.button("Fit Diagram").clicked() {
                    self.canvas.request_fit();
                }
                if ui.button("Actual Size").clicked() {
                    self.canvas.actual_size();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
                ui.label(
                    "An independent, open-source UML modeler that reads and writes StarUML files.",
                );
                ui.label("Not affiliated with or endorsed by MKLabs (StarUML).");
            });
        });
    }

    fn command_button(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(spec) = self.registry.get(id) else {
            return;
        };
        let enabled = (spec.enabled)(&self.session);
        let mut button = egui::Button::new(spec.label);
        if let Some(s) = spec.shortcut {
            button = button.shortcut_text(s);
        }
        if ui.add_enabled(enabled, button).clicked() {
            self.exec(id, json!({}));
        }
    }

    /// The tools of the active diagram type, grouped as in StarUML.
    fn toolbox(&mut self, ui: &mut egui::Ui) {
        ui.selectable_value(&mut self.tool, Tool::Select, "Select");
        let diagram_type = self
            .session
            .active_diagram
            .as_deref()
            .and_then(|d| self.session.doc.get(d))
            .map(|d| d.type_name.clone())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for group in uml::tool_groups(&diagram_type) {
                    ui.separator();
                    ui.weak(group.label);
                    for id in group.tools {
                        let tool = match uml::tool_on(id, &diagram_type) {
                            Some(uml::Tool::Node(spec)) => Tool::Node(spec),
                            Some(uml::Tool::Edge(spec)) => Tool::Edge(spec),
                            None => continue,
                        };
                        let label = match tool {
                            Tool::Node(n) => n.label,
                            Tool::Edge(e) => e.label,
                            Tool::Select => "Select",
                        };
                        ui.selectable_value(&mut self.tool, tool, label);
                    }
                }
            });
    }

    fn path_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = &mut self.path_prompt else {
            return;
        };
        let (title, buf) = match prompt {
            PathPrompt::Open(b) => ("Open model from path", b),
            PathPrompt::Save(b) => ("Save model to path", b),
        };
        let mut done = None;
        egui::Modal::new(egui::Id::new("path")).show(ctx, |ui| {
            ui.label(title);
            let r = ui.add(
                egui::TextEdit::singleline(buf)
                    .desired_width(420.0)
                    .hint_text("/path/to/model.mdj"),
            );
            r.request_focus();
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() || enter {
                    done = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    done = Some(false);
                }
            });
        });
        let Some(ok) = done else { return };
        match self.path_prompt.take() {
            Some(PathPrompt::Open(p)) if ok && !p.trim().is_empty() => {
                self.request(Discard::OpenPath(p.trim().to_owned()))
            }
            Some(PathPrompt::Save(p)) if ok && !p.trim().is_empty() => {
                self.save_to(Some(p.trim().to_owned()))
            }
            _ => {}
        }
    }

    fn mermaid_dialog(&mut self, ctx: &egui::Context) {
        let Some(prompt) = &mut self.mermaid else {
            return;
        };
        let mut action = None;
        egui::Modal::new(egui::Id::new("mermaid")).show(ctx, |ui| {
            ui.strong("Import Mermaid");
            ui.label(
                "Paste a classDiagram, sequenceDiagram or stateDiagram. It becomes a new diagram.",
            );
            ui.horizontal(|ui| {
                for (label, text) in MERMAID_EXAMPLES {
                    if ui.button(format!("{label} example")).clicked() {
                        prompt.text = (*text).to_owned();
                        prompt.error = None;
                    }
                }
            });
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut prompt.text)
                            .code_editor()
                            .desired_rows(14)
                            .desired_width(520.0)
                            .hint_text("classDiagram\n    Animal <|-- Duck"),
                    );
                });
            if let Some(e) = &prompt.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.horizontal(|ui| {
                let ready = !prompt.text.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Import")).clicked() {
                    action = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    action = Some(false);
                }
            });
        });
        match action {
            Some(true) => {
                let text = prompt.text.clone();
                match self.registry.run(
                    &mut self.session,
                    "import.mermaid",
                    &json!({ "text": text }),
                ) {
                    Ok(r) => {
                        let skipped = r["warnings"].as_array().map_or(0, Vec::len);
                        self.status = if skipped == 0 {
                            "Imported the Mermaid diagram".into()
                        } else {
                            format!(
                                "Imported the Mermaid diagram; skipped {skipped}: {}",
                                r["warnings"][0].as_str().unwrap_or_default()
                            )
                        };
                        self.mermaid = None;
                    }
                    Err(e) => {
                        if let Some(p) = &mut self.mermaid {
                            p.error = Some(e.to_string());
                        }
                    }
                }
            }
            Some(false) => self.mermaid = None,
            None => {}
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm.clone() else {
            return;
        };
        let mut choice = None;
        egui::Modal::new(egui::Id::new("discard")).show(ctx, |ui| {
            ui.label(format!(
                "{} has unsaved changes. Discard them?",
                self.session.file_name
            ));
            ui.horizontal(|ui| {
                if ui.button("Discard").clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        if let Some(discard) = choice {
            self.confirm = None;
            if discard {
                self.perform(action);
            }
        }
    }
}

impl eframe::App for UmlApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let diagram_before = self.session.active_diagram.clone();
        self.handle_file_events();
        self.run_remote_jobs();
        self.shortcuts(&ctx);

        let title = format!(
            "{}{} — {APP_NAME}",
            self.session.file_name,
            if self.session.doc.is_modified() {
                " *"
            } else {
                ""
            }
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        let mut commands: Vec<(&'static str, Value)> = Vec::new();

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(if self.status.is_empty() {
                    "Ready"
                } else {
                    &self.status
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(format!("{:.0}%", self.canvas.zoom() * 100.0));
                });
            });
        });
        egui::Panel::left("explorer")
            .resizable(true)
            .default_size(240.0)
            .min_size(180.0)
            .show(ui, |ui| {
                ui.strong("Model Explorer");
                ui.separator();
                commands.extend(explorer::show(ui, &self.session));
            });
        egui::Panel::right("properties")
            .resizable(true)
            .default_size(300.0)
            .min_size(260.0)
            .show(ui, |ui| {
                commands.extend(self.properties.show(ui, &self.session));
            });
        egui::Panel::left("toolbox")
            .resizable(false)
            .exact_size(170.0)
            .show(ui, |ui| self.toolbox(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let selection = self.session.selection.clone();
                let tool = self.tool;
                if self.scene().is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.weak("No diagram open. Double-click one in the Model Explorer.")
                    });
                    return;
                }
                let scene = self.scene.take();
                if let Some((rev, d, scene)) = scene {
                    let out = self.canvas.show(ui, &scene, &selection, tool);
                    self.scene = Some((rev, d, scene));
                    commands.extend(out.commands);
                    if out.tool_done {
                        self.tool = Tool::Select;
                    }
                    if out.rename {
                        self.properties.focus_name = true;
                    }
                }
            });

        for (id, params) in commands {
            self.exec(id, params);
        }
        self.confirm_dialog(&ctx);
        self.path_dialog(&ctx);
        self.mermaid_dialog(&ctx);
        if self.session.active_diagram != diagram_before {
            self.canvas.request_fit();
        }
        self.drive_screenshot(&ctx);
        self.drive_captures(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use egui::accesskit::Role;
    use egui::{Key, Modifiers, Pos2, Vec2};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{Queryable, by};

    use super::*;

    struct NoFiles;
    impl FileService for NoFiles {
        fn open_model(&mut self) {}
        fn save(&mut self, _: FileKind, _: &str, _: Option<&str>, _: Vec<u8>) {}
        fn poll(&mut self) -> Vec<FileEvent> {
            Vec::new()
        }
    }

    fn harness() -> Harness<'static, UmlApp> {
        Harness::builder()
            .with_size(Vec2::new(1280.0, 800.0))
            .build_eframe(|cc| UmlApp::new(cc, Box::new(NoFiles)))
    }

    fn views_of_type<'a>(app: &'a UmlApp, t: &str) -> Vec<&'a mdj::Element> {
        app.session
            .doc
            .iter()
            .filter(|e| e.type_name == t)
            .collect()
    }

    fn drag(h: &mut Harness<'static, UmlApp>, from: Pos2, to: Pos2) {
        h.drag_at(from);
        h.run();
        for i in 1..=5 {
            h.hover_at(from + (to - from) * (i as f32 / 5.0));
            h.run();
        }
        h.drop_at(to);
        h.run();
    }

    fn click(h: &mut Harness<'static, UmlApp>, at: Pos2) {
        h.drag_at(at);
        h.run();
        h.drop_at(at);
        h.run();
    }

    fn center_of(h: &Harness<'static, UmlApp>, view: &str) -> Pos2 {
        let app = h.state();
        let el = app.session.doc.get(view).unwrap();
        let p = render::Point::new(
            el.num("left", 0.0) + el.num("width", 0.0) / 2.0,
            el.num("top", 0.0) + el.num("height", 0.0) / 2.0,
        );
        app.canvas.to_screen(p)
    }

    /// Place a class with the toolbox, by clicking where it should go.
    fn place_class(h: &mut Harness<'static, UmlApp>, at: render::Point) -> String {
        h.get_by_role_and_label(Role::Button, "Class").click();
        h.run();
        let screen = h.state().canvas.to_screen(at);
        let before = views_of_type(h.state(), "UMLClassView").len();
        click(h, screen);
        let app = h.state();
        assert_eq!(
            views_of_type(app, "UMLClassView").len(),
            before + 1,
            "a class was placed"
        );
        assert_eq!(app.tool, Tool::Select, "the tool resets after one use");
        app.session.selection[0].clone()
    }

    #[test]
    fn remote_jobs_run_between_frames() {
        let (jobs, rx) = std::sync::mpsc::channel::<RemoteJob>();
        let mut h = Harness::builder()
            .with_size(Vec2::new(1280.0, 800.0))
            .build_eframe(|cc| UmlApp::new(cc, Box::new(NoFiles)).with_remote(rx));
        h.run();

        // An agent draws a class and looks at what the user sees.
        let (seen_tx, seen) = std::sync::mpsc::channel::<Value>();
        jobs.send(Box::new(move |r: &mut Remote<'_>| {
            let (reg, s) = r.parts();
            reg.run(
                s,
                "diagram.add",
                &json!({"type": "UMLClassDiagram", "name": "Live"}),
            )
            .unwrap();
            reg.run(
                s,
                "node.create",
                &json!({"kind": "Class", "x": 60, "y": 60}),
            )
            .unwrap();
            seen_tx.send(r.inspect()).unwrap();
        }))
        .unwrap();
        h.run();
        let seen = seen.try_recv().unwrap();
        assert_eq!(seen["activeDiagram"]["name"], "Live");
        assert_eq!(seen["unsavedChanges"], true);
        assert_eq!(seen["tool"], "Select");
        assert_eq!(seen["dialogs"], json!([]));
        let app = h.state();
        assert_eq!(views_of_type(app, "UMLClassView").len(), 1);
        assert!(
            matches!(&app.scene, Some((_, d, _)) if app.session.active_diagram.as_ref() == Some(d)),
            "the window draws the agent's diagram"
        );

        // A window capture is handed over when the frame arrives.
        let (shot_tx, shot) = std::sync::mpsc::channel::<[usize; 2]>();
        jobs.send(Box::new(move |r: &mut Remote<'_>| {
            r.screenshot(Box::new(move |image| shot_tx.send(image.size).unwrap()));
        }))
        .unwrap();
        h.run();
        assert!(h.state().capture_requested);
        h.input_mut().events.push(egui::Event::Screenshot {
            viewport_id: egui::ViewportId::ROOT,
            user_data: egui::UserData::default(),
            image: std::sync::Arc::new(egui::ColorImage::filled([3, 2], egui::Color32::RED)),
        });
        h.run();
        assert_eq!(shot.try_recv().unwrap(), [3, 2]);
        assert!(!h.state().capture_requested);

        // Replacing the model resets the view onto the new one.
        h.state_mut().tool = match uml::tool("Class") {
            Some(uml::Tool::Node(n)) => Tool::Node(n),
            _ => panic!("no Class tool"),
        };
        jobs.send(Box::new(|r: &mut Remote<'_>| {
            let (_, s) = r.parts();
            *s = Session::default();
            r.session_replaced();
        }))
        .unwrap();
        h.run();
        assert_eq!(h.state().tool, Tool::Select);
        assert!(views_of_type(h.state(), "UMLClassView").is_empty());
    }

    #[test]
    fn place_move_delete_and_undo_with_the_mouse() {
        let mut h = harness();
        h.run();
        let view = place_class(&mut h, render::Point::new(100.0, 100.0));
        let (left, top) = {
            let v = h.state().session.doc.get(&view).unwrap();
            (v.num("left", 0.0), v.num("top", 0.0))
        };
        assert_eq!((left, top), (100.0, 100.0));

        let c = center_of(&h, &view);
        drag(&mut h, c, c + Vec2::new(120.0, 60.0));
        let v = h.state().session.doc.get(&view).unwrap();
        assert_eq!(
            (v.num("left", 0.0), v.num("top", 0.0)),
            (220.0, 160.0),
            "dragged by (120, 60)"
        );

        h.key_press(Key::Delete);
        h.run();
        assert!(
            h.state().session.doc.get(&view).is_none(),
            "Delete removes the view"
        );
        assert_eq!(
            views_of_type(h.state(), "UMLClass").len(),
            1,
            "but keeps the class"
        );

        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run();
        assert!(
            h.state().session.doc.get(&view).is_some(),
            "Ctrl+Z brings it back"
        );
    }

    #[test]
    fn draw_a_generalization_between_two_classes() {
        let mut h = harness();
        h.run();
        let child = place_class(&mut h, render::Point::new(80.0, 300.0));
        let parent = place_class(&mut h, render::Point::new(80.0, 60.0));
        h.get_by_role_and_label(Role::Button, "Generalization")
            .click();
        h.run();
        let (from, to) = (center_of(&h, &child), center_of(&h, &parent));
        drag(&mut h, from, to);

        let app = h.state();
        let edges = views_of_type(app, "UMLGeneralizationView");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].ref_id("tail"), Some(child.as_str()));
        assert_eq!(edges[0].ref_id("head"), Some(parent.as_str()));
        let points = mdj::values::parse_points(edges[0].str("points").unwrap_or(""));
        assert!(points.len() >= 2, "the new edge was routed: {points:?}");
    }

    #[test]
    fn communication_diagram_by_mouse() {
        let mut h = harness();
        h.run();
        h.state_mut()
            .exec("diagram.add", json!({"type": "UMLCommunicationDiagram"}));
        h.run();
        let place = |h: &mut Harness<'static, UmlApp>, x: f64, y: f64| {
            h.get_by_role_and_label(Role::Button, "Lifeline").click();
            h.run();
            let at = h.state().canvas.to_screen(render::Point::new(x, y));
            click(h, at);
            h.state().session.selection[0].clone()
        };
        let a = place(&mut h, 100.0, 200.0);
        let b = place(&mut h, 450.0, 200.0);
        assert_eq!(views_of_type(h.state(), "UMLCommLifelineView").len(), 2);

        h.get_by_role_and_label(Role::Button, "Forward Message")
            .click();
        h.run();
        let (from, to) = (center_of(&h, &a), center_of(&h, &b));
        drag(&mut h, from, to);
        assert_eq!(views_of_type(h.state(), "UMLCommMessageView").len(), 1);
        assert_eq!(views_of_type(h.state(), "UMLConnectorView").len(), 1);

        // Dropped on the connector line (away from the first arrow), a
        // reverse message rides on the same connector.
        h.get_by_role_and_label(Role::Button, "Reverse Message")
            .click();
        h.run();
        let on_line = h.state().canvas.to_screen(render::Point::new(250.0, 215.0));
        let from = center_of(&h, &b);
        drag(&mut h, from, on_line);
        let app = h.state();
        assert_eq!(
            views_of_type(app, "UMLCommMessageView").len(),
            2,
            "{}",
            app.status
        );
        assert_eq!(views_of_type(app, "UMLConnectorView").len(), 1);
    }

    #[test]
    fn interaction_overview_by_mouse() {
        let mut h = harness();
        h.run();
        h.state_mut().exec(
            "diagram.add",
            json!({"type": "UMLInteractionOverviewDiagram"}),
        );
        h.run();
        let mut place = |tool: &str, x: f64, y: f64| {
            h.get_by_role_and_label(Role::Button, tool).click();
            h.run();
            let at = h.state().canvas.to_screen(render::Point::new(x, y));
            click(&mut h, at);
            h.state().session.selection[0].clone()
        };
        let start = place("Initial", 60.0, 120.0);
        let call = place("Interaction Use", 160.0, 200.0);
        let app = h.state();
        assert_eq!(
            app.session.doc.get(&call).unwrap().type_name,
            "UMLInteractionUseView"
        );
        h.get_by_role_and_label(Role::Button, "Control Flow")
            .click();
        h.run();
        let (from, to) = (center_of(&h, &start), center_of(&h, &call));
        drag(&mut h, from, to);
        assert_eq!(views_of_type(h.state(), "UMLControlFlowView").len(), 1);
    }

    #[test]
    fn timing_diagram_by_mouse() {
        fn click_tool(h: &mut Harness<'static, UmlApp>, tool: &str, at: render::Point) -> String {
            h.get_by_role_and_label(Role::Button, tool).click();
            h.run();
            let at = h.state().canvas.to_screen(at);
            click(h, at);
            h.state().session.selection[0].clone()
        }
        let mut h = harness();
        h.run();
        h.state_mut()
            .exec("diagram.add", json!({"type": "UMLTimingDiagram"}));
        h.run();
        // Each tool lands inside what was clicked: lifelines in the frame,
        // a state in the lifeline, a segment on the state's row.
        let lifeline = click_tool(&mut h, "Lifeline", render::Point::new(300.0, 300.0));
        let app = h.state();
        let diagram = app.session.active_diagram.clone().unwrap();
        let scene = render::build_scene(&app.session.doc, &diagram, app.session.fonts());
        let band = scene.node_rect(&lifeline).unwrap();
        let state = click_tool(&mut h, "State/Condition", band.center());
        // States start 25 px down the band, one 25 px row each.
        let row = render::Point::new(band.center().x, band.y + 25.0 + 12.0);
        let segment = click_tool(&mut h, "Time Segment", row);
        let doc = &h.state().session.doc;
        let kind = |v: &str| doc.get(v).unwrap().type_name.clone();
        assert_eq!(kind(&lifeline), "UMLTimingLifelineView");
        assert_eq!(kind(&state), "UMLTimingStateView");
        assert_eq!(kind(&segment), "UMLTimeSegmentView");
        assert_eq!(
            doc.get(&segment).unwrap().ref_id("containerView"),
            Some(state.as_str())
        );
    }

    #[test]
    fn component_diagram_toolbox_places_a_port_on_a_component() {
        let mut h = harness();
        h.run();
        h.state_mut()
            .exec("diagram.add", json!({"type": "UMLComponentDiagram"}));
        h.run();
        assert!(
            h.query_by_role_and_label(Role::Button, "Port").is_some(),
            "toolbox switched"
        );
        assert!(
            h.query_by_role_and_label(Role::Button, "Enumeration")
                .is_none()
        );

        h.get_by_role_and_label(Role::Button, "Component").click();
        h.run();
        let at = h.state().canvas.to_screen(render::Point::new(100.0, 100.0));
        click(&mut h, at);
        let comp = h.state().session.selection[0].clone();

        h.get_by_role_and_label(Role::Button, "Port").click();
        h.run();
        // Just inside the component's right edge (it is 140 wide).
        let at = h.state().canvas.to_screen(render::Point::new(236.0, 125.0));
        click(&mut h, at);
        let app = h.state();
        let port = app.session.doc.get(&app.session.selection[0]).unwrap();
        assert_eq!(port.type_name, "UMLPortView");
        assert_eq!(port.ref_id("containerView"), Some(comp.as_str()));
        assert_eq!(port.num("left", 0.0) + 7.0, 240.0, "snapped to the border");
    }

    #[test]
    fn import_mermaid_from_the_file_menu() {
        let mut h = harness();
        h.run();
        h.get_by_role_and_label(Role::Button, "File").click();
        h.run();
        h.get_by_role_and_label(Role::Button, "Import Mermaid…")
            .click();
        h.run();
        // A broken text keeps the dialog open with the error.
        h.get_by_role_and_label(Role::Button, "State example")
            .click();
        h.run();
        h.state_mut().mermaid.as_mut().unwrap().text = "flowchart TD\nA-->B".into();
        h.run();
        h.get_by_role_and_label(Role::Button, "Import").click();
        h.run();
        let error = h.state().mermaid.as_ref().and_then(|m| m.error.clone());
        assert!(error.is_some_and(|e| e.contains("flowchart")));

        h.get_by_role_and_label(Role::Button, "Class example")
            .click();
        h.run();
        h.get_by_role_and_label(Role::Button, "Import").click();
        h.run();
        assert!(h.state().mermaid.is_none(), "{:?}", h.state().status);
        assert_eq!(views_of_type(h.state(), "UMLClass").len(), 4);
        assert_eq!(views_of_type(h.state(), "UMLInterface").len(), 1);
        let active = h.state().session.active_diagram.clone().unwrap();
        let d = h.state().session.doc.get(&active).unwrap();
        assert_eq!(d.type_name, "UMLClassDiagram");
        // One undo step takes the whole import away.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run();
        assert!(views_of_type(h.state(), "UMLClass").is_empty());
    }

    #[test]
    fn renaming_through_the_properties_panel_is_one_undo_step() {
        let mut h = harness();
        h.run();
        place_class(&mut h, render::Point::new(100.0, 100.0));
        let name_box = h.get(by().role(Role::TextInput).value("Class1"));
        name_box.click();
        h.run();
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run();
        h.event(egui::Event::Text("Customer".into()));
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(views_of_type(h.state(), "UMLClass")[0].name(), "Customer");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run();
        assert_eq!(views_of_type(h.state(), "UMLClass")[0].name(), "Class1");
    }
}
