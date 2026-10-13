//! The control channel and the MCP bridge, over real loopback sockets, with
//! a stand-in for the window (same registry and session, no egui).

// Test code: failing loudly is the point.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use automation::bridge::BridgeClient;
use automation::control::{self, Live, Options, Pixels, Request, ScreenshotDone, Token};
use automation::mcp::Server;
use automation::workspace::Workspace;
use base64::Engine as _;
use engine::{Registry, Session};
use serde_json::{Value, json};

fn temp_dir() -> std::path::PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Clocks may tick in microseconds (macOS): tests started together
    // would share a folder without the counter.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("rust-uml-control-{}-{n}-{k}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The window, minus the window.
#[derive(Default)]
struct FakeApp {
    reg: Registry,
    session: Session,
    replaced: usize,
}

impl Live for FakeApp {
    fn parts(&mut self) -> (&Registry, &mut Session) {
        (&self.reg, &mut self.session)
    }
    fn session_replaced(&mut self) {
        self.replaced += 1;
    }
    fn inspect(&mut self) -> Value {
        json!({ "file": self.session.file_name, "activeDiagram": self.session.active_diagram })
    }
    fn screenshot(&mut self, done: ScreenshotDone) {
        done(Ok(Pixels {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
        }));
    }
}

struct Running {
    addr: SocketAddr,
    token: Token,
    app: Arc<Mutex<FakeApp>>,
}

/// Serve the control channel with a "UI thread" that runs every request.
fn start(workspace: Option<Workspace>) -> Running {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let token = Token::generate().unwrap();
    let app = Arc::new(Mutex::new(FakeApp::default()));
    let (tx, rx) = mpsc::channel::<Request>();
    control::serve(listener, token.clone(), Options::default(), move |req| {
        tx.send(req).unwrap();
    })
    .unwrap();
    let ui = app.clone();
    std::thread::spawn(move || {
        for req in rx {
            control::dispatch(&mut *ui.lock().unwrap(), workspace.as_ref(), req);
        }
    });
    Running { addr, token, app }
}

/// A connection speaking the protocol by hand.
struct Raw {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Raw {
    fn connect(addr: SocketAddr) -> Self {
        let s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        Self {
            writer: s.try_clone().unwrap(),
            reader: BufReader::new(s),
        }
    }

    fn line(&mut self, text: &str) -> Option<Value> {
        writeln!(self.writer, "{text}").unwrap();
        self.read()
    }

    fn read(&mut self) -> Option<Value> {
        let mut buf = String::new();
        match self.reader.read_line(&mut buf) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(serde_json::from_str(&buf).unwrap()),
        }
    }

    fn send(&mut self, id: i64, method: &str, params: Value) -> Value {
        let reply = self
            .line(&json!({"id": id, "method": method, "params": params}).to_string())
            .expect("a reply");
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    fn ok(&mut self, method: &str, params: Value) -> Value {
        let r = self.send(7, method, params);
        assert_eq!(r["ok"], true, "{method}: {r}");
        r["result"].clone()
    }

    fn err(&mut self, method: &str, params: Value) -> String {
        let r = self.send(7, method, params);
        assert_eq!(r["ok"], false, "{method} should fail: {r}");
        r["error"].as_str().unwrap().to_owned()
    }

    fn authed(r: &Running) -> Self {
        let mut c = Self::connect(r.addr);
        let hello = c.ok("auth", json!({ "token": r.token.as_str() }));
        assert_eq!(hello["app"], "rust-uml");
        c
    }
}

/// The text of a tool result, parsed as JSON when it is.
fn tool_text(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    serde_json::from_str(text).unwrap_or_else(|_| json!(text))
}

#[test]
fn nothing_runs_before_authentication() {
    let r = start(None);

    let mut c = Raw::connect(r.addr);
    let e = c.err("engine.execute", json!({"command": "file.new"}));
    assert!(e.contains("authenticate first"), "{e}");
    assert!(c.read().is_none(), "the connection should be closed");

    let mut c = Raw::connect(r.addr);
    let e = c.err("auth", json!({"token": "0".repeat(64)}));
    assert_eq!(e, "wrong token");
    assert!(c.read().is_none());

    let mut c = Raw::connect(r.addr);
    let reply = c.line("GET / HTTP/1.1").unwrap();
    assert_eq!(reply["ok"], false);
    assert!(c.read().is_none());

    let mut c = Raw::connect(r.addr);
    assert!(c.err("auth", json!({})).contains("wrong token"));

    assert!(!r.app.lock().unwrap().session.doc.is_modified());
}

#[test]
fn commands_and_tools_run_on_the_open_model() {
    let r = start(None);
    let mut c = Raw::authed(&r);

    assert_eq!(c.ok("ping", json!({})), json!({}));
    let e = c.err("auth", json!({"token": r.token.as_str()}));
    assert_eq!(e, "already authenticated");

    let d = c.ok(
        "engine.execute",
        json!({"command": "diagram.add", "params": {"type": "UMLClassDiagram", "name": "Shop"}}),
    );
    let made = c.ok(
        "engine.execute",
        json!({"command": "node.create", "params": {"kind": "Class", "x": 40, "y": 40, "name": "Cart"}}),
    );
    assert!(made["view"].is_string(), "{made}");
    {
        let app = r.app.lock().unwrap();
        assert!(app.session.doc.is_modified());
        assert!(app.session.active_diagram.is_some(), "{d}");
    }

    let list = c.ok("engine.commands", json!({}));
    let undo = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "edit.undo")
        .unwrap();
    assert_eq!(undo["enabled"], true);

    let info = c.ok("tool.call", json!({"name": "model_info"}));
    assert_eq!(info["isError"], false);
    let diagrams = tool_text(&info)["diagrams"].clone();
    assert!(
        diagrams
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["name"] == "Shop"),
        "{diagrams}"
    );
    let bad = c.ok(
        "tool.call",
        json!({"name": "command_run", "arguments": {"id": "nope"}}),
    );
    assert_eq!(bad["isError"], true);
    assert!(
        c.err("tool.call", json!({"name": "ui_inspect"}))
            .contains("unknown tool")
    );

    let seen = c.ok("ui.inspect", json!({}));
    assert!(seen["activeDiagram"].is_string());
    let shot = c.ok("ui.screenshot", json!({}));
    assert_eq!(
        (shot["width"].as_u64(), shot["height"].as_u64()),
        (Some(2), Some(1))
    );
    let png = base64::engine::general_purpose::STANDARD
        .decode(shot["png_base64"].as_str().unwrap())
        .unwrap();
    assert_eq!(&png[1..4], b"PNG");

    // Throwing away the user's work needs saying so.
    let e = c.err("engine.execute", json!({"command": "file.new"}));
    assert!(e.contains("unsaved changes"), "{e}");
    let fresh = c.ok(
        "tool.call",
        json!({"name": "model_new", "arguments": {"discard": true}}),
    );
    assert_eq!(fresh["isError"], false, "{fresh}");
    {
        let app = r.app.lock().unwrap();
        assert_eq!(app.replaced, 1);
        assert!(!app.session.doc.is_modified());
    }
    c.ok("engine.execute", json!({"command": "file.new"}));
    assert_eq!(r.app.lock().unwrap().replaced, 2);

    // Malformed requests are answered, and the connection keeps serving.
    assert!(c.err("nope", json!({})).contains("unknown method"));
    assert!(
        c.err("ui.inspect", json!({"x": 1}))
            .contains("unknown parameter")
    );
    assert!(c.err("engine.execute", json!({})).contains("`command`"));
    assert!(
        c.err(
            "engine.execute",
            json!({"command": "file.new", "params": [1]})
        )
        .contains("must be an object")
    );
    let r2 = c
        .line(r#"{"id": 3, "method": "ping", "params": [1]}"#)
        .unwrap();
    assert_eq!(
        (r2["id"].as_i64(), r2["ok"].as_bool()),
        (Some(3), Some(false))
    );
    assert_eq!(c.line("not json").unwrap()["ok"], false);
    c.ok("ping", json!({}));
}

#[test]
fn agent_files_stay_in_the_control_root() {
    let dir = temp_dir();
    let r = start(Some(Workspace::new(&dir).unwrap()));
    let mut c = Raw::authed(&r);

    let saved = c.ok(
        "tool.call",
        json!({"name": "model_save", "arguments": {"path": "a.mdj"}}),
    );
    assert_eq!(saved["isError"], false, "{saved}");
    assert!(dir.join("a.mdj").is_file());
    let outside = std::env::temp_dir().join("rust-uml-escape.mdj");
    let refused = c.ok(
        "tool.call",
        json!({"name": "model_save", "arguments": {"path": outside.to_str().unwrap()}}),
    );
    assert_eq!(refused["isError"], true);
    assert!(!outside.exists());

    let opened = c.ok(
        "tool.call",
        json!({"name": "model_open", "arguments": {"path": "a.mdj"}}),
    );
    assert_eq!(opened["isError"], false, "{opened}");
    assert_eq!(r.app.lock().unwrap().replaced, 1);
    std::fs::remove_dir_all(dir).unwrap();

    let r = start(None);
    let mut c = Raw::authed(&r);
    let no = c.ok(
        "tool.call",
        json!({"name": "model_save", "arguments": {"path": "a.mdj"}}),
    );
    assert_eq!(no["isError"], true);
    assert!(tool_text(&no).as_str().unwrap().contains("--control-root"));
}

#[test]
fn timeouts_say_whether_the_request_ran() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let token = Token::generate().unwrap();
    // A window that is not drawing: requests pile up. The first is then
    // claimed (started) but never answered.
    let parked: Arc<Mutex<Vec<Request>>> = Arc::default();
    let p = parked.clone();
    let options = Options {
        reply_timeout: Duration::from_millis(200),
    };
    control::serve(listener, token.clone(), options, move |req| {
        p.lock().unwrap().push(req);
    })
    .unwrap();
    let r = Running {
        addr,
        token,
        app: Arc::default(),
    };
    let mut c = Raw::authed(&r);

    let e = c.err("ping", json!({}));
    assert!(e.starts_with("timeout: not run"), "{e}");
    let first = parked.lock().unwrap().remove(0);
    assert!(
        !first.start(),
        "a request reported as not run must never run"
    );

    std::thread::spawn({
        let parked = parked.clone();
        move || {
            loop {
                if let Some(req) = parked.lock().unwrap().pop() {
                    assert!(req.start());
                    std::mem::forget(req);
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    });
    let e = c.err("ping", json!({}));
    assert!(e.starts_with("timeout: no reply"), "{e}");
}

#[test]
fn token_files() {
    let dir = temp_dir();
    let path = dir.join("token");
    let t = Token::load_or_create(&path).unwrap();
    assert_eq!(Token::load_or_create(&path).unwrap(), t);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let e = Token::load_or_create(&path).unwrap_err();
        assert!(e.contains("other users"), "{e}");
    }
    let junk = dir.join("junk");
    std::fs::write(&junk, "hello").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&junk, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(Token::load_or_create(&junk).is_err());
    assert!(Token::load_or_create(&dir.join("no/such/dir/token")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

/// An MCP client talking to `rust-uml-cli mcp --bridge`.
fn mcp(server: &mut Server, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    serde_json::from_str(&server.handle_line(&line.to_string()).unwrap()).unwrap()
}

#[test]
fn the_mcp_bridge_drives_the_window() {
    let r = start(None);
    let client = BridgeClient::new(r.addr, r.token.clone()).unwrap();
    let mut s = Server::bridge(client);

    let init = mcp(
        &mut s,
        "initialize",
        json!({"protocolVersion": "2025-06-18"}),
    );
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("running rust-uml window"));
    let tools = mcp(&mut s, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in ["ui_inspect", "ui_screenshot", "command_run", "model_save"] {
        assert!(names.contains(&n), "{n} missing from {names:?}");
    }

    let made = mcp(
        &mut s,
        "tools/call",
        json!({"name": "command_run", "arguments": {"id": "diagram.add", "params": {"type": "UMLClassDiagram"}}}),
    );
    assert_eq!(made["result"]["isError"], false, "{made}");
    assert!(r.app.lock().unwrap().session.active_diagram.is_some());

    let seen = mcp(&mut s, "tools/call", json!({"name": "ui_inspect"}));
    assert!(tool_text(&seen["result"])["activeDiagram"].is_string());
    let shot = mcp(&mut s, "tools/call", json!({"name": "ui_screenshot"}));
    let image = &shot["result"]["content"][0];
    assert_eq!(image["type"], "image");
    assert_eq!(image["mimeType"], "image/png");
    let extra = mcp(
        &mut s,
        "tools/call",
        json!({"name": "ui_screenshot", "arguments": {"x": 1}}),
    );
    assert_eq!(extra["result"]["isError"], true);
    let unknown = mcp(&mut s, "tools/call", json!({"name": "nope"}));
    assert!(
        unknown["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown tool")
    );

    // The local server doesn't offer the window's tools.
    let local = mcp(&mut Server::new(None), "tools/list", json!({}));
    assert!(!local.to_string().contains("ui_screenshot"));
}

#[test]
fn bridge_errors_say_what_happened() {
    let r = start(None);
    let wrong = BridgeClient::new(r.addr, Token::generate().unwrap()).unwrap();
    let mut s = Server::bridge(wrong);
    let reply = mcp(&mut s, "tools/call", json!({"name": "model_info"}));
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("refused the token") && text.contains("not sent"),
        "{text}"
    );

    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = closed.local_addr().unwrap();
    drop(closed);
    let mut c = BridgeClient::new(addr, Token::generate().unwrap()).unwrap();
    let e = c.call("ping", &json!({}), true).unwrap_err();
    assert!(e.contains("was not sent"), "{e}");

    assert!(
        BridgeClient::new("10.0.0.1:7878".parse().unwrap(), Token::generate().unwrap()).is_err()
    );
}

#[test]
fn an_edit_is_never_sent_twice() {
    // An app that authenticates, takes one request, and dies.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let token = Token::generate().unwrap();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut w = stream;
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            writeln!(w, r#"{{"id": 0, "ok": true, "result": {{}}}}"#).unwrap();
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) > 0 {
                log.lock().unwrap().push(line.clone());
            }
        }
    });
    let mut c = BridgeClient::new(addr, token).unwrap();
    let e = c
        .call("engine.execute", &json!({"command": "file.new"}), false)
        .unwrap_err();
    assert!(e.contains("may have been applied"), "{e}");
    assert_eq!(seen.lock().unwrap().len(), 1, "the edit was resent");

    // A read is retried once on a fresh connection.
    let e = c.call("ui.inspect", &json!({}), true).unwrap_err();
    assert!(e.contains("may have been applied"), "{e}");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(seen.lock().unwrap().len(), 3);
}
