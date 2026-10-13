//! The running app's control channel: loopback TCP, one JSON object per line.
//!
//! `rust-uml --control <port> --control-token-file <path>` listens on
//! 127.0.0.1 only. The first line on every connection must be
//! `{"id": 0, "method": "auth", "params": {"token": "<64 hex digits>"}}`;
//! nothing else runs before it, and a wrong token closes the connection.
//! After that each request `{"id", "method", "params"}` gets one reply,
//! `{"id", "ok": true, "result"}` or `{"id", "ok": false, "error"}`.
//!
//! [`serve`] is only the transport: it hands each [`Request`] to a callback,
//! which the app forwards to its UI thread, where [`dispatch`] runs it on the
//! [`Live`] app. Methods and their params: `docs/agents.md`.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use base64::Engine as _;
use engine::{Registry, Session};
use serde_json::{Map, Value, json};

use crate::mcp::{Target, call_tool};
use crate::workspace::Workspace;

/// Longest request or reply line (a screenshot of a large window in base64
/// is a few MB).
pub const MAX_LINE: usize = 64 << 20;

/// Connections served at once; more are refused.
pub const MAX_CONNECTIONS: usize = 8;

/// How long a new connection may take to authenticate.
const AUTH_TIMEOUT: Duration = Duration::from_secs(30);

/// Every method [`dispatch`] knows, for error messages.
pub const METHODS: &[&str] = &[
    "ping",
    "engine.execute",
    "engine.commands",
    "tool.call",
    "ui.inspect",
    "ui.screenshot",
];

/// The shared secret: 32 random bytes as 64 lowercase hex digits.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(…)")
    }
}

impl Token {
    /// A fresh token from the operating system's secure random numbers.
    pub fn generate() -> Result<Self, String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| format!("no secure random numbers: {e}"))?;
        Ok(Self(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let t = text.trim();
        if t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(t.to_ascii_lowercase()))
        } else {
            Err("a control token is 64 hexadecimal digits".into())
        }
    }

    /// Read the token in `path`, or create the file with a fresh one. On Unix
    /// a new file is readable by its owner only, and an existing file that
    /// other users can read is refused.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        let shown = path.display();
        match std::fs::read_to_string(path) {
            Ok(text) => {
                check_private(path)?;
                Self::parse(&text).map_err(|e| format!("{shown}: {e}"))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let token = Self::generate()?;
                write_private(path, &token.0).map_err(|e| format!("{shown}: {e}"))?;
                Ok(token)
            }
            Err(e) => Err(format!("{shown}: {e}")),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compare in constant time, so the reply time says nothing about how
    /// much of a guess was right.
    fn matches(&self, given: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), given.trim().as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

fn write_private(path: &Path, token: &str) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(file, "{token}")
}

fn check_private(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} can be read by other users: `chmod 600` it, or delete it to get a new token",
                path.display()
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

const QUEUED: u8 = 0;
const STARTED: u8 = 1;
const ABANDONED: u8 = 2;

/// One authenticated request, waiting to run on the UI thread.
pub struct Request {
    pub method: String,
    pub params: Value,
    state: Arc<AtomicU8>,
    reply: mpsc::Sender<Result<Value, String>>,
}

impl Request {
    /// A request and the receiver its reply arrives on.
    pub fn new(method: &str, params: Value) -> (Self, mpsc::Receiver<Result<Value, String>>) {
        let (tx, rx) = mpsc::channel();
        let req = Self {
            method: method.to_owned(),
            params,
            state: Arc::new(AtomicU8::new(QUEUED)),
            reply: tx,
        };
        (req, rx)
    }

    /// Claim the request for running. False when the caller already gave up
    /// waiting: then it must not run, because the caller was told it didn't.
    pub fn start(&self) -> bool {
        self.state
            .compare_exchange(QUEUED, STARTED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn reply(self, result: Result<Value, String>) {
        // The connection may have closed meanwhile; nobody to tell.
        let _ = self.reply.send(result);
    }
}

/// Transport settings.
#[derive(Debug, Clone)]
pub struct Options {
    /// How long a request may wait for its reply.
    pub reply_timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            reply_timeout: Duration::from_secs(60),
        }
    }
}

/// Serve the control channel on `listener` (bound to 127.0.0.1) from a
/// background thread. Each authenticated request goes to `sink`, which must
/// get it to [`dispatch`] (or reply to it) soon.
pub fn serve(
    listener: TcpListener,
    token: Token,
    options: Options,
    sink: impl Fn(Request) + Send + Sync + 'static,
) -> io::Result<()> {
    let sink: Arc<dyn Fn(Request) + Send + Sync> = Arc::new(sink);
    let open = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("control".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) {
                    continue;
                }
                if open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                    open.fetch_sub(1, Ordering::SeqCst);
                    let mut s = &stream;
                    let _ = send(&mut s, &failure(&Value::Null, "too many connections"));
                    continue;
                }
                let (token, options, sink, open2) =
                    (token.clone(), options.clone(), sink.clone(), open.clone());
                let spawned = std::thread::Builder::new()
                    .name("control-connection".into())
                    .spawn(move || {
                        let _ = connection(stream, &token, &options, &*sink);
                        open2.fetch_sub(1, Ordering::SeqCst);
                    });
                if spawned.is_err() {
                    open.fetch_sub(1, Ordering::SeqCst);
                }
            }
        })?;
    Ok(())
}

fn connection(
    stream: TcpStream,
    token: &Token,
    options: &Options,
    sink: &(dyn Fn(Request) + Send + Sync),
) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    stream.set_read_timeout(Some(AUTH_TIMEOUT))?;
    let mut reader = BufReader::new(stream);

    let Some(line) = read_line(&mut reader, MAX_LINE)? else {
        return Ok(());
    };
    let refusal = match parse(&line) {
        Err((id, e)) => Some((id, e)),
        Ok((id, method, _)) if method != "auth" => Some((
            id,
            r#"authenticate first: {"id": 0, "method": "auth", "params": {"token": "…"}}"#.into(),
        )),
        Ok((id, _, params)) => match params.get("token").and_then(Value::as_str) {
            Some(t) if token.matches(t) => {
                let hello = json!({ "app": "rust-uml", "version": env!("CARGO_PKG_VERSION") });
                send(&mut writer, &success(&id, hello))?;
                None
            }
            _ => Some((id, "wrong token".into())),
        },
    };
    if let Some((id, e)) = refusal {
        return send(&mut writer, &failure(&id, &e));
    }
    reader.get_ref().set_read_timeout(None)?;

    while let Some(line) = read_line(&mut reader, MAX_LINE)? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match parse(&line) {
            Err((id, e)) => failure(&id, &e),
            Ok((id, method, _)) if method == "auth" => failure(&id, "already authenticated"),
            Ok((id, method, params)) => run(&id, &method, params, options, sink),
        };
        send(&mut writer, &reply)?;
    }
    Ok(())
}

/// Hand a request to the app and wait for its reply.
fn run(
    id: &Value,
    method: &str,
    params: Value,
    options: &Options,
    sink: &(dyn Fn(Request) + Send + Sync),
) -> Value {
    let (req, rx) = Request::new(method, params);
    let state = req.state.clone();
    sink(req);
    let never_ran = || {
        state
            .compare_exchange(QUEUED, ABANDONED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    };
    match rx.recv_timeout(options.reply_timeout) {
        Ok(Ok(v)) => success(id, v),
        Ok(Err(e)) => failure(id, &e),
        Err(RecvTimeoutError::Timeout) if never_ran() => failure(
            id,
            "timeout: not run (the window was not drawing, e.g. minimised); safe to send again",
        ),
        Err(RecvTimeoutError::Timeout) => failure(
            id,
            &format!(
                "timeout: no reply in {}s; it may still be running and still apply, so check before sending it again",
                options.reply_timeout.as_secs()
            ),
        ),
        Err(RecvTimeoutError::Disconnected) if never_ran() => {
            failure(id, "not run: the app is closing")
        }
        Err(RecvTimeoutError::Disconnected) => failure(id, "the app dropped the request"),
    }
}

/// A request line: `(id, method, params)`, or `(id, error)`.
fn parse(line: &str) -> Result<(Value, String, Value), (Value, String)> {
    let msg: Value =
        serde_json::from_str(line).map_err(|e| (Value::Null, format!("not JSON: {e}")))?;
    let Some(obj) = msg.as_object() else {
        return Err((Value::Null, "a request is a JSON object".into()));
    };
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Err((id, "missing `method`".into()));
    };
    let params = match obj.get("params") {
        None | Some(Value::Null) => json!({}),
        Some(p @ Value::Object(_)) => p.clone(),
        Some(_) => return Err((id, "`params` must be an object".into())),
    };
    Ok((id, method.to_owned(), params))
}

fn success(id: &Value, result: Value) -> Value {
    json!({ "id": id, "ok": true, "result": result })
}

fn failure(id: &Value, error: &str) -> Value {
    json!({ "id": id, "ok": false, "error": error })
}

pub(crate) fn send(w: &mut impl Write, v: &Value) -> io::Result<()> {
    writeln!(w, "{v}")?;
    w.flush()
}

/// One line without its newline; `None` at the end of the stream. A line
/// longer than `max` bytes is an error (the connection is then dropped).
pub(crate) fn read_line(r: &mut impl BufRead, max: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    loop {
        let chunk = r.fill_buf()?;
        if chunk.is_empty() {
            if buf.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = chunk.iter().position(|b| *b == b'\n');
        let n = newline.unwrap_or(chunk.len());
        buf.extend_from_slice(&chunk[..n]);
        r.consume(n + usize::from(newline.is_some()));
        if buf.len() > max {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
        }
        if newline.is_some() {
            break;
        }
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8"))
}

/// A window's pixels, row by row, 4 bytes (RGBA) each.
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Called once a screenshot is ready (or failed).
pub type ScreenshotDone = Box<dyn FnOnce(Result<Pixels, String>) + Send>;

/// The running app, as [`dispatch`] sees it.
pub trait Live {
    /// The command registry and the open model (a tool needs both at once).
    fn parts(&mut self) -> (&Registry, &mut Session);
    /// The open model was replaced (new, open): reset the view onto it.
    fn session_replaced(&mut self);
    /// What the user sees: open diagram, selection, tool, zoom, dialogs.
    fn inspect(&mut self) -> Value;
    /// Capture the window; `done` runs once the next frame is captured.
    fn screenshot(&mut self, done: ScreenshotDone);
}

/// Run `req` on the app (call this on the UI thread). File tools are
/// confined to `workspace`; without one they refuse.
pub fn dispatch(live: &mut dyn Live, workspace: Option<&Workspace>, mut req: Request) {
    if !req.start() {
        return;
    }
    let method = std::mem::take(&mut req.method);
    let params = std::mem::take(&mut req.params);
    let result = match method.as_str() {
        "ping" => only(&params, &[]).map(|()| json!({})),
        "engine.execute" => execute(live, &params),
        "engine.commands" => only(&params, &[]).map(|()| commands(live)),
        "tool.call" => tool(live, workspace, &params),
        "ui.inspect" => only(&params, &[]).map(|()| live.inspect()),
        "ui.screenshot" => match only(&params, &[]) {
            Ok(()) => {
                live.screenshot(Box::new(move |r| req.reply(r.and_then(encode))));
                return;
            }
            Err(e) => Err(e),
        },
        _ => Err(format!(
            "unknown method `{method}` (methods: {})",
            METHODS.join(", ")
        )),
    };
    req.reply(result);
}

/// Refuse params outside `allowed`.
fn only(params: &Value, allowed: &[&str]) -> Result<(), String> {
    let empty = Map::new();
    let keys = params.as_object().unwrap_or(&empty).keys();
    match keys.into_iter().find(|k| !allowed.contains(&k.as_str())) {
        None => Ok(()),
        Some(k) if allowed.is_empty() => Err(format!("unknown parameter `{k}` (takes none)")),
        Some(k) => Err(format!(
            "unknown parameter `{k}` (accepts: {})",
            allowed.join(", ")
        )),
    }
}

fn execute(live: &mut dyn Live, params: &Value) -> Result<Value, String> {
    only(params, &["command", "params"])?;
    let command = params["command"]
        .as_str()
        .ok_or("`command` must be a command id (see engine.commands)")?;
    let args = match &params["params"] {
        Value::Null => json!({}),
        p @ Value::Object(_) => p.clone(),
        _ => return Err(format!("`params` for {command} must be an object")),
    };
    let new_model = command == "file.new";
    let (reg, session) = live.parts();
    if new_model && session.doc.is_modified() {
        return Err(
            "the open model has unsaved changes: save it first (tool.call model_save), \
                    or use tool.call model_new with discard: true"
                .into(),
        );
    }
    let result = reg
        .run(session, command, &args)
        .map_err(|e| e.to_string())?;
    if new_model {
        live.session_replaced();
    }
    Ok(result)
}

fn commands(live: &mut dyn Live) -> Value {
    let (reg, session) = live.parts();
    let list: Vec<Value> = reg
        .specs()
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "label": c.label, "params": c.params,
                "menu": c.menu, "shortcut": c.shortcut, "enabled": (c.enabled)(session),
            })
        })
        .collect();
    Value::Array(list)
}

fn tool(
    live: &mut dyn Live,
    workspace: Option<&Workspace>,
    params: &Value,
) -> Result<Value, String> {
    only(params, &["name", "arguments"])?;
    let name = params["name"]
        .as_str()
        .ok_or("`name` must be an MCP tool name (model_info, command_run, …)")?;
    let (reg, session) = live.parts();
    let mut target = Target::new(reg, session, workspace);
    let result = call_tool(&mut target, name, &params["arguments"]);
    let replaced = target.replaced;
    if replaced {
        live.session_replaced();
    }
    result
}

fn encode(p: Pixels) -> Result<Value, String> {
    let png = render::encode_rgba_png(p.width, p.height, &p.rgba)?;
    Ok(json!({
        "png_base64": base64::engine::general_purpose::STANDARD.encode(png),
        "width": p.width,
        "height": p.height,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_bounded() {
        let mut r = io::Cursor::new(b"short\nthis one is far too long\nlast".to_vec());
        assert_eq!(read_line(&mut r, 8).unwrap().as_deref(), Some("short"));
        assert!(read_line(&mut r, 8).is_err());

        let mut r = io::Cursor::new(b"a\n\nb".to_vec());
        assert_eq!(read_line(&mut r, 8).unwrap().as_deref(), Some("a"));
        assert_eq!(read_line(&mut r, 8).unwrap().as_deref(), Some(""));
        assert_eq!(read_line(&mut r, 8).unwrap().as_deref(), Some("b"));
        assert_eq!(read_line(&mut r, 8).unwrap(), None);
    }

    #[test]
    fn tokens() {
        let t = Token::generate().unwrap();
        assert_eq!(t.as_str().len(), 64);
        assert_ne!(t, Token::generate().unwrap());
        assert!(t.matches(t.as_str()));
        assert!(t.matches(&format!(" {}\n", t.as_str())));
        assert!(!t.matches(&t.as_str()[..63]));
        assert!(!t.matches(""));
        assert!(Token::parse("abc").is_err());
        assert!(Token::parse(&"g".repeat(64)).is_err());
        assert_eq!(format!("{t:?}"), "Token(…)");
    }
}
