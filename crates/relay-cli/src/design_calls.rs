//! What an agent's `design.apply` does in one call at the MCP door, beyond the op itself: picture
//! what the list changed (`render`), so a change and a look at it are one tool call, not two; and
//! send a list the engine refused again with only its corrections (`fix`, `drop`), so one bad
//! edit of a hundred costs that edit, not the hundred again. Both are this server's, for the
//! session it serves: the engine's `design.apply` and `design.render` are unchanged.

use relay_bus::ops::design::{
    MotionChecks, RENDER_BOXES_MAX, RENDER_FRAMES_MAX, RENDER_FRAMES_MIN, RENDER_NODES_MAX, RENDER_SCALE_MAX, RENDER_SCALE_MIN, RENDER_SIDE_MAX,
    RENDER_SIDE_MIN, RENDER_TIMEOUT_MAX_MS, RENDER_TIMEOUT_MIN_MS,
};
use serde_json::{json, Map, Value};
use std::sync::{Mutex, PoisonError};

/// The last list the engine refused, by design, as it was sent: what `fix` and `drop` correct.
static REFUSED: Mutex<Vec<(i64, Vec<Value>)>> = Mutex::new(Vec::new());

/// The arguments design.apply's tool takes besides the op's own, added to its input schema;
/// `ops` is no longer required, since `fix` stands for it.
pub(crate) fn extend_tool(tool: &mut Value) {
    let motion: Vec<Value> = [MotionChecks::Frames, MotionChecks::End, MotionChecks::Off].iter().map(|m| json!(m)).collect();
    let options = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "page": {"type": "string"},
            "nodes": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": RENDER_NODES_MAX},
            "max_side": {"type": "integer", "minimum": RENDER_SIDE_MIN, "maximum": RENDER_SIDE_MAX},
            "scale": {"type": "number", "minimum": RENDER_SCALE_MIN, "maximum": RENDER_SCALE_MAX},
            "boxes_limit": {"type": "integer", "minimum": 0, "maximum": RENDER_BOXES_MAX},
            "at_ms": {"type": "number", "minimum": 0},
            "frames": {"type": "integer", "minimum": RENDER_FRAMES_MIN, "maximum": RENDER_FRAMES_MAX},
            "motion_checks": {"enum": motion},
            "timeout_ms": {"type": "integer", "minimum": RENDER_TIMEOUT_MIN_MS, "maximum": RENDER_TIMEOUT_MAX_MS},
        },
    });
    if let Some(about) = tool["description"].as_str() {
        tool["description"] = json!(format!("{about}; render: true answers with that picture in the same call, and fix sends a refused list again with only its corrections"));
    }
    let Some(schema) = tool.get_mut("inputSchema") else { return };
    if let Some(props) = schema["properties"].as_object_mut() {
        props.insert("render".into(), json!({
            "description": "Also picture what the list changed, in this same call: design.render's picture and issues of the outermost nodes it added, changed or moved (since_rev: its prev_rev), or of the nodes or page named here. true, or design.render's options; boxes only when boxes_limit is given. Needs the desktop app: without one the edits still apply and the answer says why there is no picture.",
            "anyOf": [{"type": "boolean"}, options],
        }));
        props.insert("fix".into(), json!({
            "description": "In place of ops: send again the list design.apply last refused for this design in this session, each edit given here put in place by its index in that list ({\"63\": {\"op\": \"update\", …}}), the rest as sent. {} sends it again as it was (after design.stale, with a new base_rev). Still all or none.",
            "type": "object",
            "additionalProperties": {"type": "object"},
        }));
        props.insert("drop".into(), json!({
            "description": "With fix: take these edits, by their index in the refused list, out of it.",
            "type": "array",
            "items": {"type": "integer", "minimum": 0},
        }));
    }
    if let Some(required) = schema["required"].as_array_mut() {
        required.retain(|field| field != "ops");
    }
}

/// design.apply's arguments made the op's own: `render` taken out and returned (`None` when not
/// asked for), and with `fix` or `drop`, `ops` rebuilt from the list last refused. The text for
/// the model when they do not fit together.
pub(crate) fn prepare(arguments: &mut Value) -> Result<Option<Map<String, Value>>, String> {
    let Some(args) = arguments.as_object_mut() else { return Ok(None) };
    let render = match args.remove("render") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(Map::new()),
        Some(Value::Object(options)) => Some(options),
        Some(other) => return Err(format!("render: give true or design.render's options, not {other}")),
    };
    // As `unstring` takes them: an object or a list a model sent as the string of its JSON.
    let parsed = |v: Option<Value>| match v {
        Some(Value::String(text)) => serde_json::from_str(&text).ok().or(Some(Value::String(text))),
        other => other,
    };
    let (fix, drop) = (parsed(args.remove("fix")), parsed(args.remove("drop")));
    if fix.is_none() && drop.is_none() {
        if !args.contains_key("ops") {
            return Err("give ops, or fix to correct the list design.apply last refused".into());
        }
        return Ok(render);
    }
    if args.contains_key("ops") {
        return Err("give ops or fix, not both: fix corrects the list design.apply last refused".into());
    }
    let fix = match fix {
        None => Map::new(),
        Some(Value::Object(fix)) => fix,
        Some(other) => return Err(format!("fix: an object of corrected edits by index ({{\"63\": {{…}}}}), not {other}")),
    };
    let drop: Vec<u64> = match drop {
        None => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(|v| v.as_u64().ok_or_else(|| format!("drop: indexes, not {v}"))).collect::<Result<_, _>>()?,
        Some(other) => return Err(format!("drop: a list of indexes, not {other}")),
    };
    let design = args.get("design_id").and_then(Value::as_i64).ok_or("fix: give the design_id of the list refused")?;
    let refused = REFUSED.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(id, _)| *id == design).map(|(_, ops)| ops.clone());
    let Some(refused) = refused else {
        return Err(format!("fix: no list of design {design} was refused in this session; send ops"));
    };
    args.insert("ops".into(), Value::Array(rebuild(&refused, &fix, &drop)?));
    Ok(render)
}

/// `refused` with each edit `fix` names (by its index there) in its place and the ones `drop`
/// names taken out.
fn rebuild(refused: &[Value], fix: &Map<String, Value>, drop: &[u64]) -> Result<Vec<Value>, String> {
    let index = |key: &str| key.parse::<usize>().ok().filter(|at| *at < refused.len());
    let mut ops = refused.to_vec();
    for (key, edit) in fix {
        let at = index(key).ok_or_else(|| format!("fix: {key:?} is not an index of the refused list (0 to {})", refused.len().saturating_sub(1)))?;
        ops[at] = edit.clone();
    }
    if let Some(far) = drop.iter().find(|at| **at as usize >= refused.len()) {
        return Err(format!("drop: {far} is not an index of the refused list (0 to {})", refused.len().saturating_sub(1)));
    }
    let kept: Vec<Value> = ops.into_iter().enumerate().filter(|(at, _)| !drop.contains(&(*at as u64))).map(|(_, op)| op).collect();
    if kept.is_empty() {
        return Err("drop: that takes out every edit; nothing is left to send".into());
    }
    Ok(kept)
}

/// The engine refused the list `arguments` sent: keep it for a `fix`, and say in the refusal
/// how to send only the corrections.
pub(crate) fn refused(arguments: &Value, error: &mut Value) {
    let (Some(design), Some(ops)) = (arguments.get("design_id").and_then(Value::as_i64), arguments.get("ops").and_then(Value::as_array)) else {
        return;
    };
    if ops.len() < 2 {
        return;
    }
    let mut kept = REFUSED.lock().unwrap_or_else(PoisonError::into_inner);
    kept.retain(|(id, _)| *id != design);
    kept.push((design, ops.clone()));
    let at = error.pointer("/details/index").and_then(Value::as_u64).map_or_else(|| "<index>".to_string(), |at| at.to_string());
    let others = ops.len() - 1;
    error["resend"] = json!(format!(
        "Send only the corrections: design.apply {{\"design_id\": {design}, \"fix\": {{\"{at}\": <the edit, corrected>}}}} (or \"drop\": [{at}]); the other {others} edits are kept here and still apply all or none. After design.stale: \"fix\": {{}} with the new base_rev."
    ));
}

/// A list of `design` applied: nothing of it is left to fix.
pub(crate) fn applied(design: Option<i64>) {
    if let Some(design) = design {
        REFUSED.lock().unwrap_or_else(PoisonError::into_inner).retain(|(id, _)| *id != design);
    }
}

/// The design.render payload that pictures what an applied list did: the options asked for, on
/// its design, of what changed after its `prev_rev` unless nodes are named, and without boxes
/// unless a `boxes_limit` is given.
pub(crate) fn render_payload(mut options: Map<String, Value>, applied: &Value) -> Value {
    options.insert("design_id".into(), applied["design_id"].clone());
    if !options.contains_key("nodes") {
        options.insert("since_rev".into(), applied["prev_rev"].clone());
    }
    options.entry("boxes_limit").or_insert(json!(0));
    Value::Object(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edits(n: usize) -> Vec<Value> {
        (0..n).map(|i| json!({"op": "update", "id": format!("n{i}"), "set": {"text": i}})).collect()
    }

    #[test]
    fn render_comes_out_of_the_ops_arguments() {
        let mut args = json!({"design_id": 3, "ops": edits(1), "render": true});
        assert_eq!(prepare(&mut args), Ok(Some(Map::new())));
        assert!(args.get("render").is_none(), "the engine never sees it");
        let mut args = json!({"design_id": 3, "ops": edits(1), "render": {"max_side": 900}});
        assert_eq!(prepare(&mut args).unwrap().unwrap()["max_side"], 900);
        let mut args = json!({"design_id": 3, "ops": edits(1), "render": false});
        assert_eq!(prepare(&mut args), Ok(None));
        assert!(prepare(&mut json!({"design_id": 3})).unwrap_err().starts_with("give ops"));
    }

    #[test]
    fn what_changed_is_pictured_without_boxes() {
        let applied = json!({"design_id": 3, "rev": 9, "prev_rev": 8});
        assert_eq!(render_payload(Map::new(), &applied), json!({"design_id": 3, "since_rev": 8, "boxes_limit": 0}));
        let named = json!({"nodes": ["card"], "boxes_limit": 50}).as_object().unwrap().clone();
        assert_eq!(render_payload(named, &applied), json!({"design_id": 3, "nodes": ["card"], "boxes_limit": 50}));
    }

    #[test]
    fn a_refused_list_is_fixed_by_index() {
        let mut error = json!({"code": "design.node_invalid", "details": {"index": 63}});
        let sent = json!({"design_id": 41, "ops": edits(118)});
        refused(&sent, &mut error);
        assert!(error["resend"].as_str().unwrap().contains(r#""fix": {"63": <the edit, corrected>}"#), "{error}");
        assert!(error["resend"].as_str().unwrap().contains("the other 117 edits"));
        let corrected = json!({"op": "update", "id": "n63", "set": {"text": "ok"}});
        let mut args = json!({"design_id": 41, "fix": {"63": corrected}, "drop": [2], "base_rev": 5});
        assert_eq!(prepare(&mut args), Ok(None));
        let ops = args["ops"].as_array().unwrap();
        assert_eq!(ops.len(), 117);
        assert_eq!(ops[62], corrected, "index 63 of the refused list, one earlier for the drop");
        assert_eq!(ops[2]["id"], "n3");
        assert_eq!(args["base_rev"], 5, "the rest of the payload is as sent");
        // Wrong indexes, both ops and fix, and a design with nothing refused, are said.
        assert!(prepare(&mut json!({"design_id": 41, "fix": {"118": {}}})).unwrap_err().contains("0 to 117"));
        assert!(prepare(&mut json!({"design_id": 41, "fix": {}, "ops": edits(1)})).unwrap_err().starts_with("give ops or fix"));
        assert!(prepare(&mut json!({"design_id": 7, "fix": {}})).unwrap_err().contains("no list of design 7"));
        // Once a list of the design applies, nothing is left to fix.
        applied(Some(41));
        assert!(prepare(&mut json!({"design_id": 41, "fix": {}})).is_err());
    }

    #[test]
    fn the_tool_takes_render_and_fix_and_ops_is_optional() {
        let mut tool = json!({"inputSchema": {"type": "object", "properties": {"design_id": {}, "ops": {}}, "required": ["design_id", "ops"]}});
        extend_tool(&mut tool);
        let props = tool["inputSchema"]["properties"].as_object().unwrap();
        assert!(props.contains_key("render") && props.contains_key("fix") && props.contains_key("drop"));
        assert_eq!(tool["inputSchema"]["required"], json!(["design_id"]));
        assert_eq!(props["render"]["anyOf"][1]["properties"]["motion_checks"]["enum"], json!(["frames", "end", "off"]));
    }
}
