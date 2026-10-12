//! The same app in a browser. Build and serve with trunk:
//! `cd apps/web && trunk serve` (see docs/development.md).

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use eframe::wasm_bindgen::JsCast;
    use ui::{FileEvent, FileKind, FileService, UmlApp};

    pub fn start() {
        wasm_bindgen_futures::spawn_local(async {
            let canvas = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("app"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            let Some(canvas) = canvas else {
                web_sys::console::error_1(&"no <canvas id=\"app\"> in the page".into());
                return;
            };
            let started = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| {
                        let files = WebFiles {
                            ctx: cc.egui_ctx.clone(),
                            events: Rc::default(),
                        };
                        Ok(Box::new(UmlApp::new(cc, Box::new(files))))
                    }),
                )
                .await;
            if let Err(e) = started {
                web_sys::console::error_1(&e);
            }
        });
    }

    /// Opening uses the browser's file picker; saving downloads the file.
    struct WebFiles {
        ctx: eframe::egui::Context,
        events: Rc<RefCell<Vec<FileEvent>>>,
    }

    impl FileService for WebFiles {
        fn open_model(&mut self) {
            let events = Rc::clone(&self.events);
            let ctx = self.ctx.clone();
            let dialog = rfd::AsyncFileDialog::new()
                .add_filter("StarUML model", &["mdj", "mfj"])
                .pick_file();
            wasm_bindgen_futures::spawn_local(async move {
                let Some(file) = dialog.await else { return };
                let name = file.file_name();
                let bytes = file.read().await;
                let event = match String::from_utf8(bytes) {
                    Ok(text) => FileEvent::Opened {
                        name,
                        path: None,
                        text,
                    },
                    Err(_) => FileEvent::Failed(format!("{name} is not a text file")),
                };
                events.borrow_mut().push(event);
                ctx.request_repaint();
            });
        }

        fn save(
            &mut self,
            kind: FileKind,
            suggested_name: &str,
            _path: Option<&str>,
            bytes: Vec<u8>,
        ) {
            let event = match download(kind, suggested_name, &bytes) {
                Ok(()) => FileEvent::Saved {
                    kind,
                    name: suggested_name.to_owned(),
                    path: None,
                },
                Err(e) => FileEvent::Failed(format!("Download failed: {e:?}")),
            };
            self.events.borrow_mut().push(event);
        }

        fn poll(&mut self) -> Vec<FileEvent> {
            std::mem::take(&mut *self.events.borrow_mut())
        }
    }

    fn download(kind: FileKind, name: &str, bytes: &[u8]) -> Result<(), wasm_bindgen::JsValue> {
        let mime = match kind {
            FileKind::Model => "application/json",
            FileKind::Svg => "image/svg+xml",
            FileKind::Png => "image/png",
        };
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        web_sys::Url::revoke_object_url(&url)
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("rust-uml-web runs in a browser: cd apps/web && trunk serve");
}
