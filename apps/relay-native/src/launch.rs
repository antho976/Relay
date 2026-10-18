//! The New agents sheet: how the crew runs, the crew itself in a rail, the chosen agent's
//! settings beside it, and a footer that launches them all.
//!
//! The crew is built by adding and removing agents rather than by picking a count, and each
//! agent's task queue shows its order as it is ticked: the first task becomes the agent's
//! current one (RA-558). Six agent slots exist from the start; the rail shows the ones in the
//! crew, so an agent removed and added again starts from the defaults.
use super::*;
use std::rc::Weak;

/// The most agents one launch configures.
const AGENTS: usize = 6;

/// The reasoning efforts the effort keys offer, in key order.
const EFFORTS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// The task columns the filter keys offer, in key order; "" is every open column.
const COLUMNS: [(&str, &str); 5] = [("", "All open"), ("backlog", "Backlog"), ("ready", "Ready"), ("active", "Active"), ("in_review", "In review")];

/// One choice made with a row of grouped toggle keys (RA-690). The keys are the control: the
/// chosen index lives in a cell beside them, not in a hidden widget the form has to keep in
/// the tree. The smoke harness drives the row, named, through its keys as a person would.
/// A handler connected to keys should hold [`Choice::downgrade`], not a clone: a clone holds
/// the row, and a row whose key handler holds the row is never freed.
#[derive(Clone)]
struct Choice {
    row: gtk::Box,
    keys: Vec<gtk::ToggleButton>,
    index: Rc<Cell<u32>>,
}

#[derive(Clone)]
struct WeakChoice {
    row: glib::WeakRef<gtk::Box>,
    keys: Vec<glib::WeakRef<gtk::ToggleButton>>,
    index: Rc<Cell<u32>>,
}

impl Choice {
    /// Groups `keys`, already in `row`, with key `initial` chosen.
    fn new(row: gtk::Box, keys: Vec<gtk::ToggleButton>, initial: u32) -> Self {
        let index = Rc::new(Cell::new(initial));
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                key.set_group(keys.first());
            }
            key.set_active(i as u32 == initial);
            let index = index.clone();
            key.connect_toggled(move |key| {
                if key.is_active() {
                    index.set(i as u32);
                }
            });
        }
        Choice { row, keys, index }
    }
    fn selected(&self) -> u32 {
        self.index.get()
    }
    fn set_selected(&self, index: u32) {
        if let Some(key) = self.keys.get(index as usize) {
            key.set_active(true);
        }
    }
    /// Runs `changed` with the new index each time another key is chosen; `selected()`
    /// already returns it.
    fn connect_changed(&self, changed: impl Fn(u32) + 'static) {
        let changed = Rc::new(changed);
        for (i, key) in self.keys.iter().enumerate() {
            let changed = changed.clone();
            key.connect_toggled(move |key| {
                if key.is_active() {
                    changed(i as u32);
                }
            });
        }
    }
    fn set_sensitive(&self, sensitive: bool) {
        self.row.set_sensitive(sensitive);
    }
    fn downgrade(&self) -> WeakChoice {
        WeakChoice {
            row: self.row.downgrade(),
            keys: self.keys.iter().map(|key| key.downgrade()).collect(),
            index: self.index.clone(),
        }
    }
}

impl WeakChoice {
    fn upgrade(&self) -> Option<Choice> {
        Some(Choice {
            row: self.row.upgrade()?,
            keys: self.keys.iter().map(|key| key.upgrade()).collect::<Option<_>>()?,
            index: self.index.clone(),
        })
    }
}

/// A row of grouped keys, one per caption, styled by `class`.
fn segments(choices: &[&str], initial: u32, class: &str) -> Choice {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    row.add_css_class(class);
    let keys: Vec<gtk::ToggleButton> = choices
        .iter()
        .map(|title| {
            let key = gtk::ToggleButton::with_label(title);
            row.append(&key);
            key
        })
        .collect();
    Choice::new(row, keys, initial)
}

/// A settings line: its caption in a fixed column, the control beside it.
fn form_row(caption: &str, value: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.add_css_class("launch-row");
    let words = label(caption, "launch-row-label");
    words.set_size_request(76, -1);
    words.set_valign(gtk::Align::Center);
    row.append(&words);
    row.append(value);
    row
}

fn provider_name(index: u32) -> &'static str {
    if index == 1 { "Codex" } else { "Claude" }
}

fn role_name(index: u32) -> &'static str {
    match index {
        1 => "Reviewer",
        2 => "Docs",
        _ => "Builder",
    }
}

fn column_name(column: &str) -> String {
    COLUMNS
        .iter()
        .find(|(key, _)| *key == column)
        .map(|(_, title)| title.to_string())
        .unwrap_or_else(|| column.replace('_', " "))
}

fn tasks_phrase(n: usize) -> String {
    if n == 1 { "1 task".into() } else { format!("{n} tasks") }
}

/// What the task rows built from `tasks` show, to tell whether a newer read changes any.
fn shown(tasks: &[Value]) -> Vec<(Option<i64>, &str, &str, &str)> {
    tasks.iter().filter(|t| text(t, "column") != "done").map(|t| (t["id"].as_i64(), text(t, "title"), text(t, "body"), text(t, "column"))).collect()
}

/// The order an agent takes its tasks in: the one the sheet was opened from, then the others
/// in the order they were ticked. `active` is every selected id in card (board-column) order;
/// one with no recorded tick keeps that order at the end.
fn task_order(active: &[i64], preselect: Option<i64>, ticks: &[i64]) -> Vec<i64> {
    let mut order: Vec<i64> = preselect.filter(|id| active.contains(id)).into_iter().collect();
    for id in ticks.iter().chain(active) {
        if active.contains(id) && !order.contains(id) {
            order.push(*id);
        }
    }
    order
}

/// Note a card being ticked (to the end of the sequence) or unticked (out of it).
fn record_tick(ticks: &mut Vec<i64>, id: i64, active: bool) {
    ticks.retain(|t| *t != id);
    if active {
        ticks.push(id);
    }
}

/// Where a review group puts each member, by its place in the rail: the lead builder first,
/// the reviewer last, a second builder between them. Launch order is lead, reviewer, then the
/// second builder, each pairing with the one before it.
fn group_launch_order(members: &[usize]) -> Vec<usize> {
    match members {
        [lead, builder, reviewer] => vec![*lead, *reviewer, *builder],
        _ => members.to_vec(),
    }
}

/// One task in an agent's list: the key, its place in the queue, and what the filter reads.
struct TaskRow {
    id: i64,
    key: gtk::ToggleButton,
    order: gtk::Label,
    column: String,
    content: String,
}

/// One agent's settings, shown beside the rail while it is chosen.
struct Profile {
    root: gtk::Box,
    provider: Choice,
    /// Each provider key and the line under its name (account and version).
    provider_cards: Vec<(gtk::ToggleButton, gtk::Label)>,
    role: Choice,
    role_note: gtk::Label,
    model: gtk::Entry,
    effort: Choice,
    worktree: gtk::Entry,
    prompt: gtk::TextView,
    writes: gtk::Switch,
    ui_access: gtk::Switch,
    /// Claude only: bill the session to the API key in the keyring, not the signed-in account.
    api_key: gtk::Switch,
    more: gtk::Revealer,
    more_summary: gtk::Label,
    task_section: gtk::Box,
    shared_note: gtk::Label,
    task_box: gtk::Box,
    task_empty: gtk::Label,
    task_search: gtk::SearchEntry,
    task_filter: Choice,
    task_count: gtk::Label,
    tasks: RefCell<Vec<TaskRow>>,
    /// The task the sheet was opened from, and the other ids in the order they were ticked:
    /// the first selected task becomes the agent's current one (RA-558).
    preselect: Cell<Option<i64>>,
    ticks: RefCell<Vec<i64>>,
    /// Whether the task rows were built: only once the agent is first shown.
    filled: Cell<bool>,
}

impl Profile {
    fn new(index: usize, sheet: Weak<Sheet>) -> Rc<Self> {
        let changed = move || {
            if let Some(sheet) = sheet.upgrade() {
                sheet.render();
            }
        };
        let changed: Rc<dyn Fn()> = Rc::new(changed);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_widget_name(&format!("launch-profile-{index}"));
        root.add_css_class("launch-profile");

        let form = gtk::Box::new(gtk::Orientation::Vertical, 12);
        form.add_css_class("launch-form");
        let providers = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        providers.set_widget_name(&format!("launch-provider-{index}"));
        providers.set_homogeneous(true);
        providers.set_hexpand(true);
        let mut provider_cards = Vec::new();
        for (icon, title) in [("claude", "Claude Code"), ("codex", "Codex")] {
            let key = gtk::ToggleButton::new();
            key.add_css_class("launch-provider");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let mark = crate::icons::image(icon, 16);
            mark.add_css_class("launch-provider-mark");
            mark.set_valign(gtk::Align::Center);
            content.append(&mark);
            let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
            words.set_hexpand(true);
            words.set_valign(gtk::Align::Center);
            words.append(&label(title, "launch-provider-name"));
            let status = label("Checking…", "launch-provider-status");
            status.set_ellipsize(gtk::pango::EllipsizeMode::End);
            status.set_width_chars(1);
            words.append(&status);
            content.append(&words);
            let radio = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            radio.add_css_class("launch-radio");
            radio.set_valign(gtk::Align::Center);
            content.append(&radio);
            key.set_child(Some(&content));
            key.update_property(&[gtk::accessible::Property::Label(title)]);
            providers.append(&key);
            provider_cards.push((key, status));
        }
        let keys = provider_cards.iter().map(|(key, _)| key.clone()).collect();
        let provider = Choice::new(providers.clone(), keys, 0);
        form.append(&form_row("Provider", &providers));

        let role = segments(&["Builder", "Reviewer", "Docs"], 0, "launch-segmented");
        let role_line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        role_line.append(&role.row);
        let role_note = label("Set by the review group", "launch-hint");
        role_note.set_valign(gtk::Align::Center);
        role_line.append(&role_note);
        form.append(&form_row("Role", &role_line));

        let model = gtk::Entry::builder().placeholder_text("Provider default").hexpand(true).build();
        model.set_widget_name(&format!("launch-model-{index}"));
        model.set_width_chars(1);
        form.append(&form_row("Model", &model));

        let effort = segments(&EFFORTS, 3, "launch-segmented");
        effort.row.set_widget_name(&format!("launch-effort-{index}"));
        // Minimal is Codex's alone and max Claude's alone.
        effort.keys[0].set_visible(false);
        let weak_effort = effort.downgrade();
        provider.connect_changed(move |provider| {
            let Some(effort) = weak_effort.upgrade() else {
                return;
            };
            let codex = provider == 1;
            effort.keys[0].set_visible(codex);
            effort.keys[5].set_visible(!codex);
            if (codex && effort.selected() == 5) || (!codex && effort.selected() == 0) {
                effort.set_selected(3);
            }
        });
        let effort_line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        effort_line.append(&effort.row);
        form.append(&form_row("Effort", &effort_line));
        for choice in [&provider, &role, &effort] {
            let changed = changed.clone();
            choice.connect_changed(move |_| changed());
        }
        root.append(&form);

        // Instructions, worktree and permissions: folded under one line that says what they are.
        let more_key = gtk::Button::new();
        more_key.add_css_class("launch-disclosure");
        let more_line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let chevron = crate::icons::image("chevron-right", 14);
        more_line.append(&chevron);
        more_line.append(&label("Instructions, worktree and permissions", "launch-disclosure-title"));
        let more_summary = label("", "launch-disclosure-summary");
        more_summary.set_hexpand(true);
        more_summary.set_xalign(1.0);
        more_summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
        more_summary.set_width_chars(1);
        more_line.append(&more_summary);
        more_key.set_child(Some(&more_line));
        root.append(&more_key);
        let more = gtk::Revealer::new();
        more.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        more.set_transition_duration(140);
        let options = gtk::Box::new(gtk::Orientation::Vertical, 14);
        options.add_css_class("launch-options");
        let prompt = gtk::TextView::new();
        prompt.set_wrap_mode(gtk::WrapMode::WordChar);
        prompt.set_accepts_tab(false);
        prompt.update_property(&[gtk::accessible::Property::Label("Instructions")]);
        let well = gtk::Box::new(gtk::Orientation::Vertical, 0);
        well.add_css_class("launch-prompt");
        well.set_overflow(gtk::Overflow::Hidden);
        well.append(&prompt);
        let instructions = gtk::Box::new(gtk::Orientation::Vertical, 6);
        instructions.append(&label("Instructions", "launch-field-label"));
        instructions.append(&well);
        let note = label("Sent with the agent's launch brief as its first prompt.", "launch-hint");
        note.set_wrap(true);
        instructions.append(&note);
        options.append(&instructions);
        // Blank means the project default: a new worktree, or the primary checkout when an
        // enabled plugin asks for it (D160). Only a typed value is sent.
        let worktree = gtk::Entry::builder()
            .placeholder_text("Project default — or new, primary, or an absolute path")
            .hexpand(true)
            .build();
        worktree.set_width_chars(1);
        let place = gtk::Box::new(gtk::Orientation::Vertical, 6);
        place.append(&label("Worktree", "launch-field-label"));
        place.append(&worktree);
        options.append(&place);
        let mut switches = Vec::new();
        let mut lines = Vec::new();
        for (caption, hint, on) in [
            ("Agent bus writes", "Let it write files, stage and commit through Relay, with guardrails checked as it goes.", true),
            ("UI control", "Let it drive the Relay window.", false),
            ("Use API key", "Bill this session to your API key instead of your subscription. The key stays in \
              Relay's key proxy, where no agent can read it; programs running as you can still spend through it.", false),
        ] {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.add_css_class("launch-switch");
            let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
            words.set_hexpand(true);
            words.append(&label(caption, "launch-switch-title"));
            words.append(&label(hint, "launch-hint"));
            line.append(&words);
            let switch = gtk::Switch::new();
            switch.set_active(on);
            switch.set_valign(gtk::Align::Center);
            switch.update_property(&[gtk::accessible::Property::Label(caption)]);
            line.append(&switch);
            options.append(&line);
            switches.push(switch);
            lines.push(line);
        }
        // Codex has no API-key setting here, so the switch is Claude's alone.
        let api_line = lines.pop().expect("the API key line");
        api_line.set_visible(provider.selected() == 0);
        let weak_line = api_line.downgrade();
        provider.connect_changed(move |provider| {
            if let Some(line) = weak_line.upgrade() {
                line.set_visible(provider == 0);
            }
        });
        more.set_child(Some(&options));
        root.append(&more);
        let m = more.downgrade();
        more_key.connect_clicked(move |_| {
            let Some(more) = m.upgrade() else { return };
            let open = !more.reveals_child();
            more.set_reveal_child(open);
            chevron.set_paintable(crate::icons::image(if open { "chevron-down" } else { "chevron-right" }, 14).paintable().as_ref());
        });
        let writes = switches.remove(0);
        let ui_access = switches.remove(0);
        let api_key = switches.remove(0);
        for switch in [&writes, &ui_access, &api_key] {
            let changed = changed.clone();
            switch.connect_active_notify(move |_| changed());
        }
        let c = changed.clone();
        worktree.connect_changed(move |_| c());
        let c = changed.clone();
        prompt.buffer().connect_changed(move |_| c());

        // The task queue.
        let task_section = gtk::Box::new(gtk::Orientation::Vertical, 10);
        task_section.add_css_class("launch-tasks");
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        head.append(&label("Tasks", "launch-section-title"));
        let task_count = label("None queued", "launch-count-pill");
        task_count.set_valign(gtk::Align::Center);
        head.append(&task_count);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        head.append(&spacer);
        let task_search = gtk::SearchEntry::new();
        task_search.add_css_class("search");
        task_search.set_placeholder_text(Some("Search tasks"));
        task_search.set_width_chars(18);
        head.append(&task_search);
        task_section.append(&head);
        let hint = label("Tick them in the order the agent should take them: the first becomes its current task.", "launch-hint");
        hint.set_wrap(true);
        task_section.append(&hint);
        let task_filter = segments(&COLUMNS.map(|(_, title)| title), 0, "launch-filter");
        task_section.append(&task_filter.row);
        let task_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        task_box.add_css_class("launch-task-list");
        let task_empty = label("Loading open tasks…", "launch-hint");
        task_empty.add_css_class("launch-task-empty");
        task_box.append(&task_empty);
        task_section.append(&task_box);
        root.append(&task_section);
        let shared_note = label("", "launch-shared-note");
        shared_note.set_wrap(true);
        shared_note.set_visible(false);
        root.append(&shared_note);

        let profile = Rc::new(Self {
            root,
            provider,
            provider_cards,
            role,
            role_note,
            model,
            effort,
            worktree,
            prompt,
            writes,
            ui_access,
            api_key,
            more,
            more_summary,
            task_section,
            shared_note,
            task_box,
            task_empty,
            task_search,
            task_filter,
            task_count,
            tasks: RefCell::default(),
            preselect: Cell::new(None),
            ticks: RefCell::default(),
            filled: Cell::new(false),
        });
        profile.summarize();
        profile
    }

    fn selected_tasks(&self) -> Vec<i64> {
        let active: Vec<i64> = self.tasks.borrow().iter().filter(|row| row.key.is_active()).map(|row| row.id).collect();
        task_order(&active, self.preselect.get(), &self.ticks.borrow())
    }

    fn payload(&self, project: i64, role: Option<&str>) -> Value {
        // A review group keeps its own fresh worktree unless the user typed one: groups in the
        // primary checkout collide with solo agents already there.
        let group = role.is_some();
        let role = role.unwrap_or(match self.role.selected() {
            1 => "reviewer",
            2 => "docs",
            _ => "builder",
        });
        let model = self.model.text().trim().to_string();
        let effort = EFFORTS.get(self.effort.selected() as usize);
        let claude = self.provider.selected() == 0;
        let mut payload = json!({
            "project_id": project,
            "provider": if claude { "claude" } else { "codex" },
            "role": role,
            "model": if model.is_empty() { None } else { Some(model) },
            "effort": effort,
            "bus_writes": self.writes.is_active(),
            "allow_ui": self.ui_access.is_active(),
            "prompt": self.prompt(),
        });
        // Sent only when on: an engine older than the option refuses a payload that names it.
        if claude && self.api_key.is_active() {
            payload["use_api_key"] = json!(true);
        }
        let worktree = self.worktree.text().trim().to_string();
        if !worktree.is_empty() {
            payload["worktree"] = json!(worktree);
        } else if group {
            payload["worktree"] = json!("new");
        }
        payload
    }

    fn prompt(&self) -> String {
        let b = self.prompt.buffer();
        b.text(&b.start_iter(), &b.end_iter(), false).to_string()
    }

    /// The folded options, said in one line.
    fn summarize(&self) {
        let mut parts = Vec::new();
        if !self.prompt().trim().is_empty() {
            parts.push("Instructions".to_string());
        }
        let worktree = self.worktree.text().trim().to_string();
        parts.push(if worktree.is_empty() { "Default worktree".into() } else { format!("Worktree: {worktree}") });
        parts.push(if self.writes.is_active() { "Bus writes".into() } else { "No bus writes".into() });
        if self.ui_access.is_active() {
            parts.push("UI control".into());
        }
        if self.provider.selected() == 0 && self.api_key.is_active() {
            parts.push("API key".into());
        }
        self.more_summary.set_text(&parts.join(" · "));
    }

    /// Number the ticked tasks in queue order and count them.
    fn renumber(&self) {
        let order = self.selected_tasks();
        for row in self.tasks.borrow().iter() {
            match order.iter().position(|id| *id == row.id) {
                Some(place) => {
                    row.order.set_text(&(place + 1).to_string());
                    row.order.add_css_class("on");
                }
                None => {
                    row.order.set_text("");
                    row.order.remove_css_class("on");
                }
            }
        }
        if order.is_empty() {
            self.task_count.set_text("None queued");
            self.task_count.remove_css_class("on");
        } else {
            self.task_count.set_text(&format!("{} queued", order.len()));
            self.task_count.add_css_class("on");
        }
    }

    /// Back to the defaults, for an agent added again after it was removed.
    fn reset(&self) {
        if let Some((key, _)) = self.provider_cards.iter().find(|(key, _)| key.is_sensitive()) {
            key.set_active(true);
        }
        self.role.set_sensitive(true);
        self.role.set_selected(0);
        self.model.set_text("");
        self.effort.set_selected(3);
        self.worktree.set_text("");
        self.prompt.buffer().set_text("");
        self.writes.set_active(true);
        self.ui_access.set_active(false);
        self.api_key.set_active(false);
        self.more.set_reveal_child(false);
        self.preselect.set(None);
        for row in self.tasks.borrow().iter() {
            row.key.set_active(false);
        }
        self.ticks.borrow_mut().clear();
        self.renumber();
    }

    /// Build this agent's task rows from the open tasks. Each agent gets its own, so they are
    /// built only for an agent the user opens, once.
    fn fill_tasks(self: &Rc<Self>, tasks: &[Value], preselect: Option<i64>) {
        if self.filled.replace(true) {
            return;
        }
        self.preselect.set(preselect);
        self.add_rows(tasks, preselect.as_slice());
        let weak = Rc::downgrade(self);
        let filter: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(p) = weak.upgrade() {
                p.filter();
            }
        });
        let f = filter.clone();
        self.task_search.connect_search_changed(move |_| f());
        self.task_filter.connect_changed(move |_| filter());
        self.filter();
    }

    /// The rows again from a newer read of the open tasks, for a draft opened again: a task
    /// still open keeps its tick and its place in the queue, one added since appears unticked.
    fn refill_tasks(self: &Rc<Self>, tasks: &[Value]) {
        if !self.filled.get() {
            return;
        }
        let ticked = self.selected_tasks();
        for row in self.tasks.take() {
            self.task_box.remove(&row.key);
        }
        self.add_rows(tasks, &ticked);
        self.filter();
    }

    /// A row per open task, those in `ticked` ticked, with the empty line after them.
    fn add_rows(self: &Rc<Self>, tasks: &[Value], ticked: &[i64]) {
        for row in tasks.iter().filter(|t| text(t, "column") != "done") {
            let id = row["id"].as_i64().unwrap_or(0);
            let key = gtk::ToggleButton::new();
            key.add_css_class("launch-task");
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let order = label("", "launch-task-order");
            order.set_xalign(0.5);
            order.set_valign(gtk::Align::Center);
            line.append(&order);
            let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
            words.set_hexpand(true);
            words.set_valign(gtk::Align::Center);
            let title = label(text(row, "title"), "launch-task-title");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_width_chars(1);
            words.append(&title);
            let body = text(row, "body").lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
            if !body.is_empty() {
                let desc = label(body, "launch-task-desc");
                desc.set_ellipsize(gtk::pango::EllipsizeMode::End);
                desc.set_width_chars(1);
                words.append(&desc);
            }
            line.append(&words);
            let column = label(&column_name(text(row, "column")), "launch-task-column");
            column.set_valign(gtk::Align::Center);
            line.append(&column);
            let number = label(&format!("#{id}"), "launch-task-id");
            number.set_xalign(1.0);
            number.set_valign(gtk::Align::Center);
            line.append(&number);
            key.set_child(Some(&line));
            key.set_tooltip_text(Some(text(row, "title")));
            key.update_property(&[gtk::accessible::Property::Label(text(row, "title"))]);
            key.set_active(ticked.contains(&id));
            let weak = Rc::downgrade(self);
            key.connect_toggled(move |key| {
                if let Some(p) = weak.upgrade() {
                    record_tick(&mut p.ticks.borrow_mut(), id, key.is_active());
                    p.renumber();
                    if let Some(sheet) = SHEET.with(|s| s.borrow().clone()) {
                        sheet.render();
                    }
                }
            });
            self.task_box.append(&key);
            self.tasks.borrow_mut().push(TaskRow {
                id,
                key,
                order,
                column: text(row, "column").to_string(),
                content: format!("#{id} {} {}", text(row, "title"), text(row, "body")).to_lowercase(),
            });
        }
        // The empty line goes last, under whatever rows the filter leaves.
        self.task_box.reorder_child_after(&self.task_empty, self.task_box.last_child().as_ref());
        self.renumber();
    }

    /// Show the rows the search and the column keys leave, and say so when none are left.
    fn filter(&self) {
        let query = self.task_search.text().trim().to_lowercase();
        let column = COLUMNS[self.task_filter.selected() as usize].0;
        let mut shown = 0;
        let tasks = self.tasks.borrow();
        for row in tasks.iter() {
            let visible = (column.is_empty() || column == row.column) && (query.is_empty() || row.content.contains(&query));
            row.key.set_visible(visible);
            shown += usize::from(visible);
        }
        self.task_empty.set_visible(shown == 0);
        self.task_empty.set_text(if tasks.is_empty() {
            "No open tasks. The agent can start without one."
        } else {
            "No tasks match."
        });
    }
}

/// An agent's line in the rail: its key, and the key that takes it out of the crew.
struct Member {
    root: gtk::Box,
    key: gtk::ToggleButton,
    badge: gtk::Label,
    name: gtk::Label,
    facts: gtk::Label,
    remove: gtk::Button,
}

/// The sheet while it is open, and after it is closed unlaunched, until it opens again. Its
/// widgets' handlers hold it weakly; the one strong reference is `SHEET`, dropped by a launch,
/// a project switch or a reconnect, or replaced by a fresh form.
struct Sheet {
    form: gtk::Box,
    profiles: Vec<Rc<Profile>>,
    /// The crew, as profile slots in rail order.
    members: RefCell<Vec<usize>>,
    current: Cell<usize>,
    mode: Choice,
    mode_hint: gtk::Label,
    rail: gtk::Box,
    rail_members: Vec<Member>,
    add: gtk::Button,
    add_caption: gtk::Label,
    stack: gtk::Stack,
    start: gtk::Button,
    start_label: gtk::Label,
    status: gtk::Label,
    /// The project and engine connection the form was built for: a click after either
    /// changed must not launch into the new one.
    project: i64,
    generation: u64,
    /// The open tasks, once task.list answers; each agent's rows are built from them when the
    /// agent is first shown.
    open_tasks: RefCell<Option<Vec<Value>>>,
    /// What stopped the form loading, or what the provider check could not do.
    failure: RefCell<Option<String>>,
    provider_error: RefCell<Option<String>>,
}

thread_local! {
    static SHEET: RefCell<Option<Rc<Sheet>>> = const { RefCell::new(None) };
}

impl Sheet {
    fn group(&self) -> bool {
        self.mode.selected() == 1
    }

    /// What the rail calls the member in `place`.
    fn title(&self, place: usize, count: usize) -> String {
        if !self.group() {
            return format!("Agent {}", place + 1);
        }
        match place {
            _ if place + 1 == count => "Reviewer".into(),
            0 if count == 3 => "Builder 1".into(),
            0 => "Builder".into(),
            _ => format!("Builder {}", place + 1),
        }
    }

    /// Bring every part of the sheet in line with the crew and the chosen agent.
    fn render(&self) {
        let members = self.members.borrow().clone();
        let group = self.group();
        let count = members.len();
        let mut previous: Option<gtk::Widget> = None;
        for (place, slot) in members.iter().enumerate() {
            let member = &self.rail_members[*slot];
            member.root.set_visible(true);
            self.rail.reorder_child_after(&member.root, previous.as_ref());
            previous = Some(member.root.clone().upcast());
            let profile = &self.profiles[*slot];
            let name = self.title(place, count);
            member.badge.set_text(&(place + 1).to_string());
            member.name.set_text(&name);
            let provider = provider_name(profile.provider.selected());
            member.facts.set_text(&if group {
                format!("{provider} · {}", EFFORTS[profile.effort.selected() as usize])
            } else {
                format!("{provider} · {}", role_name(profile.role.selected()))
            });
            member.key.update_property(&[gtk::accessible::Property::Label(&name)]);
            member.remove.set_visible(if group { count == 3 && place == 1 } else { place > 0 });
            member.remove.set_tooltip_text(Some(&format!("Remove {name}")));
            member.remove.update_property(&[gtk::accessible::Property::Label(&format!("Remove {name}"))]);
            // A review group's roles follow from the rail: the last member reviews.
            if group {
                profile.role.set_selected(if place + 1 == count { 1 } else { 0 });
            }
            profile.role.set_sensitive(!group);
            profile.role_note.set_visible(group);
            profile.worktree.set_sensitive(!group || place == 0);
            profile.task_section.set_visible(!group || place == 0);
            profile.shared_note.set_visible(group && place > 0);
            let lead = self.title(0, count);
            profile.shared_note.set_text(&if place + 1 == count {
                format!("The reviewer reads the builders' work on the shared branch and takes no tasks of its own. The group works from {lead}'s queue.")
            } else {
                format!("Works on the shared branch from {lead}'s task queue.")
            });
            profile.summarize();
        }
        for (slot, member) in self.rail_members.iter().enumerate() {
            if !members.contains(&slot) {
                member.root.set_visible(false);
            }
        }
        self.rail.reorder_child_after(&self.add, previous.as_ref());
        let limit = if group { 3 } else { AGENTS };
        self.add.set_sensitive(count < limit);
        self.add_caption.set_text(if group { "Add a builder" } else { "Add an agent" });
        self.add.set_tooltip_text(Some(&if count < limit {
            if group { "Add a second builder on the shared branch".to_string() } else { format!("Add another agent (up to {AGENTS})") }
        } else if group {
            "A review group has at most two builders".into()
        } else {
            format!("{AGENTS} agents is the most one launch starts")
        }));
        // A review group is two or three agents; a bigger crew stays separate.
        self.mode.keys[1].set_sensitive(group || count <= 3);
        self.mode.keys[1].set_tooltip_text(Some(if group || count <= 3 {
            "Builders and one read-only reviewer share one branch"
        } else {
            "A review group has two or three agents: remove some first"
        }));
        self.mode_hint.set_text(if group {
            "Builders and one read-only reviewer share one branch and the lead's task queue."
        } else {
            "Each agent gets its own worktree, role, settings and tasks."
        });
        if !members.contains(&self.current.get()) {
            if let Some(first) = members.first() {
                self.rail_members[*first].key.set_active(true);
            }
        }
        self.start_label.set_text(&match (group, count) {
            (true, _) => "Launch review group".to_string(),
            (false, 1) => "Launch agent".to_string(),
            (false, n) => format!("Launch {n} agents"),
        });
        self.render_status(&members);
    }

    /// The footer's line: what is loading or failed, else how much work goes out.
    fn render_status(&self, members: &[usize]) {
        self.status.remove_css_class("error");
        if let Some(failure) = self.failure.borrow().as_ref() {
            self.status.set_text(failure);
            self.status.add_css_class("error");
            return;
        }
        if self.open_tasks.borrow().is_none() {
            self.status.set_text("Loading open tasks…");
            return;
        }
        let group = self.group();
        let takers: Vec<&usize> = members.iter().take(if group { 1 } else { members.len() }).collect();
        let queued: usize = takers.iter().map(|slot| self.profiles[**slot].selected_tasks().len()).sum();
        let mut line = match (queued, group, members.len()) {
            (0, _, 1) => "No task queued: the agent starts without one.".to_string(),
            (0, _, _) => "No tasks queued: the agents start without one.".to_string(),
            (n, true, _) => format!("{} queued for the group.", tasks_phrase(n)),
            (n, false, 1) => format!("{} queued.", tasks_phrase(n)),
            (n, false, _) => {
                let with = takers.iter().filter(|slot| !self.profiles[***slot].selected_tasks().is_empty()).count();
                format!("{} queued across {with} of {} agents.", tasks_phrase(n), members.len())
            }
        };
        if let Some(error) = self.provider_error.borrow().as_ref() {
            line.push_str(&format!(" Could not check the provider CLIs: {error}"));
        }
        self.status.set_text(&line);
    }

    /// The keyboard starts on the chosen mode key, inside the sheet, so Ctrl+Enter launches and
    /// Enter or Space on the title bar's key cannot close it unseen.
    fn focus_start(&self) {
        if let Some(key) = self.mode.keys.get(self.mode.selected() as usize) {
            key.grab_focus();
        }
    }

    /// Show `slot`'s settings, building its task rows the first time.
    fn show(&self, slot: usize) {
        self.current.set(slot);
        self.stack.set_visible_child_name(&format!("agent-{slot}"));
        if let Some(tasks) = self.open_tasks.borrow().as_ref() {
            self.profiles[slot].fill_tasks(tasks, None);
        }
    }

    fn add_member(&self) {
        let group = self.group();
        let slot = {
            let members = self.members.borrow();
            if members.len() >= if group { 3 } else { AGENTS } {
                return;
            }
            match (0..AGENTS).find(|slot| !members.contains(slot)) {
                Some(slot) => slot,
                None => return,
            }
        };
        self.profiles[slot].reset();
        {
            let mut members = self.members.borrow_mut();
            // A group's new member is its second builder, before the reviewer.
            let count = members.len();
            let at = if group { count.saturating_sub(1).max(1).min(count) } else { count };
            members.insert(at, slot);
        }
        self.render();
        self.rail_members[slot].key.set_active(true);
    }

    fn remove_member(&self, slot: usize) {
        let place = {
            let mut members = self.members.borrow_mut();
            let Some(place) = members.iter().position(|s| *s == slot) else { return };
            if place == 0 {
                return;
            }
            members.remove(place);
            place
        };
        if self.current.get() == slot {
            let previous = self.members.borrow()[place - 1];
            self.rail_members[previous].key.set_active(true);
        }
        self.render();
    }
}

impl Ui {
    pub fn show_launch(self: &Rc<Self>, task: Option<i64>) {
        if self.project.get() == 0 {
            let workspace = self.workspaces.borrow().first().cloned();
            self.open_repository_in(workspace);
            return;
        }
        if self.launch_busy.get() {
            self.show_error("A launch is in progress. Allocated sessions will appear on the wall.");
            return;
        }
        // Already open (Ctrl+N again): keep what the user has filled in.
        if self.launch.reveals_child() {
            match SHEET.with(|s| s.borrow().clone()) {
                Some(sheet) => sheet.focus_start(),
                None => {
                    self.launch_box.child_focus(gtk::DirectionType::TabForward);
                }
            }
            return;
        }
        if !self.dismiss_panels() {
            return;
        }
        // What had the keyboard gets it back when the sheet hides, as a panel's opener does.
        let opener = gtk::prelude::GtkWindowExt::focus(&self.window);
        let project = self.project.get();
        let generation = self.generation.get();
        // A draft closed unlaunched comes back as it was left, for the same project and engine
        // connection. Opening for a task starts from that task instead.
        let kept = SHEET.with(|s| s.borrow().clone()).filter(|sheet| {
            task.is_none()
                && sheet.project == project
                && sheet.generation == generation
                && sheet.failure.borrow().is_none()
                && sheet.open_tasks.borrow().is_some()
                && self.launch_box.first_child().as_ref() == Some(sheet.form.upcast_ref())
        });
        if kept.is_none() {
            clear(&self.launch_box);
        }
        self.launch.set_reveal_child(true);
        // Closed, the form stays for the next opening. Launched (launch_crew lets go of SHEET
        // first), it goes once the sheet has finished hiding, with its row per open task for
        // every agent shown.
        let hidden: Rc<Cell<Option<glib::SignalHandlerId>>> = Rc::default();
        let id = {
            let hidden = hidden.clone();
            let launch_box = self.launch_box.downgrade();
            self.launch.connect_child_revealed_notify(move |launch| {
                if launch.is_child_revealed() || launch.reveals_child() {
                    return;
                }
                if SHEET.with(|s| s.borrow().is_none()) {
                    if let Some(launch_box) = launch_box.upgrade() {
                        clear(&launch_box);
                    }
                }
                if let Some(opener) = opener.as_ref().filter(|w| w.is_sensitive() && w.is_mapped()) {
                    opener.grab_focus();
                }
                if let Some(id) = hidden.take() {
                    launch.disconnect(id);
                }
            })
        };
        hidden.set(Some(id));
        if let Some(sheet) = kept {
            sheet.focus_start();
            self.reread_launch_tasks(&sheet);
            return;
        }
        let sheet = self.launch_sheet(project, generation);
        SHEET.with(|s| *s.borrow_mut() = Some(sheet.clone()));
        self.launch_box.append(&sheet.form);
        sheet.profiles[0].reset();
        sheet.rail_members[0].key.set_active(true);
        sheet.render();
        sheet.focus_start();

        let ui = self.clone();
        let weak = Rc::downgrade(&sheet);
        glib::spawn_future_local(async move {
            let (task_result, providers) = tokio::join!(
                ui.call("task.list", json!({"project_id": project})),
                ui.call("provider.list", json!({}))
            );
            let Some(sheet) = weak.upgrade() else { return };
            if !launch_is_current(&ui, project, generation, &sheet.form) {
                if launch_is_shown(&ui, &sheet.form) {
                    *sheet.failure.borrow_mut() = Some(STALE.into());
                    sheet.render();
                }
                return;
            }
            // A failed check says so and leaves both providers open: session.create still
            // refuses one that is not installed.
            let (provider_data, provider_error) = match providers {
                Ok(v) => (rows(&v, "providers"), None),
                Err(e) => (Vec::new(), Some(e.to_string())),
            };
            let tasks = match task_result {
                Ok(v) => rows(&v, "tasks"),
                Err(e) => {
                    *sheet.failure.borrow_mut() = Some(format!("Could not load the open tasks: {e}"));
                    sheet.render();
                    return;
                }
            };
            for profile in &sheet.profiles {
                for (i, (key, status)) in profile.provider_cards.iter().enumerate() {
                    let name = if i == 0 { "claude" } else { "codex" };
                    let info = provider_data.iter().find(|p| text(p, "provider") == name);
                    let installed = info.is_some_and(|p| p["installed"] == true);
                    key.set_sensitive(installed || provider_error.is_some());
                    let account = info
                        .and_then(|p| p["signed_in_as"].as_str())
                        .map(|s| format!("Signed in as {s}"))
                        .unwrap_or_else(|| {
                            if provider_error.is_some() {
                                "Could not check the CLI".into()
                            } else if installed {
                                "Not signed in".into()
                            } else {
                                "CLI not installed".into()
                            }
                        });
                    let version = info.and_then(|p| p["version"].as_str());
                    status.set_text(&match version {
                        Some(version) => format!("{account} · {version}"),
                        None => account.clone(),
                    });
                    key.set_tooltip_text(Some(&format!(
                        "{account}\n{}\n{}",
                        version.map(|v| format!("Version {v}")).unwrap_or_else(|| "Version unavailable".into()),
                        if info.is_some_and(|p| p["guarded"] == true) { "Guarded by Relay's guardrails" } else { "Unguarded" }
                    )));
                }
                // Start on an installed provider: a Codex-only machine opens on Codex.
                let cards = &profile.provider_cards;
                if !cards[profile.provider.selected() as usize].0.is_sensitive() {
                    if let Some((key, _)) = cards.iter().find(|(key, _)| key.is_sensitive()) {
                        key.set_active(true);
                    }
                }
            }
            *sheet.provider_error.borrow_mut() = provider_error;
            // Agent 1 now, with the task the sheet was opened for; any other agent when it is
            // first chosen.
            sheet.profiles[0].fill_tasks(&tasks, task);
            sheet.profiles[sheet.current.get()].fill_tasks(&tasks, None);
            *sheet.open_tasks.borrow_mut() = Some(tasks);
            sheet.start.set_sensitive(true);
            sheet.render();
        });
    }

    /// A kept draft's open tasks, read again: the board may have moved on while it was away.
    /// Unread, the list it had stands; unchanged, its rows stay as they are.
    fn reread_launch_tasks(self: &Rc<Self>, sheet: &Rc<Sheet>) {
        let (ui, weak, project, generation) = (self.clone(), Rc::downgrade(sheet), sheet.project, sheet.generation);
        glib::spawn_future_local(async move {
            let Ok(v) = ui.call("task.list", json!({"project_id": project})).await else { return };
            let Some(sheet) = weak.upgrade().filter(|sheet| launch_is_current(&ui, project, generation, &sheet.form)) else { return };
            let tasks = rows(&v, "tasks");
            if sheet.open_tasks.borrow().as_deref().is_some_and(|before| shown(before) == shown(&tasks)) {
                return;
            }
            for profile in &sheet.profiles {
                profile.refill_tasks(&tasks);
            }
            *sheet.open_tasks.borrow_mut() = Some(tasks);
            sheet.render();
        });
    }

    /// A draft belongs to the project and engine connection it was written for: switching
    /// projects or reconnecting lets it go.
    pub(super) fn forget_launch(&self) {
        if SHEET.with(|s| s.borrow_mut().take()).is_some() && !self.launch.reveals_child() {
            clear(&self.launch_box);
        }
    }

    /// Build the sheet's widgets for `project`.
    fn launch_sheet(self: &Rc<Self>, project: i64, generation: u64) -> Rc<Sheet> {
        Rc::new_cyclic(|weak: &Weak<Sheet>| {
            let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
            form.add_css_class("launch-sheet");
            form.set_vexpand(true);

            // Heading: what this is, and where the agents will work.
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            head.add_css_class("launch-head");
            let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
            words.set_hexpand(true);
            words.set_valign(gtk::Align::Center);
            words.append(&label("New agents", "launch-title"));
            let (name, base) = self
                .projects
                .borrow()
                .iter()
                .find(|p| p["id"].as_i64() == Some(project))
                .map(|p| (text(p, "name").to_string(), text(p, "base_branch").to_string()))
                .unwrap_or_default();
            let context = label(&if base.is_empty() { name } else { format!("{name} · from {base}") }, "launch-context");
            context.set_ellipsize(gtk::pango::EllipsizeMode::End);
            context.set_width_chars(1);
            words.append(&context);
            head.append(&words);
            let close = icon_button("close", "Close new agents (Esc)");
            close.set_valign(gtk::Align::Center);
            let w = Rc::downgrade(self);
            close.connect_clicked(move |_| {
                if let Some(ui) = w.upgrade() {
                    if !ui.launch_busy.get() {
                        ui.launch.set_reveal_child(false);
                    }
                }
            });
            head.append(&close);
            form.append(&head);

            // How the crew runs.
            let mode_bar = gtk::Box::new(gtk::Orientation::Horizontal, 14);
            mode_bar.add_css_class("launch-mode-bar");
            let mode_row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            mode_row.add_css_class("launch-segmented");
            mode_row.set_widget_name("launch-mode");
            mode_row.set_valign(gtk::Align::Center);
            let mut mode_keys = Vec::new();
            for (icon, title) in [("user", "Separate worktrees"), ("merge", "Review group")] {
                let key = gtk::ToggleButton::new();
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 7);
                line.append(&crate::icons::image(icon, 14));
                line.append(&label(title, ""));
                key.set_child(Some(&line));
                key.update_property(&[gtk::accessible::Property::Label(title)]);
                mode_row.append(&key);
                mode_keys.push(key);
            }
            let mode = Choice::new(mode_row.clone(), mode_keys, 0);
            mode_bar.append(&mode_row);
            let mode_hint = label("", "launch-hint");
            mode_hint.set_wrap(true);
            // Wrapped, not widening the sheet: its natural width is bounded too.
            mode_hint.set_width_chars(24);
            mode_hint.set_max_width_chars(44);
            mode_hint.set_hexpand(true);
            mode_hint.set_valign(gtk::Align::Center);
            mode_bar.append(&mode_hint);
            form.append(&mode_bar);
            let w = weak.clone();
            mode.connect_changed(move |chosen| {
                let Some(sheet) = w.upgrade() else { return };
                // A review group needs a reviewer beside its builder.
                if chosen == 1 && sheet.members.borrow().len() < 2 {
                    sheet.add_member();
                }
                sheet.render();
            });

            // The crew rail beside the chosen agent's settings.
            let split = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            split.set_vexpand(true);
            let rail = gtk::Box::new(gtk::Orientation::Vertical, 2);
            rail.add_css_class("launch-rail");
            rail.set_size_request(176, -1);
            // Its rows expand within it; the rail itself keeps its width.
            rail.set_hexpand(false);
            let stack = gtk::Stack::new();
            stack.set_hhomogeneous(false);
            stack.set_vhomogeneous(false);
            let profiles: Vec<Rc<Profile>> = (0..AGENTS).map(|i| Profile::new(i, weak.clone())).collect();
            let mut rail_members: Vec<Member> = Vec::new();
            for (slot, profile) in profiles.iter().enumerate() {
                stack.add_named(&profile.root, Some(&format!("agent-{slot}")));
                let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                root.add_css_class("launch-member");
                let key = gtk::ToggleButton::new();
                key.add_css_class("launch-member-key");
                key.set_hexpand(true);
                if let Some(first) = rail_members.first() {
                    key.set_group(Some(&first.key));
                }
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                let badge = label("", "launch-member-badge");
                badge.set_xalign(0.5);
                badge.set_valign(gtk::Align::Center);
                line.append(&badge);
                let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
                words.set_valign(gtk::Align::Center);
                words.set_hexpand(true);
                let name = label("", "launch-member-name");
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                name.set_width_chars(1);
                words.append(&name);
                let facts = label("", "launch-member-facts");
                facts.set_ellipsize(gtk::pango::EllipsizeMode::End);
                facts.set_width_chars(1);
                words.append(&facts);
                line.append(&words);
                key.set_child(Some(&line));
                root.append(&key);
                let remove = icon_button("close", "Remove");
                remove.add_css_class("launch-member-remove");
                remove.set_child(Some(&crate::icons::image("close", 12)));
                remove.set_valign(gtk::Align::Center);
                root.append(&remove);
                let w = weak.clone();
                let r = root.downgrade();
                key.connect_toggled(move |key| {
                    if let Some(root) = r.upgrade() {
                        if key.is_active() {
                            root.add_css_class("selected");
                        } else {
                            root.remove_css_class("selected");
                        }
                    }
                    if key.is_active() {
                        if let Some(sheet) = w.upgrade() {
                            sheet.show(slot);
                        }
                    }
                });
                let w = weak.clone();
                remove.connect_clicked(move |_| {
                    if let Some(sheet) = w.upgrade() {
                        sheet.remove_member(slot);
                    }
                });
                root.set_visible(false);
                rail.append(&root);
                rail_members.push(Member { root, key, badge, name, facts, remove });
            }
            let add = gtk::Button::new();
            add.add_css_class("launch-add");
            add.set_widget_name("launch-add");
            let add_line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let plus = crate::icons::image_with_stroke("plus", 12, 2.0);
            plus.add_css_class("launch-add-mark");
            plus.set_valign(gtk::Align::Center);
            add_line.append(&plus);
            let add_caption = label("Add an agent", "");
            add_line.append(&add_caption);
            add.set_child(Some(&add_line));
            let w = weak.clone();
            add.connect_clicked(move |_| {
                if let Some(sheet) = w.upgrade() {
                    sheet.add_member();
                }
            });
            rail.append(&add);
            split.append(&rail);
            let editor = gtk::Box::new(gtk::Orientation::Vertical, 0);
            editor.add_css_class("launch-editor");
            editor.append(&stack);
            let scroll = scrolled(&editor);
            scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            split.append(&scroll);
            form.append(&split);

            // Footer: where things stand, and the launch.
            let foot = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            foot.add_css_class("launch-foot");
            let status = label("Loading open tasks…", "launch-status");
            status.set_wrap(true);
            status.set_hexpand(true);
            status.set_valign(gtk::Align::Center);
            status.set_selectable(true);
            foot.append(&status);
            let keycap = label("Ctrl Enter", "launch-keycap");
            keycap.set_valign(gtk::Align::Center);
            foot.append(&keycap);
            let start = gtk::Button::new();
            start.add_css_class("primary");
            start.add_css_class("launch-go");
            start.set_widget_name("launch-start");
            start.set_valign(gtk::Align::Center);
            start.set_sensitive(false);
            let start_label = label("Launch agent", "");
            start.set_child(Some(&start_label));
            foot.append(&start);
            form.append(&foot);

            // Ctrl+Enter launches from anywhere in the sheet, the instructions included.
            let shortcuts = gtk::ShortcutController::new();
            shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
            let s = start.downgrade();
            shortcuts.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string("<Control>Return|<Control>KP_Enter"),
                Some(gtk::CallbackAction::new(move |_, _| {
                    if let Some(start) = s.upgrade().filter(|s| s.is_sensitive() && s.is_mapped()) {
                        start.emit_clicked();
                    }
                    glib::Propagation::Stop
                })),
            ));
            form.add_controller(shortcuts);

            let ui = Rc::downgrade(self);
            let w = weak.clone();
            start.connect_clicked(move |key| {
                let (Some(ui), Some(sheet)) = (ui.upgrade(), w.upgrade()) else { return };
                ui.launch_crew(&sheet, key);
            });

            Sheet {
                form,
                profiles,
                members: RefCell::new(vec![0]),
                current: Cell::new(0),
                mode,
                mode_hint,
                rail,
                rail_members,
                add,
                add_caption,
                stack,
                start,
                start_label,
                status,
                project,
                generation,
                open_tasks: RefCell::default(),
                failure: RefCell::default(),
                provider_error: RefCell::default(),
            }
        })
    }

    /// Create, queue and start every agent in the crew. The sheet steps aside on the click and
    /// the new agents appear at once, each pane saying which step it is on; the worktree fetch
    /// and checkout happen behind them.
    fn launch_crew(self: &Rc<Self>, sheet: &Rc<Sheet>, key: &gtk::Button) {
        let ui = self;
        let project = sheet.project;
        if !launch_is_current(ui, project, sheet.generation, &sheet.form) || sheet.open_tasks.borrow().is_none() {
            // Still on screen after the engine reconnected: say so instead of doing nothing.
            if launch_is_shown(ui, &sheet.form) && sheet.open_tasks.borrow().is_some() {
                ui.show_error(STALE);
            }
            return;
        }
        if ui.launch_busy.replace(true) {
            return;
        }
        let group = sheet.group();
        let members = sheet.members.borrow().clone();
        let reviewer = members.last().copied();
        let order = if group { group_launch_order(&members) } else { members.clone() };
        let mut crew = Vec::new();
        for slot in order {
            let p = &sheet.profiles[slot];
            if !p.provider_cards[p.provider.selected() as usize].0.is_sensitive() {
                ui.launch_busy.set(false);
                sheet.rail_members[slot].key.set_active(true);
                ui.show_error("Choose an installed provider for every agent.");
                return;
            }
            let payload = p.payload(project, group.then_some(if Some(slot) == reviewer { "reviewer" } else { "builder" }));
            let tasks = if !group || slot == members[0] { p.selected_tasks() } else { Vec::new() };
            crew.push((payload, tasks, p.prompt()));
        }
        key.set_sensitive(false);
        let key = key.clone();
        ui.launch_box.set_sensitive(false);
        let clicked = std::time::Instant::now();
        // Launched, the draft is spent: the sheet hides without keeping it.
        SHEET.with(|s| s.borrow_mut().take());
        ui.launch.set_reveal_child(false);
        ui.open_project(project, "agents");
        let payloads: Vec<Value> = crew.iter().map(|(payload, _, _)| payload.clone()).collect();
        let placeholders = ui.launch_placeholders(project, &payloads);
        tracing::debug!(agents = placeholders.len(), elapsed_us = clicked.elapsed().as_micros() as u64, "launch: placeholders visible");
        let ui = ui.clone();
        glib::spawn_future_local(async move {
            let mut touched: Vec<String> = placeholders.clone();
            let agents = crew.len();
            // What the failure message can truthfully say: sessions that exist, and tasks
            // that were meant for one of them but are not on its queue.
            let mut created: Vec<String> = Vec::new();
            let mut unstaged: Vec<i64> = Vec::new();
            let result = async {
                // session.create refreshes new-branch refs before allocation.
                let mut allocated: Vec<(Value, String)> = Vec::new();
                for (index, (mut payload, tasks, prompt)) in crew.into_iter().enumerate() {
                    let joining = group && index > 0;
                    ui.launch_progress(&placeholders[index], if joining { "Joining the shared worktree…" } else { "Preparing worktree…" });
                    if joining {
                        payload["pair_with"] = json!(text(&allocated[index - 1].0, "name"));
                        payload.as_object_mut().unwrap().remove("worktree");
                    }
                    if let Some(task) = tasks.first() {
                        payload["task_id"] = json!(task);
                    }
                    let session = ui.call("session.create", payload).await?;
                    let name = text(&session, "name").to_string();
                    ui.launch_adopt(&placeholders[index], &session, "Worktree ready · waiting to start…");
                    touched.push(name.clone());
                    created.push(name.clone());
                    tracing::debug!(session = %name, elapsed_ms = clicked.elapsed().as_millis() as u64, "launch: session created");
                    // Stage this agent's queue now, so a later agent's failure cannot cost it.
                    unstaged.extend(&tasks);
                    for task in &tasks {
                        ui.launch_progress(&name, &format!("Queueing task #{task}…"));
                        ui.call("task.dispatch", json!({"task_id": task, "session": name, "start": false})).await?;
                        unstaged.retain(|t| t != task);
                    }
                    allocated.push((session, prompt));
                }
                // Start reviewers first so their mailbox is live before builders publish files.
                allocated.sort_by_key(|(s, _)| text(s, "role") != "reviewer");
                for (session, prompt) in allocated {
                    let name = text(&session, "name").to_string();
                    ui.launch_progress(&name, "Starting the provider…");
                    let started = ui.call("session.spawn", json!({"session": name, "prompt": prompt})).await;
                    ui.launch_done(&name);
                    ui.upsert_session(&started?);
                    tracing::debug!(session = %name, elapsed_ms = clicked.elapsed().as_millis() as u64, "launch: provider started");
                }
                Ok::<(), Error>(())
            }
            .await;
            ui.launch_abort(&touched);
            if let Err(e) = result {
                let mut message = if created.is_empty() {
                    format!("Launch failed: {e}. No session was created.")
                } else {
                    format!("Launch incomplete: {e}. Created {}; start them individually from the wall.", created.join(", "))
                };
                if !created.is_empty() && created.len() < agents {
                    message.push_str(&format!(" {} of {agents} agents were not created.", agents - created.len()));
                }
                if !unstaged.is_empty() {
                    message.push_str(&format!(" Not queued: {}.", unstaged.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(", ")));
                }
                ui.show_error(&message);
            }
            ui.launch_busy.set(false);
            ui.launch_box.set_sensitive(true);
            key.set_sensitive(true);
            ui.refresh();
        });
    }
}

const STALE: &str = "This form is out of date after a reconnect. Close it and open New agents again.";

fn launch_is_current(ui: &Ui, project: i64, generation: u64, form: &gtk::Box) -> bool {
    ui.project.get() == project && ui.generation.get() == generation && launch_is_shown(ui, form)
}

/// This form is the one the sheet shows, whatever has changed under it.
fn launch_is_shown(ui: &Ui, form: &gtk::Box) -> bool {
    ui.launch.reveals_child() && ui.launch_box.first_child().as_ref() == Some(form.upcast_ref())
}

#[cfg(test)]
mod tests {
    use super::{group_launch_order, record_tick, task_order};

    #[test]
    fn the_opening_task_comes_first_then_the_tick_order() {
        // Card order is backlog first: #3 (backlog), #7 (backlog), #12 (ready).
        let mut ticks = Vec::new();
        record_tick(&mut ticks, 7, true);
        record_tick(&mut ticks, 3, true);
        assert_eq!(task_order(&[3, 7, 12], Some(12), &ticks), [12, 7, 3]);
        // Unticking drops a task; ticking it again puts it last.
        record_tick(&mut ticks, 7, false);
        assert_eq!(task_order(&[3, 12], Some(12), &ticks), [12, 3]);
        record_tick(&mut ticks, 7, true);
        assert_eq!(task_order(&[3, 7, 12], Some(12), &ticks), [12, 3, 7]);
    }

    #[test]
    fn an_unticked_opening_task_is_left_out() {
        assert_eq!(task_order(&[3, 7], Some(12), &[7, 3]), [7, 3]);
        assert_eq!(task_order(&[3, 7], None, &[7]), [7, 3]);
        assert!(task_order(&[], Some(12), &[]).is_empty());
    }

    #[test]
    fn a_review_group_starts_its_lead_then_its_reviewer() {
        // Rail order is lead, second builder, reviewer; each pairs with the one before it.
        assert_eq!(group_launch_order(&[0, 4, 1]), [0, 1, 4]);
        assert_eq!(group_launch_order(&[2, 0]), [2, 0]);
    }
}
