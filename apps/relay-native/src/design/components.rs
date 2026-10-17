//! Components on the board: make the selected frame one, place copies of it, jump from a copy
//! to its component, reset a copy's own changes, or cut it loose. The engine keeps copies in
//! step (`design.apply`'s component sync); these are the person's handles on it.

use super::board::Board;
use relay_bus::design::{NodeKind, NodeOp};
use serde_json::{json, Map, Value};

fn set(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()
}

impl Board {
    /// The instance root a node sits in (itself included).
    pub(super) fn instance_root(&self, id: &str) -> Option<String> {
        let scene = self.doc.scene.borrow();
        std::iter::once(id).chain(scene.ancestors(id)).find(|i| scene.node(i).is_some_and(|n| n.instance_of.is_some())).map(str::to_string)
    }

    /// Make the selected frame a component (Ctrl+Alt+K); anything else is framed first.
    pub(super) fn make_component(&self) {
        let ids = self.selected();
        let frame = match ids.as_slice() {
            [one] if self.doc.scene.borrow().node(one).is_some_and(|n| n.kind == NodeKind::Frame) => Some(one.clone()),
            _ => None,
        };
        let id = match frame {
            Some(id) => id,
            None => {
                self.group(NodeKind::Frame);
                match self.selected().as_slice() {
                    [one] => one.clone(),
                    _ => return,
                }
            }
        };
        if self.instance_root(&id).is_some() {
            self.toast("A copy of a component cannot be one itself: detach it first");
            return;
        }
        self.commit(vec![NodeOp::Update { id, set: set(&[("component", json!(true))]) }]);
        self.toast("Made a component: its copies follow it. Place one from Assets.");
    }

    /// A copy of the selected component, beside it.
    pub(super) fn create_instance(&self) {
        let Some(id) = self.selected().into_iter().next() else {
            return;
        };
        let Some(n) = self.doc.scene.borrow().node(&id).filter(|n| n.component == Some(true)).cloned() else {
            self.toast("Select a component to make a copy of it");
            return;
        };
        let copy = self.doc.scene.borrow_mut().fresh_named(&format!("{}-copy", id));
        let op = NodeOp::Insert {
            item: format!("#{id}"),
            parent: n.parent.clone().unwrap_or_default(),
            x: n.x + n.w + 40.0,
            y: n.y,
            index: None,
            id: Some(copy.clone()),
            set: None,
        };
        if self.commit(vec![op]) {
            self.select(vec![copy]);
        }
    }

    /// Select the component the selected copy follows.
    pub(super) fn go_to_component(&self) {
        let Some(root) = self.selected().first().and_then(|id| self.instance_root(id)) else {
            return;
        };
        let target = self.doc.scene.borrow().node(&root).and_then(|n| n.instance_of.clone());
        let Some(target) = target.filter(|t| self.doc.scene.borrow().node(t).is_some()) else {
            self.toast("Its component is gone");
            return;
        };
        let page = self.doc.scene.borrow().page_of(&target).map(str::to_string);
        if let Some(page) = page {
            self.set_page(&page);
        }
        self.select(vec![target]);
        let r = self.selection_bounds();
        self.fit(r, true);
    }

    /// Forget a copy's own changes: the component's values come back.
    pub(super) fn reset_overrides(&self) {
        let Some(root) = self.selected().first().and_then(|id| self.instance_root(id)) else {
            return;
        };
        let scene = self.doc.scene.borrow();
        let ops = scene
            .subtree(&root)
            .into_iter()
            .filter(|id| scene.node(id).is_some_and(|n| n.overridden.is_some()))
            .map(|id| NodeOp::Update { id, set: set(&[("overridden", Value::Null)]) })
            .collect::<Vec<_>>();
        drop(scene);
        if ops.is_empty() {
            self.toast("This copy has no changes of its own");
        } else {
            self.commit(ops);
        }
    }

    /// Cut a copy loose: ordinary layers that no longer follow the component.
    pub(super) fn detach_instance(&self) {
        let Some(root) = self.selected().first().and_then(|id| self.instance_root(id)) else {
            return;
        };
        let scene = self.doc.scene.borrow();
        let ops = scene
            .subtree(&root)
            .into_iter()
            .filter(|id| scene.node(id).is_some_and(|n| n.master.is_some() || n.instance_of.is_some()))
            .map(|id| NodeOp::Update { id, set: set(&[("master", Value::Null), ("overridden", Value::Null), ("instance_of", Value::Null)]) })
            .collect::<Vec<_>>();
        drop(scene);
        self.commit(ops);
    }
}
