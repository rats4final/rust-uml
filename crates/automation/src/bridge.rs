//! The client side of the control channel ([`crate::control`]), used by
//! `rust-uml-cli mcp --bridge` to drive a running window.
//!
//! It keeps one authenticated connection and reconnects when it breaks. An
//! edit is never sent twice: if the connection fails after sending, the error
//! says the edit may have been applied. Reads are retried once.

use std::io::BufReader;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::{Value, json};

use crate::control::{MAX_LINE, Token, read_line, send};

/// How long to wait for a reply: under the app's own 60 s, so the app's
/// clearer timeout message is the one callers see.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(55);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct BridgeClient {
    addr: SocketAddr,
    token: Token,
    timeout: Duration,
    conn: Option<Conn>,
    next_id: u64,
}

struct Conn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

enum CallError {
    /// Nothing reached the app.
    NotSent(String),
    /// The connection broke after the request was sent.
    Lost(String),
    /// The app answered with an error.
    App(String),
}

impl BridgeClient {
    /// A client for the app at `addr`, which must be on this computer.
    pub fn new(addr: SocketAddr, token: Token) -> Result<Self, String> {
        if !addr.ip().is_loopback() {
            return Err(format!(
                "{addr}: the control channel only listens on this computer; use 127.0.0.1:<port>"
            ));
        }
        Ok(Self {
            addr,
            token,
            timeout: CALL_TIMEOUT,
            conn: None,
            next_id: 1,
        })
    }

    /// Wait at most `timeout` for each reply.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Run a control method. `read_only` calls are retried once on a fresh
    /// connection if the old one broke; edits never are.
    pub fn call(&mut self, method: &str, params: &Value, read_only: bool) -> Result<Value, String> {
        let first = self.try_call(method, params);
        let result = match first {
            Err(CallError::Lost(_)) if read_only => self.try_call(method, params),
            other => other,
        };
        result.map_err(|e| match e {
            CallError::NotSent(e) => format!(
                "could not reach rust-uml at {}: {e}. The request was not sent. Is the app \
                 running with --control {}?",
                self.addr,
                self.addr.port()
            ),
            CallError::Lost(e) => format!(
                "lost the connection to rust-uml ({e}). The request may have been applied: \
                 check (ui_inspect, diagram_inspect) before trying again"
            ),
            CallError::App(e) => e,
        })
    }

    fn try_call(&mut self, method: &str, params: &Value) -> Result<Value, CallError> {
        if self.conn.is_none() {
            self.conn = Some(self.connect()?);
        }
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({ "id": id, "method": method, "params": params });
        let outcome = match &mut self.conn {
            Some(conn) => exchange(conn, &request, id),
            None => Err(CallError::NotSent("not connected".into())),
        };
        if matches!(outcome, Err(CallError::Lost(_) | CallError::NotSent(_))) {
            self.conn = None;
        }
        outcome
    }

    fn connect(&mut self) -> Result<Conn, CallError> {
        let not_sent = |e: std::io::Error| CallError::NotSent(e.to_string());
        let stream = TcpStream::connect_timeout(&self.addr, CONNECT_TIMEOUT).map_err(not_sent)?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(not_sent)?;
        let writer = stream.try_clone().map_err(not_sent)?;
        let mut conn = Conn {
            reader: BufReader::new(stream),
            writer,
        };
        let auth = json!({ "id": 0, "method": "auth", "params": { "token": self.token.as_str() } });
        match exchange(&mut conn, &auth, 0) {
            Ok(_) => Ok(conn),
            Err(CallError::App(e)) => Err(CallError::NotSent(format!(
                "the app refused the token ({e}); pass the same --control-token-file as the app"
            ))),
            Err(CallError::Lost(e) | CallError::NotSent(e)) => Err(CallError::NotSent(e)),
        }
    }
}

/// Send one request and read lines until the reply with its id.
fn exchange(conn: &mut Conn, request: &Value, id: u64) -> Result<Value, CallError> {
    send(&mut conn.writer, request).map_err(|e| CallError::Lost(e.to_string()))?;
    loop {
        let line = match read_line(&mut conn.reader, MAX_LINE) {
            Ok(Some(line)) => line,
            Ok(None) => return Err(CallError::Lost("the app closed the connection".into())),
            Err(e) => return Err(CallError::Lost(e.to_string())),
        };
        let Ok(reply) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if reply["id"] != json!(id) {
            continue;
        }
        return if reply["ok"] == json!(true) {
            Ok(reply["result"].clone())
        } else {
            Err(CallError::App(
                reply["error"].as_str().unwrap_or("error").to_owned(),
            ))
        };
    }
}
