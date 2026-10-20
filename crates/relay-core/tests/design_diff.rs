//! The canvas runs every edit ahead of the engine (relay_client's `Scene`), so the two must run
//! `design.apply` by the same rules (I-11). A seeded generator writes lists of edits, valid and
//! not, and each list runs through the engine and through `Scene::apply_edit` (strict, all or
//! none), then `confirm_all` of the engine's answer, as the board does: the two refuse the same
//! lists, and after each list hold the same nodes, children in the same order and the same
//! colours and styles, before the confirm as after it. Every few lists an earlier one is
//! undone, lenient, as the board's undo does: where the engine skips nothing the Scene must run
//! every edit of the undo, and where it fails one the engine must have skipped it.
//!
//! Then the geometric invariant: group, frame, ungroup and reparent (relay_client's edit
//! builders) leave every node they move where it was drawn and as big (TM-8, `edit::freeze`).
//!
//! Seeds are fixed, so a failure names a seed and a step that reproduce it.

mod common;

use common::{call, committed_repo, engine_with_project, ok};
use relay_bus::design::{Node, NodeKind, NodeOp, Styles};
use relay_bus::ops::design::{ApplyOut, GetOut};
use relay_client::design::edit;
use relay_client::design::layout::{self, Measure};
use relay_client::design::Scene;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

const SEEDS: u64 = 50;
const STEPS: usize = 60;

/// xorshift64*: the same lists on every machine, no dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as usize) as i64
    }
    /// True `pct` times in a hundred.
    fn odds(&mut self, pct: usize) -> bool {
        self.below(100) < pct
    }
    fn pick<'a, T>(&mut self, of: &'a [T]) -> &'a T {
        &of[self.below(of.len())]
    }
}

fn project() -> (Arc<Engine>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let ws = root.path().join("ws");
    let repo = ws.join("app");
    committed_repo(&repo, &[("README.md", "# app\n")]);
    let engine = engine_with_project(root.path(), &ws, &repo);
    (engine, root)
}

fn read(e: &Engine, design: i64) -> GetOut {
    serde_json::from_value(ok(e, "design.get", json!({"design_id": design}))).unwrap()
}

/// A design as the engine reads it, in the Scene.
fn scene_of(got: &GetOut) -> Scene {
    Scene::new(got.nodes.clone()).with_styles(Styles { colors: got.colors.clone(), styles: got.styles.clone() })
}

/// What two sides must agree on: each node as stored, each parent's children bottom to top, and
/// the colours and styles.
type State = (BTreeMap<String, Node>, BTreeMap<String, Vec<String>>, Styles);

fn state(nodes: Vec<Node>, styles: Styles) -> State {
    let mut kids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in &nodes {
        kids.entry(n.parent.clone().unwrap_or_default()).or_default().push(n.id.clone());
    }
    (nodes.into_iter().map(|n| (n.id.clone(), n)).collect(), kids, styles)
}

fn engine_state(got: GetOut) -> State {
    state(got.nodes, Styles { colors: got.colors, styles: got.styles })
}

fn scene_state(scene: &Scene) -> State {
    state(scene.to_nodes(), scene.styles().clone())
}

/// Where two states part, in a line or two.
fn diff(engine: &State, scene: &State) -> String {
    let mut out = Vec::new();
    for (id, n) in &engine.0 {
        match scene.0.get(id) {
            None => out.push(format!("scene lacks {id}")),
            Some(m) if m != n => {
                let (a, b) = (serde_json::to_value(n).unwrap(), serde_json::to_value(m).unwrap());
                let keys: Vec<String> = a.as_object().unwrap().keys().chain(b.as_object().unwrap().keys())
                    .filter(|k| a.get(*k) != b.get(*k)).cloned().collect();
                let shown: Vec<String> = keys.iter().map(|k| format!("{k}: engine {} scene {}", a.get(k).unwrap_or(&Value::Null), b.get(k).unwrap_or(&Value::Null))).collect();
                out.push(format!("{id} differs: {}", shown.join(", ")));
            }
            _ => {}
        }
    }
    out.extend(scene.0.keys().filter(|id| !engine.0.contains_key(*id)).map(|id| format!("engine lacks {id}")));
    for (parent, kids) in &engine.1 {
        if scene.1.get(parent) != Some(kids) {
            out.push(format!("children of {parent:?}: engine {kids:?} scene {:?}", scene.1.get(parent)));
        }
    }
    if engine.2 != scene.2 {
        out.push(format!("styles: engine {:?} scene {:?}", engine.2, scene.2));
    }
    out.join("; ")
}

/// Writes lists of edits against what the design holds now (read from the Scene, which mirrors
/// it), about one edit in six of them meant to be refused.
struct Gen {
    rng: Rng,
    next: usize,
}

const COLORS: [&str; 4] = ["c0", "c1", "c2", "c3"];
const STYLES: [&str; 3] = ["s0", "s1", "s2"];

impl Gen {
    fn hex(&mut self) -> String {
        format!("#{:06X}", self.rng.next() & 0xFF_FFFF)
    }

    fn fresh(&mut self) -> String {
        self.next += 1;
        format!("n{}", self.next)
    }

    fn paint(&mut self) -> Value {
        match self.rng.below(5) {
            0 => json!(format!("${}", self.rng.pick(&COLORS))),
            1 => Value::Null,
            _ => json!(self.hex()),
        }
    }

    fn containers(scene: &Scene) -> Vec<String> {
        scene.to_nodes().into_iter().filter(|n| n.kind.holds_children()).map(|n| n.id).collect()
    }

    fn any_node(&mut self, scene: &Scene) -> String {
        let all: Vec<String> = scene.to_nodes().into_iter().map(|n| n.id).collect();
        if self.rng.odds(4) || all.is_empty() {
            return "gone".into();
        }
        self.rng.pick(&all).clone()
    }

    /// A non-page node, mostly.
    fn some_node(&mut self, scene: &Scene) -> String {
        let all: Vec<String> = scene.to_nodes().into_iter().filter(|n| n.parent.is_some()).map(|n| n.id).collect();
        if all.is_empty() || self.rng.odds(4) {
            return self.any_node(scene);
        }
        self.rng.pick(&all).clone()
    }

    fn parent(&mut self, scene: &Scene) -> String {
        if self.rng.odds(5) {
            return self.any_node(scene);
        }
        let holders = Self::containers(scene);
        self.rng.pick(&holders).clone()
    }

    fn add(&mut self, scene: &Scene) -> Value {
        let kinds = ["frame", "frame", "group", "rect", "rect", "ellipse", "text", "line", "path"];
        let kind = *self.rng.pick(&kinds);
        let id = if self.rng.odds(4) { self.any_node(scene) } else { self.fresh() };
        let r = &mut self.rng;
        let mut node = json!({"id": id, "parent": "", "kind": kind, "x": r.int(-50, 600), "y": r.int(-50, 600)});
        if r.odds(85) {
            node["w"] = json!(r.int(-2, 300));
            node["h"] = json!(r.int(-2, 300));
        }
        match kind {
            "frame" if r.odds(40) => {
                node["layout"] = json!(*r.pick(&["row", "column", "wrap"]));
                node["gap"] = json!(r.int(0, 16));
                if r.odds(50) {
                    node["padding"] = json!([r.int(0, 12)]);
                }
            }
            "text" => node["text"] = json!(*r.pick(&["Hi", "Sign in", "A longer line of words"])),
            "line" | "path" => node["points"] = json!([[0, 0], [r.int(-40, 120), r.int(-40, 120)], [r.int(0, 80), r.int(0, 80)]]),
            _ => {}
        }
        if r.odds(25) {
            node["pin_x"] = json!(*r.pick(&["left", "right", "left_right", "center", "scale"]));
        }
        if r.odds(25) {
            node["pin_y"] = json!(*r.pick(&["top", "bottom", "top_bottom", "center", "scale"]));
        }
        if r.odds(20) {
            node["w_sizing"] = json!(*r.pick(&["fixed", "hug", "fill"]));
        }
        if r.odds(15) {
            node["rotation"] = json!(r.int(-90, 90));
        }
        if r.odds(10) {
            node["style"] = json!(*r.pick(&STYLES));
        }
        if kind != "group" && kind != "line" && kind != "path" && self.rng.odds(60) {
            node["fill"] = self.paint();
        }
        node["parent"] = json!(self.parent(scene));
        let mut op = json!({"op": "add", "node": node});
        if self.rng.odds(30) {
            op["index"] = json!(self.rng.below(4));
        }
        op
    }

    fn update(&mut self, scene: &Scene) -> Value {
        let id = self.some_node(scene);
        let mut set = serde_json::Map::new();
        for _ in 0..1 + self.rng.below(3) {
            let r = &mut self.rng;
            let (key, value) = match r.below(17) {
                0 => ("x", json!(r.int(-100, 700))),
                1 => ("y", json!(r.int(-100, 700))),
                2 => ("w", json!(r.int(-2, 400))),
                3 => ("h", json!(r.int(-2, 400))),
                4 => ("width", json!(r.int(1, 400))),
                5 => ("rotation", if r.odds(30) { Value::Null } else { json!(r.int(-180, 180)) }),
                6 => ("opacity", json!(*r.pick(&[0.0, 0.25, 0.5, 1.0, 1.5]))),
                7 => ("layout", if r.odds(30) { Value::Null } else { json!(*r.pick(&["row", "column", "wrap"])) }),
                8 => ("gap", json!(r.int(0, 24))),
                9 => ("pin_x", json!(*r.pick(&["left", "right", "left_right", "center", "scale"]))),
                10 => ("w_sizing", json!(*r.pick(&["fixed", "hug", "fill"]))),
                11 => ("hidden", json!(r.odds(50))),
                12 => ("name", json!(format!("Layer {}", r.below(9)))),
                13 => ("style", if r.odds(30) { Value::Null } else { json!(*r.pick(&STYLES)) }),
                14 => ("text", json!(*r.pick(&["Ok", "Two words", ""]))),
                15 => ("h_sizing", json!(*r.pick(&["fixed", "hug", "fill"]))),
                _ => ("bogus", json!(1)),
            };
            let value = if key == "x" && self.rng.odds(10) { self.paint() } else { value };
            set.insert(key.into(), value);
        }
        if self.rng.odds(25) {
            set.insert("fill".into(), self.paint());
        }
        json!({"op": "update", "id": id, "set": set})
    }

    fn move_op(&mut self, scene: &Scene) -> Value {
        let id = self.some_node(scene);
        let mut op = json!({"op": "move", "id": id});
        match self.rng.below(4) {
            // Among its siblings.
            0 => {}
            // Into its own subtree, or anywhere at all.
            1 => op["parent"] = json!(scene.subtree(&id).last().cloned().unwrap_or_else(|| "gone".into())),
            _ => op["parent"] = json!(self.parent(scene)),
        }
        if self.rng.odds(60) {
            op["index"] = json!(self.rng.below(5));
        }
        op
    }

    fn op(&mut self, scene: &Scene) -> Value {
        match self.rng.below(20) {
            0..=6 => self.add(scene),
            7..=11 => self.update(scene),
            12..=14 => self.move_op(scene),
            15 => json!({"op": "delete", "id": self.any_node(scene)}),
            16 => {
                let value = if self.rng.odds(10) { json!("blue") } else { json!(self.hex()) };
                json!({"op": "color", "name": *self.rng.pick(&COLORS), "value": value})
            }
            17 => {
                let set = match self.rng.below(3) {
                    0 => json!({"fill": self.paint()}),
                    1 => json!({"radius": self.rng.int(0, 16)}),
                    _ => json!({"opacity": 0.5, "stroke": format!("${}", self.rng.pick(&COLORS))}),
                };
                json!({"op": "style", "name": *self.rng.pick(&STYLES), "set": set})
            }
            18 => {
                let name = if self.rng.odds(50) { self.rng.pick(&COLORS) } else { self.rng.pick(&STYLES) };
                json!({"op": "delete_style", "name": *name})
            }
            _ => json!({"op": "add", "node": {"id": self.fresh(), "kind": "page"}}),
        }
    }

    /// A list of one to three edits. Each is written against the Scene as the list so far would
    /// leave it, so a later edit can name what an earlier one added.
    fn list(&mut self, scene: &Scene) -> Value {
        let mut trial = scene.clone();
        let mut ops = Vec::new();
        for _ in 0..1 + self.rng.below(3) {
            let op = self.op(&trial);
            if let Ok(parsed) = serde_json::from_value::<NodeOp>(op.clone()) {
                let _ = trial.apply_strict(&parsed);
            }
            ops.push(op);
        }
        Value::Array(ops)
    }
}

/// What a run of the difftest saw.
#[derive(Default, Debug)]
struct Tally {
    applied: usize,
    refused: usize,
    undone_clean: usize,
    undone_skipping: usize,
    disagreements: Vec<String>,
}

/// Take the engine's answer as the board does: each node it touched, and the colours and styles
/// whole when the list changed them.
fn confirm(scene: &mut Scene, out: &ApplyOut) {
    scene.confirm_all(out.nodes.clone().unwrap_or_default(), std::iter::empty());
    if out.colors.is_some() || out.styles.is_some() {
        let now = scene.styles().clone();
        scene.set_styles(Styles { colors: out.colors.clone().unwrap_or(now.colors), styles: out.styles.clone().unwrap_or(now.styles) });
    }
}

/// One seed: a new design, `STEPS` lists, and an undo of an earlier list every few.
fn run_seed(e: &Engine, seed: u64, tally: &mut Tally) {
    let design = ok(e, "design.create", json!({"project_id": 1, "title": format!("Diff {seed}")}))["design"]["id"].as_i64().unwrap();
    let mut scene = scene_of(&read(e, design));
    let mut gen = Gen { rng: Rng::new(seed), next: 0 };
    let mut undos: Vec<(Value, Value)> = Vec::new();
    let disagree = |tally: &mut Tally, step: usize, what: String| tally.disagreements.push(format!("seed {seed} step {step}: {what}"));
    for step in 0..STEPS {
        if step % 7 == 6 && !undos.is_empty() {
            // The last list's undo mostly, now and then an older one's, which may find its nodes
            // gone or changed since and skip those edits.
            let back = if gen.rng.odds(70) { 1 } else { 1 + gen.rng.below(undos.len().min(4)) };
            let (inverse, expect) = undos.remove(undos.len() - back);
            let answer = call(e, "design.apply", json!({"design_id": design, "ops": inverse, "expect": expect, "lenient": true, "reply": "full"}));
            let ops: Vec<NodeOp> = serde_json::from_value(inverse.clone()).unwrap();
            match answer.result {
                Some(result) if answer.ok => {
                    let out: ApplyOut = serde_json::from_value(result).unwrap();
                    let skipped: Vec<usize> = out.skipped.iter().filter_map(|c| c.at.map(|a| a as usize)).collect();
                    let mut trial = scene.clone();
                    let failed: Vec<usize> = ops.iter().enumerate().filter(|(_, op)| trial.apply(op).is_err()).map(|(i, _)| i).collect();
                    if out.skipped.is_empty() {
                        tally.undone_clean += 1;
                        if !failed.is_empty() {
                            disagree(tally, step, format!("lenient undo: the engine skipped nothing, the scene failed edits {failed:?} of {inverse}"));
                        }
                        confirm(&mut trial, &out);
                        let (want, got) = (engine_state(read(e, design)), scene_state(&trial));
                        if want != got {
                            disagree(tally, step, format!("after the lenient undo {inverse}: {}", diff(&want, &got)));
                        }
                    } else {
                        tally.undone_skipping += 1;
                        if let Some(i) = failed.iter().find(|i| !skipped.contains(i)) {
                            disagree(tally, step, format!("lenient undo: the scene failed edit {i}, which the engine ran ({:?}), of {inverse}", out.skipped));
                        }
                    }
                }
                _ => disagree(tally, step, format!("lenient undo refused: {:?}", answer.error)),
            }
            // The board reads the design again after an undo that skipped; the test does always.
            scene = scene_of(&read(e, design));
            continue;
        }
        let list = gen.list(&scene);
        let answer = call(e, "design.apply", json!({"design_id": design, "ops": list, "reply": "full"}));
        let mut trial = scene.clone();
        let mine = match serde_json::from_value::<Vec<NodeOp>>(list.clone()) {
            Ok(ops) => trial.apply_edit(&ops),
            Err(e) => Err(format!("does not read: {e}")),
        };
        match (answer.ok, mine) {
            (true, Ok(())) => {
                tally.applied += 1;
                let out: ApplyOut = serde_json::from_value(answer.result.unwrap()).unwrap();
                let want = engine_state(read(e, design));
                // What the canvas drew before the engine answered.
                let predicted = scene_state(&trial);
                if want != predicted {
                    disagree(tally, step, format!("{list} ran differently: {}", diff(&want, &predicted)));
                }
                confirm(&mut trial, &out);
                let confirmed = scene_state(&trial);
                if want != confirmed && want == predicted {
                    disagree(tally, step, format!("{list}: the confirm undid a match: {}", diff(&want, &confirmed)));
                }
                if let (Some(inverse), Some(expect)) = (out.inverse, out.expect) {
                    undos.push((serde_json::to_value(inverse).unwrap(), serde_json::to_value(expect).unwrap()));
                }
            }
            (false, Err(_)) => tally.refused += 1,
            (true, Err(why)) => disagree(tally, step, format!("{list}: the engine ran it, the scene refused: {why}")),
            (false, Ok(())) => {
                let error = answer.error.unwrap();
                disagree(tally, step, format!("{list}: the scene ran it, the engine refused: {} {}", error.code, error.message));
            }
        }
        // Each list starts from the engine's design, so one disagreement does not cascade.
        scene = scene_of(&read(e, design));
    }
}

#[test]
fn the_scene_runs_every_list_as_the_engine_does() {
    let (e, _root) = project();
    let mut tally = Tally::default();
    for seed in 0..SEEDS {
        run_seed(&e, seed, &mut tally);
    }
    // The generator reaches both sides of every rule it tests, and the undo path both ways.
    assert!(tally.applied > 1000 && tally.refused > 200, "{tally:?}");
    assert!(tally.undone_clean > 100 && tally.undone_skipping > 0, "{tally:?}");
    eprintln!("{} applied, {} refused, {} undone clean, {} skipping", tally.applied, tally.refused, tally.undone_clean, tally.undone_skipping);
    let shown: Vec<&String> = tally.disagreements.iter().take(12).collect();
    assert!(tally.disagreements.is_empty(), "{} disagreements, the first:\n{}", tally.disagreements.len(), shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}

// --- the geometric invariant ------------------------------------------------------------------

/// 8 px a character, 20 px a line: text keeps one size wherever it sits.
struct Mono;
impl Measure for Mono {
    fn measure(&self, n: &Node, _wrap: Option<f64>) -> (f64, f64) {
        (n.text.as_deref().unwrap_or("").chars().count() as f64 * 8.0, 20.0)
    }
}

/// A page of nested frames and groups, some laid out, holding boxes of every sizing, run
/// through the engine so it is a design the engine would store.
fn random_page(e: &Engine, design: i64, rng: &mut Rng) -> Scene {
    let mut ops = Vec::new();
    let mut holders = vec![("page-1".to_string(), false)];
    for i in 0..18 {
        let (parent, laid) = holders[rng.below(holders.len())].clone();
        let id = format!("k{i}");
        let mut node = json!({"id": id, "parent": parent, "x": rng.int(0, 400), "y": rng.int(0, 400), "w": rng.int(10, 200), "h": rng.int(10, 200)});
        match rng.below(6) {
            0 | 1 => {
                node["kind"] = json!("frame");
                let layout = rng.odds(50);
                if layout {
                    node["layout"] = json!(*rng.pick(&["row", "column", "wrap"]));
                    node["gap"] = json!(rng.int(0, 12));
                    node["padding"] = json!([rng.int(0, 10)]);
                    if rng.odds(40) {
                        node["w_sizing"] = json!("hug");
                    }
                }
                holders.push((id.clone(), layout));
            }
            2 if !laid => {
                node["kind"] = json!("group");
                holders.push((id.clone(), false));
            }
            3 => {
                node["kind"] = json!("text");
                node["text"] = json!("Words here");
            }
            _ => node["kind"] = json!("rect"),
        }
        if laid && node["kind"] != "group" && rng.odds(40) {
            let hugs = node["kind"] != "rect";
            node["w_sizing"] = json!(*rng.pick(if hugs { &["fill", "hug", "fixed"][..] } else { &["fill", "fixed"][..] }));
        }
        // A fixed width: a group's comes from its children, a text's from its words.
        let fixed = node["kind"] == "rect" || (node["kind"] == "frame" && node.get("w_sizing").is_none());
        if !laid && fixed && rng.odds(30) {
            node["pin_x"] = json!(*rng.pick(&["right", "left_right", "center", "scale"]));
        }
        ops.push(json!({"op": "add", "node": node}));
    }
    ok(e, "design.apply", json!({"design_id": design, "ops": ops}));
    scene_of(&read(e, design))
}

/// Run `ops` (built by an edit builder) in both: the engine must take them, and the Scene then
/// match it.
fn run_both(e: &Engine, design: i64, scene: &Scene, ops: &[NodeOp], what: &str) -> Result<Scene, String> {
    let answer = call(e, "design.apply", json!({"design_id": design, "ops": ops}));
    if !answer.ok {
        let error = answer.error.unwrap();
        return Err(format!("{what}: the engine refused {}: {} {}", serde_json::to_string(ops).unwrap(), error.code, error.message));
    }
    let after = scene_of(&read(e, design));
    let mut mine = scene.clone();
    mine.apply_edit(ops).map_err(|why| format!("{what}: the scene refused: {why}"))?;
    let (want, got) = (scene_state(&after), scene_state(&mine));
    if want != got {
        return Err(format!("{what}: {}", diff(&want, &got)));
    }
    Ok(after)
}

/// Each of `ids` drawn where it was before the edit and as big, to a pixel's rounding.
fn kept(before: &layout::Placed, after: &layout::Placed, ids: &[String], what: &str) -> Result<(), String> {
    for id in ids {
        let (Some(a), Some(b)) = (before.rect(id), after.rect(id)) else { continue };
        let near = |p: f64, q: f64| (p - q).abs() <= 1.0;
        if !(near(a.x, b.x) && near(a.y, b.y) && near(a.w, b.w) && near(a.h, b.h)) {
            return Err(format!("{what}: {id} was drawn at {a:?}, now at {b:?}"));
        }
    }
    Ok(())
}

/// Whether `parent` places its children itself: what goes into it moves by its rules, not by
/// where it was.
fn lays_out(scene: &Scene, parent: &str) -> bool {
    edit::lays_out(scene, parent)
}

#[test]
fn group_frame_ungroup_and_reparent_leave_what_they_move_where_it_was() {
    let (e, _root) = project();
    let mut failures = Vec::new();
    let mut checked = 0;
    for seed in 0..SEEDS {
        let design = ok(&e, "design.create", json!({"project_id": 1, "title": format!("Geometry {seed}")}))["design"]["id"].as_i64().unwrap();
        let mut rng = Rng::new(1000 + seed);
        let mut scene = random_page(&e, design, &mut rng);
        for step in 0..12 {
            let placed = layout::place(&scene, "page-1", &Mono);
            let nodes: Vec<Node> = scene.to_nodes().into_iter().filter(|n| n.parent.is_some() && placed.rect(&n.id).is_some()).collect();
            if nodes.is_empty() {
                break;
            }
            let what = format!("seed {seed} step {step}");
            let built: Option<(Vec<NodeOp>, Vec<String>, &str)> = match rng.below(4) {
                // Group or frame some siblings, in a parent that does not lay them out.
                k @ (0 | 1) => {
                    let first = rng.pick(&nodes).id.clone();
                    let parent = scene.parent(&first).unwrap().to_string();
                    let mut ids: Vec<String> = scene.children(&parent).iter().filter(|_| rng.odds(50)).cloned().collect();
                    ids.push(first);
                    let kind = if k == 0 { NodeKind::Group } else { NodeKind::Frame };
                    (!lays_out(&scene, &parent)).then(|| edit::group(&mut scene.clone(), &placed, &ids, kind)).flatten()
                        .map(|(ops, _)| (ops, ids, if k == 0 { "group" } else { "frame" }))
                }
                // Ungroup a group or frame whose parent does not lay out.
                2 => {
                    let holders: Vec<&Node> = nodes.iter().filter(|n| matches!(n.kind, NodeKind::Group | NodeKind::Frame)).collect();
                    let pick = (!holders.is_empty()).then(|| rng.pick(&holders).id.clone());
                    pick.filter(|id| !lays_out(&scene, scene.parent(id).unwrap()))
                        .and_then(|id| edit::ungroup(&scene, &placed, &id))
                        .map(|(ops, kids)| (ops, kids, "ungroup"))
                }
                // Into another container that does not lay out, where it is drawn now.
                _ => {
                    let id = rng.pick(&nodes).id.clone();
                    let homes: Vec<String> = scene.to_nodes().into_iter()
                        .filter(|n| n.kind.holds_children() && n.id != id && !scene.is_inside(&n.id, &id) && !lays_out(&scene, &n.id))
                        .map(|n| n.id).collect();
                    let home = rng.pick(&homes).clone();
                    let r = placed.rect(&id).unwrap();
                    Some((edit::reparent(&scene, &placed, &id, &home, None, (r.x, r.y)), vec![id], "reparent"))
                }
            };
            let Some((ops, moved, kind)) = built else { continue };
            let what = format!("{what} {kind} {moved:?}");
            match run_both(&e, design, &scene, &ops, &what) {
                Ok(after) => {
                    checked += 1;
                    let now = layout::place(&after, "page-1", &Mono);
                    if let Err(why) = kept(&placed, &now, &moved, &what) {
                        failures.push(why);
                    }
                    scene = after;
                }
                Err(why) => {
                    failures.push(why);
                    scene = scene_of(&read(&e, design));
                }
            }
        }
    }
    assert!(checked > 200, "only {checked} edits checked");
    assert!(failures.is_empty(), "{} of {checked} edits moved what they held:\n{}", failures.len(), failures.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}
