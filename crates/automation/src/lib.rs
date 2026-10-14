//! Driving the engine from outside: scripts, files and agents.
//!
//! Everything here dispatches through [`engine::Registry`], so a script, the
//! CLI and an AI agent over MCP do exactly what the UI does.
//!
//! - [`script`]: a list of command steps whose parameters can refer to the
//!   results of earlier steps (`"$cart.view"`).
//! - [`workspace`]: file access confined to one folder.
//! - [`mcp`]: a Model Context Protocol server (JSON-RPC over stdio lines).
//! - [`control`]: the running app's control channel (loopback TCP, token),
//!   and [`bridge`]: its client, so the MCP server can drive a live window.

pub mod bridge;
pub mod control;
pub mod icons;
pub mod mcp;
pub mod script;
pub mod workspace;

use engine::{Registry, Session};
use serde_json::{Value, json};

/// A diagram's id, given its id or its (unique) name.
pub fn find_diagram(reg: &Registry, s: &mut Session, wanted: &str) -> Result<String, String> {
    let diagrams = reg
        .run(s, "model.diagrams", &json!({}))
        .map_err(|e| e.to_string())?;
    let matches: Vec<&Value> = diagrams
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["id"] == wanted || d["name"] == wanted)
        .collect();
    match matches.as_slice() {
        [d] => Ok(d["id"].as_str().unwrap_or_default().to_owned()),
        [] => Err(format!("no diagram `{wanted}`")),
        _ => Err(format!("several diagrams are named `{wanted}`; use its id")),
    }
}

/// The diagram types (without `diagram_type`), or what `node.create` and
/// `edge.create` accept on one diagram type, by toolbox group.
pub fn toolbox(diagram_type: Option<&str>) -> Result<Value, String> {
    let Some(wanted) = diagram_type else {
        let types: Vec<Value> = uml::DIAGRAMS
            .iter()
            .map(|(t, label, _)| json!({ "type": t, "label": label }))
            .collect();
        return Ok(Value::Array(types));
    };
    let Some(&(ty, label, _)) = uml::DIAGRAMS
        .iter()
        .find(|(t, l, _)| *t == wanted || l.eq_ignore_ascii_case(wanted))
    else {
        return Err(format!("unknown diagram type `{wanted}`"));
    };
    let groups: Vec<Value> = uml::tool_groups(ty)
        .map(|g| {
            let tools: Vec<Value> = g
                .tools
                .iter()
                .filter_map(|id| uml::tool(id))
                .map(|tool| match tool {
                    uml::Tool::Node(n) => json!({ "kind": n.id, "label": n.label, "shape": "box" }),
                    uml::Tool::Edge(e) => {
                        json!({ "kind": e.id, "label": e.label, "shape": "line" })
                    }
                })
                .collect();
            json!({ "group": g.label, "tools": tools })
        })
        .collect();
    Ok(json!({ "type": ty, "label": label, "groups": groups }))
}
