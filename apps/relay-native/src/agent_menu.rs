//! Agent pane controls: the settings popover, the phone install, and the optimistic open and
//! close paths that make an agent appear or disappear the moment it is asked to.
//!
//! The wall used to wait for the engine and then re-list everything: a close answered only after
//! the child's TERM grace, and the pane went away after four more round trips and a full wall
//! rebuild. Now the pane leaves on the click and comes back only if the engine refuses; a launch
//! shows its panes before the worktree exists; and `session.changed` carries the whole row, so it
//! is applied in place instead of triggering a `session.list`.
use super::*;
use std::collections::HashMap;
use std::rc::Weak;
use std::time::Instant;

#[derive(Default)]
struct Agents {
    /// Sessions the user closed whose close has not answered yet. Kept off the wall even if a
    /// stale list or event still carries them.
    closing: BTreeSet<String>,
    /// Panes picked with Shift+click on their headers; Close in any of their menus closes them all.
    selected: BTreeSet<String>,
    /// Sessions (or launch placeholders) with a start in flight, and the step they are on.
    launching: BTreeMap<String, String>,
    /// One settings key per pane, kept across header rebuilds so an open popover survives the
    /// state changes an agent produces every few seconds.
    keys: HashMap<String, (Weak<Pane>, gtk::Button)>,
    /// Each pane's focus key, so a layout change can turn its corners in or out.
    focus_keys: HashMap<String, glib::WeakRef<gtk::Button>>,
    /// The last phone install per session: run id and what it last said.
    phone: HashMap<String, Phone>,
    next_placeholder: u32,
}

#[derive(Clone, Default)]
struct Phone {
    run: Option<i64>,
    status: String,
    busy: bool,
    /// The label of the popover currently showing this session, if one is open.
    view: Option<glib::WeakRef<gtk::Label>>,
    stop: Option<glib::WeakRef<gtk::Button>>,
    install: Option<glib::WeakRef<gtk::Button>>,
}

thread_local! {
    static AGENTS: RefCell<Agents> = RefCell::new(Agents::default());
}

fn with<T>(f: impl FnOnce(&mut Agents) -> T) -> T {
    AGENTS.with(|agents| f(&mut agents.borrow_mut()))
}

pub(in crate::app) fn is_closing(name: &str) -> bool {
    with(|a| a.closing.contains(name))
}

fn launching(name: &str) -> Option<String> {
    with(|a| a.launching.get(name).cloned())
}

/// A popover row: an icon that says what it does, a caption that says it again, a tooltip
/// that says what happens next.
fn row_key(icon: &str, caption: &str, tooltip: &str) -> gtk::Button {
    let key = gtk::Button::new();
    key.add_css_class("agent-menu-row");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.append(&crate::icons::image(icon, 16));
    let text = label(caption, "agent-menu-caption");
    text.set_hexpand(true);
    row.append(&text);
    key.set_child(Some(&row));
    key.set_tooltip_text(Some(tooltip));
    key.update_property(&[gtk::accessible::Property::Label(caption)]);
    key
}

/// A compact key for the row under the identity: icon and one word.
fn tool_key(icon: &str, caption: &str, tooltip: &str) -> gtk::Button {
    let key = gtk::Button::new();
    key.add_css_class("agent-menu-tool");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    row.append(&crate::icons::image(icon, 14));
    row.append(&label(caption, "agent-menu-caption"));
    key.set_child(Some(&row));
    key.set_tooltip_text(Some(tooltip));
    key.update_property(&[gtk::accessible::Property::Label(caption)]);
    key
}

const MENU_WIDTH: i32 = 320;

/// Cap the scrolling part so the whole popover fits on the roomier side of its key, however
/// small the pane it belongs to; a confirmation opening in the footer takes its room from the
/// scrolling part, and gives it back when it closes.
fn fit_menu(key: &gtk::Button, scroller: &gtk::ScrolledWindow, foot: &gtk::Box) {
    let Some(root) = key.root() else { return };
    let Some(bounds) = key.compute_bounds(&root) else { return };
    let window = root.height() as f32;
    // The arrow, the popover's border and a margin from the window edge.
    let room = bounds.y().max(window - bounds.y() - bounds.height()) - 28.0;
    let base = room as i32 - foot.measure(gtk::Orientation::Vertical, MENU_WIDTH).1;
    let mut strips = Vec::new();
    let mut child = foot.first_child();
    while let Some(widget) = child {
        if let Some(revealer) = widget.downcast_ref::<gtk::Revealer>() {
            strips.push(revealer.clone());
        }
        child = widget.next_sibling();
    }
    let apply: Rc<dyn Fn()> = Rc::new({
        let (scroller, strips) = (scroller.downgrade(), strips.iter().map(|r| r.downgrade()).collect::<Vec<_>>());
        move || {
            let Some(scroller) = scroller.upgrade() else { return };
            let open: i32 = strips
                .iter()
                .filter_map(|r| r.upgrade())
                .filter(|r| r.reveals_child())
                .filter_map(|r| r.child())
                .map(|strip| strip.measure(gtk::Orientation::Vertical, MENU_WIDTH).1)
                .sum();
            scroller.set_max_content_height((base - open).max(120));
        }
    });
    apply();
    for revealer in strips {
        let apply = apply.clone();
        revealer.connect_reveal_child_notify(move |_| apply());
    }
}

fn section(title: &str) -> gtk::Label {
    let heading = label(title, "agent-menu-section");
    heading.set_xalign(0.0);
    heading
}

fn bus_code(error: &Error) -> Option<&str> {
    match error {
        Error::Bus(error) => Some(error.code.as_str()),
        _ => None,
    }
}

/// `device.run` refusals in words a person can act on.
fn phone_error(error: &Error) -> String {
    match bus_code(error) {
        // The engine looks for gradlew at the root, then in the one top-level folder that has
        // one; two such folders are as good as none (`gradle_wrapper` in handlers/device.rs).
        Some("device.gradle_missing") => "No Android project in this agent's worktree: no gradlew at its root or in a single top-level folder, and no run command in Project settings.".into(),
        Some("device.not_ready") | Some("device.none") => "The phone is not connected or has not authorized this computer. Reconnect it and accept the USB debugging prompt.".into(),
        Some("device.worktree") => "This agent's worktree is missing or does not belong to the project.".into(),
        Some("device.busy") => error.to_string(),
        Some("avd.sdk_missing") | Some("device.adb_missing") => format!("Android SDK not found: {error}"),
        _ => error.to_string(),
    }
}

impl Ui {
    /// Apply a `session.changed` row in place. Returns false when the payload is not a row, so
    /// the caller falls back to a full refresh.
    pub(in crate::app) fn apply_session_event(self: &Rc<Self>, ev: &str, payload: &Value) -> bool {
        if ev != "session.changed" || !payload["name"].is_string() || !payload["state"].is_string() {
            return false;
        }
        let name = text(payload, "name").to_string();
        let closed = text(payload, "state") == "closed";
        if closed {
            // Settles a close whose answer timed out while the engine was still removing its worktree.
            with(|a| {
                a.closing.remove(&name);
                a.phone.remove(&name);
            });
        }
        // The board shows a session only as a name on its task, so only a new task link changes it.
        let mut relinked = !closed && payload["task_id"].is_i64();
        {
            let mut all = self.sidebar_sessions.borrow_mut();
            let at = all.iter().position(|s| text(s, "name") == name);
            relinked &= at.is_none_or(|i| all[i]["task_id"] != payload["task_id"]);
            match at {
                Some(i) if closed => {
                    all.remove(i);
                }
                Some(i) => all[i] = payload.clone(),
                None if !closed => all.push(payload.clone()),
                None => {}
            }
        }
        if payload["project_id"].as_i64() == Some(self.project.get()) && !is_closing(&name) {
            let mut sessions = self.sessions.borrow_mut();
            match sessions.iter().position(|s| text(s, "name") == name) {
                Some(i) if closed => {
                    sessions.remove(i);
                }
                Some(i) => sessions[i] = payload.clone(),
                // A launch adopts its own new sessions into its placeholders' places; until it
                // does, a fresh `created` row would read as an interrupted launch.
                None if !closed && !(self.launch_busy.get() && text(payload, "state") == "created") => {
                    sessions.push(payload.clone())
                }
                None => {}
            }
        }
        self.render_projects();
        self.reconcile();
        // The board shows this project's tasks only. A state
        // change a card shows (blocked, done) arrives as its own task.changed.
        let here = payload["project_id"].as_i64() == Some(self.project.get());
        let page = self.page.borrow().clone();
        match page.as_str() {
            "board" if here && relinked => self.refresh_page(),
            _ => {}
        }
        true
    }

    /// Put one session row on the wall (or update it) without a round trip.
    pub(in crate::app) fn upsert_session(self: &Rc<Self>, session: &Value) {
        if !session["name"].is_string() {
            return;
        }
        self.apply_session_event("session.changed", session);
    }

    /// Close agents with the same options: each pane leaves now, the engine is told after, and a
    /// pane comes back only if the engine refuses. Several are closed one after another, so their
    /// worktree and branch removals do not race on the shared repository. Uncommitted work in any
    /// of them is asked about once, for all of them.
    pub(super) fn close_agents(self: &Rc<Self>, names: Vec<String>, payload: Value) {
        let clicked = Instant::now();
        with(|a| {
            for name in &names {
                a.closing.insert(name.clone());
                a.selected.remove(name);
            }
        });
        self.sessions.borrow_mut().retain(|s| !names.iter().any(|n| n == text(s, "name")));
        self.reconcile();
        tracing::debug!(count = names.len(), elapsed_us = clicked.elapsed().as_micros() as u64, "close: panes removed");
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let mut dirty = Vec::new();
            for name in names {
                let mut payload = payload.clone();
                payload["session"] = json!(name);
                let answer = ui.close_answer(&name, &payload).await;
                tracing::debug!(session = %name, elapsed_ms = clicked.elapsed().as_millis() as u64, "close: engine answered");
                if let Some(refusal) = answer {
                    dirty.push((name, refusal));
                }
            }
            if dirty.is_empty() {
                return;
            }
            // Removing a worktree would delete uncommitted work. The popover is gone by now, so
            // the question is asked here; only a yes deletes it (RA-405).
            let dialog = match dirty.as_slice() {
                // The agent wrote after the close began: the engine stopped it and kept its
                // worktree, so there is no agent left to keep, only the worktree.
                [(name, (true, message))] => gtk::AlertDialog::builder()
                    .message(format!("{name} has already been stopped, and its uncommitted work kept"))
                    .detail(format!("{message}. Discarding the changes removes the worktree and deletes them for good."))
                    .buttons(["Keep the worktree", "Discard changes"]),
                [(name, (false, message))] => gtk::AlertDialog::builder()
                    .message(format!("Discard {name}'s uncommitted changes?"))
                    .detail(format!(
                        "{message}. Closing with its worktree removed deletes them for good. To keep them, commit or stash them, or close it keeping the worktree."
                    ))
                    .buttons(["Keep the agent", "Discard changes and close"]),
                _ => gtk::AlertDialog::builder()
                    .message(format!("Discard uncommitted changes in {} agents?", dirty.len()))
                    .detail(format!(
                        "{}.\n\nThe others are closed. Closing these with their worktrees removed deletes their changes for good; keeping them keeps the agents and their worktrees.",
                        dirty.iter().map(|(name, (_, message))| format!("{name}: {message}")).collect::<Vec<_>>().join("\n")
                    ))
                    .buttons(["Keep them", "Discard changes and close all"]),
            }
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
            if dialog.choose_future(Some(&ui.window)).await.ok() == Some(1) {
                let mut payload = payload;
                payload["discard_changes"] = json!(true);
                ui.close_agents(dirty.into_iter().map(|(name, _)| name).collect(), payload);
            } else {
                ui.refresh();
            }
        });
    }

    /// The engine half of one close, once its pane has left. Settles everything but a dirty
    /// worktree, which comes back as (already stopped, the engine's words) for the caller to ask
    /// about.
    async fn close_answer(self: &Rc<Self>, name: &str, payload: &Value) -> Option<(bool, String)> {
        let result = self.call("session.close", payload.clone()).await;
        if matches!(result, Err(crate::client::Error::Timeout)) {
            // Removing a large worktree can outlast the request: the close is still running,
            // and its session.changed(closed) settles it. The pane comes back only if that
            // never arrives. Watched on the side, so the next close in a batch is not held up.
            self.show_error(&format!("Still closing {name}: removing its worktree takes a while."));
            let (ui, name) = (self.clone(), name.to_string());
            glib::spawn_future_local(async move {
                for _ in 0..120 {
                    glib::timeout_future_seconds(5).await;
                    if !is_closing(&name) {
                        return;
                    }
                }
                with(|a| a.closing.remove(&name));
                ui.show_error(&format!("{name} has not finished closing; check its worktree before closing it again."));
                ui.refresh();
            });
            return None;
        }
        with(|a| a.closing.remove(name));
        match result {
            Ok(_) => {
                self.sidebar_sessions.borrow_mut().retain(|s| text(s, "name") != name);
                self.render_projects();
                self.render_status_counts();
                None
            }
            Err(Error::Bus(error)) if error.code == "worktree.dirty" => {
                let stopped = error.details.as_ref().and_then(|d| d["stopped"].as_bool()) == Some(true);
                Some((stopped, error.message))
            }
            Err(error) => {
                self.show_error(&format!("Could not close {name}: {error}"));
                self.refresh();
                None
            }
        }
    }

    /// Start, park, wake or resume from the pane header. Starts show their progress on the pane
    /// at once; the returned row is applied in place.
    pub(super) fn agent_op(self: &Rc<Self>, op: &'static str, name: &str, key: Option<&gtk::Button>) {
        let name = name.to_string();
        let starting = matches!(op, "session.spawn" | "session.wake" | "session.resume" | "session.clear_restorable");
        if starting {
            self.launch_progress(&name, "Starting the provider…");
        }
        if let Some(key) = key {
            key.set_sensitive(false);
        }
        let key = key.cloned();
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let result = ui.call(op, json!({"session": name})).await;
            if starting {
                ui.launch_done(&name);
            }
            match result {
                Ok(session) if session["name"].is_string() => ui.upsert_session(&session),
                Ok(_) => ui.refresh(),
                Err(error) => {
                    ui.show_error(&error.to_string());
                    ui.refresh();
                }
            }
            if let Some(key) = key {
                key.set_sensitive(true);
            }
        });
    }

    // ------------------------------------------------------------------ launch placeholders

    /// Panes for agents that do not exist yet, so a launch shows up on the click rather than
    /// after the worktree fetch and checkout.
    pub(in crate::app) fn launch_placeholders(self: &Rc<Self>, project: i64, payloads: &[Value]) -> Vec<String> {
        let mut names = Vec::new();
        for payload in payloads {
            let n = with(|a| {
                a.next_placeholder += 1;
                a.next_placeholder
            });
            let name = format!("new agent {n}");
            with(|a| a.launching.insert(name.clone(), "Preparing worktree…".into()));
            self.sessions.borrow_mut().push(json!({
                "name": name, "state": "created", "project_id": project, "placeholder": true,
                "provider": payload["provider"], "role": payload["role"].as_str().unwrap_or("builder"),
                "branch": payload["branch"], "worktree": "",
            }));
            names.push(name);
        }
        self.reconcile();
        names
    }

    /// Say which step a launching pane is on.
    pub(in crate::app) fn launch_progress(self: &Rc<Self>, name: &str, step: &str) {
        with(|a| a.launching.insert(name.to_string(), step.to_string()));
        if let Some(pane) = self.panes.borrow().get(name) {
            pane.show_progress(step);
            clear(&pane.slate_actions);
            pane.slate_actions.append(&spinner());
        }
    }

    /// Swap a placeholder for the session the engine created, in the same place on the wall.
    pub(in crate::app) fn launch_adopt(self: &Rc<Self>, placeholder: &str, session: &Value, step: &str) {
        let name = text(session, "name").to_string();
        with(|a| {
            a.launching.remove(placeholder);
            a.launching.insert(name.clone(), step.to_string());
        });
        {
            let mut sessions = self.sessions.borrow_mut();
            sessions.retain(|s| text(s, "name") != placeholder && text(s, "name") != name);
            sessions.push(session.clone());
        }
        {
            let mut ordered = self.ordered.borrow_mut();
            ordered.retain(|n| *n != name);
            if let Some(slot) = ordered.iter_mut().find(|n| n.as_str() == placeholder) {
                *slot = name.clone();
            }
        }
        if !self.sidebar_sessions.borrow().iter().any(|s| text(s, "name") == name) {
            self.sidebar_sessions.borrow_mut().push(session.clone());
        }
        self.reconcile();
        self.launch_progress(&name, step);
    }

    /// The start answered (either way): the pane renders from its row again.
    pub(in crate::app) fn launch_done(self: &Rc<Self>, name: &str) {
        with(|a| a.launching.remove(name));
        self.rendered_sessions.borrow_mut().remove(name);
    }

    /// A launch is over, finished or stopped short: placeholders leave, and sessions that exist
    /// keep their panes and render from their rows again.
    pub(in crate::app) fn launch_abort(self: &Rc<Self>, names: &[String]) {
        for name in names {
            with(|a| a.launching.remove(name));
            self.rendered_sessions.borrow_mut().remove(name);
        }
        self.sessions
            .borrow_mut()
            .retain(|s| !(s["placeholder"] == true && names.iter().any(|p| p == text(s, "name"))));
        self.reconcile();
    }

    // ------------------------------------------------------------------ pane header

    /// Remove the header keys `session_actions` rebuilds, keeping this pane's settings key (and
    /// any popover open on it) in place.
    pub(super) fn clear_agent_actions(&self, pane: &Rc<Pane>) {
        let keep = with(|a| a.keys.get(pane.name()).filter(|(owner, _)| owner.as_ptr() == Rc::as_ptr(pane)).map(|(_, key)| key.clone()));
        let mut child = pane.actions.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if keep.as_ref().map(|key| key.upcast_ref::<gtk::Widget>()) != Some(&widget) {
                pane.actions.remove(&widget);
            }
        }
    }

    /// While a pane is a launch in flight it shows its progress and nothing else. True when
    /// `session_actions` has nothing more to build.
    pub(super) fn agent_overrides(self: &Rc<Self>, _pane: &Rc<Pane>, session: &Value) -> bool {
        let name = text(session, "name");
        let placeholder = session["placeholder"] == true;
        if text(session, "state") != "created" && !placeholder {
            return false;
        }
        let Some(step) = launching(name) else {
            return placeholder;
        };
        self.launch_progress(name, &step);
        true
    }

    fn shown_alone(&self, name: &str) -> bool {
        *self.mode.borrow() == "focus" && self.focused.borrow().as_deref() == Some(name)
    }

    /// Expand while the agent shares the wall, collapse while it is shown alone.
    fn paint_focus_key(&self, key: &gtk::Button, name: &str) {
        let alone = self.shown_alone(name);
        if key.child().is_some() && key.has_css_class("alone") == alone {
            return;
        }
        let (icon, caption) = if alone { ("shrink", "Back to the wall") } else { ("expand", "Focus this agent (show it alone)") };
        key.set_child(Some(&crate::icons::image(icon, 15)));
        key.set_tooltip_text(Some(caption));
        key.update_property(&[gtk::accessible::Property::Label(caption)]);
        if alone { key.add_css_class("alone") } else { key.remove_css_class("alone") }
    }

    /// Bring every focus key in line with the wall's mode; `layout` calls it after each change.
    pub(super) fn sync_focus_keys(&self) {
        let keys: Vec<(String, gtk::Button)> = with(|a| {
            a.focus_keys.retain(|_, key| key.upgrade().is_some());
            a.focus_keys.iter().filter_map(|(name, key)| Some((name.clone(), key.upgrade()?))).collect()
        });
        for (name, key) in keys {
            self.paint_focus_key(&key, &name);
        }
    }

    /// Focus and Agent settings, last in the header.
    pub(super) fn agent_controls(self: &Rc<Self>, pane: &Rc<Pane>, session: &Value) {
        let name = text(session, "name").to_string();
        let zoom = gtk::Button::new();
        zoom.add_css_class("quiet");
        zoom.add_css_class("icon-key");
        self.paint_focus_key(&zoom, &name);
        let weak = Rc::downgrade(self);
        let n = name.clone();
        zoom.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let already = ui.shown_alone(&n);
                *ui.focused.borrow_mut() = Some(n.clone());
                ui.set_mode(if already { "grid" } else { "focus" });
            }
        });
        with(|a| a.focus_keys.insert(name.clone(), zoom.downgrade()));
        pane.actions.append(&zoom);
        let existing = with(|a| {
            a.keys.retain(|_, (owner, _)| owner.strong_count() > 0);
            a.keys.get(&name).filter(|(owner, _)| owner.as_ptr() == Rc::as_ptr(pane)).map(|(_, key)| key.clone())
        });
        let key = match existing {
            Some(key) => key,
            None => {
                let key = icon_button("sliders", "Agent settings");
                key.set_child(Some(&crate::icons::image("sliders", 15)));
                key.add_css_class("agent-settings-key");
                let weak = Rc::downgrade(self);
                let n = name.clone();
                key.connect_clicked(move |key| {
                    if let Some(ui) = weak.upgrade() {
                        ui.agent_settings(key, &n);
                    }
                });
                with(|a| a.keys.insert(name.clone(), (Rc::downgrade(pane), key.clone())));
                key
            }
        };
        if key.parent().is_none() {
            pane.actions.append(&key);
        } else if let Some(last) = pane.actions.last_child().filter(|last| last != key.upcast_ref::<gtk::Widget>()) {
            // Moved within its box, never re-parented: an open popover stays open.
            pane.actions.reorder_child_after(&key, Some(&last));
        }
    }

    // ------------------------------------------------------------------ selection

    /// Shift+click on a pane's header adds it to the selection or takes it out; a plain click
    /// on any header clears it. Keys in the header take their own clicks first.
    pub(super) fn install_pane_selection(self: &Rc<Self>, pane: &Rc<Pane>, name: &str) {
        let click = gtk::GestureClick::new();
        click.set_button(gtk::gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        let name = name.to_string();
        click.connect_pressed(move |gesture, _, _, _| {
            let Some(ui) = weak.upgrade() else { return };
            if gesture.current_event_state().contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                // Claimed, so a Shift+click does not also start dragging the pane.
                gesture.set_state(gtk::EventSequenceState::Claimed);
                ui.toggle_selected(&name);
            } else if !with(|a| a.selected.is_empty()) {
                ui.clear_selection();
            }
        });
        pane.header.add_controller(click);
        if with(|a| a.selected.contains(pane.name())) {
            pane.root.add_css_class("selected");
        }
    }

    fn toggle_selected(&self, name: &str) {
        let on = with(|a| if a.selected.remove(name) { false } else { a.selected.insert(name.to_string()) });
        if let Some(pane) = self.panes.borrow().get(name) {
            if on { pane.root.add_css_class("selected") } else { pane.root.remove_css_class("selected") }
        }
    }

    pub(super) fn clear_selection(&self) {
        let names = with(|a| std::mem::take(&mut a.selected));
        let panes = self.panes.borrow();
        for name in names {
            if let Some(pane) = panes.get(&name) {
                pane.root.remove_css_class("selected");
            }
        }
    }

    /// The selected panes still on this wall, in wall order; the selection is the batch only
    /// while it includes `name`, the pane whose menu is open.
    fn selection_with(&self, name: &str) -> Vec<String> {
        let selected = with(|a| a.selected.clone());
        if !selected.contains(name) {
            return Vec::new();
        }
        let sessions = self.sessions.borrow();
        let here = |n: &String| sessions.iter().any(|s| text(s, "name") == n && s["placeholder"] != true);
        let mut names: Vec<String> = self.ordered.borrow().iter().filter(|n| selected.contains(*n) && here(n)).cloned().collect();
        let rest: Vec<String> = selected.iter().filter(|n| !names.contains(n) && here(n)).cloned().collect();
        names.extend(rest);
        names
    }

    // ------------------------------------------------------------------ settings popover

    fn agent_settings(self: &Rc<Self>, key: &gtk::Button, name: &str) {
        let Some(session) = self.sessions.borrow().iter().find(|s| text(s, "name") == name).cloned() else {
            return;
        };
        let popover = gtk::Popover::new();
        popover.add_css_class("agent-settings");
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_parent(key);
        popover.connect_closed(|popover| {
            let popover = popover.clone();
            glib::idle_add_local_once(move || popover.unparent());
        });
        // What the agent is and how it runs scrolls; Close stays pinned under it, so a pane
        // squeezed small by a full wall can still be closed from its own menu.
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.set_size_request(MENU_WIDTH, -1);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.add_css_class("agent-menu");
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .child(&body)
            .build();
        let foot = gtk::Box::new(gtk::Orientation::Vertical, 2);
        foot.add_css_class("agent-menu-foot");
        frame.append(&scroller);
        frame.append(&foot);
        popover.set_child(Some(&frame));
        self.agent_settings_body(&popover, &body, &foot, &session);
        fit_menu(key, &scroller, &foot);
        popover.popup();
    }

    fn agent_settings_body(self: &Rc<Self>, popover: &gtk::Popover, body: &gtk::Box, foot: &gtk::Box, session: &Value) {
        let name = text(session, "name").to_string();
        let state = text(session, "state").to_string();

        // Identity.
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("agent-menu-head");
        let title = label(&name, "agent-menu-title");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&title);
        let chip = label(&state.replace('_', " ").to_uppercase(), "agent-menu-state");
        chip.set_valign(gtk::Align::Center);
        chip.add_css_class(match state.as_str() {
            "running" | "spawning" | "idle" => "live",
            "blocked" => "held",
            _ => "off",
        });
        head.append(&chip);
        body.append(&head);
        let meta = label(&format!("{} · {}", text(session, "provider"), text(session, "role")), "agent-menu-meta");
        meta.set_xalign(0.0);
        body.append(&meta);
        for (icon, value, ellipsize) in [
            ("branch", text(session, "branch"), gtk::pango::EllipsizeMode::Middle),
            ("folder", text(session, "worktree"), gtk::pango::EllipsizeMode::Start),
        ] {
            if value.is_empty() {
                continue;
            }
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            line.add_css_class("agent-menu-path");
            line.append(&crate::icons::image(icon, 12));
            let path = label(value, "mono");
            path.set_selectable(true);
            // Selectable by pointer, but not the first stop for focus: opening the menu would
            // otherwise park a text caret in the branch name.
            path.set_focusable(false);
            path.set_ellipsize(ellipsize);
            path.set_hexpand(true);
            path.set_xalign(0.0);
            path.set_tooltip_text(Some(value));
            line.append(&path);
            body.append(&line);
        }

        // Place on the wall and the launch brief: one row of keys.
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tools.add_css_class("agent-menu-tools");
        tools.set_homogeneous(true);
        for (icon, caption, tip, delta) in [
            ("arrow-up", "Earlier", "Move this agent one place earlier on the wall", -1),
            ("arrow-down", "Later", "Move this agent one place later on the wall", 1),
        ] {
            let key = tool_key(icon, caption, tip);
            let weak = Rc::downgrade(self);
            let n = name.clone();
            key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    let mut order = ui.ordered.borrow_mut();
                    if let Some(i) = order.iter().position(|s| *s == n) {
                        let next = (i as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
                        order.swap(i, next);
                    }
                    drop(order);
                    ui.layout();
                    ui.save_layout();
                }
            });
            tools.append(&key);
        }
        let brief = tool_key("brief", "Brief", "Read the instructions and peer brief this agent was started with");
        let weak = Rc::downgrade(self);
        let n = name.clone();
        let p = popover.downgrade();
        brief.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            let n = n.clone();
            glib::spawn_future_local(async move {
                match ui.call("session.brief", json!({"session": n})).await {
                    Ok(v) => {
                        let Some((window, sheet)) = ui.sheet("Launch brief", 760) else { return };
                        let view = gtk::TextView::new();
                        view.set_editable(false);
                        view.set_monospace(true);
                        view.set_wrap_mode(gtk::WrapMode::WordChar);
                        view.buffer().set_text(text(&v, "text"));
                        sheet.append(&scrolled(&view));
                        window.present();
                    }
                    Err(e) => ui.show_error(&e.to_string()),
                }
            });
        });
        tools.append(&brief);
        let designs = tool_key("design", "Designs", "This terminal's designs: what its agent drew, and canvases tied to it");
        designs.set_widget_name("agent-menu-designs");
        let weak = Rc::downgrade(self);
        let n = name.clone();
        let p = popover.downgrade();
        designs.connect_clicked(move |_| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            crate::design::show(&ui, Some(n.clone()));
        });
        tools.append(&designs);
        body.append(&tools);

        // Permissions: a switch takes effect when flipped, and flips back if the engine refuses.
        body.append(&section("PERMISSIONS"));
        for (caption, hint, field) in [
            ("Agent bus writes", "Change tasks, notes and settings through Relay", "bus_writes"),
            ("UI control", "Drive the Relay window", "allow_ui"),
        ] {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            line.add_css_class("agent-menu-switch");
            let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
            words.set_hexpand(true);
            let title = label(caption, "agent-menu-caption");
            title.set_xalign(0.0);
            words.append(&title);
            let sub = label(hint, "agent-menu-hint");
            sub.set_xalign(0.0);
            sub.set_wrap(true);
            sub.set_max_width_chars(34);
            words.append(&sub);
            line.append(&words);
            let switch = gtk::Switch::new();
            switch.set_valign(gtk::Align::Center);
            switch.set_active(session[field] == true);
            switch.update_property(&[gtk::accessible::Property::Label(caption)]);
            line.append(&switch);
            body.append(&line);
            let reverting = Rc::new(Cell::new(false));
            let weak = Rc::downgrade(self);
            let n = name.clone();
            switch.connect_active_notify(move |switch| {
                if reverting.get() {
                    return;
                }
                let Some(ui) = weak.upgrade() else { return };
                let on = switch.is_active();
                switch.set_sensitive(false);
                let (switch, n, reverting) = (switch.clone(), n.clone(), reverting.clone());
                glib::spawn_future_local(async move {
                    match ui.call("session.update", json!({"session": n, field: on})).await {
                        Ok(session) => ui.upsert_session(&session),
                        Err(e) => {
                            ui.show_error(&e.to_string());
                            reverting.set(true);
                            switch.set_active(!on);
                            reverting.set(false);
                        }
                    }
                    switch.set_sensitive(true);
                });
            });
        }

        // Model and effort: editable until the agent starts, then only shown.
        body.append(&section("MODEL"));
        if !session["spawned_at"].is_null() {
            for (caption, value, fallback) in [("Model", text(session, "model"), "Provider default"), ("Effort", text(session, "effort"), "Default")] {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                line.add_css_class("agent-menu-fact");
                line.set_tooltip_text(Some("Fixed once the agent has started"));
                let key = label(caption, "agent-menu-hint");
                key.set_xalign(0.0);
                line.append(&key);
                let shown = label(if value.is_empty() { fallback } else { value }, "mono");
                shown.set_hexpand(true);
                shown.set_xalign(1.0);
                shown.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                if value.is_empty() {
                    shown.add_css_class("unset");
                }
                line.append(&shown);
                body.append(&line);
            }
        } else {
            self.launch_options(popover, body, session);
        }

        // Linked repositories.
        if state != "closed" {
            body.append(&section("LINKED REPOS"));
            self.linked_section(body, session);
        }

        // Phone.
        body.append(&section("PHONE"));
        self.phone_section(body, session);

        // Session: pinned under the scrolling part.
        // An exited agent (it quit, crashed or lost its login) starts fresh the same way.
        if matches!(state.as_str(), "restorable" | "exited") {
            let fresh = row_key("refresh", "Start fresh", "Start a new provider conversation in this same session and worktree; the saved context is cleared");
            let confirm = gtk::Revealer::new();
            let weak = Rc::downgrade(self);
            let n = name.clone();
            let p = popover.downgrade();
            confirm_strip(&confirm, "Clear the saved provider conversation and start fresh here?", "Start fresh", None, move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(ui) = weak.upgrade() {
                    ui.agent_op("session.clear_restorable", &n, None);
                }
            });
            let c = confirm.clone();
            fresh.connect_clicked(move |_| c.set_reveal_child(!c.reveals_child()));
            foot.append(&fresh);
            foot.append(&confirm);
        }
        // Shift+clicked panes: Close here closes every one of them, with the same options.
        let batch = self.selection_with(&name);
        let batch = if batch.len() > 1 { batch } else { Vec::new() };
        if !batch.is_empty() {
            let picked = row_key("close", &format!("Clear selection ({} agents)", batch.len()), "Deselect the Shift+clicked panes; Close then closes only this agent");
            let weak = Rc::downgrade(self);
            let p = popover.downgrade();
            picked.connect_clicked(move |_| {
                if let Some(p) = p.upgrade() {
                    p.popdown();
                }
                if let Some(ui) = weak.upgrade() {
                    ui.clear_selection();
                }
            });
            foot.append(&picked);
        }
        let close = if batch.is_empty() {
            row_key("power", "Close session…", "Stop the agent and take it off the wall; the worktree is kept unless you choose otherwise, and the branch unless its work is already merged")
        } else {
            row_key("power", &format!("Close {} sessions…", batch.len()), "Stop every selected agent and take them off the wall; worktrees are kept unless you choose otherwise, and branches unless their work is already merged")
        };
        close.add_css_class("agent-menu-danger");
        let confirm = gtk::Revealer::new();
        let options = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let remove = gtk::CheckButton::with_label(if batch.is_empty() { "Also remove the worktree" } else { "Also remove their worktrees" });
        let purge = gtk::CheckButton::with_label("Purge build output");
        purge.set_sensitive(false);
        let p = purge.clone();
        remove.connect_toggled(move |remove| {
            p.set_sensitive(remove.is_active());
            if !remove.is_active() {
                p.set_active(false);
            }
        });
        options.append(&remove);
        options.append(&purge);
        let question = if batch.is_empty() {
            "Stop this agent and close its pane? Its branch is kept unless its work is already merged.".to_string()
        } else {
            format!("Stop these {} agents and close their panes? {}. Each branch is kept unless its work is already merged.", batch.len(), batch.join(", "))
        };
        let weak = Rc::downgrade(self);
        let n = name.clone();
        let p = popover.downgrade();
        confirm_strip(&confirm, &question, "Close", Some(&options), move |_| {
            if let Some(p) = p.upgrade() {
                p.popdown();
            }
            if let Some(ui) = weak.upgrade() {
                let names = if batch.is_empty() { vec![n.clone()] } else { batch.clone() };
                ui.close_agents(names, json!({"session": n, "remove_worktree": remove.is_active(), "purge_build": purge.is_active()}));
            }
        });
        let c = confirm.clone();
        close.connect_clicked(move |_| c.set_reveal_child(!c.reveals_child()));
        foot.append(&close);
        foot.append(&confirm);
    }

    /// Model and effort for an agent that has not started yet, saved together.
    fn launch_options(self: &Rc<Self>, popover: &gtk::Popover, body: &gtk::Box, session: &Value) {
        let name = text(session, "name").to_string();
        let options = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let model = gtk::Entry::builder().text(text(session, "model")).placeholder_text("Provider default").hexpand(true).build();
        let effort = gtk::Entry::builder().text(text(session, "effort")).placeholder_text("Default").width_chars(8).build();
        for (caption, entry) in [("Model", &model), ("Effort", &effort)] {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 3);
            let title = label(caption, "agent-menu-field");
            title.set_xalign(0.0);
            column.append(&title);
            column.append(entry);
            column.set_hexpand(caption == "Model");
            options.append(&column);
        }
        body.append(&options);
        let save = gtk::Button::with_label("Save");
        save.add_css_class("primary");
        save.add_css_class("agent-menu-save");
        save.set_halign(gtk::Align::End);
        save.set_sensitive(false);
        let original = (text(session, "model").to_string(), text(session, "effort").to_string());
        let dirty: Rc<dyn Fn()> = Rc::new({
            let (model, effort, save, original) = (model.downgrade(), effort.downgrade(), save.downgrade(), original.clone());
            move || {
                let (Some(model), Some(effort), Some(save)) = (model.upgrade(), effort.upgrade(), save.upgrade()) else { return };
                save.set_sensitive((model.text().to_string(), effort.text().to_string()) != original);
            }
        });
        for entry in [&model, &effort] {
            let dirty = dirty.clone();
            entry.connect_changed(move |_| dirty());
        }
        let weak = Rc::downgrade(self);
        let p = popover.downgrade();
        save.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            // Only what changed: the engine refuses an empty model or effort.
            let mut payload = json!({"session": name});
            for (field, entry, old) in [("model", &model, &original.0), ("effort", &effort, &original.1)] {
                let value = entry.text().trim().to_string();
                if !value.is_empty() && value != *old {
                    payload[field] = value.into();
                }
            }
            if payload.as_object().is_some_and(|fields| fields.len() == 1) {
                ui.show_error("Nothing to save: a model or effort cannot be cleared back to the provider default.");
                return;
            }
            key.set_sensitive(false);
            let key = key.clone();
            let p = p.clone();
            glib::spawn_future_local(async move {
                match ui.call("session.update", payload).await {
                    Ok(session) => {
                        ui.upsert_session(&session);
                        if let Some(p) = p.upgrade() {
                            p.popdown();
                        }
                    }
                    Err(e) => {
                        ui.show_error(&e.to_string());
                        key.set_sensitive(true);
                    }
                }
            });
        });
        body.append(&save);
    }

    // ------------------------------------------------------------------ linked repositories

    /// Other repositories cloned into this agent's checkout (`session.repo.*`): the list, read
    /// when the popover opens, and a field to link one more. A removal that would lose unpushed
    /// work asks first.
    fn linked_section(self: &Rc<Self>, body: &gtk::Box, session: &Value) {
        let name = text(session, "name").to_string();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        body.append(&list);
        let add = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let url = gtk::Entry::new();
        url.set_placeholder_text(Some("owner/repo or a git URL"));
        url.set_hexpand(true);
        url.update_property(&[gtk::accessible::Property::Label("Repository to link")]);
        add.append(&url);
        let link = gtk::Button::with_label("Link");
        link.add_css_class("quiet");
        link.set_tooltip_text(Some("Clone it into this agent's worktree under .relay/linked/, where the agent can read and edit it"));
        add.append(&link);
        body.append(&add);
        let status = label("", "agent-menu-hint");
        status.set_wrap(true);
        status.set_max_width_chars(36);
        status.set_xalign(0.0);
        status.set_visible(false);
        body.append(&status);
        self.load_linked(&name, &list);

        let weak = Rc::downgrade(self);
        let (n, l, st) = (name.clone(), list.downgrade(), status.downgrade());
        let start: Rc<dyn Fn()> = Rc::new({
            let (url, link) = (url.downgrade(), link.downgrade());
            move || {
                let (Some(ui), Some(url), Some(link)) = (weak.upgrade(), url.upgrade(), link.upgrade()) else { return };
                let wanted = url.text().trim().to_string();
                if wanted.is_empty() {
                    return;
                }
                link.set_sensitive(false);
                url.set_sensitive(false);
                if let Some(st) = st.upgrade() {
                    st.set_text(&format!("Cloning {wanted}…"));
                    st.set_visible(true);
                }
                let (n, l, st) = (n.clone(), l.clone(), st.clone());
                glib::spawn_future_local(async move {
                    let result = ui.call("session.repo.add", json!({"session": n, "url": wanted})).await;
                    link.set_sensitive(true);
                    url.set_sensitive(true);
                    let said = match &result {
                        Ok(repo) => {
                            url.set_text("");
                            format!("Linked {} at {}", text(repo, "name"), text(repo, "path"))
                        }
                        Err(e) => format!("Could not link it: {e}"),
                    };
                    if let Some(st) = st.upgrade() {
                        st.set_text(&said);
                    }
                    if let Some(l) = l.upgrade() {
                        ui.load_linked(&n, &l);
                    }
                });
            }
        });
        let go = start.clone();
        link.connect_clicked(move |_| go());
        url.connect_activate(move |_| start());
    }

    /// Fill `list` with this session's linked repositories, each with a remove key.
    fn load_linked(self: &Rc<Self>, session: &str, list: &gtk::Box) {
        let ui = self.clone();
        let (session, list) = (session.to_string(), list.downgrade());
        glib::spawn_future_local(async move {
            let result = ui.call("session.repo.list", json!({"session": session})).await;
            let Some(list) = list.upgrade() else { return };
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let repos = match result {
                Ok(out) => out["repos"].as_array().cloned().unwrap_or_default(),
                Err(e) => {
                    list.append(&label(&format!("Could not list them: {e}"), "agent-menu-hint"));
                    return;
                }
            };
            if repos.is_empty() {
                list.append(&label("None. Link another repository this agent needs to read or edit.", "agent-menu-hint"));
            }
            for repo in repos {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
                words.set_hexpand(true);
                let repo_name = text(&repo, "name").to_string();
                let title = label(&repo_name, "agent-menu-caption");
                title.set_xalign(0.0);
                words.append(&title);
                let branch = text(&repo, "branch");
                let mut hint = if branch.is_empty() { "detached HEAD".to_string() } else { branch.to_string() };
                if repo["unpushed"].is_object() {
                    hint.push_str(" · has unpushed work");
                }
                let sub = label(&hint, "agent-menu-hint");
                sub.set_xalign(0.0);
                words.append(&sub);
                line.set_tooltip_text(Some(text(&repo, "path")));
                line.append(&words);
                let remove = gtk::Button::new();
                remove.set_child(Some(&crate::icons::image("trash", 16)));
                remove.add_css_class("quiet");
                remove.set_valign(gtk::Align::Center);
                remove.set_tooltip_text(Some("Delete this clone; you are asked first if it holds work that is not pushed"));
                remove.update_property(&[gtk::accessible::Property::Label(&format!("Remove {repo_name}"))]);
                let weak = Rc::downgrade(&ui);
                let (s, l) = (session.clone(), list.downgrade());
                remove.connect_clicked(move |key| {
                    let Some(ui) = weak.upgrade() else { return };
                    key.set_sensitive(false);
                    let (s, l, repo_name, key) = (s.clone(), l.clone(), repo_name.clone(), key.clone());
                    glib::spawn_future_local(async move {
                        ui.remove_linked(&s, &repo_name).await;
                        key.set_sensitive(true);
                        if let Some(l) = l.upgrade() {
                            ui.load_linked(&s, &l);
                        }
                    });
                });
                line.append(&remove);
                list.append(&line);
            }
        });
    }

    /// `session.repo.remove`; when the clone holds unpushed work, say what and delete it only on
    /// a yes.
    async fn remove_linked(self: &Rc<Self>, session: &str, repo: &str) {
        let payload = json!({"session": session, "name": repo});
        match self.call("session.repo.remove", payload.clone()).await {
            Ok(_) => {}
            Err(Error::Bus(error)) if error.code == "session.repo_unpushed" => {
                let dialog = gtk::AlertDialog::builder()
                    .message(format!("Delete {repo} with work that is not pushed?"))
                    .detail(format!("{}. To keep it, commit and push from inside the repository first.", error.message))
                    .buttons(["Keep it", "Delete anyway"])
                    .cancel_button(0)
                    .default_button(0)
                    .modal(true)
                    .build();
                if dialog.choose_future(Some(&self.window)).await.ok() == Some(1) {
                    let mut payload = payload;
                    payload["discard_changes"] = json!(true);
                    if let Err(e) = self.call("session.repo.remove", payload).await {
                        self.show_error(&format!("Could not remove {repo}: {e}"));
                    }
                }
            }
            Err(e) => self.show_error(&format!("Could not remove {repo}: {e}")),
        }
    }

    // ------------------------------------------------------------------ build & install on phone

    fn phone_section(self: &Rc<Self>, body: &gtk::Box, session: &Value) {
        let name = text(session, "name").to_string();
        let install = row_key("phone-install", "Build & install on phone", "Build this agent's branch with Gradle, install it on the connected phone over ADB and launch it");
        body.append(&install);
        let devices = gtk::DropDown::from_strings(&[]);
        devices.set_visible(false);
        devices.add_css_class("agent-menu-devices");
        body.append(&devices);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        line.add_css_class("agent-menu-phone");
        let status = label("", "agent-menu-hint");
        status.set_wrap(true);
        status.set_max_width_chars(36);
        status.set_hexpand(true);
        status.set_selectable(true);
        line.append(&status);
        let stop = gtk::Button::with_label("Stop");
        stop.add_css_class("quiet");
        stop.set_valign(gtk::Align::Start);
        line.append(&stop);
        body.append(&line);
        let phone = with(|a| {
            let phone = a.phone.entry(name.clone()).or_default();
            phone.view = Some(status.downgrade());
            phone.stop = Some(stop.downgrade());
            phone.install = Some(install.downgrade());
            phone.clone()
        });
        status.set_text(&phone.status);
        status.set_visible(!phone.status.is_empty());
        stop.set_visible(phone.busy && phone.run.is_some());
        install.set_sensitive(!phone.busy);
        let weak = Rc::downgrade(self);
        let n = name.clone();
        stop.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            let Some(run) = with(|a| a.phone.get(&n).and_then(|p| p.run)) else { return };
            key.set_sensitive(false);
            let key = key.clone();
            glib::spawn_future_local(async move {
                if let Err(e) = ui.call("device.run.stop", json!({"run_id": run})).await {
                    ui.show_error(&e.to_string());
                }
                key.set_sensitive(true);
            });
        });
        let weak = Rc::downgrade(self);
        let n = name.clone();
        let serials: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let (project, worktree) = (session["project_id"].as_i64().unwrap_or(0), text(session, "worktree").to_string());
        install.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            key.set_sensitive(false);
            let (key, devices, serials, n, worktree) = (key.clone(), devices.clone(), serials.clone(), n.clone(), worktree.clone());
            glib::spawn_future_local(async move {
                let chosen = if devices.is_visible() {
                    serials.borrow().get(devices.selected() as usize).cloned()
                } else {
                    None
                };
                let device = match chosen {
                    Some(device) => device,
                    None => {
                        phone_status(&n, "Looking for a phone…", true, None);
                        let listed = match ui.call("device.list", json!({})).await {
                            Ok(v) => rows(&v, "devices"),
                            Err(e) => {
                                phone_status(&n, &format!("Could not list devices: {e}"), false, None);
                                key.set_sensitive(true);
                                return;
                            }
                        };
                        let ready: Vec<(String, String)> = listed.iter().filter(|d| text(d, "state") == "device")
                            .map(|d| (text(d, "serial").to_string(), text(d, "model").to_string())).collect();
                        if ready.is_empty() {
                            let message = if listed.iter().any(|d| text(d, "state") == "unauthorized") {
                                "A phone is connected but has not authorized this computer. Accept the USB debugging prompt on the phone, then try again."
                            } else if !listed.is_empty() {
                                "The connected phone is not ready (offline). Reconnect the cable, then try again."
                            } else {
                                "No phone found. Connect one with USB debugging enabled, then try again."
                            };
                            phone_status(&n, message, false, None);
                            key.set_sensitive(true);
                            return;
                        }
                        if ready.len() > 1 {
                            let model = gtk::StringList::new(&ready.iter().map(|(serial, model)| format!("{model} · {serial}")).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>());
                            devices.set_model(Some(&model));
                            devices.set_visible(true);
                            *serials.borrow_mut() = ready;
                            phone_status(&n, "Several phones are connected. Choose one, then press Build & install again.", false, None);
                            key.set_sensitive(true);
                            return;
                        }
                        ready[0].clone()
                    }
                };
                // The key stays off while the run lives: Stop ends it, then it can run again.
                ui.install_on_phone(n, project, worktree, device).await;
                key.set_sensitive(true);
            });
        });
    }

    /// `device.run` against this agent's worktree, on a connection of its own so Gradle and
    /// logcat output never queue behind the shell's control traffic.
    async fn install_on_phone(self: &Rc<Self>, name: String, project: i64, worktree: String, (serial, model): (String, String)) {
        phone_status(&name, &format!("Building {name}'s branch for {model}…"), true, None);
        let (client, notices) = match Client::connect(&self.rt, self.path.clone()).await {
            Ok(connected) => connected,
            Err(e) => {
                phone_status(&name, &e.to_string(), false, None);
                return;
            }
        };
        if let Err(e) = client.request(&self.rt, "bus.subscribe", json!({"events": ["run.changed"]})).await {
            phone_status(&name, &e.to_string(), false, None);
            return;
        }
        let run = match client.request(&self.rt, "device.run", json!({"project_id": project, "device": serial, "worktree": worktree})).await {
            Ok(run) => run["id"].as_i64(),
            Err(e) => {
                phone_status(&name, &phone_error(&e), false, None);
                return;
            }
        };
        phone_status(&name, &format!("Building for {model}…"), true, run);
        self.refresh_status();
        let mut last = String::new();
        let mut installed = false;
        while let Ok(notice) = notices.recv().await {
            match notice {
                Notice::Frame(frame) if frame.stream == "logcat" => {
                    let line = frame.data.as_str().unwrap_or("").trim();
                    if line.is_empty() {
                        continue;
                    }
                    last = line.chars().take(220).collect();
                    if !installed {
                        phone_status(&name, &format!("Building for {model} · {last}"), true, run);
                    }
                }
                Notice::Event(event) if event.ev == "run.changed" && event.payload["id"].as_i64() == run && run.is_some() => {
                    match text(&event.payload, "state") {
                        "running" => {
                            installed = true;
                            phone_status(&name, &format!("Installed and launched on {model}. Logcat is attached; Stop ends it."), true, run);
                            self.show_error(&format!("{name}: installed and launched on {model}"));
                        }
                        "failed" => {
                            let detail = if last.is_empty() { String::from("see Devices for the full log") } else { last.clone() };
                            phone_status(&name, &format!("Build or install failed: {detail}"), false, run);
                            self.show_error(&format!("{name}: phone install failed"));
                            break;
                        }
                        "stopped" | "finished" | "done" => {
                            phone_status(&name, if installed { "Stopped. The app stays installed." } else { "Stopped before the install finished." }, false, run);
                            break;
                        }
                        _ => {}
                    }
                }
                Notice::Disconnected(e) => {
                    phone_status(&name, &e.to_string(), false, run);
                    break;
                }
                _ => {}
            }
        }
        drop(client);
    }
}

/// Record and show where a phone install is.
fn phone_status(name: &str, status: &str, busy: bool, run: Option<i64>) {
    let phone = with(|a| {
        let phone = a.phone.entry(name.to_string()).or_default();
        phone.status = status.to_string();
        phone.busy = busy;
        // A new install starts with no run, so Stop can never reach the previous one.
        phone.run = run;
        phone.clone()
    });
    if let Some(view) = phone.view.as_ref().and_then(|view| view.upgrade()) {
        view.set_text(status);
        view.set_visible(!status.is_empty());
    }
    if let Some(stop) = phone.stop.as_ref().and_then(|stop| stop.upgrade()) {
        stop.set_visible(busy && phone.run.is_some());
    }
    // A popover opened while a run was live shows its own key; it comes back with the run's end.
    if let Some(install) = phone.install.as_ref().and_then(|install| install.upgrade()) {
        install.set_sensitive(!busy);
    }
}

/// An inline confirmation: a sentence, optional extra choices, and Cancel / the action.
fn confirm_strip(revealer: &gtk::Revealer, caption: &str, action: &str, extra: Option<&gtk::Box>, run: impl Fn(&gtk::Button) + 'static) {
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(120);
    let strip = gtk::Box::new(gtk::Orientation::Vertical, 6);
    strip.add_css_class("agent-menu-confirm");
    let words = label(caption, "agent-menu-caption");
    words.set_wrap(true);
    words.set_max_width_chars(36);
    strip.append(&words);
    if let Some(extra) = extra {
        strip.append(extra);
    }
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    keys.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("quiet");
    let go = gtk::Button::with_label(action);
    go.add_css_class("agent-menu-confirm-go");
    // Weak: Cancel sits inside the revealer.
    let r = revealer.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(r) = r.upgrade() {
            r.set_reveal_child(false);
        }
    });
    go.connect_clicked(move |key| run(key));
    keys.append(&cancel);
    keys.append(&go);
    strip.append(&keys);
    revealer.set_child(Some(&strip));
}
