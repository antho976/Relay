//! Renders `schema/bus.v1.json` (BUS.md §0.5): envelope shapes, every op's payload and
//! result, and the shared `$defs`. CI compares the committed file with [`render_pretty`].

use crate::envelope::{Event, Frame, Request, Response};
use crate::error::BusError;
use crate::registry::{OpInfo, Registry};
use schemars::generate::SchemaSettings;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// The full schema document as JSON. It cannot change within a build, so it is generated once
/// per process; `bus.schema {}` then costs a clone rather than a walk of every op.
pub fn render() -> Value {
    static DOC: OnceLock<Value> = OnceLock::new();
    DOC.get_or_init(generate).clone()
}

fn generate() -> Value {
    let settings = SchemaSettings::draft2020_12().with(|s| {
        s.definitions_path = "#/$defs/".into();
        s.inline_subschemas = false;
    });
    let mut g = settings.into_generator();

    let envelope = json!({
        "request": g.subschema_for::<Request>(),
        "response": g.subschema_for::<Response>(),
        "event": g.subschema_for::<Event>(),
        "frame": g.subschema_for::<Frame>(),
        "error": g.subschema_for::<BusError>(),
    });

    let mut ops = Map::new();
    for e in Registry::global().entries() {
        let payload = (e.payload_schema)(&mut g);
        let result = (e.result_schema)(&mut g);
        let mut info = serde_json::to_value(OpInfo::from_entry(e, false)).expect("OpInfo serializes");
        let obj = info.as_object_mut().expect("object");
        obj.remove("name");
        obj.remove("implemented");
        obj.insert("payload".into(), payload.to_value());
        obj.insert("result".into(), result.to_value());
        ops.insert(e.name.to_string(), info);
    }

    let defs: Map<String, Value> = g.take_definitions(true).into_iter().collect();

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "relay://bus/v1",
        "title": "RELAY v4 command bus",
        "description": "Generated from crates/relay-bus by `relay schema`. Do not edit; see docs/engine/BUS.md.",
        "version": crate::envelope::ENVELOPE_V,
        "op_count": ops.len(),
        "envelope": envelope,
        "ops": ops,
        "$defs": defs,
    })
}

/// Pretty JSON with a trailing newline — byte-stable for the CI diff.
pub fn render_pretty() -> String {
    let mut s = serde_json::to_string_pretty(&render()).expect("schema serializes");
    s.push('\n');
    s
}

/// One op's `{payload, result}` schema for `bus.schema {op}`. Each half stands alone (MCP lifts
/// the payload out as a tool's `inputSchema`; see [`standalone`]), and each type is described
/// once: the result writes a type the payload already describes without its descriptions.
pub fn render_op(name: &str) -> Option<Value> {
    let e = Registry::global().get(name)?;
    let (payload, described) = standalone(e.payload_schema, &HashSet::new());
    let (result, _) = standalone(e.result_schema, &described);
    // `implemented` is a property of a *running* engine, not of the schema. Reporting it here
    // contradicted `bus.ops` for every op; the static document simply does not answer it.
    let mut meta = serde_json::to_value(OpInfo::from_entry(e, false)).expect("OpInfo serializes");
    if let Some(object) = meta.as_object_mut() {
        object.remove("implemented");
    }
    Some(json!({ "op": name, "meta": meta, "payload": payload, "result": result }))
}

/// One schema that stands alone, and the names of the types it writes: the type itself at its
/// root (an object schema, as MCP's `inputSchema` must be), each type it names once written in
/// place, and each it names more than once, or that holds itself (`Entry.children`), written once
/// in its own `$defs` and referenced from there. Inlining every type wrote DesignNode out three
/// times in `design.apply` (TM-3). A type in `described` is written without its descriptions.
fn standalone(schema: fn(&mut schemars::SchemaGenerator) -> schemars::Schema, described: &HashSet<String>) -> (Value, HashSet<String>) {
    let settings = SchemaSettings::draft2020_12().with(|s| {
        s.definitions_path = "#/$defs/".into();
        s.inline_subschemas = false;
    });
    let mut g = settings.into_generator();
    let mut root = schema(&mut g).to_value();
    let mut defs: Map<String, Value> = g.take_definitions(true).into_iter().collect();
    for (name, def) in defs.iter_mut().filter(|(name, _)| described.contains(*name)) {
        let had = def.get("description").is_some();
        undescribed(def);
        if let (true, Some(object)) = (had, def.as_object_mut()) {
            object.insert("description".into(), json!(format!("{name}, as the payload describes it")));
        }
    }
    let mut uses: HashMap<&str, usize> = HashMap::new();
    count_refs(&root, &mut uses);
    defs.values().for_each(|def| count_refs(def, &mut uses));
    let kept: HashSet<&str> = defs.keys().map(String::as_str)
        .filter(|name| uses.get(name).copied().unwrap_or(0) > 1 || reaches(name, &defs[*name], &defs, &mut HashSet::new()))
        .collect();
    // A root that is a kept type (one that holds itself) is written out at the root as well.
    if let Some(name) = ref_name(&root).filter(|name| kept.contains(name)) {
        root = with_siblings(defs[name].clone(), &root);
    }
    inline(&mut root, &defs, &kept);
    let mut own = Map::new();
    for (name, def) in &defs {
        if kept.contains(name.as_str()) {
            let mut def = def.clone();
            inline(&mut def, &defs, &kept);
            own.insert(name.clone(), def);
        }
    }
    if let (false, Some(object)) = (own.is_empty(), root.as_object_mut()) {
        object.insert("$defs".into(), Value::Object(own));
    }
    (root, defs.into_iter().map(|(name, _)| name).collect())
}

/// `schema` without its `description`s: in the schemas it holds, never in the values it gives
/// (`const`, `enum`, `default`, `examples`), and a property named `description` stays.
fn undescribed(schema: &mut Value) {
    let Value::Object(map) = schema else { return };
    map.remove("description");
    for (key, value) in map.iter_mut() {
        match (key.as_str(), value) {
            ("properties" | "patternProperties" | "$defs" | "dependentSchemas", Value::Object(named)) => named.values_mut().for_each(undescribed),
            ("anyOf" | "oneOf" | "allOf" | "prefixItems", Value::Array(each)) => each.iter_mut().for_each(undescribed),
            ("items" | "additionalProperties" | "not" | "if" | "then" | "else" | "contains" | "propertyNames"
                | "unevaluatedItems" | "unevaluatedProperties", sub) => undescribed(sub),
            _ => {}
        }
    }
}

/// The definition a `{"$ref": "#/$defs/<name>", …}` schema names.
fn ref_name(schema: &Value) -> Option<&str> {
    schema.get("$ref")?.as_str()?.strip_prefix("#/$defs/")
}

fn count_refs<'a>(schema: &'a Value, uses: &mut HashMap<&'a str, usize>) {
    match schema {
        Value::Object(map) => {
            if let Some(name) = ref_name(schema) {
                *uses.entry(name).or_default() += 1;
            }
            map.values().for_each(|v| count_refs(v, uses));
        }
        Value::Array(items) => items.iter().for_each(|v| count_refs(v, uses)),
        _ => {}
    }
}

/// Whether `schema` names `target`, directly or through the definitions it names.
fn reaches<'a>(target: &str, schema: &'a Value, defs: &'a Map<String, Value>, seen: &mut HashSet<&'a str>) -> bool {
    match schema {
        Value::Object(map) => {
            if let Some(name) = ref_name(schema) {
                if name == target {
                    return true;
                }
                if seen.insert(name) && defs.get(name).is_some_and(|def| reaches(target, def, defs, seen)) {
                    return true;
                }
            }
            map.values().any(|v| reaches(target, v, defs, seen))
        }
        Value::Array(items) => items.iter().any(|v| reaches(target, v, defs, seen)),
        _ => false,
    }
}

/// `def` where `site` referenced it: the keys beside the reference (a field's description, an
/// `x-unit`) laid over the definition's, as an inlined schema has them.
fn with_siblings(def: Value, site: &Value) -> Value {
    let siblings: Vec<(&String, &Value)> = site.as_object().into_iter().flatten().filter(|(key, _)| *key != "$ref").collect();
    match def {
        Value::Object(mut map) => {
            for (key, value) in siblings {
                map.insert(key.clone(), value.clone());
            }
            Value::Object(map)
        }
        other if siblings.is_empty() => other,
        other => {
            let mut map: Map<String, Value> = siblings.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            map.insert("allOf".into(), json!([other]));
            Value::Object(map)
        }
    }
}

/// Every reference in `schema` to a definition not `kept` replaced by the definition itself. A
/// definition not kept is named once and never holds itself, so this ends.
fn inline(schema: &mut Value, defs: &Map<String, Value>, kept: &HashSet<&str>) {
    if let Some(def) = ref_name(schema).filter(|name| !kept.contains(name)).and_then(|name| defs.get(name)) {
        *schema = with_siblings(def.clone(), schema);
    }
    match schema {
        Value::Object(map) => map.values_mut().for_each(|v| inline(v, defs, kept)),
        Value::Array(items) => items.iter_mut().for_each(|v| inline(v, defs, kept)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undescribed_drops_descriptions_and_keeps_fields_and_values() {
        let mut schema = json!({
            "description": "A skill.",
            "type": "object",
            "properties": {
                "description": {"type": "string", "description": "What it is for."},
                "kind": {"oneOf": [{"const": "a", "description": "The first."}], "default": {"description": "kept"}},
                "tags": {"type": "array", "items": {"type": "string", "description": "A tag."}},
            },
        });
        undescribed(&mut schema);
        assert_eq!(schema, json!({
            "type": "object",
            "properties": {
                "description": {"type": "string"},
                "kind": {"oneOf": [{"const": "a"}], "default": {"description": "kept"}},
                "tags": {"type": "array", "items": {"type": "string"}},
            },
        }));
    }

    /// A type named once is written in place; one named twice, or that holds itself, once in `$defs`.
    #[test]
    fn only_types_named_twice_or_holding_themselves_stay_referenced() {
        let defs: Map<String, Value> = serde_json::from_value(json!({
            "Once": {"type": "string"},
            "Twice": {"type": "integer"},
            "Tree": {"type": "object", "properties": {"children": {"type": "array", "items": {"$ref": "#/$defs/Tree"}}}},
        })).unwrap();
        let root = json!({"type": "object", "properties": {
            "a": {"$ref": "#/$defs/Once", "description": "A."}, "b": {"$ref": "#/$defs/Twice"}, "c": {"$ref": "#/$defs/Twice"},
            "t": {"$ref": "#/$defs/Tree"},
        }});
        let mut uses = HashMap::new();
        count_refs(&root, &mut uses);
        defs.values().for_each(|def| count_refs(def, &mut uses));
        assert_eq!((uses["Once"], uses["Twice"], uses["Tree"]), (1, 2, 2));
        assert!(reaches("Tree", &defs["Tree"], &defs, &mut HashSet::new()));
        assert!(!reaches("Once", &defs["Once"], &defs, &mut HashSet::new()));
        let kept: HashSet<&str> = ["Twice", "Tree"].into();
        let mut inlined = root.clone();
        inline(&mut inlined, &defs, &kept);
        assert_eq!(inlined["properties"]["a"], json!({"type": "string", "description": "A."}));
        assert_eq!(inlined["properties"]["b"], json!({"$ref": "#/$defs/Twice"}));
    }
}
