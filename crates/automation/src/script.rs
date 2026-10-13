//! Scripts: commands run in order, where a step can use earlier results.
//!
//! ```json
//! [
//!   {"cmd": "diagram.add", "params": {"type": "Class Diagram", "name": "Shop"}},
//!   {"cmd": "node.create", "params": {"kind": "Class", "x": 40, "y": 40, "name": "Cart"}, "as": "cart"},
//!   {"cmd": "node.create", "params": {"kind": "Class", "x": 320, "y": 40, "name": "Item"}, "as": "item"},
//!   {"cmd": "edge.create", "params": {"kind": "Composition", "tail": "$cart.view", "head": "$item.view"}}
//! ]
//! ```
//!
//! A string that is exactly `$name` or `$name.path.to.field` is replaced by
//! that part of the result of the step saved `as` name (`$prev` is the step
//! just before). Path parts that are numbers index into lists. Any other
//! string is left alone; start one with `$$` to mean a literal `$`.

use std::collections::HashMap;

use engine::{Registry, Session};
use serde_json::{Map, Value};

/// Steps run before a script is cut off, so a script can't run forever.
pub const MAX_STEPS: usize = 100_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub cmd: String,
    pub params: Value,
    /// Name the result is saved under, for later `$name` references.
    pub name: Option<String>,
}

impl Step {
    pub fn new(cmd: impl Into<String>, params: Value) -> Self {
        Self {
            cmd: cmd.into(),
            params,
            name: None,
        }
    }
}

/// Read steps from a JSON list, or an object with a `steps` list.
pub fn parse_steps(v: &Value) -> Result<Vec<Step>, String> {
    let list = match v {
        Value::Array(list) => list,
        Value::Object(o) => match o.get("steps") {
            Some(Value::Array(list)) => list,
            _ => return Err("a script is a list of steps, or {\"steps\": [...]}".into()),
        },
        _ => return Err("a script is a list of steps, or {\"steps\": [...]}".into()),
    };
    if list.len() > MAX_STEPS {
        return Err(format!("a script has at most {MAX_STEPS} steps"));
    }
    list.iter()
        .enumerate()
        .map(|(i, s)| parse_step(s).map_err(|e| format!("step {}: {e}", i + 1)))
        .collect()
}

fn parse_step(v: &Value) -> Result<Step, String> {
    let Value::Object(o) = v else {
        return Err(
            "a step is an object like {\"cmd\": \"node.create\", \"params\": {...}}".into(),
        );
    };
    if let Some(k) = o
        .keys()
        .find(|k| !["cmd", "params", "as"].contains(&k.as_str()))
    {
        return Err(format!("unknown key `{k}` (a step has cmd, params, as)"));
    }
    let cmd = match o.get("cmd") {
        Some(Value::String(c)) if !c.is_empty() => c.clone(),
        _ => return Err("`cmd` must be a command id".into()),
    };
    let params = match o.get("params") {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(p @ Value::Object(_)) => p.clone(),
        Some(_) => return Err("`params` must be an object".into()),
    };
    let name = match o.get("as") {
        None | Some(Value::Null) => None,
        Some(Value::String(n)) if is_name(n) && n != "prev" => Some(n.clone()),
        Some(_) => {
            return Err("`as` must be a name of letters, digits and _ (not `prev`)".into());
        }
    };
    Ok(Step { cmd, params, name })
}

fn is_name(n: &str) -> bool {
    let mut chars = n.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Why a script stopped.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    /// 0-based index of the step that failed.
    pub step: usize,
    pub cmd: String,
    pub error: String,
    /// Results of the steps before it.
    pub results: Vec<Value>,
    /// Whether the session was put back as it was before the script.
    pub rolled_back: bool,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "step {} (`{}`): {}", self.step + 1, self.cmd, self.error)?;
        if self.rolled_back {
            write!(f, "; nothing was changed")
        } else {
            write!(f, "; the {} steps before it stay applied", self.step)
        }
    }
}

/// Run `steps` in order and return one result per step.
///
/// With `atomic`, a failing step puts the whole session back as it was
/// before the script (document, undo history, open diagram, selection), and
/// a successful script is one undo step; otherwise each step is its own undo
/// step and the steps before a failure stay applied.
pub fn run(
    reg: &Registry,
    session: &mut Session,
    steps: &[Step],
    atomic: bool,
) -> Result<Vec<Value>, Failure> {
    let saved = atomic.then(|| {
        (
            session.doc.clone(),
            session.active_diagram.clone(),
            session.selection.clone(),
        )
    });
    let depth = session.doc.undo_depth();
    let mut bindings: HashMap<String, Value> = HashMap::new();
    let mut results = Vec::with_capacity(steps.len());
    for (i, step) in steps.iter().enumerate() {
        let outcome = resolve(&step.params, &bindings)
            .and_then(|p| reg.run(session, &step.cmd, &p).map_err(|e| e.to_string()));
        match outcome {
            Ok(result) => {
                if let Some(n) = &step.name {
                    bindings.insert(n.clone(), result.clone());
                }
                bindings.insert("prev".into(), result.clone());
                results.push(result);
            }
            Err(error) => {
                let rolled_back = match saved {
                    Some((doc, active, selection)) => {
                        session.doc = doc;
                        session.active_diagram = active;
                        session.selection = selection;
                        true
                    }
                    None => false,
                };
                return Err(Failure {
                    step: i,
                    cmd: step.cmd.clone(),
                    error,
                    results,
                    rolled_back,
                });
            }
        }
    }
    if atomic {
        session.doc.squash_undo_since(depth);
    }
    Ok(results)
}

/// Replace `$name.path` strings in `v` with the values they point to.
pub fn resolve(v: &Value, bindings: &HashMap<String, Value>) -> Result<Value, String> {
    Ok(match v {
        Value::String(s) => match s.strip_prefix('$') {
            Some(rest) if rest.starts_with('$') => Value::String(rest.to_owned()),
            Some(rest) if is_reference(rest) => lookup(rest, bindings)?,
            _ => v.clone(),
        },
        Value::Array(list) => Value::Array(
            list.iter()
                .map(|x| resolve(x, bindings))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| Ok((k.clone(), resolve(x, bindings)?)))
                .collect::<Result<_, String>>()?,
        ),
        _ => v.clone(),
    })
}

fn is_reference(s: &str) -> bool {
    let mut parts = s.split('.');
    parts.next().is_some_and(is_name)
        && parts.all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

fn lookup(reference: &str, bindings: &HashMap<String, Value>) -> Result<Value, String> {
    let mut parts = reference.split('.');
    let head = parts.next().unwrap_or_default();
    let mut cur = bindings
        .get(head)
        .ok_or_else(|| format!("`${reference}`: no earlier step saved as `{head}`"))?;
    for part in parts {
        let next = match cur {
            Value::Array(list) => part.parse::<usize>().ok().and_then(|i| list.get(i)),
            Value::Object(o) => o.get(part),
            _ => None,
        };
        cur = next.ok_or_else(|| format!("`${reference}`: no `{part}` in {cur}"))?;
    }
    Ok(cur.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn references_resolve_into_nested_params() {
        let mut b = HashMap::new();
        b.insert(
            "cart".to_owned(),
            json!({"view": "V1", "list": [{"id": "X"}]}),
        );
        let p = json!({
            "a": "$cart.view",
            "b": ["$cart.list.0.id"],
            "c": "$$cart",
            "d": "$100 price",
            "e": "plain",
        });
        let r = resolve(&p, &b).unwrap();
        assert_eq!(
            r,
            json!({"a": "V1", "b": ["X"], "c": "$cart", "d": "$100 price", "e": "plain"})
        );
        assert!(resolve(&json!("$missing.view"), &b).is_err());
        assert!(resolve(&json!("$cart.nope"), &b).is_err());
    }

    #[test]
    fn steps_parse_strictly() {
        let ok = parse_steps(&json!([{"cmd": "x", "as": "a_1"}])).unwrap();
        assert_eq!(ok[0].params, json!({}));
        assert_eq!(ok[0].name.as_deref(), Some("a_1"));
        assert!(parse_steps(&json!({"steps": []})).unwrap().is_empty());
        for bad in [
            json!("x"),
            json!([1]),
            json!([{"cmd": ""}]),
            json!([{"cmd": "x", "param": {}}]),
            json!([{"cmd": "x", "params": []}]),
            json!([{"cmd": "x", "as": "1a"}]),
            json!([{"cmd": "x", "as": "prev"}]),
        ] {
            assert!(parse_steps(&bad).is_err(), "{bad}");
        }
    }

    fn shop() -> Vec<Step> {
        parse_steps(&json!([
            {"cmd": "diagram.add", "params": {"type": "Class Diagram", "name": "Shop"}},
            {"cmd": "node.create", "params": {"kind": "Class", "x": 40, "y": 40, "name": "Cart"}, "as": "cart"},
            {"cmd": "node.create", "params": {"kind": "Class", "x": 320, "y": 40, "name": "Item"}, "as": "item"},
            {"cmd": "edge.create", "params": {"kind": "Composition", "tail": "$cart.view", "head": "$item.view"}},
            {"cmd": "feature.add", "params": {"classifier": "$cart.model", "kind": "attribute", "name": "total"}},
            {"cmd": "element.set", "params": {"id": "$prev.id", "values": {"type": "double"}}},
        ]))
        .unwrap()
    }

    #[test]
    fn a_script_builds_a_diagram() {
        let reg = Registry::default();
        let mut s = Session::default();
        let results = run(&reg, &mut s, &shop(), true).unwrap();
        assert_eq!(results.len(), 6);
        let d = reg.run(&mut s, "diagram.inspect", &json!({})).unwrap();
        assert_eq!(d["boxes"].as_array().unwrap().len(), 2);
        assert_eq!(d["lines"][0]["modelType"], "UMLAssociation");
        let total = s.doc.get(results[4]["id"].as_str().unwrap()).unwrap();
        assert_eq!(total.str("type"), Some("double"));
        // The whole script is one undo step.
        assert!(s.doc.undo());
        assert!(!s.doc.can_undo());
    }

    #[test]
    fn a_failing_atomic_script_changes_nothing() {
        let reg = Registry::default();
        let mut s = Session::default();
        let before = s.to_json().unwrap();
        let mut steps = shop();
        steps.push(Step::new(
            "node.create",
            json!({"kind": "NoSuchThing", "x": 0, "y": 0}),
        ));
        let err = run(&reg, &mut s, &steps, true).unwrap_err();
        assert_eq!(err.step, 6);
        assert!(err.rolled_back);
        assert_eq!(err.results.len(), 6);
        assert_eq!(s.to_json().unwrap(), before);
        assert!(!s.doc.can_undo());

        // Not atomic: the steps before the failure stay.
        let err = run(&reg, &mut s, &steps, false).unwrap_err();
        assert!(!err.rolled_back);
        assert!(err.to_string().contains("step 7"));
        assert_ne!(s.to_json().unwrap(), before);
    }
}
