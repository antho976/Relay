//! The layered guardrail editor. One form, three scopes — global, one workspace, one project —
//! and every field says whether it is set at this level or inherited, and from where. Only
//! the fields you changed are saved, so an untouched field keeps following its parent.
use crate::app::{button, label, text, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    Global,
    Workspace(i64),
    Project(i64),
}

impl Scope {
    fn payload(self) -> Value {
        match self {
            Scope::Global => json!({}),
            Scope::Workspace(id) => json!({"workspace_id": id}),
            Scope::Project(id) => json!({"project_id": id}),
        }
    }
    /// The `GuardrailLayer` name the engine reports for this scope.
    fn layer(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Workspace(_) => "workspace",
            Scope::Project(_) => "project",
        }
    }
    fn index(self) -> usize {
        match self {
            Scope::Global => 0,
            Scope::Workspace(_) => 1,
            Scope::Project(_) => 2,
        }
    }
}

enum Kind {
    Count(f64, f64),
    Percent,
    Toggle,
    List,
}

struct Spec {
    path: &'static str,
    title: &'static str,
    hint: &'static str,
    kind: Kind,
}

const GROUPS: &[(&str, &[Spec])] = &[
    ("Commit size", &[
        Spec { path: "caps.files", title: "Files per commit", hint: "A commit that changes more files is refused.", kind: Kind::Count(1., 1_000_000.) },
        Spec { path: "caps.lines", title: "Lines per commit", hint: "Added plus removed lines, across the whole commit.", kind: Kind::Count(1., 100_000_000.) },
    ]),
    ("Large rewrites", &[
        Spec { path: "destructive_write.min_removed_lines", title: "Removed lines", hint: "A write that removes more lines than this is held for you.", kind: Kind::Count(0., 1_000_000.) },
        Spec { path: "destructive_write.min_removed_pct", title: "Removed share of a file (%)", hint: "Or removes more than this share of an existing file.", kind: Kind::Percent },
        Spec { path: "destructive_write.min_file_lines", title: "Ignore the share rule below (lines)", hint: "One line of a two-line file is 50%; short files are judged by lines only.", kind: Kind::Count(0., 1_000_000.) },
        Spec { path: "destructive_write.allow_if_recoverable", title: "Allow rewrites git can restore", hint: "Committed, unmodified or ignored files can always come back.", kind: Kind::Toggle },
    ]),
    ("Paths and commands", &[
        Spec { path: "protected_paths", title: "Protected paths", hint: "Worktree-relative paths or globs agents may not write. One per line.", kind: Kind::List },
        Spec { path: "denied_commands", title: "Denied commands", hint: "Commands agents may not run, matched word by word. One per line.", kind: Kind::List },
        Spec { path: "allowed_write_roots", title: "Extra write roots", hint: "Absolute directories outside the worktree agents may write to. One per line.", kind: Kind::List },
    ]),
    ("Pushes", &[
        Spec { path: "check_on_push", title: "Check an agent's push", hint: "Runs the project's build command, or its detected tests, first. A failing check refuses the push.", kind: Kind::Toggle },
    ]),
];

fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    path.split('.').fold(value, |v, part| &v[part])
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::Array(items) if items.is_empty() => "none".into(),
        Value::Array(items) if items.len() == 1 => format!("1 entry ({})", items[0].as_str().unwrap_or("")),
        Value::Array(items) => format!("{} entries", items.len()),
        Value::Bool(true) => "on".into(),
        Value::Bool(false) => "off".into(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 => format!("{f:.0}"),
            Some(f) => format!("{f:.1}"),
            None => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn layer_name(layer: &str) -> &'static str {
    match layer {
        "global" => "Global",
        "workspace" => "the workspace",
        "project" => "this project",
        _ => "Relay's default",
    }
}

enum Input {
    Spin(gtk::SpinButton),
    Switch(gtk::Switch),
    List(gtk::TextView),
}

impl Input {
    fn get(&self) -> Value {
        match self {
            Input::Spin(spin) if spin.digits() > 0 => json!(spin.value()),
            Input::Spin(spin) => json!(spin.value_as_int()),
            Input::Switch(switch) => json!(switch.is_active()),
            Input::List(view) => {
                let buffer = view.buffer();
                let all = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                json!(all.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>())
            }
        }
    }
    fn set(&self, value: &Value) {
        match self {
            Input::Spin(spin) => spin.set_value(value.as_f64().unwrap_or(0.)),
            Input::Switch(switch) => switch.set_active(value.as_bool().unwrap_or(false)),
            Input::List(view) => view.buffer().set_text(
                &value.as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"),
            ),
        }
    }
    fn on_change(&self, f: impl Fn() + 'static) {
        match self {
            Input::Spin(spin) => {
                spin.connect_value_changed(move |_| f());
            }
            Input::Switch(switch) => {
                switch.connect_active_notify(move |_| f());
            }
            Input::List(view) => {
                view.buffer().connect_changed(move |_| f());
            }
        }
    }
}

struct Row {
    spec: &'static Spec,
    input: Input,
    source: gtk::Label,
    reset: gtk::Button,
    /// Reset was pressed: save sends `null`, and the value follows the parent again.
    cleared: Cell<bool>,
}

pub struct Editor {
    ui: Weak<Ui>,
    scope: Cell<Scope>,
    /// The last `guardrail.config.layers` answer.
    layers: RefCell<Value>,
    rows: Vec<Row>,
    toggles: Vec<gtk::ToggleButton>,
    target: gtk::DropDown,
    targets: RefCell<Vec<i64>>,
    note: gtk::Label,
    status: gtk::Label,
    save: gtk::Button,
    discard: gtk::Button,
    picker: gtk::Box,
    /// Set while the form is filled programmatically, so it does not read as an edit.
    filling: Cell<bool>,
    generation: Cell<u64>,
}

impl Editor {
    fn dirty(&self) -> usize {
        let layers = self.layers.borrow();
        let effective = &layers["effective"];
        if effective.is_null() {
            return 0;
        }
        self.rows
            .iter()
            .filter(|row| row.cleared.get() || !same(&row.input.get(), at(effective, row.spec.path)))
            .count()
    }

    fn refresh_row(&self, row: &Row) {
        let layers = self.layers.borrow();
        if layers["effective"].is_null() {
            // Nothing read for this scope (yet): no caption, and nothing to reset.
            row.source.set_text("");
            row.reset.set_visible(false);
            return;
        }
        let layer = self.scope.get().layer();
        let source = layers["sources"][row.spec.path].as_str().unwrap_or("default");
        let inherited = at(&layers["inherited"], row.spec.path);
        let here = source == layer;
        let changed = !same(&row.input.get(), at(&layers["effective"], row.spec.path));
        let (caption, class) = if row.cleared.get() {
            (format!("Will follow the inherited value ({}) once saved", shown(inherited)), "pending")
        } else if changed {
            ("Changed · not saved".to_string(), "pending")
        } else if here {
            (format!("Set here · would otherwise be {}", shown(inherited)), "here")
        } else {
            (format!("Inherited from {}", layer_name(source)), "inherited")
        };
        row.source.set_text(&caption);
        for name in ["pending", "here", "inherited"] {
            row.source.remove_css_class(name);
        }
        row.source.add_css_class(class);
        row.reset.set_visible((here || changed) && !row.cleared.get());
        row.reset.set_tooltip_text(Some(&format!("Use the inherited value: {}", shown(inherited))));
    }

    fn refresh(&self) {
        for row in &self.rows {
            self.refresh_row(row);
        }
        let dirty = self.dirty();
        self.save.set_sensitive(dirty > 0);
        self.discard.set_sensitive(dirty > 0);
        // Switching scope with edits pending would apply them to the wrong layer.
        self.picker.set_sensitive(dirty == 0);
        self.picker.set_tooltip_text((dirty > 0).then_some("Save or discard your changes first"));
        if dirty > 0 {
            self.status.set_text(&format!("{dirty} unsaved change{}", if dirty == 1 { "" } else { "s" }));
        } else if self.status.text().contains("unsaved change") {
            self.status.set_text("");
        }
    }

    fn fill(&self) {
        let layers = self.layers.borrow().clone();
        self.filling.set(true);
        for row in &self.rows {
            row.cleared.set(false);
            row.input.set(at(&layers["effective"], row.spec.path));
        }
        self.filling.set(false);
        self.refresh();
    }

    fn describe_scope(&self, ui: &Ui) {
        let note = match self.scope.get() {
            Scope::Global => "Applies to every workspace and project, unless one of them sets its own value.".to_string(),
            Scope::Workspace(id) => {
                let name = ui.workspaces.borrow().iter().find(|w| w["id"].as_i64() == Some(id)).map(|w| text(w, "name").to_string()).unwrap_or_default();
                format!("Applies to every project in {}, unless the project sets its own value. Unset fields follow Global.", if name.is_empty() { "this workspace".into() } else { name })
            }
            Scope::Project(id) => {
                let name = ui.projects.borrow().iter().find(|p| p["id"].as_i64() == Some(id)).map(|p| text(p, "name").to_string()).unwrap_or_default();
                format!("Applies to {} only. Unset fields follow its workspace, then Global.", if name.is_empty() { "this project".into() } else { name })
            }
        };
        self.note.set_text(&note);
    }

    /// Point the target list at the scope's kind, selecting `scope`'s own id.
    fn fill_targets(&self, ui: &Ui) {
        let scope = self.scope.get();
        let (names, ids): (Vec<String>, Vec<i64>) = match scope {
            Scope::Global => (Vec::new(), Vec::new()),
            Scope::Workspace(_) => ui.workspaces.borrow().iter()
                .filter_map(|w| Some((text(w, "name").to_string(), w["id"].as_i64()?)))
                .unzip(),
            Scope::Project(_) => {
                let spaces = ui.workspaces.borrow();
                ui.projects.borrow().iter()
                    .filter_map(|p| {
                        let space = spaces.iter().find(|w| w["id"] == p["workspace_id"]);
                        let name = match space {
                            Some(space) => format!("{} / {}", text(space, "name"), text(p, "name")),
                            None => text(p, "name").to_string(),
                        };
                        Some((name, p["id"].as_i64()?))
                    })
                    .unzip()
            }
        };
        let selected = match scope {
            Scope::Global => None,
            Scope::Workspace(id) | Scope::Project(id) => ids.iter().position(|candidate| *candidate == id),
        };
        let list = gtk::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>());
        self.filling.set(true);
        self.target.set_model(Some(&list));
        self.target.set_selected(selected.map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
        self.filling.set(false);
        *self.targets.borrow_mut() = ids;
        self.target.set_visible(scope != Scope::Global);
    }

    /// Read the layers again; `done` is the status once they are in (a save's confirmation
    /// would otherwise be cleared by the reload it starts).
    fn load(self: &Rc<Self>, done: &'static str) {
        let Some(ui) = self.ui.upgrade() else { return };
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        let scope = self.scope.get();
        self.describe_scope(&ui);
        // Forget the previous scope's layers until this one's arrive: a Reset judged against
        // them would clear a field at the new scope that was never shown. With no layers,
        // dirty() is 0, so the Reset keys hide and Save and Discard turn off; a failed read
        // leaves it that way, inputs off too.
        *self.layers.borrow_mut() = Value::Null;
        for row in &self.rows {
            row.cleared.set(false);
            row.input_widget_sensitive(false);
        }
        self.refresh();
        self.status.set_text("Loading…");
        let editor = self.clone();
        glib::spawn_future_local(async move {
            let result = ui.call("guardrail.config.layers", scope.payload()).await;
            if editor.generation.get() != generation {
                return;
            }
            match result {
                Ok(layers) => {
                    *editor.layers.borrow_mut() = layers;
                    for row in &editor.rows {
                        row.input_widget_sensitive(true);
                    }
                    editor.fill();
                    editor.status.set_text(done);
                }
                Err(e) => editor.status.set_text(&format!(
                    "Cannot read guardrail layers: {e}. A Relay engine older than this window does not know them; restart it after rebuilding."
                )),
            }
        });
    }

    fn patch(&self) -> Value {
        let layers = self.layers.borrow();
        let mut patch = Map::new();
        for row in &self.rows {
            let value = if row.cleared.get() {
                Value::Null
            } else {
                let value = row.input.get();
                if same(&value, at(&layers["effective"], row.spec.path)) {
                    continue;
                }
                value
            };
            let mut parts: Vec<&str> = row.spec.path.split('.').collect();
            let leaf = parts.pop().unwrap_or_default();
            let mut node = &mut patch;
            for part in parts {
                node = node.entry(part.to_string()).or_insert_with(|| json!({})).as_object_mut().expect("patch groups are objects");
            }
            node.insert(leaf.to_string(), value);
        }
        Value::Object(patch)
    }

    fn save(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        // Nothing read for this scope: every field would read as changed against nothing.
        if self.layers.borrow()["effective"].is_null() {
            return;
        }
        let patch = self.patch();
        if patch.as_object().is_none_or(Map::is_empty) {
            self.status.set_text("Nothing to save.");
            return;
        }
        let scope = self.scope.get();
        let mut payload = scope.payload();
        payload["patch"] = patch;
        self.save.set_sensitive(false);
        self.status.set_text("Saving…");
        let editor = self.clone();
        glib::spawn_future_local(async move {
            match ui.call("guardrail.config.set", payload).await {
                Ok(_) => {
                    let saved = match scope {
                        Scope::Global => "Saved. Applies everywhere it is not overridden.",
                        Scope::Workspace(_) => "Saved for this workspace.",
                        Scope::Project(_) => "Saved for this project.",
                    };
                    if editor.scope.get() == scope {
                        editor.load(saved);
                    } else {
                        editor.status.set_text(saved);
                    }
                }
                Err(e) => {
                    editor.status.set_text(&format!("Not saved: {e}"));
                    editor.save.set_sensitive(true);
                }
            }
        });
    }

    fn set_scope(self: &Rc<Self>, scope: Scope) {
        if self.scope.get() == scope {
            return;
        }
        self.scope.set(scope);
        if let Some(ui) = self.ui.upgrade() {
            self.fill_targets(&ui);
        }
        self.load("");
    }
}

impl Row {
    fn input_widget_sensitive(&self, on: bool) {
        match &self.input {
            Input::Spin(w) => w.set_sensitive(on),
            Input::Switch(w) => w.set_sensitive(on),
            Input::List(w) => w.set_sensitive(on),
        }
    }
}

/// The scope a project's Settings page opens on: its own workspace and project when chosen.
fn default_target(ui: &Ui, kind: usize) -> Scope {
    let project = ui.project.get();
    match kind {
        0 => Scope::Global,
        1 => {
            let workspace = ui.projects.borrow().iter()
                .find(|p| p["id"].as_i64() == Some(project))
                .and_then(|p| p["workspace_id"].as_i64())
                .or_else(|| ui.workspaces.borrow().first().and_then(|w| w["id"].as_i64()));
            workspace.map_or(Scope::Global, Scope::Workspace)
        }
        _ if project > 0 => Scope::Project(project),
        _ => ui.projects.borrow().first().and_then(|p| p["id"].as_i64()).map_or(Scope::Global, Scope::Project),
    }
}

/// The editor, opened on the global layer, for the Settings page. The scope picker moves it.
pub fn editor(ui: &Rc<Ui>) -> gtk::Box {
    build(ui, Scope::Global).0
}

/// Build the editor. The returned widget keeps the editor alive; nothing else needs to.
fn build(ui: &Rc<Ui>, scope: Scope) -> (gtk::Box, Weak<Editor>) {
    use crate::tools::settings::{card, row, section};
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("guardrail-editor");

    let picker = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    picker.add_css_class("guardrail-scope");
    let toggles: Vec<gtk::ToggleButton> = ["Global", "Workspace", "Project"]
        .iter()
        .map(|caption| {
            let toggle = gtk::ToggleButton::with_label(caption);
            toggle.add_css_class("guardrail-scope-key");
            toggle
        })
        .collect();
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    keys.add_css_class("settings-segmented");
    for toggle in &toggles {
        if toggle != &toggles[0] {
            toggle.set_group(Some(&toggles[0]));
        }
        keys.append(toggle);
    }
    picker.append(&keys);
    let target = gtk::DropDown::from_strings(&[] as &[&str]);
    target.add_css_class("settings-select");
    target.add_css_class("guardrail-target");
    target.set_size_request(200, -1);
    picker.append(&target);
    let scope_section = section(&root, "Applies to", None::<&gtk::Widget>);
    let scope_card = card(&scope_section);
    let (_, note) = row(&scope_card, "Level", "", Some(&picker));
    note.add_css_class("guardrail-scope-note");
    note.set_visible(true);

    let mut built = Vec::new();
    for (group, specs) in GROUPS {
        let block = section(&root, group, None::<&gtk::Widget>);
        let block = card(&block);
        block.add_css_class("guardrail-group");
        for spec in specs.iter() {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 12);
            row.add_css_class("settings-row");
            row.add_css_class("guardrail-field");
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            let copy = gtk::Box::new(gtk::Orientation::Vertical, 4);
            copy.set_hexpand(true);
            copy.set_valign(gtk::Align::Center);
            copy.append(&label(spec.title, "settings-row-title"));
            let hint = label(spec.hint, "settings-row-hint");
            hint.set_wrap(true);
            copy.append(&hint);
            let source = label("", "guardrail-source");
            source.set_wrap(true);
            copy.append(&source);
            line.append(&copy);
            let reset = button("Reset", "settings-link");
            reset.set_valign(gtk::Align::Center);
            reset.set_visible(false);
            let input = match spec.kind {
                Kind::Count(min, max) => {
                    let spin = gtk::SpinButton::with_range(min, max, 1.);
                    spin.set_digits(0);
                    Input::Spin(spin)
                }
                Kind::Percent => {
                    let spin = gtk::SpinButton::with_range(0., 100., 0.5);
                    spin.set_digits(2);
                    Input::Spin(spin)
                }
                Kind::Toggle => Input::Switch(gtk::Switch::new()),
                Kind::List => {
                    let view = gtk::TextView::new();
                    view.set_monospace(true);
                    view.set_wrap_mode(gtk::WrapMode::WordChar);
                    view.set_top_margin(8);
                    view.set_bottom_margin(8);
                    view.set_left_margin(10);
                    view.set_right_margin(10);
                    Input::List(view)
                }
            };
            line.append(&reset);
            let name = format!("guardrail-field:{}", spec.path);
            match &input {
                Input::Spin(w) => w.set_widget_name(&name),
                Input::Switch(w) => w.set_widget_name(&name),
                Input::List(w) => w.set_widget_name(&name),
            }
            match &input {
                Input::Spin(spin) => {
                    spin.set_valign(gtk::Align::Center);
                    spin.set_width_chars(8);
                    line.append(spin);
                }
                Input::Switch(switch) => {
                    switch.add_css_class("settings-switch");
                    switch.set_valign(gtk::Align::Center);
                    line.append(switch);
                }
                Input::List(_) => {}
            }
            row.append(&line);
            if let Input::List(view) = &input {
                let scroll = crate::app::scrolled(view);
                scroll.set_min_content_height(84);
                scroll.set_vexpand(false);
                scroll.add_css_class("guardrail-list");
                scroll.set_overflow(gtk::Overflow::Hidden);
                row.append(&scroll);
            }
            block.append(&row);
            built.push(Row { spec, input, source, reset, cleared: Cell::new(false) });
        }
    }

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    footer.add_css_class("guardrail-footer");
    let status = label("", "settings-caption");
    status.set_hexpand(true);
    status.set_wrap(true);
    footer.append(&status);
    let discard = button("Discard", "settings-button");
    let save = button("Save guardrails", "primary");
    save.add_css_class("settings-primary");
    save.set_widget_name("guardrail-save");
    discard.set_sensitive(false);
    save.set_sensitive(false);
    footer.append(&discard);
    footer.append(&save);
    root.append(&footer);

    let editor = Rc::new(Editor {
        ui: Rc::downgrade(ui),
        scope: Cell::new(scope),
        layers: RefCell::new(Value::Null),
        rows: built,
        toggles,
        target,
        targets: RefCell::default(),
        note,
        status,
        save,
        discard,
        picker,
        filling: Cell::new(false),
        generation: Cell::new(0),
    });

    for (index, row) in editor.rows.iter().enumerate() {
        let weak = Rc::downgrade(&editor);
        row.input.on_change(move || {
            let Some(editor) = weak.upgrade() else { return };
            if editor.filling.get() {
                return;
            }
            let row = &editor.rows[index];
            if row.cleared.get() && !same(&row.input.get(), at(&editor.layers.borrow()["inherited"], row.spec.path)) {
                row.cleared.set(false);
            }
            editor.refresh();
        });
        let weak = Rc::downgrade(&editor);
        row.reset.connect_clicked(move |_| {
            let Some(editor) = weak.upgrade() else { return };
            let row = &editor.rows[index];
            let layers = editor.layers.borrow().clone();
            if layers["effective"].is_null() {
                return;
            }
            let layer = editor.scope.get().layer();
            let here = layers["sources"][row.spec.path].as_str() == Some(layer);
            editor.filling.set(true);
            // Set here: go back to the inherited value. Merely edited: back to what is saved.
            row.input.set(at(if here { &layers["inherited"] } else { &layers["effective"] }, row.spec.path));
            editor.filling.set(false);
            row.cleared.set(here);
            editor.refresh();
        });
    }
    for (index, toggle) in editor.toggles.iter().enumerate() {
        let weak = Rc::downgrade(&editor);
        toggle.connect_toggled(move |toggle| {
            let Some(editor) = weak.upgrade() else { return };
            if !toggle.is_active() || editor.filling.get() || editor.scope.get().index() == index {
                return;
            }
            let Some(ui) = editor.ui.upgrade() else { return };
            editor.set_scope(default_target(&ui, index));
        });
    }
    let weak = Rc::downgrade(&editor);
    editor.target.connect_selected_notify(move |target| {
        let Some(editor) = weak.upgrade() else { return };
        if editor.filling.get() {
            return;
        }
        let Some(id) = editor.targets.borrow().get(target.selected() as usize).copied() else { return };
        let next = match editor.scope.get() {
            Scope::Global => return,
            Scope::Workspace(_) => Scope::Workspace(id),
            Scope::Project(_) => Scope::Project(id),
        };
        editor.scope.set(next);
        editor.load("");
    });
    let weak = Rc::downgrade(&editor);
    editor.save.connect_clicked(move |_| {
        if let Some(editor) = weak.upgrade() {
            editor.save();
        }
    });
    let weak = Rc::downgrade(&editor);
    editor.discard.connect_clicked(move |_| {
        if let Some(editor) = weak.upgrade() {
            editor.fill();
            editor.status.set_text("Changes discarded.");
        }
    });

    editor.filling.set(true);
    editor.toggles[scope.index()].set_active(true);
    editor.filling.set(false);
    editor.fill_targets(ui);
    editor.load("");
    // The root owns the editor through this handler; its children only hold weak references,
    // so dropping the widget drops everything.
    let weak = Rc::downgrade(&editor);
    let keep = editor.clone();
    root.connect_destroy(move |_| {
        // Any answer still in flight now lands on nothing.
        keep.generation.set(keep.generation.get() + 1);
    });
    (root, weak)
}

fn open(ui: &Rc<Ui>, scope: Scope) {
    let Some(panel) = crate::panel::Panel::toggle(ui, "Guardrails", 640) else { return };
    let (root, editor) = build(ui, scope);
    panel.body.append(&root);
    panel.set_guard(move || editor.upgrade().is_none_or(|editor| editor.dirty() == 0));
    panel.present();
}

/// Guardrail settings for one project, in a panel.
pub fn open_project_guardrails(ui: &Rc<Ui>, project: i64) {
    open(ui, Scope::Project(project));
}

/// Guardrail settings for one workspace, in a panel.
pub fn open_workspace_guardrails(ui: &Rc<Ui>, workspace: i64) {
    open(ui, Scope::Workspace(workspace));
}
