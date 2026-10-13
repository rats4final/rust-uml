//! An MCP conversation, as an agent would have it.

// Test code: failing loudly is the point.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use automation::mcp::Server;
use automation::workspace::Workspace;
use serde_json::{Value, json};

fn temp_dir() -> std::path::PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("rust-uml-mcp-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Client {
    server: Server,
    next: i64,
}

impl Client {
    fn new(server: Server) -> Self {
        Self { server, next: 0 }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let line = json!({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params});
        let reply = self.server.handle_line(&line.to_string()).expect("a reply");
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["id"], self.next);
        reply
    }

    /// Call a tool; returns (text or image content, isError).
    fn call(&mut self, tool: &str, args: Value) -> (Value, bool) {
        let r = self.request("tools/call", json!({"name": tool, "arguments": args}));
        let result = &r["result"];
        let first = &result["content"][0];
        let content = match first["type"].as_str() {
            Some("text") => serde_json::from_str(first["text"].as_str().unwrap())
                .unwrap_or_else(|_| first["text"].clone()),
            _ => first.clone(),
        };
        (content, result["isError"].as_bool().unwrap())
    }

    fn ok(&mut self, tool: &str, args: Value) -> Value {
        let (v, err) = self.call(tool, args);
        assert!(!err, "{tool} failed: {v}");
        v
    }
}

#[test]
fn handshake_and_tool_list() {
    let mut c = Client::new(Server::new(None));
    let init = c.request(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "rust-uml");
    assert!(
        init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("node.create")
    );

    // An unknown version gets our newest one.
    let init = c.request("initialize", json!({"protocolVersion": "1999-01-01"}));
    assert_eq!(
        init["result"]["protocolVersion"],
        automation::mcp::PROTOCOL_VERSIONS[0]
    );

    // Notifications get no reply.
    assert!(
        c.server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none()
    );
    assert!(c.server.handle_line("   ").is_none());

    let tools = c.request("tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for wanted in [
        "model_info",
        "command_run",
        "command_batch",
        "render_preview",
        "diagram_inspect",
    ] {
        assert!(names.contains(&wanted), "{wanted} missing from {names:?}");
    }
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        assert!(t["annotations"]["readOnlyHint"].is_boolean());
    }
}

#[test]
fn protocol_errors() {
    let mut s = Server::new(None);
    let r: Value = serde_json::from_str(&s.handle_line("{nope").unwrap()).unwrap();
    assert_eq!(r["error"]["code"], -32700);
    assert_eq!(r["id"], Value::Null);
    let r: Value = serde_json::from_str(
        &s.handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"nope"}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(r["error"]["code"], -32601);
    let r: Value = serde_json::from_str(
        &s.handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nope"}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(r["error"]["code"], -32602);
    // Batches answer each request.
    let r: Value = serde_json::from_str(
        &s.handle_line(
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"x"}]"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(r.as_array().unwrap().len(), 1);
}

#[test]
fn an_agent_builds_looks_at_and_saves_a_diagram() {
    let dir = temp_dir();
    let mut c = Client::new(Server::new(Some(Workspace::new(&dir).unwrap())));

    let toolbox = c.ok("toolbox", json!({"diagram_type": "Class Diagram"}));
    assert!(toolbox["groups"].to_string().contains("UMLClass"));

    let built = c.ok(
        "command_batch",
        json!({"steps": [
            {"cmd": "diagram.add", "params": {"type": "UMLClassDiagram", "name": "Shop"}},
            {"cmd": "node.create", "params": {"kind": "Class", "x": 40, "y": 40, "name": "Cart"}, "as": "cart"},
            {"cmd": "node.create", "params": {"kind": "Class", "x": 320, "y": 40, "name": "Item"}, "as": "item"},
            {"cmd": "edge.create", "params": {"kind": "Composition", "tail": "$cart.view", "head": "$item.view"}},
        ]}),
    );
    assert_eq!(built["results"].as_array().unwrap().len(), 4);

    let seen = c.ok("diagram_inspect", json!({"diagram": "Shop"}));
    assert_eq!(seen["boxes"].as_array().unwrap().len(), 2);
    assert_eq!(seen["lines"].as_array().unwrap().len(), 1);

    let (image, err) = c.call("render_preview", json!({"max_side": 256}));
    assert!(!err);
    assert_eq!(image["type"], "image");
    assert_eq!(image["mimeType"], "image/png");
    assert!(image["data"].as_str().unwrap().len() > 100);

    // Unknown arguments and bad steps are tool errors the agent can read.
    let (msg, err) = c.call("render_preview", json!({"size": 10}));
    assert!(err && msg.as_str().unwrap().contains("unknown argument `size`"));
    let (msg, err) = c.call(
        "command_batch",
        json!({"steps": [{"cmd": "node.create", "params": {"kind": "Class", "x": 0, "y": 0}}, {"cmd": "nope"}]}),
    );
    assert!(err && msg.as_str().unwrap().contains("nothing was changed"));
    assert_eq!(
        c.ok("diagram_inspect", json!({}))["boxes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // Mermaid in, a laid-out diagram out.
    let imported = c.ok(
        "mermaid_import",
        json!({"text": "classDiagram\nAnimal <|-- Duck\nAnimal : +int age", "name": "Zoo"}),
    );
    assert!(imported["warnings"].as_array().unwrap().is_empty());
    let zoo = c.ok("diagram_inspect", json!({"diagram": "Zoo"}));
    assert_eq!(zoo["boxes"].as_array().unwrap().len(), 2);
    let (msg, err) = c.call("mermaid_import", json!({"text": "pie\n\"a\": 1"}));
    assert!(err && msg.as_str().unwrap().contains("Mermaid"));

    // Unsaved work is protected.
    let (msg, err) = c.call("model_new", json!({}));
    assert!(err && msg.as_str().unwrap().contains("unsaved"));

    c.ok("model_save", json!({"path": "shop.mdj"}));
    c.ok(
        "diagram_export",
        json!({"path": "shop.svg", "diagram": "Shop"}),
    );
    assert!(
        std::fs::read_to_string(dir.join("shop.svg"))
            .unwrap()
            .contains("Cart")
    );
    let (msg, err) = c.call("model_save", json!({"path": "../shop.mdj"}));
    assert!(err && msg.as_str().unwrap().contains(".."));

    c.ok("model_new", json!({}));
    let info = c.ok("model_open", json!({"path": "shop.mdj"}));
    assert_eq!(info["unsavedChanges"], false);
    assert!(info["diagrams"].to_string().contains("Shop"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn no_workspace_means_no_files() {
    let mut c = Client::new(Server::new(None));
    let (msg, err) = c.call("model_open", json!({"path": "x.mdj"}));
    assert!(err && msg.as_str().unwrap().contains("--root"));
}
