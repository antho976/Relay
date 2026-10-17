//! One open design and its conversation with the engine. An edit runs on the local scene at
//! once and goes out through `design.apply`, one at a time and in order; the engine's answer
//! confirms the nodes and carries the edits that undo it, which are the undo stack. When
//! someone else (an agent, another window) changes the design, `design.changed` brings a newer
//! `rev` and the design is read again once nothing of ours is in flight.

use crate::app::Ui;
use relay_bus::design::NodeOp;
use relay_bus::ops::design::{ApplyOut, GetOut};
use relay_bus::types::Design;
use relay_client::design::Scene;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::{Rc, Weak};

type Listener = Rc<dyn Fn(&Change)>;

/// Undo steps kept per design.
const HISTORY: usize = 200;

enum Pending {
    Edit(Vec<NodeOp>),
    Undo,
    Redo,
}

/// What a listener hears.
#[derive(Debug, Clone)]
pub enum Change {
    /// The scene changed under an edit of ours (or a preview).
    Scene,
    /// The design was read again after someone else changed it: these nodes differ, and this
    /// actor (`user` or a session) changed it.
    Reloaded { changed: HashSet<String>, by: String },
    /// The design row changed: title, terminal, deleted.
    Design,
    /// Saving state: in flight or settled.
    Saving(bool),
    Error(String),
}

pub struct Doc {
    pub id: i64,
    ui: Weak<Ui>,
    pub design: RefCell<Design>,
    pub scene: RefCell<Scene>,
    pub assets_dir: RefCell<String>,
    /// The `rev` our scene matches, as far as the engine has told us.
    rev: Cell<i64>,
    queue: RefCell<VecDeque<Pending>>,
    sending: Cell<bool>,
    /// Someone else changed the design: read it again once the queue is empty.
    stale: Cell<bool>,
    /// The highest `rev` an event announced while ours were in flight.
    seen: Cell<i64>,
    /// A gesture or text edit is under way: a re-read would pull the scene from under it.
    hold: Cell<bool>,
    reading: Cell<bool>,
    undo: RefCell<Vec<Vec<NodeOp>>>,
    redo: RefCell<Vec<Vec<NodeOp>>>,
    listeners: RefCell<Vec<Listener>>,
}

fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| format!("unexpected answer: {e}"))
}

impl Doc {
    pub async fn open(ui: &Rc<Ui>, id: i64) -> Result<Rc<Doc>, String> {
        let started = std::env::var_os("RELAY_DESIGN_TRACE").is_some().then(std::time::Instant::now);
        let got: GetOut = decode(ui.call("design.get", json!({"design_id": id})).await.map_err(|e| e.to_string())?)?;
        if let Some(t) = started {
            println!("DESIGN_TRACE open {:.3} ms {} nodes", t.elapsed().as_secs_f64() * 1e3, got.nodes.len());
        }
        Ok(Rc::new(Doc {
            id,
            ui: Rc::downgrade(ui),
            rev: Cell::new(got.design.rev),
            design: RefCell::new(got.design),
            scene: RefCell::new(Scene::new(got.nodes)),
            assets_dir: RefCell::new(got.assets_dir),
            queue: RefCell::default(),
            sending: Cell::new(false),
            stale: Cell::new(false),
            seen: Cell::new(0),
            hold: Cell::new(false),
            reading: Cell::new(false),
            undo: RefCell::default(),
            redo: RefCell::default(),
            listeners: RefCell::default(),
        }))
    }

    pub fn listen(&self, f: impl Fn(&Change) + 'static) {
        self.listeners.borrow_mut().push(Rc::new(f));
    }

    fn tell(&self, change: Change) {
        let listeners: Vec<_> = self.listeners.borrow().clone();
        for l in listeners {
            l(&change);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.borrow().is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.borrow().is_empty()
    }

    pub fn busy(&self) -> bool {
        self.sending.get() || !self.queue.borrow().is_empty()
    }

    /// Run `ops` on the scene now and send them. One call is one undo step.
    pub fn commit(self: &Rc<Self>, ops: Vec<NodeOp>) -> bool {
        if ops.is_empty() {
            return true;
        }
        if let Err(e) = self.scene.borrow_mut().apply_all(&ops) {
            self.tell(Change::Error(e));
            return false;
        }
        self.redo.borrow_mut().clear();
        self.queue.borrow_mut().push_back(Pending::Edit(ops));
        self.tell(Change::Scene);
        self.pump();
        true
    }

    pub fn undo(self: &Rc<Self>) {
        self.queue.borrow_mut().push_back(Pending::Undo);
        self.pump();
    }

    pub fn redo(self: &Rc<Self>) {
        self.queue.borrow_mut().push_back(Pending::Redo);
        self.pump();
    }

    /// Hold re-reads while a gesture or text edit is under way; releasing runs one owed.
    pub fn set_hold(self: &Rc<Self>, hold: bool) {
        self.hold.set(hold);
        if !hold && self.stale.get() && !self.busy() {
            self.reread();
        }
    }

    /// `design.changed` for this design.
    pub fn event(self: &Rc<Self>, payload: &Value) {
        let Ok(row) = serde_json::from_value::<Design>(payload.clone()) else {
            return;
        };
        let rev = row.rev;
        let row_changed = {
            let current = self.design.borrow();
            current.title != row.title || current.session != row.session || current.deleted_at != row.deleted_at
        };
        if row_changed && rev >= self.design.borrow().rev {
            *self.design.borrow_mut() = row;
            self.tell(Change::Design);
        }
        if rev <= self.rev.get() {
            return;
        }
        if self.busy() {
            self.seen.set(self.seen.get().max(rev));
        } else if self.hold.get() {
            self.stale.set(true);
        } else {
            self.reread();
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
            loop {
                let Some(next) = doc.queue.borrow_mut().pop_front() else {
                    break;
                };
                let (ops, undoing) = match next {
                    Pending::Edit(ops) => (ops, None),
                    Pending::Undo => match doc.undo.borrow_mut().pop() {
                        Some(ops) => (ops, Some(true)),
                        None => continue,
                    },
                    Pending::Redo => match doc.redo.borrow_mut().pop() {
                        Some(ops) => (ops, Some(false)),
                        None => continue,
                    },
                };
                let payload = json!({"design_id": doc.id, "ops": ops, "lenient": undoing.is_some()});
                match ui.call("design.apply", payload).await.map_err(|e| e.to_string()).and_then(decode::<ApplyOut>) {
                    Ok(out) => {
                        if out.prev_rev != doc.rev.get() {
                            doc.stale.set(true);
                        }
                        doc.rev.set(out.design.rev);
                        if undoing.is_some() {
                            // Not run ahead of the engine: run it now, as the engine did.
                            let mut scene = doc.scene.borrow_mut();
                            let mut diverged = false;
                            for op in &ops {
                                diverged |= scene.apply(op).is_err();
                            }
                            if diverged {
                                doc.stale.set(true);
                            }
                        }
                        {
                            let mut scene = doc.scene.borrow_mut();
                            // The engine added or removed nodes we did not (a component's
                            // copies following it): read the design again once we are done.
                            if out.nodes.iter().any(|n| scene.node(&n.id).is_none()) || out.removed.iter().any(|id| scene.node(id).is_some()) {
                                doc.stale.set(true);
                            }
                            let synced = out.nodes.iter().any(|n| scene.node(&n.id).is_some_and(|mine| mine.master.is_some() && mine != n));
                            for node in out.nodes {
                                scene.confirm(node);
                            }
                            if synced {
                                drop(scene);
                                doc.tell(Change::Scene);
                            }
                        }
                        let target = match undoing {
                            Some(true) => &doc.redo,
                            _ => &doc.undo,
                        };
                        let mut stack = target.borrow_mut();
                        stack.push(out.inverse);
                        if stack.len() > HISTORY {
                            stack.remove(0);
                        }
                        drop(stack);
                        let row_changed = doc.design.borrow().title != out.design.title;
                        *doc.design.borrow_mut() = out.design;
                        if undoing.is_some() {
                            doc.tell(Change::Scene);
                        }
                        if row_changed {
                            doc.tell(Change::Design);
                        }
                    }
                    Err(error) => {
                        // Our scene ran ahead with edits the engine refused: take its copy.
                        doc.queue.borrow_mut().clear();
                        doc.stale.set(true);
                        doc.tell(Change::Error(error));
                    }
                }
            }
            doc.sending.set(false);
            if doc.seen.get() > doc.rev.get() {
                doc.stale.set(true);
            }
            doc.tell(Change::Saving(false));
            if doc.stale.get() && !doc.hold.get() {
                doc.reread();
            }
        });
    }

    /// Read the design again, and say which nodes differ from ours.
    pub fn reread(self: &Rc<Self>) {
        if self.reading.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.reading.set(true);
        let doc = self.clone();
        glib::spawn_future_local(async move {
            let got = ui
                .call("design.get", json!({"design_id": doc.id}))
                .await
                .map_err(|e| e.to_string())
                .and_then(decode::<GetOut>);
            doc.reading.set(false);
            let got = match got {
                Ok(got) => got,
                Err(error) => {
                    doc.tell(Change::Error(error));
                    return;
                }
            };
            if doc.busy() || doc.hold.get() {
                // Ours went out meanwhile: read again when they are answered.
                doc.stale.set(true);
                return;
            }
            doc.stale.set(false);
            doc.seen.set(0);
            let changed: HashSet<String> = {
                let old = doc.scene.borrow();
                got.nodes.iter().filter(|n| old.node(&n.id) != Some(n)).map(|n| n.id.clone()).collect()
            };
            let by = got.design.updated_by.clone();
            doc.rev.set(got.design.rev);
            *doc.design.borrow_mut() = got.design;
            *doc.scene.borrow_mut() = Scene::new(got.nodes);
            *doc.assets_dir.borrow_mut() = got.assets_dir;
            doc.tell(Change::Reloaded { changed, by });
            doc.tell(Change::Design);
        });
    }

    pub fn session(&self) -> Option<String> {
        self.design.borrow().session.clone()
    }

    pub fn title(&self) -> String {
        self.design.borrow().title.clone()
    }
}
