//! Amounts as a model reads and writes them (docs/THREADS.md, "Amounts for models").
//!
//! The ledger counts money in minor units and investment quantities and prices at 1e-8, and the
//! bus says so in each field's schema (`x-unit`: `minor`, `minor:<currency field>` or `e8`).
//! People's apps read those integers; a model misreads them (15799 cents read as $15,799), so
//! the MCP door turns every such field into a decimal string in the currency, "157.99", the
//! way Arbiter already gives its amounts, and reads the same back into integers.
//!
//! `minor` is in the reading's own currency (its top-level `currency`, else the ledger's);
//! `minor:<field>` in the currency the sibling `<field>` names, else the same.
//!
//! The rest of what a model would misread is reshaped too, in results only:
//! - basis points (`bps`, 1250) become a percent ("12.50") under `_pct`;
//! - fractions (`fraction`, 0.68) become a percent ("68") under `_pct`;
//! - Unix seconds (`unix`) become a UTC time ("2025-10-11T00:00:00Z");
//! - what only an app draws with (`omit`: palette colours, icons, sync ids) is left out.
//!
//! `omit` also hides a payload field only the person's clients send: the tool schema a model
//! writes from leaves it out ([`agent_schema`]), so it never learns of it, and an answer leaves
//! out what it would only send back (`design.apply`'s `expect`, which goes with an undo).

use relay_money::csv::major_string;
use serde_json::{Map, Value};

/// Decimal places of `code`, read the way the ledger reads a code ("jpy" is JPY); two for a code
/// the ledger does not know.
pub fn digits(code: &str) -> u32 {
    relay_money::money::fraction_digits(&code.trim().to_ascii_uppercase()).unwrap_or(2)
}

/// Whether anything in `schema` carries a unit.
pub fn has_units(schema: &Value) -> bool {
    match schema {
        Value::Object(map) => map.contains_key("x-unit") || map.values().any(has_units),
        Value::Array(items) => items.iter().any(has_units),
        _ => false,
    }
}

/// `schema` as a model is shown it: every field with a unit is a decimal string or a time,
/// renamed or left out as [`to_agent`] does to the values.
pub fn agent_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = Map::new();
            let mut gone: Vec<(String, Option<String>)> = Vec::new();
            for (key, value) in map {
                if key == "properties" {
                    if let Value::Object(props) = value {
                        let mut kept = Map::new();
                        for (name, prop) in props {
                            let renamed = agent_key(name, prop);
                            if renamed.as_deref() != Some(name.as_str()) {
                                gone.push((name.clone(), renamed.clone()));
                            }
                            if let Some(renamed) = renamed {
                                kept.insert(renamed, agent_schema(prop));
                            }
                        }
                        out.insert(key.clone(), Value::Object(kept));
                        continue;
                    }
                }
                out.insert(key.clone(), agent_schema(value));
            }
            if let Some(Value::Array(required)) = out.get_mut("required") {
                required.retain_mut(|r| match gone.iter().find(|(old, _)| r == old) {
                    Some((_, Some(new))) => {
                        *r = Value::String(new.clone());
                        true
                    }
                    Some((_, None)) => false,
                    None => true,
                });
            }
            out.remove("x-agent-name");
            let Some(unit) = out.remove("x-unit").and_then(|u| u.as_str().map(str::to_string)) else {
                return Value::Object(out);
            };
            let types = out.get("type").cloned().unwrap_or(Value::Null);
            let is = |t: &str| types == t || types.as_array().is_some_and(|a| a.iter().any(|x| x == t));
            if is("array") {
                out.insert("items".into(), decimal_schema(&unit, false, None));
                return Value::Object(out);
            }
            decimal_schema(&unit, is("null"), out.get("description").and_then(Value::as_str))
        }
        Value::Array(items) => Value::Array(items.iter().map(agent_schema).collect()),
        other => other.clone(),
    }
}

/// The name a model sees for `key`, given its schema; `None` when it is left out. A field's own
/// `x-agent-name` says it plainly where the bus's name would be misread (`pace_delta` read as
/// "under pace").
fn agent_key(key: &str, schema: &Value) -> Option<String> {
    let unit = schema.get("x-unit").and_then(Value::as_str);
    if let Some(name) = schema.get("x-agent-name").and_then(Value::as_str).filter(|_| unit != Some("omit")) {
        return Some(name.to_string());
    }
    match unit {
        Some("omit") => None,
        Some("bps") => Some(format!("{}_pct", key.strip_suffix("_bps").unwrap_or(key))),
        Some("fraction") => Some(format!("{}_pct", key.strip_suffix("_fraction").unwrap_or(key))),
        _ => Some(key.to_string()),
    }
}

fn decimal_schema(unit: &str, nullable: bool, about: Option<&str>) -> Value {
    let what = match unit {
        "e8" => "A decimal number as a string, e.g. \"1.5\"".to_string(),
        "bps" | "fraction" => "A percentage as a decimal string, e.g. \"12.5\" for 12.5%".to_string(),
        "unix" => "A UTC time, e.g. \"2025-10-11T00:00:00Z\"".to_string(),
        u => match u.strip_prefix("minor:") {
            Some(field) => format!("A decimal amount in `{field}` as a string, e.g. \"42.18\" (not cents)"),
            None => "A decimal amount in the currency as a string, e.g. \"42.18\" (not cents)".to_string(),
        },
    };
    // What the field means stays; how it was counted does not.
    let about = about.filter(|a| !a.contains("inor units") && !a.contains("1e-8") && !a.contains("Unix seconds") && !a.contains("fraction"));
    let description = about.map_or(what.clone(), |a| format!("{a} {what}."));
    let kind = if nullable { serde_json::json!(["string", "null"]) } else { serde_json::json!("string") };
    serde_json::json!({"type": kind, "description": description})
}

/// Turn every integer with a unit in `value` into a decimal string. `ledger` is the ledger's
/// decimal places, for a reading that does not name its currency.
pub fn to_agent(value: &mut Value, schema: &Value, ledger: u32) {
    let root = value.get("currency").and_then(Value::as_str).map_or(ledger, digits);
    walk(value, schema, schema, root, None, true, &mut |unit, v, places| {
        let shown = match (unit, &*v) {
            ("e8", Value::Number(n)) => n.as_i64().map(|n| trimmed(major_string(n, 8))),
            ("bps", Value::Number(n)) => n.as_i64().map(|n| major_string(n, 2)),
            ("fraction", Value::Number(n)) => n.as_f64().map(|f| trimmed(format!("{:.1}", f * 100.0))),
            ("unix", Value::Number(n)) => n.as_i64().and_then(|n| jiff::Timestamp::from_second(n).ok()).map(|t| t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()),
            (u, Value::Number(n)) if u.starts_with("minor") => n.as_i64().map(|n| major_string(n, places)),
            _ => None,
        };
        if let Some(shown) = shown {
            *v = Value::String(shown);
        }
        Ok(())
    })
    .expect("formatting never fails");
}

/// Turn every decimal with a unit in `value` (a string, or a number) into the integer the bus
/// takes. Refuses one with more decimal places than its unit has.
pub fn to_engine(value: &mut Value, schema: &Value, ledger: u32) -> Result<(), String> {
    let root = value.get("currency").and_then(Value::as_str).map_or(ledger, digits);
    walk(value, schema, schema, root, None, false, &mut |unit, v, places| {
        let places = match unit {
            "e8" => 8,
            u if u.starts_with("minor") => places,
            // Nothing a model writes is counted in anything else.
            _ => return Ok(()),
        };
        let text = match v {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            _ => return Ok(()),
        };
        *v = Value::from(parse_decimal(&text, places).ok_or_else(|| {
            format!("{text:?} is not a decimal amount with at most {places} decimal places, like \"42.18\"")
        })?);
        Ok(())
    })
}

type Apply<'a> = dyn FnMut(&str, &mut Value, u32) -> Result<(), String> + 'a;

/// Visit each value whose schema has a unit, with the decimal places it is counted in.
/// `siblings` is the object holding `value`, for a `minor:<field>` unit. On the way to a model
/// (`agent`), fields are renamed and left out as [`agent_key`] says. A type the schema names
/// more than once is a `$ref` into `half`'s `$defs` (`bus.schema {op}` writes it once there).
#[allow(clippy::too_many_arguments)]
fn walk(value: &mut Value, schema: &Value, half: &Value, root: u32, siblings: Option<&Map<String, Value>>, agent: bool, apply: &mut Apply) -> Result<(), String> {
    // A unit beside a reference is the field's own; else the type's.
    let schema = match schema.get("x-unit") {
        Some(_) => schema,
        None => resolved(schema, half),
    };
    if let Some(unit) = schema.get("x-unit").and_then(Value::as_str) {
        let places = unit.strip_prefix("minor:")
            .and_then(|field| siblings?.get(field)?.as_str())
            .map_or(root, digits);
        return match value {
            Value::Array(items) => items.iter_mut().try_for_each(|item| apply(unit, item, places)),
            Value::Null => Ok(()),
            other => apply(unit, other, places),
        };
    }
    match value {
        Value::Object(map) => {
            // The siblings a `minor:<field>` unit reads its currency from, copied only when a field
            // of this object has one: a design.apply list (which carries an `omit`) is not copied
            // whole on its way to the engine.
            let reads_sibling = |key: &String| {
                property(schema, key, half, 0).and_then(|sub| sub.get("x-unit")).and_then(Value::as_str).is_some_and(|u| u.starts_with("minor:"))
            };
            let around = map.keys().any(reads_sibling).then(|| map.clone());
            let mut moves = Vec::new();
            for (key, field) in map.iter_mut() {
                let Some(sub) = property(schema, key, half, 0) else { continue };
                // Nothing in a field whose schema has no unit is counted, renamed or left out.
                if !units_in(sub, half, &mut Vec::new()) && sub.get("x-agent-name").is_none() {
                    continue;
                }
                walk(field, sub, half, root, around.as_ref(), agent, apply)?;
                let renamed = agent_key(key, sub);
                if agent && renamed.as_deref() != Some(key.as_str()) {
                    moves.push((key.clone(), renamed));
                }
            }
            for (key, renamed) in moves {
                if let (Some(field), Some(renamed)) = (map.remove(&key), renamed) {
                    map.insert(renamed, field);
                }
            }
            Ok(())
        }
        Value::Array(items) => match items_schema(schema, half, 0) {
            Some(sub) => items.iter_mut().try_for_each(|item| walk(item, sub, half, root, None, agent, apply)),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

/// How far references and branches are followed: a type that holds itself stops there.
const HOPS: u8 = 8;

/// `schema` with its local reference (`#/$defs/…`) followed in `half`.
fn resolved<'a>(mut schema: &'a Value, half: &'a Value) -> &'a Value {
    for _ in 0..HOPS {
        match schema.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix('#')).and_then(|at| half.pointer(at)) {
            Some(target) => schema = target,
            None => break,
        }
    }
    schema
}

/// Whether anything in `schema` carries a unit, through the references it makes into `half`.
/// `seen` holds the references already followed, so a type that holds itself is read once.
fn units_in<'a>(schema: &'a Value, half: &'a Value, seen: &mut Vec<&'a str>) -> bool {
    match schema {
        Value::Object(map) => {
            if map.contains_key("x-unit") {
                return true;
            }
            if let Some(at) = map.get("$ref").and_then(Value::as_str) {
                if !seen.contains(&at) {
                    seen.push(at);
                    if at.strip_prefix('#').and_then(|at| half.pointer(at)).is_some_and(|target| units_in(target, half, seen)) {
                        return true;
                    }
                }
            }
            map.values().any(|v| units_in(v, half, seen))
        }
        Value::Array(items) => items.iter().any(|v| units_in(v, half, seen)),
        _ => false,
    }
}

/// The schema of `key` in an object schema, looking into `anyOf`/`oneOf`/`allOf` branches.
fn property<'a>(schema: &'a Value, key: &str, half: &'a Value, hops: u8) -> Option<&'a Value> {
    let schema = resolved(schema, half);
    schema.pointer(&format!("/properties/{}", key.replace('~', "~0").replace('/', "~1")))
        .or_else(|| branches(schema).filter(|_| hops < HOPS).find_map(|b| property(b, key, half, hops + 1)))
}

fn items_schema<'a>(schema: &'a Value, half: &'a Value, hops: u8) -> Option<&'a Value> {
    let schema = resolved(schema, half);
    schema.get("items").or_else(|| branches(schema).filter(|_| hops < HOPS).find_map(|b| items_schema(b, half, hops + 1)))
}

fn branches(schema: &Value) -> impl Iterator<Item = &Value> {
    ["anyOf", "oneOf", "allOf"].into_iter().filter_map(|k| schema.get(k)?.as_array()).flatten()
}

fn trimmed(text: String) -> String {
    if !text.contains('.') {
        return text;
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// "157.99" at 2 places is 15799. A sign, digits, and optionally a point and at most `places`
/// digits that count (zeros past them are fine). Nothing else: no grouping, no currency sign.
fn parse_decimal(text: &str, places: u32) -> Option<i64> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if body.contains('.') && frac.is_empty() {
        return None;
    }
    let places = places as usize;
    if frac.len() > places && frac[places..].bytes().any(|b| b != b'0') {
        return None;
    }
    let mut value: i64 = 0;
    for b in int.bytes().chain(frac.bytes().chain(std::iter::repeat(b'0')).take(places)) {
        value = value.checked_mul(10)?.checked_add(i64::from(b - b'0'))?;
    }
    Some(if negative { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(op: &str) -> Value {
        relay_bus::schema::render_op(op).unwrap()
    }

    #[test]
    fn an_entry_reads_in_dollars_and_cents() {
        let mut tx = json!({"id": 91, "type": "EXPENSE", "amount": 15799, "account_id": 2, "category_id": 7, "color": 4, "note": "Phone case"});
        to_agent(&mut tx, &schema("money.tx.add")["result"], 2);
        assert_eq!(tx["amount"], "157.99");
        // Ids, colours and counts are not money.
        assert_eq!(tx["id"], 91);
        assert_eq!(tx["account_id"], 2);
        assert_eq!(tx["color"], 4);
    }

    #[test]
    fn a_summary_converts_every_figure_and_nothing_else() {
        let mut summary = json!({
            "currency": "CAD", "fraction_digits": 2, "month_start_day": 1,
            "period": {"days": 31, "days_left": 22},
            "pace": {"budget": 290000, "spent": 196300, "daily_allowance": 4300, "total_days": 31, "spent_fraction": 0.68},
            "income": 0, "spent": 196300, "net_worth": -1234,
            "budgets": [{"category_id": 7, "budget": 15000, "spent": 15799, "left": -799, "pace_delta": 799, "color": 4}],
            "accounts": [{"id": 2, "balance": 100005}],
        });
        to_agent(&mut summary, &schema("money.summary")["result"], 2);
        assert_eq!(summary["budgets"][0]["budget"], "150.00");
        assert_eq!(summary["budgets"][0]["spent"], "157.99");
        assert_eq!(summary["budgets"][0]["left"], "-7.99");
        assert_eq!(summary["budgets"][0]["over_pace_by"], "7.99");
        assert!(summary["budgets"][0].get("pace_delta").is_none());
        assert_eq!(summary["pace"]["daily_allowance"], "43.00");
        assert_eq!(summary["net_worth"], "-12.34");
        assert_eq!(summary["accounts"][0]["balance"], "1000.05");
        assert_eq!(summary["pace"]["total_days"], 31);
        assert_eq!(summary["period"]["days_left"], 22);
        // Percent, under a name that says so; the decimals a model no longer needs are gone.
        assert_eq!(summary["pace"]["spent_pct"], "68");
        assert!(summary["pace"].get("spent_fraction").is_none());
        assert!(summary.get("fraction_digits").is_none());
        assert_eq!(summary["budgets"][0].get("color"), None);
        assert_eq!(summary["budgets"][0]["category_id"], 7);
    }

    #[test]
    fn basis_points_read_as_percent() {
        let mut portfolio = json!({"currency": "CAD", "value": 100000, "gain_bps": 1250, "accounts": [{"id": 1, "return_bps": -75, "gain_bps": null}], "allocation": [{"value": 100000, "share_bps": 10000}]});
        to_agent(&mut portfolio, &schema("money.invest.summary")["result"], 2);
        assert_eq!(portfolio["gain_pct"], "12.50");
        assert_eq!(portfolio["accounts"][0]["return_pct"], "-0.75");
        assert_eq!(portfolio["accounts"][0]["gain_pct"], Value::Null);
        assert_eq!(portfolio["allocation"][0]["share_pct"], "100.00");
        assert!(portfolio.get("gain_bps").is_none());
    }

    #[test]
    fn unix_seconds_read_as_utc_times() {
        let mut stats = json!({"whole": {"start": 1760140800, "end": 1760227200, "return_pct": 1.5, "trades": 2}, "trades": [{"buy_at": 1760140800, "sell_at": null, "buy_price": 100.5, "pnl": 1.0, "fees": 0.1}]});
        let result = &schema("arbiter.backtest")["result"];
        assert!(has_units(result));
        to_agent(&mut stats, result, 2);
        assert_eq!(stats["whole"]["start"], "2025-10-11T00:00:00Z");
        assert_eq!(stats["trades"][0]["buy_at"], "2025-10-11T00:00:00Z");
        assert_eq!(stats["trades"][0]["sell_at"], Value::Null);
        assert_eq!(stats["trades"][0]["buy_price"], 100.5);
        assert_eq!(stats["whole"]["return_pct"], 1.5);
    }

    #[test]
    fn an_entry_keeps_what_its_card_draws() {
        // Threads' entry card reads the icon and colour from the tool's answer.
        let mut tx = json!({"id": 91, "uid": "d778", "type": "EXPENSE", "amount": 15799, "icon": "bag", "color": 4});
        to_agent(&mut tx, &schema("money.tx.add")["result"], 2);
        assert_eq!((tx["icon"].clone(), tx["color"].clone()), (json!("bag"), json!(4)));
        assert!(tx.get("uid").is_none());
    }

    #[test]
    fn a_reading_in_a_currency_without_cents_has_none() {
        let mut page = json!({"currency": "JPY", "income": 1500, "spent": 0, "budgets": [], "accounts": []});
        to_agent(&mut page, &schema("money.summary")["result"], 2);
        assert_eq!(page["income"], "1500");
        // A page without its own currency reads in the ledger's.
        let mut page = json!({"transactions": [{"amount": 1500}], "income": 0, "spent": 1500});
        to_agent(&mut page, &schema("money.tx.list")["result"], 0);
        assert_eq!(page["transactions"][0]["amount"], "1500");
    }

    #[test]
    fn quantities_and_prices_read_as_plain_decimals() {
        let mut portfolio = json!({"currency": "CAD", "value": 381200, "holdings": [{"currency": "USD", "quantity": 150000000, "price": 3812000000_i64, "value": 571800}]});
        to_agent(&mut portfolio, &schema("money.invest.summary")["result"], 2);
        assert_eq!(portfolio["holdings"][0]["quantity"], "1.5");
        assert_eq!(portfolio["holdings"][0]["price"], "38.12");
        assert_eq!(portfolio["holdings"][0]["value"], "5718.00");
        assert_eq!(portfolio["value"], "3812.00");
    }

    #[test]
    fn an_activity_counts_in_its_own_currency() {
        let mut list = json!({"activities": [{"type": "FX", "currency": "JPY", "amount": 10000, "fee": 0, "quantity": 0, "to_currency": "CAD", "to_amount": 9050}]});
        to_agent(&mut list, &schema("money.invest.list")["result"], 2);
        assert_eq!(list["activities"][0]["amount"], "10000");
        assert_eq!(list["activities"][0]["to_amount"], "90.50");
        assert_eq!(list["activities"][0]["quantity"], "0");
    }

    #[test]
    fn a_chart_series_is_dollars_too() {
        let mut series = json!({"currency": "CAD", "fraction_digits": 2, "labels": ["Week 1"], "label_colors": [null], "series": [{"name": "Spending", "values": [15799, 0]}]});
        to_agent(&mut series, &schema("money.series")["result"], 2);
        assert_eq!(series["series"][0]["values"], json!(["157.99", "0.00"]));
        assert!(series.get("label_colors").is_none());
    }

    #[test]
    fn what_a_model_writes_becomes_minor_units() {
        let payload = &schema("money.tx.add")["payload"];
        for (given, expect) in [(json!("157.99"), 15799), (json!(157.99), 15799), (json!("150"), 15000), (json!(12), 1200), (json!("0.5"), 50), (json!("7.990"), 799)] {
            let mut args = json!({"type": "EXPENSE", "amount": given.clone(), "date": "2026-10-10", "account_id": 2});
            to_engine(&mut args, payload, 2).unwrap();
            assert_eq!(args["amount"], expect, "{given}");
            assert_eq!(args["account_id"], 2);
        }
        for bad in [json!("157.999"), json!("$157.99"), json!("1,234.56"), json!("12."), json!("abc")] {
            let mut args = json!({"type": "EXPENSE", "amount": bad.clone(), "date": "2026-10-10", "account_id": 2});
            assert!(to_engine(&mut args, payload, 2).is_err(), "{bad}");
        }
        // An edit that leaves the amount alone stays without one.
        let mut patch = json!({"id": 91, "note": "Phone case"});
        to_engine(&mut patch, &schema("money.tx.update")["payload"], 2).unwrap();
        assert_eq!(patch, json!({"id": 91, "note": "Phone case"}));
    }

    #[test]
    fn an_activity_is_written_in_its_currency() {
        let payload = &schema("money.invest.add")["payload"];
        let mut args = json!({"account_id": 3, "type": "BUY", "date": "2026-10-10", "symbol": "XEQT", "quantity": "1.5", "amount": "45.10", "fee": "0"});
        to_engine(&mut args, payload, 2).unwrap();
        assert_eq!((args["quantity"].clone(), args["amount"].clone(), args["fee"].clone()), (json!(150000000), json!(4510), json!(0)));
        let mut args = json!({"account_id": 3, "type": "FX", "date": "2026-10-10", "currency": "JPY", "amount": "10000", "to_currency": "CAD", "to_amount": "90.5"});
        to_engine(&mut args, payload, 2).unwrap();
        assert_eq!((args["amount"].clone(), args["to_amount"].clone()), (json!(10000), json!(9050)));
    }

    #[test]
    fn a_proposal_is_written_in_dollars_too() {
        // Each payload in a batch is its op's own, and converts as that op's would.
        let payload = &schema("thread.propose")["payload"];
        let mut args = json!({"why": "Last three months", "action": {"money.budget.set": [{"category_id": 7, "amount": "380"}, {"amount": "3000.50"}]}});
        to_engine(&mut args, payload, 2).unwrap();
        assert_eq!(args["action"]["money.budget.set"], json!([{"category_id": 7, "amount": 38000}, {"amount": 300050}]));
        let mut args = json!({"why": "Typo", "action": {"money.tx.update": [{"id": 4, "amount": "12.5"}, {"id": 5, "category_id": 3}]}});
        to_engine(&mut args, payload, 2).unwrap();
        assert_eq!(args["action"]["money.tx.update"], json!([{"id": 4, "amount": 1250}, {"id": 5, "category_id": 3}]));
        let mut ids = json!({"why": "Twice", "action": {"money.tx.delete": [{"id": 4}]}});
        to_engine(&mut ids, payload, 2).unwrap();
        assert_eq!(ids["action"]["money.tx.delete"], json!([{"id": 4}]));
        // And reads back so: the budgets it acts on, and an edit's entry before and after.
        let mut view = json!({"id": 1, "currency": "CAD", "action": {"money.budget.set": [{"category_id": 7, "amount": 38000}]},
            "items": [{"state": "same", "budget": 32000, "budget_after": 38000, "entry": {"id": 4, "amount": 1599}}]});
        to_agent(&mut view, &schema("thread.propose")["result"], 2);
        assert_eq!(view["action"]["money.budget.set"][0]["amount"], "380.00");
        assert_eq!((view["items"][0]["budget"].clone(), view["items"][0]["budget_after"].clone()), (json!("320.00"), json!("380.00")));
        assert_eq!(view["items"][0]["entry"]["amount"], "15.99");
        let mut changed = json!({"id": 4, "amount": 2100, "previous": {"id": 4, "amount": 1200}});
        to_agent(&mut changed, &schema("money.tx.update")["result"], 2);
        assert_eq!((changed["amount"].clone(), changed["previous"]["amount"].clone()), (json!("21.00"), json!("12.00")));
        let shown = agent_schema(payload).to_string();
        assert!(shown.contains("not cents") && !shown.contains("x-unit"), "{shown}");
    }

    /// `x-unit: omit` on a payload field hides it from what a model is shown: design.apply's
    /// `expect` is the person's clients' alone (their undo sends it), so an agent's tool lists
    /// no such field, its answers leave it out, and what it writes goes to the engine as written.
    #[test]
    fn design_apply_hides_expect_from_agents() {
        let schema = schema("design.apply");
        assert!(has_units(&schema["payload"]) && has_units(&schema["result"]));
        let shown = agent_schema(&schema["payload"]);
        let fields: Vec<&str> = shown["properties"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(fields, ["design_id", "ops", "base_rev", "lenient", "reply"]);
        assert!(!shown.to_string().contains("x-unit"));
        let mut payload = json!({"design_id": 4, "ops": [{"op": "update", "id": "a", "set": {"fill": "#fff", "x": 12}}], "lenient": true,
            "expect": [{"id": "a", "set": {"fill": "#000"}}]});
        let written = payload.clone();
        to_engine(&mut payload, &schema["payload"], 2).unwrap();
        assert_eq!(payload, written, "nothing in it is money");
        let mut answer = json!({"design_id": 4, "rev": 9, "prev_rev": 8, "ids": ["a"], "removed_count": 0,
            "inverse": [{"op": "update", "id": "a", "set": {"fill": "#000"}}], "expect": [{"id": "a", "set": {"fill": "#fff"}}]});
        to_agent(&mut answer, &schema["result"], 2);
        assert!(answer.get("expect").is_none(), "{answer}");
        assert_eq!(answer["inverse"][0]["set"]["fill"], "#000", "the rest as it came");
        assert!(agent_schema(&schema["result"])["properties"].get("expect").is_none());
    }

    /// `bus.schema {op}` writes a type a half names twice once, in its `$defs` (TM-3): an amount
    /// inside one still reads in dollars, and the type's own `omit` still leaves a field out.
    #[test]
    fn an_amount_in_a_type_named_twice_still_converts() {
        let result = &schema("thread.proposal.get")["result"];
        let entry = &result["properties"]["items"]["items"]["properties"]["entry"];
        assert!(entry.to_string().contains("\"$ref\":\"#/$defs/MoneyTx\""), "{entry}");
        assert!(result["$defs"]["MoneyAccount"]["properties"]["balance"]["x-unit"] == "minor");
        let mut view = json!({"id": 3, "currency": "CAD", "items": [
            {"state": "same", "entry": {"id": 4, "uid": "d778", "amount": 1599}, "entry_after": {"id": 4, "amount": 2100}},
            {"state": "changed", "account": {"id": 2, "balance": 100005}, "account_after": null},
        ]});
        to_agent(&mut view, result, 2);
        assert_eq!((view["items"][0]["entry"]["amount"].clone(), view["items"][0]["entry_after"]["amount"].clone()), (json!("15.99"), json!("21.00")));
        assert!(view["items"][0]["entry"].get("uid").is_none(), "{view}");
        assert_eq!(view["items"][1]["account"]["balance"], "1000.05");
        assert_eq!(view["items"][1]["account_after"], Value::Null);
        assert_eq!(view["items"][0]["entry"]["id"], 4);
    }

    #[test]
    fn a_currency_in_lower_case_counts_its_own_places() {
        // The ledger takes " jpy" as JPY, so its places are JPY's, not the two of an unknown code.
        assert_eq!((digits("jpy"), digits(" kwd "), digits("cad")), (0, 3, 2));
        let payload = &schema("money.invest.add")["payload"];
        let mut args = json!({"account_id": 3, "type": "FX", "date": "2026-10-10", "currency": "jpy", "amount": "5000", "to_currency": " kwd", "to_amount": "1.5"});
        to_engine(&mut args, payload, 2).unwrap();
        assert_eq!((args["amount"].clone(), args["to_amount"].clone()), (json!(5000), json!(1500)));
        let mut page = json!({"currency": "jpy", "income": 1500, "spent": 0, "budgets": [], "accounts": []});
        to_agent(&mut page, &schema("money.summary")["result"], 2);
        assert_eq!(page["income"], "1500");
    }

    #[test]
    fn the_schema_a_model_sees_asks_for_decimal_strings() {
        let shown = agent_schema(&schema("money.tx.add")["payload"]);
        assert_eq!(shown["properties"]["amount"]["type"], "string");
        assert!(shown["properties"]["amount"]["description"].as_str().unwrap().contains("not cents"));
        assert_eq!(shown["properties"]["account_id"]["type"], "integer");
        let shown = agent_schema(&schema("money.tx.update")["payload"]);
        assert_eq!(shown["properties"]["amount"]["type"], json!(["string", "null"]));
        let shown = agent_schema(&schema("money.invest.add")["payload"]);
        let quantity = shown["properties"]["quantity"]["description"].as_str().unwrap();
        assert!(!quantity.contains("1e-8"), "{quantity}");
        let shown = agent_schema(&schema("money.series")["result"]);
        assert_eq!(shown["properties"]["series"]["items"]["properties"]["values"]["items"]["type"], "string");
        assert!(shown["properties"].get("label_colors").is_none());
        let required: Vec<&str> = shown["required"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(!required.contains(&"label_colors") && !required.contains(&"fraction_digits"), "{required:?}");
        let shown = agent_schema(&schema("money.invest.summary")["result"]);
        assert_eq!(shown["properties"]["gain_pct"]["type"], json!(["string", "null"]));
        assert!(shown["properties"].get("gain_bps").is_none());
        let shown = agent_schema(&schema("money.summary")["result"]);
        let budget = &shown["properties"]["budgets"]["items"]["properties"];
        assert_eq!(budget["over_pace_by"]["type"], "string");
        assert!(budget.get("pace_delta").is_none() && !shown.to_string().contains("x-agent-name"));
        let shown = agent_schema(&schema("arbiter.backtest")["result"]);
        assert!(shown.to_string().contains("2025-10-11T00:00:00Z"));
        assert!(!has_units(&shown));
        assert!(!has_units(&schema("gym.summary")["result"]));
    }
}
