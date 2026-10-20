//! One open design and its conversation with the engine. An edit runs on the local scene at
//! once and goes out through `design.apply`, one at a time and in order, with what the scene
//! showed of the fields it writes (`expect`), so it never overwrites a change it did not see;
//! the engine's answer confirms the nodes and carries the edits that undo it, which are the
//! undo stack. When someone else (an agent, another window) changes the design,
//! `design.changed` brings a newer `rev` and the design is read again once nothing of ours is
//! in flight.
//!
//! Every board showing a design shares one [`Shared`] (B28): one scene, one queue to the engine
//! and one undo history, however many boards and windows show it. A board holds it through a
//! [`Doc`] of its own, which carries what is the board's alone: whether a gesture of its holds
//! rereads back, and what it listens with. Both end when the board does.

use crate::app::Ui;
use crate::client::Error;
use relay_bus::design::{Conflict, ConflictReason, Expect, NodeOp, Styles};
use relay_bus::ops::design::{ApplyOut, GetOut};
use relay_bus::ops::picture::PictureCredit;
use relay_bus::types::Design;
use relay_bus::BusError;
use relay_client::design::edit::expect_for;
use relay_client::design::names::display_name;
use relay_client::design::Scene;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ops::Deref;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

type Listener = Rc<dyn Fn(&Change)>;

/// Undo steps kept per design.
const HISTORY: usize = 200;
/// The `design.changed` notices a design keeps between two reads, for who changed what (TM-13).
const NOTICES: usize = 64;

/// An edit, undo or redo not sent yet, or waiting to run again on the engine's copy (B9).
enum Pending {
    /// An edit, the selection it was made on, what the fields it writes held in the scene it
    /// was made on (W8), and the board that made it.
    Edit { ops: Vec<NodeOp>, selection: Vec<String>, expect: Vec<Expect>, from: Weak<Doc> },
    /// An undo (`undo`) or a redo, the selection when it was asked for, and the board that asked.
    Step { undo: bool, selection: Vec<String>, from: Weak<Doc> },
}

impl Pending {
    /// The edits it has already run on the scene.
    fn ops(&self) -> &[NodeOp] {
        match self {
            Pending::Edit { ops, .. } => ops,
            Pending::Step { .. } => &[],
        }
    }
}

/// An undo (or redo) step.
#[derive(Clone)]
struct Step {
    ops: Vec<NodeOp>,
    /// What the engine said its nodes hold once the change it undoes is made, sent back with
    /// it, so the engine leaves out what was changed since (W7, B1).
    expect: Vec<Expect>,
    /// The selection to put back with it.
    selection: Vec<String>,
    /// Which step it is, so that an answer takes off its stack only the step it ran.
    serial: u64,
}

/// An undo or redo on its way.
struct Run {
    undo: bool,
    serial: u64,
    /// The selection its step puts back.
    back: Vec<String>,
}

/// A request on its way: what went out, and what the answer is for.
struct Sent {
    ops: Vec<NodeOp>,
    expect: Vec<Expect>,
    /// The selection the step undoing this one puts back.
    keep: Vec<String>,
    /// The undo or redo step it runs; none for an edit.
    run: Option<Run>,
    from: Weak<Doc>,
}

impl Sent {
    fn payload(&self, design: i64) -> Value {
        let mut payload = json!({"design_id": design, "ops": self.ops, "reply": "full"});
        if self.run.is_some() {
            payload["lenient"] = json!(true);
        }
        if !self.expect.is_empty() {
            payload["expect"] = json!(self.expect);
        }
        payload
    }
}

/// How a request failed, for what the engine may have done with it.
#[derive(Debug)]
enum Failure {
    /// Never sent: Relay is not connected.
    Unsent,
    /// The engine refused it: none of it applied.
    Refused(Box<BusError>),
    /// The connection closed while it was out: applied or not, unknown.
    Gone(String),
    /// No answer in time, or one that did not read: applied or not, unknown.
    Unknown(String),
    /// Its answer was too large to read: it applied (B7).
    TooLarge,
}

impl From<Error> for Failure {
    fn from(e: Error) -> Failure {
        match e {
            Error::Bus(e) => Failure::Refused(e),
            Error::Disconnected | Error::Io(_) => Failure::Gone(e.to_string()),
            Error::Timeout | Error::Protocol(_) => Failure::Unknown(e.to_string()),
            Error::TooLarge(_) => Failure::TooLarge,
        }
    }
}

/// What a listener hears.
#[derive(Debug, Clone)]
pub enum Change {
    /// The scene changed under an edit of ours (or a preview).
    Scene,
    /// The design was read again after someone else changed it: these nodes differ (or sit
    /// elsewhere among their siblings), and this actor (`user` or a session) changed it.
    Reloaded { changed: HashSet<String>, by: String },
    /// The design row changed: title, terminal, deleted.
    Design,
    /// An undo or redo put back these layers' selection, as it was when the step was made.
    Selected(Vec<String>),
    /// Saving state: in flight or settled.
    Saving(bool),
    Error(String),
}

/// One design open on the canvas, shared by every board that shows it (B28).
pub struct Shared {
    pub id: i64,
    ui: Weak<Ui>,
    pub design: RefCell<Design>,
    pub scene: RefCell<Scene>,
    pub assets_dir: RefCell<String>,
    /// Where each picture came from, by `src` (`design.get`'s `credits`), for the inspector's
    /// Credit block. A credit is a fact about a file, which never changes, so a read of what
    /// changed adds to them and only a whole read replaces them.
    pub credits: RefCell<BTreeMap<String, PictureCredit>>,
    /// The `rev` our scene matches, as far as the engine has told us.
    rev: Cell<i64>,
    queue: RefCell<VecDeque<Pending>>,
    /// What was queued behind an edit the engine refused (or that could not be sent): run again
    /// on the engine's copy once it is read, rather than lost with the refused one (B9). New
    /// edits join it meanwhile, so they keep their order.
    replay: RefCell<VecDeque<Pending>>,
    sending: Cell<bool>,
    /// Someone else changed the design: read it again once nothing holds that back.
    stale: Cell<bool>,
    /// The highest `rev` an event announced since the design was last read (B78).
    seen: Cell<i64>,
    reading: Cell<bool>,
    /// Counts the engine's answers to our edits, and the edits that may have applied without
    /// one: a read asked before one of them may not hold it, and is not taken.
    answers: Cell<u64>,
    /// The engine refused the last edit sent, and none has been saved since.
    refused: Cell<bool>,
    undo: RefCell<Vec<Step>>,
    redo: RefCell<Vec<Step>>,
    steps: Cell<u64>,
    /// Who changed the design since it was last read, each with the ids its `design.changed`
    /// named (none when it did not name them all), oldest first (TM-13).
    notices: RefCell<Vec<(String, Option<Vec<String>>)>>,
    /// The rev of the last read: a change after it is noted even when an answer of ours has
    /// moved `rev` past it, since only a read shows it.
    read_rev: Cell<i64>,
    /// The next read takes the design whole: the last read's changes did not fit the scene.
    whole: Cell<bool>,
    /// The boards' docs on it.
    docs: RefCell<Vec<Weak<Doc>>>,
}

/// A board's hold on the design it shows: the [`Shared`] design itself (through `Deref`), and
/// what this board alone says and hears.
pub struct Doc {
    shared: Rc<Shared>,
    /// A gesture or text edit of this board is under way: a reread would pull the scene from
    /// under it.
    hold: Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
    /// The ids the last read found gone, for this board to take.
    removed: RefCell<Vec<String>>,
    /// The changed ids of the last read, by the agent that changed them, for this board to take.
    actors: RefCell<Vec<(String, HashSet<String>)>>,
}

impl Deref for Doc {
    type Target = Shared;
    fn deref(&self) -> &Shared {
        &self.shared
    }
}

impl Drop for Doc {
    /// A board that closes mid-gesture lets go of its hold, and a read owed meanwhile runs.
    fn drop(&mut self) {
        if self.hold.get() {
            self.shared.settle();
        }
    }
}

/// Things open by id, held weakly: each goes when the last strong reference to it does.
struct Live<T>(Vec<(i64, Weak<T>)>);

impl<T> Default for Live<T> {
    fn default() -> Self {
        Live(Vec::new())
    }
}

impl<T> Live<T> {
    fn get(&mut self, id: i64) -> Option<Rc<T>> {
        self.0.retain(|(_, w)| w.strong_count() > 0);
        self.0.iter().find(|(open, _)| *open == id).and_then(|(_, w)| w.upgrade())
    }

    fn put(&mut self, id: i64, it: &Rc<T>) {
        self.0.retain(|(_, w)| w.strong_count() > 0);
        self.0.push((id, Rc::downgrade(it)));
    }

    fn all(&mut self) -> Vec<Rc<T>> {
        self.0.retain(|(_, w)| w.strong_count() > 0);
        self.0.iter().filter_map(|(_, w)| w.upgrade()).collect()
    }
}

/// What events said of a design while its first read was out (B109).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Watch {
    /// Opens of it under way.
    opens: u32,
    /// The newest rev announced, and whether that announcement had it deleted.
    rev: i64,
    deleted: bool,
    /// Deleted for good.
    purged: bool,
}

thread_local! {
    /// Every design open on a board, or with a request of its own still out, by id (B28).
    static OPEN: RefCell<Live<Shared>> = RefCell::new(Live::default());
    /// Designs whose first read is out, with what events said of them meanwhile (B109).
    static WATCHED: RefCell<HashMap<i64, Watch>> = RefCell::new(HashMap::new());
    static TRACE: bool = std::env::var_os("RELAY_DESIGN_TRACE").is_some();
}

/// `RELAY_DESIGN_TRACE` set: print how long commits and reads take (B85).
fn trace() -> bool {
    TRACE.with(|on| *on)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| format!("unexpected answer: {e}"))
}

fn watch(id: i64) {
    WATCHED.with(|w| w.borrow_mut().entry(id).or_default().opens += 1);
}

/// Stop watching `id` for one open; what events said of it meanwhile.
fn unwatch(id: i64) -> Watch {
    WATCHED.with(|w| {
        let mut w = w.borrow_mut();
        let seen = w.get(&id).copied().unwrap_or_default();
        match w.get_mut(&id) {
            Some(e) if e.opens > 1 => e.opens -= 1,
            _ => {
                w.remove(&id);
            }
        }
        seen
    })
}

fn noticed(id: i64, rev: i64, deleted: bool, purged: bool) {
    WATCHED.with(|w| {
        if let Some(e) = w.borrow_mut().get_mut(&id) {
            e.purged |= purged;
            if rev >= e.rev {
                e.rev = rev;
                e.deleted = deleted;
            }
        }
    });
}

/// `design.changed` or `design.deleted`: to the design's doc, and to an open of it under way.
pub fn event(ev: &str, payload: &Value) {
    let id = payload["id"].as_i64().unwrap_or(0);
    let purged = ev == "design.deleted";
    noticed(id, payload["rev"].as_i64().unwrap_or(0), payload["deleted_at"].is_string(), purged);
    let Some(shared) = find(id) else {
        return;
    };
    if purged {
        shared.stop();
    } else {
        shared.event(payload);
    }
}

/// Design `id`'s doc, while a board shows it.
pub fn find(id: i64) -> Option<Rc<Shared>> {
    OPEN.with(|o| o.borrow_mut().get(id))
}

/// Read every open design again: events may have been missed while Relay was not connected,
/// or the engine may hold an older copy now (a store restored from a backup) (TM-54).
pub fn reread_all() {
    for shared in OPEN.with(|o| o.borrow_mut().all()) {
        shared.stale.set(true);
        shared.settle();
    }
}

fn deleted_text(title: &str) -> String {
    format!("“{title}” is in Deleted designs")
}

impl Doc {
    /// Design `id` for a board: the one another board shows already, else read anew.
    pub async fn open(ui: &Rc<Ui>, id: i64) -> Result<Rc<Doc>, String> {
        if let Some(shared) = find(id) {
            return Doc::opened(&shared);
        }
        watch(id);
        let started = trace().then(Instant::now);
        let answer = ui.call("design.get", json!({"design_id": id})).await;
        let missed = unwatch(id);
        let got: GetOut = decode(answer.map_err(|e| e.to_string())?)?;
        if let Some(t) = started {
            println!("DESIGN_TRACE open {:.3} ms {} nodes", ms(t.elapsed()), got.nodes.len());
        }
        // Another board opened it meanwhile: share its.
        if let Some(shared) = find(id) {
            return Doc::opened(&shared);
        }
        // A palette or gallery entry a moment old can name a design deleted since (B109).
        if missed.purged || (missed.deleted && missed.rev > got.design.rev) || got.design.deleted_at.is_some() {
            return Err(deleted_text(&got.design.title));
        }
        let shared = Shared::build(Rc::downgrade(ui), id, got);
        OPEN.with(|o| o.borrow_mut().put(id, &shared));
        // B109: a change announced while the read was out, which it may not hold.
        if missed.rev > shared.rev.get() {
            shared.seen.set(missed.rev);
            shared.stale.set(true);
            shared.settle();
        }
        Ok(Doc::of(&shared))
    }

    fn opened(shared: &Rc<Shared>) -> Result<Rc<Doc>, String> {
        if shared.design.borrow().deleted_at.is_some() {
            return Err(deleted_text(&shared.title()));
        }
        Ok(Doc::of(shared))
    }

    fn of(shared: &Rc<Shared>) -> Rc<Doc> {
        let doc = Rc::new(Doc {
            shared: shared.clone(),
            hold: Cell::new(false),
            listeners: RefCell::default(),
            removed: RefCell::default(),
            actors: RefCell::default(),
        });
        let mut docs = shared.docs.borrow_mut();
        docs.retain(|d| d.strong_count() > 0);
        docs.push(Rc::downgrade(&doc));
        doc
    }

    /// The same design for another board (a window it pops out into): its scene, queue and
    /// history, with a hold and listeners of its own.
    pub fn share(&self) -> Rc<Doc> {
        Doc::of(&self.shared)
    }

    pub fn listen(&self, f: impl Fn(&Change) + 'static) {
        self.listeners.borrow_mut().push(Rc::new(f));
    }

    fn hear(&self, change: &Change) {
        let listeners: Vec<_> = self.listeners.borrow().clone();
        for l in listeners {
            l(change);
        }
    }

    /// Run `ops` on the scene now and send them. One call is one undo step, which puts
    /// `selection` back when it is undone. They are first fitted to what each node's kind uses
    /// (`Scene::fit`), so a one-for-all edit of a mixed selection never sends what the engine's
    /// strict list refuses; an update that would change nothing is left out (B86), and an edit
    /// of more than one list takes is refused before anything runs (B44).
    pub fn commit(self: &Rc<Self>, ops: Vec<NodeOp>, selection: Vec<String>) -> bool {
        self.shared.commit(ops, selection, Rc::downgrade(self))
    }

    /// Undo the last step; `selection` is what its redo puts back.
    pub fn undo(self: &Rc<Self>, selection: Vec<String>) {
        self.shared.ask(true, selection, Rc::downgrade(self));
    }

    pub fn redo(self: &Rc<Self>, selection: Vec<String>) {
        self.shared.ask(false, selection, Rc::downgrade(self));
    }

    /// Hold re-reads while a gesture or text edit of this board is under way; releasing runs one
    /// owed, unless another board showing the design holds them too.
    pub fn set_hold(&self, hold: bool) {
        self.hold.set(hold);
        if !hold {
            self.shared.settle();
        }
    }

    /// The design row a `design.update` of this board answered with (B83).
    pub fn adopt(&self, row: Design) {
        self.shared.adopt(row);
    }

    /// The ids the last read found removed, once: an agent's deletions, which change no node
    /// that is left.
    // board.rs reads them on Change::Reloaded from native-canvas on.
    #[allow(dead_code)]
    pub fn take_removed(&self) -> Vec<String> {
        self.removed.take()
    }

    /// The nodes the last read found changed, by the agent that changed each (TM-13), once.
    // board.rs glows each agent's from native-canvas on.
    #[allow(dead_code)]
    pub fn take_actors(&self) -> Vec<(String, HashSet<String>)> {
        self.actors.take()
    }
}

/// B83: whether a `design.update` of ours answering `row_rev` leaves our scene at that rev: it
/// must be the one write since the rev the scene matches, with nothing of ours out and no read
/// owed. Otherwise the scene stays at its rev and the normal reread brings what came between.
fn adopt_rev(current: i64, row_rev: i64, idle: bool) -> bool {
    idle && row_rev == current + 1
}

impl Shared {
    fn build(ui: Weak<Ui>, id: i64, got: GetOut) -> Rc<Shared> {
        let styles = Styles { colors: got.colors, styles: got.styles };
        let rev = got.design.rev;
        Rc::new(Shared {
            id,
            ui,
            rev: Cell::new(rev),
            design: RefCell::new(got.design),
            scene: RefCell::new(Scene::new(got.nodes).with_styles(styles)),
            assets_dir: RefCell::new(got.assets_dir),
            credits: RefCell::new(got.credits),
            queue: RefCell::default(),
            replay: RefCell::default(),
            sending: Cell::new(false),
            stale: Cell::new(false),
            seen: Cell::new(0),
            reading: Cell::new(false),
            answers: Cell::new(0),
            refused: Cell::new(false),
            undo: RefCell::default(),
            redo: RefCell::default(),
            steps: Cell::new(0),
            notices: RefCell::default(),
            read_rev: Cell::new(rev),
            whole: Cell::new(false),
            docs: RefCell::default(),
        })
    }

    /// The boards' docs on it that are still open.
    fn docs(&self) -> Vec<Rc<Doc>> {
        let mut docs = self.docs.borrow_mut();
        docs.retain(|d| d.strong_count() > 0);
        docs.iter().filter_map(Weak::upgrade).collect()
    }

    /// Every board showing it hears `change`.
    fn tell(&self, change: Change) {
        for doc in self.docs() {
            doc.hear(&change);
        }
    }

    /// The board `from` is hears `change`; every board does once it has closed.
    fn tell_to(&self, from: &Weak<Doc>, change: Change) {
        match from.upgrade() {
            Some(doc) => doc.hear(&change),
            None => self.tell(change),
        }
    }

    fn held(&self) -> bool {
        self.docs.borrow().iter().any(|d| d.upgrade().is_some_and(|d| d.hold.get()))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.borrow().is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.borrow().is_empty()
    }

    /// Edits of ours are on their way, or waiting to go.
    pub fn busy(&self) -> bool {
        self.sending.get() || !self.queue.borrow().is_empty()
    }

    /// The engine refused an edit and nothing has been saved since.
    pub fn unsaved(&self) -> bool {
        self.refused.get()
    }

    /// The edits and steps held back until the engine's copy is read: while Relay is not
    /// connected, or behind a refusal (B9). Closing loses them.
    pub fn unsent(&self) -> usize {
        self.replay.borrow().len()
    }

    pub fn session(&self) -> Option<String> {
        self.design.borrow().session.clone()
    }

    pub fn title(&self) -> String {
        self.design.borrow().title.clone()
    }

    /// The app while it is connected to the engine.
    fn connected(&self) -> Option<Rc<Ui>> {
        self.ui.upgrade().filter(|ui| ui.client.borrow().is_some())
    }

    /// Read the design again if that is owed and nothing stands in the way: nothing of ours out
    /// and no gesture under way.
    fn settle(self: &Rc<Self>) {
        if self.stale.get() && !self.busy() && !self.held() {
            self.reread();
        }
    }

    fn commit(self: &Rc<Self>, mut ops: Vec<NodeOp>, selection: Vec<String>, from: Weak<Doc>) -> bool {
        if ops.is_empty() {
            return true;
        }
        let started = trace().then(Instant::now);
        // bus-1's fit first (MODEL-SPEC §1 #33), then what the scene shows of what it writes.
        self.scene.borrow().fit(&mut ops);
        if ops.is_empty() {
            return true;
        }
        let expect = expect_for(&self.scene.borrow(), &ops);
        let applied = self.scene.borrow_mut().apply_effective(ops);
        if let Some(t) = started {
            println!("DESIGN_TRACE commit {:.3} ms {} nodes", ms(t.elapsed()), self.scene.borrow().len());
        }
        let ops = match applied {
            Ok(ops) => ops,
            Err(e) => {
                self.tell_to(&from, Change::Error(e));
                return false;
            }
        };
        // Nothing changes: no step, no request, and the redo stack stays (B86).
        if ops.is_empty() {
            return true;
        }
        self.redo.borrow_mut().clear();
        let edit = Pending::Edit { expect: keep_expect(expect, &ops), ops, selection, from };
        self.tell(Change::Scene);
        if self.replay.borrow().is_empty() {
            self.queue.borrow_mut().push_back(edit);
            self.pump();
        } else {
            self.replay.borrow_mut().push_back(edit);
            self.settle();
        }
        true
    }

    fn ask(self: &Rc<Self>, undo: bool, selection: Vec<String>, from: Weak<Doc>) {
        let step = Pending::Step { undo, selection, from };
        if self.replay.borrow().is_empty() {
            self.queue.borrow_mut().push_back(step);
            self.pump();
        } else {
            self.replay.borrow_mut().push_back(step);
            self.settle();
        }
    }

    /// `design.changed` for this design.
    fn event(self: &Rc<Self>, payload: &Value) {
        let Ok(row) = serde_json::from_value::<Design>(payload.clone()) else {
            return;
        };
        let (rev, by, deleted) = (row.rev, row.updated_by.clone(), row.deleted_at.is_some());
        let row_changed = {
            let current = self.design.borrow();
            current.title != row.title || current.session != row.session || current.deleted_at != row.deleted_at
        };
        if row_changed && rev >= self.design.borrow().rev {
            *self.design.borrow_mut() = row;
            self.tell(Change::Design);
        }
        if deleted {
            self.stop();
        }
        if rev > self.read_rev.get() {
            self.note(by, ids_of(payload));
        }
        if rev <= self.rev.get() {
            return;
        }
        // Kept whatever is under way, so that a read out now, which may answer an older rev,
        // reads again (B78).
        self.seen.set(self.seen.get().max(rev));
        if !self.busy() {
            self.stale.set(true);
            self.settle();
        }
    }

    fn note(&self, by: String, ids: Option<Vec<String>>) {
        let mut notices = self.notices.borrow_mut();
        notices.push((by, ids));
        if notices.len() > NOTICES {
            notices.remove(0);
        }
    }

    /// The design was deleted: what is queued for it would only be refused.
    fn stop(&self) {
        self.queue.borrow_mut().clear();
        self.replay.borrow_mut().clear();
    }

    fn adopt(&self, row: Design) {
        let idle = !self.busy() && !self.reading.get() && !self.stale.get();
        let rev = row.rev;
        let ours = adopt_rev(self.rev.get(), rev, idle);
        let newer = rev >= self.design.borrow().rev;
        let changed = newer && *self.design.borrow() != row;
        if newer {
            *self.design.borrow_mut() = row;
        }
        if ours {
            self.rev.set(rev);
        }
        if changed {
            self.tell(Change::Design);
        }
    }

    fn pump(self: &Rc<Self>) {
        if self.sending.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.sending.set(true);
        self.tell(Change::Saving(true));
        let doc = self.clone();
        glib::spawn_future_local(async move {
            while let Some(sent) = doc.next() {
                let unsent = ui.client.borrow().is_none();
                let answer = if unsent {
                    Err(Failure::Unsent)
                } else {
                    match ui.call("design.apply", sent.payload(doc.id)).await {
                        Ok(v) => decode::<ApplyOut>(v).map_err(Failure::Unknown),
                        Err(e) => Err(Failure::from(e)),
                    }
                };
                doc.answered(sent, answer);
            }
            doc.sending.set(false);
            if doc.seen.get() > doc.rev.get() {
                doc.stale.set(true);
            }
            doc.tell(Change::Saving(false));
            doc.settle();
        });
    }

    /// The next request: an edit as it is, or the top undo or redo step, which stays on its
    /// stack until the engine has run it (B10).
    fn next(&self) -> Option<Sent> {
        loop {
            let pending = self.queue.borrow_mut().pop_front()?;
            match pending {
                Pending::Edit { ops, selection, expect, from } => return Some(Sent { ops, expect, keep: selection, run: None, from }),
                Pending::Step { undo, selection, from } => {
                    let stack = if undo { &self.undo } else { &self.redo };
                    let Some(step) = stack.borrow().last().cloned() else {
                        continue;
                    };
                    let run = Run { undo, serial: step.serial, back: step.selection };
                    return Some(Sent { ops: step.ops, expect: step.expect, keep: selection, run: Some(run), from });
                }
            }
        }
    }

    fn answered(self: &Rc<Self>, sent: Sent, answer: Result<ApplyOut, Failure>) {
        match answer {
            Ok(out) => self.applied(sent, out),
            Err(failure) => self.failed(sent, failure),
        }
    }

    /// Take the step `run` ran off its stack, if it is still on top: a new edit clears the redo
    /// stack while a redo is out.
    fn take_step(&self, run: &Run) {
        let mut stack = if run.undo { self.undo.borrow_mut() } else { self.redo.borrow_mut() };
        if stack.last().is_some_and(|s| s.serial == run.serial) {
            stack.pop();
        }
    }

    fn push_step(&self, redo: bool, ops: Vec<NodeOp>, expect: Vec<Expect>, selection: Vec<String>) {
        let serial = self.steps.get() + 1;
        self.steps.set(serial);
        let mut stack = if redo { self.redo.borrow_mut() } else { self.undo.borrow_mut() };
        stack.push(Step { ops, expect, selection, serial });
        if stack.len() > HISTORY {
            stack.remove(0);
        }
    }

    fn applied(self: &Rc<Self>, sent: Sent, out: ApplyOut) {
        self.answers.set(self.answers.get() + 1);
        self.refused.set(false);
        if out.prev_rev != self.rev.get() || out.reread {
            self.stale.set(true);
        }
        self.rev.set(out.rev);
        if let Some(run) = &sent.run {
            self.take_step(run);
        }
        let mut scene_changed = false;
        let mut kept = None;
        if let Some(run) = &sent.run {
            if !out.skipped.is_empty() {
                // The engine ran only what was not changed since: read what it made (W7, B50).
                self.stale.set(true);
                kept = Some(kept_text(run.undo, &out.skipped, &self.scene.borrow()));
            } else {
                // Not run ahead of the engine: run it now, as the engine did.
                let mut scene = self.scene.borrow_mut();
                let mut diverged = false;
                for op in &sent.ops {
                    diverged |= scene.apply(op).is_err();
                }
                if diverged {
                    self.stale.set(true);
                }
                scene_changed = true;
            }
        }
        match out.nodes {
            Some(nodes) => {
                // Not those an edit still ahead changes: the engine's copy would undo it here (B27).
                let queue = self.queue.borrow();
                let replay = self.replay.borrow();
                let ahead = queue.iter().chain(replay.iter()).flat_map(Pending::ops);
                self.scene.borrow_mut().confirm_all(nodes, ahead);
            }
            // Left out of an answer too large to send: read the design again.
            None => self.stale.set(true),
        }
        // The design's colours and styles, whole, when an edit of the list changed them; taken
        // only with nothing of ours ahead, which ran here already and may change them again.
        if (out.colors.is_some() || out.styles.is_some()) && self.queue.borrow().is_empty() && self.replay.borrow().is_empty() {
            let mut scene = self.scene.borrow_mut();
            let now = scene.styles().clone();
            scene.set_styles(Styles { colors: out.colors.unwrap_or(now.colors), styles: out.styles.unwrap_or(now.styles) });
            scene_changed = true;
        }
        // The step that undoes this one, unless nothing applied; an answer too large to carry it
        // keeps none.
        let redo = sent.run.as_ref().is_some_and(|r| r.undo);
        match out.inverse {
            Some(ops) if !ops.is_empty() => self.push_step(redo, ops, out.expect.unwrap_or_default(), sent.keep),
            Some(_) => {}
            None => self.tell_to(&sent.from, Change::Error(too_large_text(sent.run.as_ref().map(|r| r.undo)).into())),
        }
        let row_changed = match out.design {
            Some(design) => {
                let changed = {
                    let current = self.design.borrow();
                    current.title != design.title || current.session != design.session
                };
                *self.design.borrow_mut() = design;
                changed
            }
            None => {
                self.stale.set(true);
                false
            }
        };
        if scene_changed {
            self.tell(Change::Scene);
        }
        if let Some(text) = kept {
            self.tell_to(&sent.from, Change::Error(text));
        }
        if let Some(run) = sent.run {
            self.tell_to(&sent.from, Change::Selected(run.back));
        }
        if row_changed {
            self.tell(Change::Design);
        }
    }

    /// Move what is queued (after `first`) to wait for the engine's copy, and read it (B9).
    fn hold_back(&self, first: Option<Pending>) {
        let mut replay = self.replay.borrow_mut();
        replay.extend(first);
        replay.extend(self.queue.borrow_mut().drain(..));
        self.stale.set(true);
    }

    fn failed(self: &Rc<Self>, sent: Sent, failure: Failure) {
        let verb = |undo: bool| if undo { "undo" } else { "redo" };
        match failure {
            // It never left: it goes out again once the engine's copy is read (B9, B10).
            Failure::Unsent => {
                let from = sent.from.clone();
                let again = match sent.run {
                    None => Pending::Edit { ops: sent.ops, selection: sent.keep, expect: sent.expect, from: sent.from },
                    Some(run) => Pending::Step { undo: run.undo, selection: sent.keep, from: sent.from },
                };
                self.hold_back(Some(again));
                self.refused.set(true);
                self.tell_to(&from, Change::Error("Not saved yet: Relay is not connected. Your changes go out once it is back.".into()));
            }
            Failure::Refused(e) => {
                let text = match &sent.run {
                    Some(run) => {
                        self.take_step(run);
                        format!("Could not {} that step: {}. It was taken off the history.", verb(run.undo), e.message)
                    }
                    None if e.code == "design.stale" => stale_text(&e, &self.scene.borrow()),
                    None => format!("Not saved: {}", e.message),
                };
                if sent.run.is_none() {
                    self.refused.set(true);
                }
                self.hold_back(None);
                self.tell_to(&sent.from, Change::Error(text));
            }
            // It may have reached the engine: read what it has. An undo stays on its stack; what
            // is queued behind it finds the connection gone and waits (Unsent).
            Failure::Gone(said) => {
                self.answers.set(self.answers.get() + 1);
                self.stale.set(true);
                let what = match &sent.run {
                    Some(run) => format!("that {}", verb(run.undo)),
                    None => "that change".into(),
                };
                self.tell_to(&sent.from, Change::Error(format!("Could not tell whether {what} was saved: {said}")));
            }
            // It may have applied, or did (an answer too large to read): read what the engine
            // has and go on with the rest (B7). The edit keeps no step.
            Failure::Unknown(said) => {
                self.answers.set(self.answers.get() + 1);
                self.stale.set(true);
                if let Some(run) = &sent.run {
                    self.take_step(run);
                    let text = format!("Could not {} that step: {said}. It was taken off the history.", verb(run.undo));
                    self.tell_to(&sent.from, Change::Error(text));
                }
            }
            Failure::TooLarge => {
                self.answers.set(self.answers.get() + 1);
                self.stale.set(true);
                if let Some(run) = &sent.run {
                    self.take_step(run);
                }
                let text = match &sent.run {
                    None => "Saved, but it cannot be undone from here.",
                    Some(run) if run.undo => "Undone, but it cannot be redone from here.",
                    Some(_) => "Redone, but it cannot be undone again from here.",
                };
                self.tell_to(&sent.from, Change::Error(text.into()));
            }
        }
    }

    /// Read the design again, and say which nodes differ from ours: only what changed since the
    /// last read (TM-80: `since_rev`, patched into the scene in hand), or the whole design when
    /// the engine cannot answer that (too old, or an engine before it could) or the last changes
    /// did not fit.
    fn reread(self: &Rc<Self>) {
        if self.reading.get() {
            return;
        }
        // Not connected: the read waits for the connection (reread_all).
        let Some(ui) = self.connected() else {
            self.stale.set(true);
            return;
        };
        self.reading.set(true);
        let asked = self.answers.get();
        let doc = self.clone();
        let since = (!self.whole.replace(false)).then(|| self.read_rev.get()).filter(|rev| *rev > 0);
        glib::spawn_future_local(async move {
            let started = trace().then(Instant::now);
            let delta = match since {
                Some(rev) => ui.call("design.get", json!({"design_id": doc.id, "since_rev": rev})).await.ok()
                    .and_then(|v| decode::<GetOut>(v).ok())
                    .filter(|got| got.removed.is_some() && got.indexes.as_ref().is_some_and(|i| i.len() == got.nodes.len())),
                None => None,
            };
            let got = match delta {
                Some(got) => Ok(got),
                None => ui.call("design.get", json!({"design_id": doc.id})).await.map_err(|e| e.to_string()).and_then(decode::<GetOut>),
            };
            let fetched = started.map(|t| (t, t.elapsed()));
            doc.read(got, asked, fetched);
        });
    }

    /// A read asked when `asked` answers had come back: take it, run what waits on it, and say
    /// what changed.
    fn read(self: &Rc<Self>, got: Result<GetOut, String>, asked: u64, fetched: Option<(Instant, Duration)>) {
        self.reading.set(false);
        let got = match got {
            Ok(got) => got,
            Err(error) => {
                // Read again on the next answer, release, event or connection.
                self.stale.set(true);
                self.tell(Change::Error(error));
                return;
            }
        };
        if self.busy() || self.held() || self.answers.get() != asked {
            // Ours went out or were answered meanwhile, so it may not hold them; or a gesture
            // holds the scene. Read again when nothing does.
            self.stale.set(true);
            self.settle();
            return;
        }
        let decoded = fetched.map(|(t, _)| t.elapsed());
        // The engine holds an older copy than ours (a store restored from a backup): the
        // history no longer fits it (TM-54).
        let back = got.design.rev < self.rev.get();
        // An event announced a rev this read does not hold yet (B78).
        let owed = !back && self.seen.get() > got.design.rev;
        if !owed {
            self.seen.set(0);
        }
        let styles = Styles { colors: got.colors, styles: got.styles };
        let delta = got.removed.is_some() && got.indexes.is_some();
        let mut scene = match (got.removed, got.indexes) {
            // What changed since the last read, laid into the scene in hand.
            (Some(mut gone), Some(indexes)) => {
                gone.extend(got.left);
                let mut scene = self.scene.borrow().clone();
                scene.set_styles(styles);
                if let Err(why) = scene.patch(got.nodes.into_iter().zip(indexes).collect(), &gone) {
                    if trace() {
                        println!("DESIGN_TRACE reread delta did not fit ({why}); reading it whole");
                    }
                    self.whole.set(true);
                    self.stale.set(true);
                    self.settle();
                    return;
                }
                scene
            }
            _ => Scene::new(got.nodes).with_styles(styles),
        };
        self.stale.set(owed);
        // What was held back behind a refusal runs again on the engine's copy (B9).
        let (again, dropped) = rebase(&mut scene, self.replay.take());
        let built = fetched.map(|(t, _)| t.elapsed());
        let old = self.scene.replace(scene);
        let (changed, removed) = diff(&old, &self.scene.borrow());
        let nodes = self.scene.borrow().len();
        drop(old);
        let (by, actors) = credit(&self.notices.take(), &changed, &got.design.updated_by);
        if back {
            self.undo.borrow_mut().clear();
            self.redo.borrow_mut().clear();
        }
        self.rev.set(got.design.rev);
        self.read_rev.set(got.design.rev);
        *self.design.borrow_mut() = got.design;
        *self.assets_dir.borrow_mut() = got.assets_dir;
        if delta {
            self.credits.borrow_mut().extend(got.credits);
        } else {
            *self.credits.borrow_mut() = got.credits;
        }
        for doc in self.docs() {
            *doc.removed.borrow_mut() = removed.clone();
            *doc.actors.borrow_mut() = actors.clone();
        }
        self.queue.borrow_mut().extend(again);
        if let (Some((t, fetch)), Some(decoded), Some(built)) = (fetched, decoded, built) {
            let total = t.elapsed();
            println!(
                "DESIGN_TRACE reread {:.3} ms {nodes} nodes fetch {:.3} decode {:.3} build {:.3} diff {:.3}",
                ms(total),
                ms(fetch),
                ms(decoded - fetch),
                ms(built - decoded),
                ms(total - built)
            );
        }
        self.tell(Change::Reloaded { changed, by });
        self.tell(Change::Design);
        if back {
            self.tell(Change::Error("This design went back to an earlier copy, so its undo history was cleared.".into()));
        }
        if dropped > 0 {
            self.tell(Change::Error(dropped_text(dropped)));
        }
        if self.queue.borrow().is_empty() {
            self.settle();
        } else {
            self.pump();
        }
    }
}

/// `expect` narrowed to what `ops` still write: the fields their updates set and the parents of
/// the nodes they move. An update left out as changing nothing (B86) expects nothing, so the
/// engine never refuses the list over a field it does not write.
fn keep_expect(expect: Vec<Expect>, ops: &[NodeOp]) -> Vec<Expect> {
    let canonical = |key: &str| match key {
        "width" => "w".to_string(),
        "height" => "h".to_string(),
        key => key.to_string(),
    };
    let mut sets: HashMap<&str, HashSet<String>> = HashMap::new();
    let mut moved: HashSet<&str> = HashSet::new();
    for op in ops {
        match op {
            NodeOp::Update { id, set } => sets.entry(id).or_default().extend(set.keys().map(|k| canonical(k))),
            NodeOp::Move { id, .. } => {
                moved.insert(id);
            }
            _ => {}
        }
    }
    expect
        .into_iter()
        .filter_map(|mut e| {
            let keys = sets.get(e.id.as_str());
            if let Some(set) = e.set.as_mut() {
                set.retain(|key, _| keys.is_some_and(|keys| keys.contains(key)));
            }
            e.set = e.set.filter(|set| !set.is_empty());
            if !moved.contains(e.id.as_str()) {
                e.parent = None;
            }
            (e.set.is_some() || e.parent.is_some() || e.within.is_some()).then_some(e)
        })
        .collect()
}

/// Run what was held back on `scene`, the engine's copy, in order: each edit that still applies
/// (with what this scene shows of what it writes), each undo and redo as it was. How many edits
/// no longer applied.
fn rebase(scene: &mut Scene, held: VecDeque<Pending>) -> (Vec<Pending>, usize) {
    let mut again = Vec::with_capacity(held.len());
    let mut dropped = 0;
    for pending in held {
        match pending {
            Pending::Edit { ops, selection, from, .. } => {
                let expect = expect_for(scene, &ops);
                match scene.apply_effective(ops) {
                    Ok(ops) if ops.is_empty() => {}
                    Ok(ops) => again.push(Pending::Edit { expect: keep_expect(expect, &ops), ops, selection, from }),
                    Err(_) => dropped += 1,
                }
            }
            step => again.push(step),
        }
    }
    (again, dropped)
}

/// Every node of `scene`: the pages, then what is on each.
fn ids(scene: &Scene) -> Vec<String> {
    let mut out: Vec<String> = scene.pages().to_vec();
    for page in scene.pages() {
        out.extend(scene.subtree(page).into_iter().skip(1));
    }
    out
}

/// What a read changed from `old` to `new`: the nodes that draw differently, and those that sit
/// elsewhere among the same siblings, which no field of theirs says (TM-12); and the ids gone.
fn diff(old: &Scene, new: &Scene) -> (HashSet<String>, Vec<String>) {
    let now = ids(new);
    let mut changed: HashSet<String> = now.iter().filter(|id| old.node(id) != new.node(id)).cloned().collect();
    for parent in std::iter::once("").chain(now.iter().map(String::as_str)) {
        let (was, is) = (old.children(parent), new.children(parent));
        if was != is {
            changed.extend(reordered(was, is));
        }
    }
    let removed = ids(old).into_iter().filter(|id| new.node(id).is_none()).collect();
    (changed, removed)
}

/// The ids of `now` that left the order the others keep from `was`: those off a longest run in
/// the same order in both. Ids in only one of them (added, removed, moved in or out) are not.
fn reordered(was: &[String], now: &[String]) -> Vec<String> {
    let place: HashMap<&str, usize> = was.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
    let both: Vec<(&String, usize)> = now.iter().filter_map(|id| Some((id, *place.get(id.as_str())?))).collect();
    // The longest run of rising old places, by patience: the last of each length, with links.
    let mut tails: Vec<usize> = Vec::new();
    let mut link: Vec<Option<usize>> = vec![None; both.len()];
    for (i, &(_, at)) in both.iter().enumerate() {
        let k = tails.partition_point(|&t| both[t].1 < at);
        if k > 0 {
            link[i] = Some(tails[k - 1]);
        }
        if k == tails.len() {
            tails.push(i);
        } else {
            tails[k] = i;
        }
    }
    let mut stay = vec![false; both.len()];
    let mut at = tails.last().copied();
    while let Some(i) = at {
        stay[i] = true;
        at = link[i];
    }
    both.iter().zip(stay).filter(|(_, stay)| !stay).map(|((id, _), _)| (*id).clone()).collect()
}

/// The node ids a `design.changed` names (`touched` and `removed`), when it names them all.
fn ids_of(payload: &Value) -> Option<Vec<String>> {
    if payload["all"].as_bool() == Some(true) {
        return None;
    }
    let (touched, removed) = (payload["touched"].as_array(), payload["removed"].as_array());
    if touched.is_none() && removed.is_none() {
        return None;
    }
    Some(touched.into_iter().chain(removed).flatten().filter_map(|v| v.as_str().map(String::from)).collect())
}

/// Who changed what since the last read (TM-13), from the notices of that time: the actor the
/// read is credited to (the last agent that changed the design, else `updated_by`, so that the
/// person's own edit landing after an agent's does not hide the agent's), and the `changed`
/// ids by agent. A notice naming its ids takes those; an id no notice names goes to the last
/// agent. Without ids, every changed node is the last agent's (or `updated_by`'s). The person's
/// own changes (`user`) are nobody's to undo here.
fn credit(notices: &[(String, Option<Vec<String>>)], changed: &HashSet<String>, updated_by: &str) -> (String, Vec<(String, HashSet<String>)>) {
    let last_agent = notices.iter().rev().map(|(by, _)| by).find(|by| *by != "user");
    let by = last_agent.cloned().unwrap_or_else(|| updated_by.to_string());
    let named = notices.iter().any(|(_, ids)| ids.is_some());
    let mut last: HashMap<&str, &str> = HashMap::new();
    for (who, ids) in notices {
        for id in ids.iter().flatten() {
            last.insert(id, who);
        }
    }
    let mut sorted: Vec<&String> = changed.iter().collect();
    sorted.sort();
    let mut out: Vec<(String, HashSet<String>)> = Vec::new();
    for id in sorted {
        let who = match named {
            true => last.get(id.as_str()).copied().or(last_agent.map(String::as_str)),
            false => Some(by.as_str()),
        };
        let Some(who) = who.filter(|who| *who != "user") else {
            continue;
        };
        match out.iter_mut().find(|(w, _)| w == who) {
            Some((_, ids)) => {
                ids.insert(id.clone());
            }
            None => out.push((who.to_string(), HashSet::from([id.clone()]))),
        }
    }
    (by, out)
}

/// A node's name as the layers show it, or its id once it is gone.
fn name_of(scene: &Scene, id: &str) -> String {
    scene.node(id).map(display_name).unwrap_or_else(|| id.to_string())
}

/// "a, b and c", or the first three and how many more.
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        _ if items.len() > 3 => format!("{} and {} more", items[..3].join(", "), items.len() - 3),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// W8: an edit refused because what it writes changed since the scene it was made on: "Not
/// saved: Button's fill changed just before", or for several "Button (fill) and Card (place)".
fn stale_text(e: &BusError, scene: &Scene) -> String {
    let conflicts: Vec<Conflict> = e.details.as_ref().and_then(|d| serde_json::from_value(d["conflicts"].clone()).ok()).unwrap_or_default();
    let conflicts: Vec<&Conflict> = conflicts.iter().filter(|c| c.reason != ConflictReason::ParentSkipped).collect();
    let what = |c: &Conflict| match c.reason {
        ConflictReason::Fields if !c.fields.is_empty() => Some(c.fields.join(", ")),
        ConflictReason::Parent => Some("place".to_string()),
        ConflictReason::Inside => Some("new layers inside".to_string()),
        _ => None,
    };
    let said = match conflicts.as_slice() {
        [] => return format!("Not saved: {}. Showing the newer version.", e.message),
        [one] if one.reason == ConflictReason::Fields && !one.fields.is_empty() => format!("{}'s {}", name_of(scene, &one.id), listed(&one.fields)),
        [one] => name_of(scene, &one.id),
        many => {
            let parts: Vec<String> = many
                .iter()
                .map(|c| match what(c) {
                    Some(what) => format!("{} ({what})", name_of(scene, &c.id)),
                    None => name_of(scene, &c.id),
                })
                .collect();
            listed(&parts)
        }
    };
    format!("Not saved: {said} changed just before. Showing the newer version.")
}

/// W7, B50: an undo or redo the engine ran without what was changed since.
fn kept_text(undo: bool, skipped: &[Conflict], scene: &Scene) -> String {
    let items: Vec<String> = skipped
        .iter()
        .filter_map(|c| {
            let why = match c.reason {
                ConflictReason::ParentSkipped => return None,
                ConflictReason::Fields if !c.fields.is_empty() => c.fields.join(", "),
                ConflictReason::Inside => "new layers inside".into(),
                ConflictReason::Parent => "moved".into(),
                ConflictReason::Missing => "deleted".into(),
                ConflictReason::NoParent => "its place is gone".into(),
                ConflictReason::Exists => "its id is in use".into(),
                ConflictReason::TooDeep => "too deep".into(),
                ConflictReason::Unreadable => "from a newer Relay".into(),
                ConflictReason::Fields | ConflictReason::Changed => "changed".into(),
            };
            Some(format!("{} ({why})", name_of(scene, &c.id)))
        })
        .collect();
    let what = if undo { "Undo" } else { "Redo" };
    match items.len() {
        0 => format!("{what} kept what was changed since"),
        1 => format!("{what} kept 1 later change: {}", items[0]),
        n => format!("{what} kept {n} later changes: {}", listed(&items)),
    }
}

/// A step whose undo the engine's answer was too large to carry: `None` an edit, `Some(true)`
/// an undo, `Some(false)` a redo.
fn too_large_text(undo: Option<bool>) -> &'static str {
    match undo {
        None => "Saved. This change is too large to undo.",
        Some(true) => "Undone. This step is too large to redo.",
        Some(false) => "Redone. This step is too large to undo again.",
    }
}

fn dropped_text(n: usize) -> String {
    match n {
        1 => "1 later edit no longer applied and was dropped".into(),
        n => format!("{n} later edits no longer applied and were dropped"),
    }
}

#[cfg(test)]
mod tests {
    //! A doc with no app behind it sends nothing and reads nothing: `pump` and `reread` find no
    //! connection and return, so each test plays the engine itself, through `next`, `answered`,
    //! `event` and `read`, as the futures would.
    use super::*;
    use relay_bus::design::{Node, NodeKind, MAX_OPS};
    use relay_bus::ErrorKind;

    fn node(id: &str, parent: Option<&str>, kind: NodeKind, x: f64) -> Node {
        let mut n = Node::new(kind);
        n.id = id.into();
        n.parent = parent.map(Into::into);
        (n.x, n.w, n.h) = (x, 50.0, 50.0);
        n
    }

    fn named(mut n: Node, name: &str) -> Node {
        n.name = Some(name.into());
        n
    }

    /// p1 › card (frame) › button, label (rects); p1 › dot (ellipse).
    fn nodes() -> Vec<Node> {
        vec![
            node("p1", None, NodeKind::Page, 0.0),
            named(node("card", Some("p1"), NodeKind::Frame, 0.0), "Card"),
            named(node("button", Some("card"), NodeKind::Rect, 10.0), "Button"),
            named(node("label", Some("card"), NodeKind::Rect, 100.0), "Label"),
            node("dot", Some("p1"), NodeKind::Ellipse, 500.0),
        ]
    }

    fn row(rev: i64) -> Design {
        Design {
            id: 1,
            project_id: 1,
            session: None,
            title: "Home".into(),
            rev,
            pages: 1,
            nodes: 4,
            created_by: "user".into(),
            updated_by: "user".into(),
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        }
    }

    fn got(rev: i64, nodes: Vec<Node>) -> GetOut {
        GetOut { design: row(rev), nodes, assets_dir: String::new(), removed: None, indexes: None, left: Vec::new(), colors: Default::default(), styles: Default::default(), credits: Default::default() }
    }

    fn describe(change: &Change) -> String {
        match change {
            Change::Scene => "scene".into(),
            Change::Reloaded { changed, by } => {
                let mut changed: Vec<&str> = changed.iter().map(String::as_str).collect();
                changed.sort();
                format!("reloaded by {by}: {}", changed.join(" "))
            }
            Change::Design => "design".into(),
            Change::Selected(ids) => format!("selected {}", ids.join(" ")),
            Change::Saving(on) => format!("saving {on}"),
            Change::Error(e) => format!("said {e}"),
        }
    }

    type Heard = Rc<RefCell<Vec<String>>>;

    fn listen(doc: &Doc) -> Heard {
        let heard: Heard = Rc::default();
        let into = heard.clone();
        doc.listen(move |change| into.borrow_mut().push(describe(change)));
        heard
    }

    /// The design at `rev`, open on one board, and what that board hears.
    fn open(rev: i64) -> (Rc<Shared>, Rc<Doc>, Heard) {
        let shared = Shared::build(Weak::new(), 1, got(rev, nodes()));
        let doc = Doc::of(&shared);
        let heard = listen(&doc);
        (shared, doc, heard)
    }

    fn set(id: &str, set: Value) -> NodeOp {
        NodeOp::Update { id: id.into(), set: set.as_object().expect("an object").clone() }
    }

    fn x(shared: &Shared, id: &str) -> f64 {
        shared.scene.borrow().node(id).expect("in the scene").x
    }

    /// The engine's node: the scene's, at `x`.
    fn at(shared: &Shared, id: &str, x: f64) -> Node {
        let mut n = shared.scene.borrow().stored(id).expect("in the scene").clone();
        n.x = x;
        n
    }

    fn answer(prev_rev: i64, rev: i64, nodes: Vec<Node>, inverse: Option<Vec<NodeOp>>) -> ApplyOut {
        ApplyOut {
            design_id: 1,
            rev,
            prev_rev,
            rebased: false,
            ids: Vec::new(),
            removed_count: 0,
            audit_id: None,
            skipped: Vec::new(),
            warnings: Vec::new(),
            copied: Default::default(),
            design: Some(row(rev)),
            nodes: Some(nodes),
            removed: Some(Vec::new()),
            inverse,
            expect: None,
            colors: None,
            styles: None,
            reread: false,
        }
    }

    fn said(heard: &Heard) -> Vec<String> {
        heard.borrow().iter().filter_map(|h| h.strip_prefix("said ").map(String::from)).collect()
    }

    #[test]
    fn open_things_are_found_by_id_while_something_holds_them() {
        let mut live: Live<u8> = Live::default();
        let one = Rc::new(1u8);
        live.put(5, &one);
        assert!(Rc::ptr_eq(&live.get(5).expect("open"), &one));
        assert!(live.get(6).is_none(), "another id");
        assert_eq!(live.all().len(), 1);
        drop(one);
        assert!(live.get(5).is_none() && live.0.is_empty(), "gone with its last holder, and pruned");
    }

    /// B28: a window popped out of a board, or opened beside it, is the same design: one scene,
    /// one queue and one history, each board hearing what changes.
    #[test]
    fn every_board_on_a_design_shares_its_scene_and_history() {
        let (shared, board, _) = open(3);
        let window = board.share();
        let in_window = listen(&window);
        assert!(Rc::ptr_eq(&board.shared, &window.shared));
        assert!(board.commit(vec![set("button", json!({"x": 20.0}))], vec![]));
        assert_eq!(window.scene.borrow().node("button").unwrap().x, 20.0);
        assert_eq!(in_window.borrow().as_slice(), ["scene"], "the window draws the board's edit");
        let sent = shared.next().unwrap();
        shared.answered(sent, Ok(answer(3, 4, vec![at(&shared, "button", 20.0)], Some(vec![set("button", json!({"x": 10.0}))]))));
        assert!(window.can_undo(), "the window can undo what the board did");
        drop(board);
        assert_eq!(shared.docs().len(), 1, "the board's listeners went with it");
        shared.tell(Change::Design);
        assert_eq!(in_window.borrow().last().unwrap(), "design");
    }

    /// B28: a gesture holds rereads back on its board alone; a board's repeated holds and one
    /// release let go, and a board closed mid-gesture holds nothing.
    #[test]
    fn a_hold_is_each_boards_own() {
        let (shared, board, _) = open(3);
        let window = board.share();
        board.set_hold(true);
        board.set_hold(true);
        window.set_hold(true);
        board.set_hold(false);
        assert!(shared.held(), "the window still holds");
        window.set_hold(false);
        assert!(!shared.held());
        window.set_hold(true);
        drop(window);
        assert!(!shared.held(), "a board closed mid-gesture holds nothing");
    }

    /// B86: an edit that changes nothing sends nothing, keeps no step and keeps the redo stack;
    /// the expectations sent are only those of what goes out (W8).
    #[test]
    fn an_edit_that_changes_nothing_is_not_sent() {
        let (shared, doc, heard) = open(3);
        shared.push_step(true, vec![set("dot", json!({"x": 400.0}))], Vec::new(), Vec::new());
        assert!(doc.commit(vec![set("button", json!({"x": 10.0}))], vec![]));
        assert!(shared.queue.borrow().is_empty() && shared.can_redo(), "no request, and the redo is kept");
        assert!(heard.borrow().is_empty());
        assert!(doc.commit(vec![set("button", json!({"x": 10.0})), set("dot", json!({"fill": "#ff0000"}))], vec!["dot".into()]));
        assert!(!shared.can_redo(), "a real edit clears the redo stack");
        let sent = shared.next().unwrap();
        assert_eq!(sent.ops, vec![set("dot", json!({"fill": "#ff0000"}))]);
        let payload = sent.payload(1);
        assert_eq!(payload["reply"], "full");
        assert!(payload.get("lenient").is_none(), "an edit is strict");
        assert_eq!(payload["expect"], json!([{"id": "dot", "set": {"fill": null}}]), "what the scene showed of what it writes");
    }

    /// B44: an edit of more edits than one list takes is refused before anything runs.
    #[test]
    fn an_edit_past_one_list_is_refused_whole() {
        let (shared, doc, heard) = open(3);
        let ops: Vec<NodeOp> = (0..=MAX_OPS).map(|i| set("button", json!({"x": 11.0 + i as f64}))).collect();
        assert!(!doc.commit(ops, vec![]));
        assert_eq!(x(&shared, "button"), 10.0, "nothing ran");
        assert!(shared.queue.borrow().is_empty());
        assert_eq!(said(&heard), [format!("that is {} changes at once; one edit makes at most {MAX_OPS}. Select fewer layers.", MAX_OPS + 1)]);
    }

    /// B27: the engine's copy of a node a queued edit changes again is not taken: it would put
    /// the node back where the second edit moved it from.
    #[test]
    fn an_answer_leaves_what_a_queued_edit_changes() {
        let (shared, doc, _) = open(3);
        doc.commit(vec![set("button", json!({"x": 11.0}))], vec![]);
        doc.commit(vec![set("button", json!({"x": 12.0}))], vec![]);
        let sent = shared.next().unwrap();
        let mut dot = at(&shared, "dot", 500.0);
        dot.fill = Some("#00ff00".into());
        shared.answered(sent, Ok(answer(3, 4, vec![at(&shared, "button", 11.0), dot], Some(vec![set("button", json!({"x": 10.0}))]))));
        assert_eq!(x(&shared, "button"), 12.0, "the second edit still shows");
        assert_eq!(shared.scene.borrow().node("dot").unwrap().fill.as_deref(), Some("#00ff00"), "what nothing ahead changes is confirmed");
        let sent = shared.next().unwrap();
        shared.answered(sent, Ok(answer(4, 5, vec![at(&shared, "button", 12.0)], Some(vec![set("button", json!({"x": 11.0}))]))));
        assert_eq!((x(&shared, "button"), shared.undo.borrow().len(), shared.rev.get()), (12.0, 2, 5));
        assert!(!shared.stale.get(), "both answers followed on from our rev");
    }

    /// B10: an undo leaves its stack only once the engine ran it. Never sent, it waits for the
    /// connection; refused, it is taken off and said; run, it moves to the redo stack.
    #[test]
    fn an_undo_leaves_its_stack_only_once_run() {
        let (shared, doc, heard) = open(3);
        shared.push_step(false, vec![set("button", json!({"x": 30.0}))], Vec::new(), vec!["button".into()]);
        doc.undo(vec!["dot".into()]);
        let sent = shared.next().unwrap();
        assert_eq!(shared.undo.borrow().len(), 1, "still on its stack while out");
        assert_eq!(sent.payload(1)["lenient"], true);
        shared.answered(sent, Err(Failure::Unsent));
        assert_eq!(shared.undo.borrow().len(), 1);
        assert!(matches!(shared.replay.borrow().front(), Some(Pending::Step { undo: true, .. })), "it goes out once Relay is back");
        assert!(shared.unsent() == 1 && shared.unsaved(), "closing now would lose it");
        // Back: read, then it goes out.
        shared.read(Ok(got(3, nodes())), 0, None);
        let sent = shared.next().unwrap();
        let refusal = BusError::new(ErrorKind::Invalid, "design.too_large", "the design would pass 1536 KiB");
        shared.answered(sent, Err(Failure::Refused(Box::new(refusal))));
        assert!(!shared.can_undo(), "a refused step is taken off");
        assert!(said(&heard).last().unwrap().ends_with("the design would pass 1536 KiB. It was taken off the history."));
        shared.push_step(false, vec![set("button", json!({"x": 30.0}))], Vec::new(), vec!["button".into()]);
        doc.undo(vec!["dot".into()]);
        let sent = shared.next().unwrap();
        shared.answered(sent, Ok(answer(3, 4, vec![at(&shared, "button", 30.0)], Some(vec![set("button", json!({"x": 10.0}))]))));
        assert_eq!(x(&shared, "button"), 30.0, "run on the scene as the engine ran it");
        assert!(!shared.can_undo() && shared.redo.borrow()[0].selection == ["dot"]);
        assert_eq!(heard.borrow().last().unwrap(), "selected button");
    }

    /// B10: a redo whose stack a new edit cleared while it was out takes nothing off it.
    #[test]
    fn a_redo_answered_after_an_edit_takes_nothing_off() {
        let (shared, doc, _) = open(3);
        shared.push_step(true, vec![set("button", json!({"x": 30.0}))], Vec::new(), Vec::new());
        doc.redo(Vec::new());
        let sent = shared.next().unwrap();
        doc.commit(vec![set("dot", json!({"x": 1.0}))], vec![]);
        shared.push_step(true, vec![set("dot", json!({"x": 2.0}))], Vec::new(), Vec::new());
        shared.answered(sent, Ok(answer(3, 4, vec![], Some(vec![set("button", json!({"x": 10.0}))]))));
        assert_eq!(shared.redo.borrow().len(), 1, "the step pushed since stays");
    }

    /// B9: the edits queued behind a refused one run again on the engine's copy; those that no
    /// longer apply are dropped and counted, and new edits wait their turn behind them.
    #[test]
    fn a_refusal_keeps_the_edits_behind_it() {
        let (shared, doc, heard) = open(3);
        doc.commit(vec![set("button", json!({"x": 20.0}))], vec![]);
        doc.commit(vec![set("dot", json!({"x": 600.0}))], vec![]);
        doc.commit(vec![set("label", json!({"x": 120.0}))], vec![]);
        let sent = shared.next().unwrap();
        let refusal = BusError::new(ErrorKind::NotFound, "design.node_not_found", "no node \"button\"");
        shared.answered(sent, Err(Failure::Refused(Box::new(refusal))));
        assert!(shared.queue.borrow().is_empty() && shared.replay.borrow().len() == 2 && shared.unsent() == 2);
        assert!(shared.unsaved() && shared.stale.get());
        assert_eq!(said(&heard), ["Not saved: no node \"button\""]);
        doc.commit(vec![set("dot", json!({"y": 7.0}))], vec![]);
        assert_eq!(shared.replay.borrow().len(), 3, "a new edit waits behind them");
        // The engine's copy: an agent deleted the label.
        let engine: Vec<Node> = nodes().into_iter().filter(|n| n.id != "label").collect();
        shared.read(Ok(got(5, engine)), 0, None);
        assert_eq!((x(&shared, "button"), x(&shared, "dot")), (10.0, 600.0), "the refused edit is not run again; the next is");
        assert!(shared.scene.borrow().node("label").is_none());
        let queued: Vec<Vec<NodeOp>> = shared.queue.borrow().iter().map(|p| p.ops().to_vec()).collect();
        assert_eq!(queued, [vec![set("dot", json!({"x": 600.0}))], vec![set("dot", json!({"y": 7.0}))]]);
        assert_eq!(said(&heard).last().unwrap(), "1 later edit no longer applied and was dropped");
        // What they expect is what the engine's copy shows.
        let expect = match shared.queue.borrow().front() {
            Some(Pending::Edit { expect, .. }) => json!(expect),
            _ => panic!("an edit"),
        };
        assert_eq!(expect, json!([{"id": "dot", "set": {"x": 500.0}}]));
    }

    /// B78: a rev announced while a read is out, which the read does not hold, is read again;
    /// a read that holds it settles.
    #[test]
    fn a_change_announced_during_a_read_is_read_again() {
        let (shared, _doc, _) = open(3);
        shared.reading.set(true);
        shared.event(&json!(row(5)));
        assert_eq!(shared.seen.get(), 5);
        shared.read(Ok(got(4, nodes())), 0, None);
        assert_eq!(shared.rev.get(), 4);
        assert!(shared.stale.get() && shared.seen.get() == 5, "rev 5 is owed");
        shared.read(Ok(got(5, nodes())), 0, None);
        assert!(!shared.stale.get() && shared.seen.get() == 0);
    }

    /// TM-80: a read of what changed since the last one is laid into the scene in hand: changed
    /// nodes in their places, removed ones and those that left it out; one that does not fit is
    /// not taken, and the next read is whole.
    #[test]
    fn a_read_of_what_changed_is_patched_in() {
        let (shared, doc, heard) = open(3);
        let mut moved = node("button", Some("card"), NodeKind::Rect, 40.0);
        moved.name = Some("Button".into());
        let mut delta = got(4, vec![moved]);
        (delta.removed, delta.indexes) = (Some(vec!["dot".into()]), Some(vec![1]));
        shared.read(Ok(delta), 0, None);
        assert_eq!((x(&shared, "button"), shared.rev.get(), shared.read_rev.get()), (40.0, 4, 4));
        assert_eq!(shared.scene.borrow().children("card"), ["label", "button"], "at its index");
        assert!(shared.scene.borrow().node("dot").is_none() && !shared.stale.get());
        assert_eq!(heard.borrow()[0], "reloaded by user: button label", "label's place changed too");
        assert_eq!(doc.take_removed(), ["dot"]);
        // A delta that does not fit (a parent the scene does not hold) is not taken.
        let mut delta = got(5, vec![node("lost", Some("nowhere"), NodeKind::Rect, 0.0)]);
        (delta.removed, delta.indexes) = (Some(vec![]), Some(vec![0]));
        shared.read(Ok(delta), 0, None);
        assert_eq!(shared.rev.get(), 4, "nothing of it taken");
        assert!(shared.stale.get() && shared.whole.get(), "read again, whole");
    }

    /// pictures-SPEC-v2 §13.1: the credits come with the design; a read of what changed adds the
    /// ones it carries, and a whole read replaces them.
    #[test]
    fn credits_come_with_each_read() {
        let credit = |id: &str| PictureCredit {
            source: relay_bus::ops::picture::PictureSource::Openverse,
            id: id.into(),
            title: "Lake".into(),
            creator: Some("Ann".into()),
            creator_url: None,
            licence: "CC BY 2.0".into(),
            licence_url: None,
            source_url: None,
            attribution: "\"Lake\" by Ann, CC BY 2.0".into(),
            caption: "Photo: Ann, CC BY 2.0".into(),
        };
        let mut first = got(3, nodes());
        first.credits.insert("a.png".into(), credit("openverse:a"));
        let shared = Shared::build(Weak::new(), 1, first);
        let _doc = Doc::of(&shared);
        assert_eq!(shared.credits.borrow()["a.png"].id, "openverse:a");
        let mut delta = got(4, vec![]);
        (delta.removed, delta.indexes) = (Some(vec![]), Some(vec![]));
        delta.credits.insert("b.png".into(), credit("openverse:b"));
        shared.read(Ok(delta), 0, None);
        assert_eq!(shared.credits.borrow().keys().collect::<Vec<_>>(), ["a.png", "b.png"], "a delta adds");
        shared.read(Ok(got(5, nodes())), 0, None);
        assert!(shared.credits.borrow().is_empty(), "a whole read replaces");
    }

    /// A read asked before an answer of ours landed may not hold it: it is not taken.
    #[test]
    fn a_read_older_than_our_answer_is_not_taken() {
        let (shared, doc, heard) = open(3);
        let asked = shared.answers.get();
        doc.commit(vec![set("button", json!({"x": 20.0}))], vec![]);
        let sent = shared.next().unwrap();
        shared.answered(sent, Ok(answer(3, 4, vec![at(&shared, "button", 20.0)], Some(vec![set("button", json!({"x": 10.0}))]))));
        shared.read(Ok(got(3, nodes())), asked, None);
        assert_eq!((x(&shared, "button"), shared.rev.get()), (20.0, 4));
        assert!(shared.stale.get(), "read again");
        assert!(!heard.borrow().iter().any(|h| h.starts_with("reloaded")));
    }

    /// TM-12: an agent's deletions and reorders change no field of what is left; the read says
    /// what went and which layers moved among their siblings.
    #[test]
    fn a_read_says_what_was_removed_and_reordered() {
        let (shared, doc, heard) = open(3);
        let mut engine = nodes();
        engine.retain(|n| n.id != "dot");
        engine.swap(2, 3);
        shared.read(Ok(got(4, engine)), 0, None);
        assert_eq!(shared.scene.borrow().children("card"), ["label", "button"]);
        assert_eq!(heard.borrow()[0], "reloaded by user: label", "the two swapped: one of them moved");
        assert_eq!(doc.take_removed(), ["dot"]);
        assert!(doc.take_removed().is_empty(), "taken once");
        assert!(reordered(&[], &[]).is_empty());
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(reordered(&ids(&["a", "b", "c", "d"]), &ids(&["b", "c", "d", "a"])), ["a"]);
        assert_eq!(reordered(&ids(&["a", "b", "c"]), &ids(&["c", "a", "b"])), ["c"]);
        assert_eq!(reordered(&ids(&["a", "b", "c"]), &ids(&["x", "a", "c"])), Vec::<String>::new(), "added and removed are not moves");
    }

    /// TM-13: each change goes to the agent that made it, not to whoever wrote last.
    #[test]
    fn changes_are_credited_to_who_made_them() {
        let changed: HashSet<String> = ["a", "b", "c", "d"].map(String::from).into();
        let ids = |list: &[&str]| Some(list.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let by = |list: &[(&str, &[&str])]| -> Vec<(String, HashSet<String>)> {
            list.iter().map(|(who, ids)| (who.to_string(), ids.iter().map(|s| s.to_string()).collect())).collect()
        };
        // No notice: the design's last writer, as before.
        assert_eq!(credit(&[], &changed, "agent-a"), ("agent-a".into(), by(&[("agent-a", &["a", "b", "c", "d"])])));
        assert_eq!(credit(&[], &changed, "user"), ("user".into(), Vec::new()));
        // The person's edit landed after the agent's: still the agent's change.
        let notices = [("agent-a".to_string(), None), ("user".to_string(), None)];
        assert_eq!(credit(&notices, &changed, "user").0, "agent-a");
        // Named ids: each to its last namer; the person's are nobody's; the rest to the last agent.
        let notices = [("agent-a".to_string(), ids(&["a", "d"])), ("agent-b".to_string(), ids(&["b"])), ("user".to_string(), ids(&["d"]))];
        assert_eq!(credit(&notices, &changed, "user"), ("agent-b".into(), by(&[("agent-a", &["a"]), ("agent-b", &["b", "c"])])));
        // Through the doc: an agent's event, then the read.
        let (shared, doc, heard) = open(3);
        let mut event = json!(Design { updated_by: "agent-a".into(), ..row(4) });
        event["touched"] = json!(["button"]);
        shared.event(&event);
        let mut engine = nodes();
        engine[2].x = 40.0;
        let mut agent_row = got(4, engine);
        agent_row.design.updated_by = "user".into();
        shared.read(Ok(agent_row), 0, None);
        assert_eq!(heard.borrow()[0], "reloaded by agent-a: button");
        assert_eq!(doc.take_actors(), by(&[("agent-a", &["button"])]));
        assert!(shared.notices.borrow().is_empty(), "cleared by the read");
        assert_eq!(ids_of(&json!({"touched": ["a"], "removed": ["b"]})), ids(&["a", "b"]));
        assert_eq!(ids_of(&json!({"touched": ["a"], "all": true})), None, "cut short: not all named");
        assert_eq!(ids_of(&json!({})), None);
    }

    /// TM-13: an agent's change whose notice comes after our own answer moved the rev past it is
    /// still the agent's, not the person's who wrote last.
    #[test]
    fn an_agents_notice_after_our_answer_is_still_credited() {
        let (shared, doc, heard) = open(3);
        doc.commit(vec![set("dot", json!({"x": 600.0}))], vec![]);
        let sent = shared.next().unwrap();
        // The agent's apply landed first (rev 4): ours answers rev 5.
        shared.answered(sent, Ok(answer(4, 5, vec![at(&shared, "dot", 600.0)], Some(vec![set("dot", json!({"x": 500.0}))]))));
        assert!(shared.stale.get(), "someone came between");
        shared.event(&json!(Design { updated_by: "agent-a".into(), ..row(4) }));
        let mut engine = nodes();
        engine[2].x = 40.0;
        engine[4].x = 600.0;
        shared.read(Ok(got(5, engine)), shared.answers.get(), None);
        assert_eq!(heard.borrow().iter().find(|h| h.starts_with("reloaded")).unwrap(), "reloaded by agent-a: button");
    }

    /// TM-54: an engine holding an older copy than ours (a restored store) is taken, and the
    /// history, which no longer fits it, is cleared.
    #[test]
    fn an_older_copy_clears_the_history() {
        let (shared, _doc, heard) = open(3);
        shared.push_step(false, vec![set("button", json!({"x": 30.0}))], Vec::new(), Vec::new());
        shared.seen.set(9);
        shared.read(Ok(got(2, nodes())), 0, None);
        assert_eq!(shared.rev.get(), 2);
        assert!(!shared.can_undo() && !shared.stale.get() && shared.seen.get() == 0, "nothing owed from before");
        assert_eq!(said(&heard), ["This design went back to an earlier copy, so its undo history was cleared."]);
    }

    /// W7, B50: an undo the engine ran without what was changed since is read again, not run
    /// here, and the person is told what it kept.
    #[test]
    fn an_undo_that_kept_later_changes_says_which() {
        let (shared, doc, heard) = open(3);
        shared.push_step(false, vec![set("button", json!({"fill": "#000000"}))], Vec::new(), Vec::new());
        doc.undo(Vec::new());
        let sent = shared.next().unwrap();
        let mut out = answer(3, 4, Vec::new(), Some(Vec::new()));
        out.skipped = vec![
            Conflict { fields: vec!["fill".into()], ..Conflict::new(Some(0), "button", ConflictReason::Fields) },
            Conflict::new(Some(1), "card", ConflictReason::Inside),
            Conflict::new(Some(2), "gone", ConflictReason::ParentSkipped),
        ];
        shared.answered(sent, Ok(out));
        assert!(shared.stale.get() && shared.scene.borrow().node("button").unwrap().fill.is_none(), "not run here: read");
        assert_eq!(said(&heard), ["Undo kept 2 later changes: Button (fill) and Card (new layers inside)"]);
    }

    /// W8: an edit refused because what it writes changed says what changed.
    #[test]
    fn a_stale_refusal_names_what_changed() {
        let (shared, doc, heard) = open(3);
        let stale = |conflicts: Value| {
            BusError::new(ErrorKind::Conflict, "design.stale", "the design changed").with_details(json!({"rev": 4, "conflicts": conflicts}))
        };
        let scene = shared.scene.borrow();
        let one = stale(json!([{"id": "button", "reason": "fields", "fields": ["fill", "radius"]}]));
        assert_eq!(stale_text(&one, &scene), "Not saved: Button's fill and radius changed just before. Showing the newer version.");
        let two = stale(json!([{"id": "button", "reason": "fields", "fields": ["fill"]}, {"id": "card", "reason": "parent"}, {"id": "zz", "reason": "missing"}]));
        assert_eq!(stale_text(&two, &scene), "Not saved: Button (fill), Card (place) and zz changed just before. Showing the newer version.");
        assert_eq!(stale_text(&stale(json!([])), &scene), "Not saved: the design changed. Showing the newer version.");
        drop(scene);
        doc.commit(vec![set("button", json!({"fill": "#ff0000"}))], vec![]);
        let sent = shared.next().unwrap();
        shared.answered(sent, Err(Failure::Refused(Box::new(stale(json!([{"id": "button", "reason": "fields", "fields": ["fill"]}]))))));
        assert_eq!(said(&heard), ["Not saved: Button's fill changed just before. Showing the newer version."]);
        assert!(shared.unsaved() && shared.stale.get());
    }

    /// B7: an answer too large to read, or none in time, may be an edit that applied: read the
    /// design, keep the queue, and say only what is true.
    #[test]
    fn an_edit_that_may_have_applied_is_read_not_refused() {
        let (shared, doc, heard) = open(3);
        doc.commit(vec![set("button", json!({"x": 20.0}))], vec![]);
        doc.commit(vec![set("dot", json!({"x": 600.0}))], vec![]);
        doc.commit(vec![set("label", json!({"x": 120.0}))], vec![]);
        let sent = shared.next().unwrap();
        shared.answered(sent, Err(Failure::TooLarge));
        assert!(!shared.unsaved() && shared.stale.get() && !shared.can_undo());
        assert_eq!(shared.queue.borrow().len(), 2, "the rest still goes");
        assert_eq!(said(&heard), ["Saved, but it cannot be undone from here."]);
        let sent = shared.next().unwrap();
        shared.answered(sent, Err(Failure::Unknown("Request timed out.".into())));
        assert_eq!(said(&heard).len(), 1, "a timeout is read, not said");
        assert_eq!(shared.queue.borrow().len(), 1);
    }

    /// An answer too large to carry its undo keeps no step and says so; one without its nodes
    /// is read again.
    #[test]
    fn an_answer_without_its_undo_keeps_no_step() {
        let (shared, doc, heard) = open(3);
        doc.commit(vec![set("button", json!({"x": 20.0}))], vec![]);
        let sent = shared.next().unwrap();
        let mut out = answer(3, 4, Vec::new(), None);
        out.nodes = None;
        out.reread = true;
        shared.answered(sent, Ok(out));
        assert!(!shared.can_undo() && shared.stale.get());
        assert_eq!(said(&heard), ["Saved. This change is too large to undo."]);
    }

    /// The design's named colours, given whole with an answer, are taken once nothing of ours
    /// is ahead to change them again.
    #[test]
    fn an_answer_takes_the_colours_once_nothing_is_ahead() {
        let (shared, doc, _) = open(3);
        doc.commit(vec![set("button", json!({"x": 20.0}))], vec![]);
        doc.commit(vec![set("button", json!({"x": 30.0}))], vec![]);
        let colors = |hex: &str| Some([("accent".to_string(), hex.to_string())].into());
        let sent = shared.next().unwrap();
        let mut out = answer(3, 4, Vec::new(), Some(Vec::new()));
        out.colors = colors("#ff0000");
        shared.answered(sent, Ok(out));
        assert!(shared.scene.borrow().styles().colors.is_empty(), "an edit is still ahead");
        let sent = shared.next().unwrap();
        let mut out = answer(4, 5, Vec::new(), Some(Vec::new()));
        out.colors = colors("#00ff00");
        shared.answered(sent, Ok(out));
        assert_eq!(shared.scene.borrow().styles().colors["accent"], "#00ff00");
    }

    /// B83: the row a rename answers moves the scene's rev only when it is the one write since.
    #[test]
    fn a_rename_answer_moves_the_rev_only_when_it_is_the_one_write() {
        assert!(adopt_rev(5, 6, true));
        assert!(!adopt_rev(5, 7, true), "something came between");
        assert!(!adopt_rev(5, 6, false), "ours out, or a read owed");
        let (shared, doc, heard) = open(5);
        doc.adopt(Design { title: "Renamed".into(), ..row(6) });
        assert_eq!((shared.rev.get(), shared.title()), (6, "Renamed".to_string()));
        assert_eq!(heard.borrow().as_slice(), ["design"]);
        shared.stale.set(true);
        doc.adopt(Design { title: "Again".into(), ..row(7) });
        assert_eq!((shared.rev.get(), shared.title()), (6, "Again".to_string()), "the read owed brings rev 7");
    }

    /// B109: what events say of a design while an open of it reads it.
    #[test]
    fn an_open_hears_what_happened_while_it_read() {
        watch(42);
        noticed(42, 9, false, false);
        noticed(42, 8, true, false);
        watch(42);
        assert_eq!(unwatch(42), Watch { opens: 2, rev: 9, deleted: false, purged: false }, "an older event says nothing");
        noticed(42, 10, true, false);
        assert_eq!(unwatch(42), Watch { opens: 1, rev: 10, deleted: true, purged: false }, "the other open still hears");
        noticed(42, 11, false, true);
        assert_eq!(unwatch(42), Watch::default(), "nobody watches it now");
        noticed(43, 3, false, true);
        assert_eq!(WATCHED.with(|w| w.borrow().len()), 0);
    }

    /// A design deleted elsewhere takes no more of what is queued for it.
    #[test]
    fn a_deleted_design_sends_nothing_more() {
        let (shared, doc, heard) = open(3);
        doc.commit(vec![set("button", json!({"x": 20.0}))], vec![]);
        shared.event(&json!(Design { deleted_at: Some("2026-10-20T09:00:00Z".into()), ..row(4) }));
        assert!(shared.queue.borrow().is_empty() && shared.next().is_none());
        assert!(heard.borrow().contains(&"design".to_string()));
    }

    #[test]
    fn expectations_follow_what_is_still_written() {
        let expect = vec![
            Expect { id: "a".into(), set: Some(json!({"fill": null, "w": 10}).as_object().unwrap().clone()), parent: Some("p1".into()), within: None },
            Expect { id: "b".into(), set: Some(json!({"x": 1}).as_object().unwrap().clone()), parent: None, within: None },
        ];
        let ops = vec![set("a", json!({"width": 20})), NodeOp::Move { id: "a".into(), parent: None, index: Some(0), keep_position: None }];
        assert_eq!(json!(keep_expect(expect, &ops)), json!([{"id": "a", "set": {"w": 10}, "parent": "p1"}]));
    }
}
