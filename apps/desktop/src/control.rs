//! `--control`: let agents drive this window over the loopback control
//! channel (`automation::control`). Requests arrive on a background thread
//! and run on the UI thread, between frames, as `ui::RemoteJob`s.

use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use anyhow::{Context as _, anyhow};
use automation::control::{self, Live, Options, Pixels, ScreenshotDone, Token};
use automation::workspace::Workspace;
use eframe::egui;
use engine::{Registry, Session};
use serde_json::Value;
use ui::{Remote, RemoteJob};

/// `--control <port> --control-token-file <path> [--control-root <dir>]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlArgs {
    pub port: u16,
    pub token_file: PathBuf,
    pub root: Option<PathBuf>,
}

/// Bound and ready, before the window opens (so a busy port is reported
/// straight away).
pub struct Control {
    listener: TcpListener,
    token: Token,
    workspace: Option<Workspace>,
}

impl Control {
    pub fn bind(args: &ControlArgs) -> anyhow::Result<Self> {
        let token = Token::load_or_create(&args.token_file).map_err(|e| anyhow!(e))?;
        let workspace = args
            .root
            .as_ref()
            .map(Workspace::new)
            .transpose()
            .map_err(|e| anyhow!(e))?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, args.port))
            .with_context(|| format!("--control {}", args.port))?;
        let port = listener.local_addr()?.port();
        eprintln!(
            "control channel on 127.0.0.1:{port}; files: {}",
            workspace
                .as_ref()
                .map_or("none (no --control-root)".into(), |w| w
                    .root()
                    .display()
                    .to_string())
        );
        eprintln!(
            "connect an agent: rust-uml-cli mcp --bridge 127.0.0.1:{port} --control-token-file {}",
            args.token_file.display()
        );
        Ok(Self {
            listener,
            token,
            workspace,
        })
    }

    /// Start serving; the window runs the jobs that arrive on the receiver.
    pub fn serve(self, ctx: egui::Context) -> std::io::Result<mpsc::Receiver<RemoteJob>> {
        let (jobs, rx) = mpsc::channel::<RemoteJob>();
        let workspace = self.workspace.map(Arc::new);
        control::serve(self.listener, self.token, Options::default(), move |req| {
            let workspace = workspace.clone();
            let job: RemoteJob = Box::new(move |remote| {
                control::dispatch(&mut LiveWindow(remote), workspace.as_deref(), req);
            });
            // If the window is gone the job (and its request) is dropped,
            // and the caller hears that it was not run.
            let _ = jobs.send(job);
            ctx.request_repaint();
        })?;
        Ok(rx)
    }
}

struct LiveWindow<'r, 'a>(&'r mut Remote<'a>);

impl Live for LiveWindow<'_, '_> {
    fn parts(&mut self) -> (&Registry, &mut Session) {
        self.0.parts()
    }

    fn session_replaced(&mut self) {
        self.0.session_replaced();
    }

    fn inspect(&mut self) -> Value {
        self.0.inspect()
    }

    fn screenshot(&mut self, done: ScreenshotDone) {
        self.0.screenshot(Box::new(move |image| {
            let [w, h] = image.size;
            let rgba = image
                .pixels
                .iter()
                .flat_map(|c| c.to_srgba_unmultiplied())
                .collect();
            done(Ok(Pixels {
                width: w as u32,
                height: h as u32,
                rgba,
            }));
        }));
    }
}
