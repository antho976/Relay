use super::{action, current, paragraph, section};
use crate::app::{button, clear, field, label, rows, text, Ui};
use crate::client::{Client, Notice};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::rc::Rc;

pub async fn refresh(ui: &Rc<Ui>, project: i64) {
    let generation = ui.generation.get();
    let (result, avds) = tokio::join!(ui.call("device.list", json!({})), ui.call("avd.list", json!({})));
    if !current(ui, "devices", project, generation) {
        return;
    }
    let devices = match result {
        Ok(v) => rows(&v, "devices"),
        Err(e) => {
            ui.show_error(&e.to_string());
            return;
        }
    };
    let avds = avds.map(|value| rows(&value, "avds"));
    let page = &ui.pages["devices"];
    if ui.page_projects.borrow().get("devices") != Some(&project) {
        ui.page_projects.borrow_mut().remove("devices");
        clear(page);
        // The title with its refresh key at the end of the line, as the other pages' headers:
        // alone in the middle of the page it read as a stray link.
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = label("Device control", "page-title");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        head.append(&title);
        let reload = button("", "quiet");
        let caption = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        caption.append(&crate::icons::image("refresh", 13));
        caption.append(&label("Refresh", ""));
        reload.set_child(Some(&caption));
        reload.set_tooltip_text(Some("Read the devices and checkouts again"));
        reload.update_property(&[gtk::accessible::Property::Label("Refresh devices and checkouts")]);
        reload.set_valign(gtk::Align::Center);
        head.append(&reload);
        page.append(&head);
        let lede = paragraph("Run Android builds from the selected checkout. A release build needs no connected device.");
        lede.remove_css_class("body");
        lede.add_css_class("page-lede");
        page.append(&lede);
        let weak = Rc::downgrade(ui);
        reload.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.page_projects.borrow_mut().remove("devices");
                ui.refresh_page();
            }
        });
        let worktrees = worktrees(ui, project).await;
        let last = last_worktree(ui, project).await;
        if !current(ui, "devices", project, generation) {
            return;
        }
        let tree = build_form(ui, page, project, &worktrees, last.as_deref());
        mark_dirty(ui, project, vec![tree]);
        avd_form(ui, page);
        // Devices, release signing and history. Signing holds a form being typed into, so
        // it is rebuilt only when what it shows changes, not on every engine event.
        for _ in 0..3 {
            page.append(&gtk::Box::new(gtk::Orientation::Vertical, 8));
        }
        // Navigation can cancel the await above. Mark only a fully built form reusable.
        ui.page_projects
            .borrow_mut()
            .insert("devices".into(), project);
    }
    sync_targets(&devices, avds.as_deref().unwrap_or_default());
    // Read everything first, then redraw in one go: a list rebuilt across awaits shrinks
    // under someone scrolled down to the history.
    let (signing, runs) = if project > 0 {
        (
            Some(ui.call("device.signing.get", json!({"project_id":project})).await),
            Some(ui.call("device.run.list", json!({"project_id":project})).await),
        )
    } else {
        (None, None)
    };
    if !current(ui, "devices", project, generation) {
        return;
    }
    let history = page.last_child().unwrap().downcast::<gtk::Box>().unwrap();
    let signing_box = history.prev_sibling().unwrap().downcast::<gtk::Box>().unwrap();
    let list = signing_box.prev_sibling().unwrap().downcast::<gtk::Box>().unwrap();
    clear(&list);
    let all = section(&list, "Devices");
    let avds = listed_avds(&all, avds);
    device_list(ui, &all, &devices, &avds);
    match signing {
        None => clear(&signing_box),
        Some(Ok(signing)) if signing["configured"].as_bool() == Some(true) => {
            clear(&signing_box);
            let row = section(&signing_box, "Release signing");
            let enabled = signing["enabled"].as_bool() == Some(true);
            row.append(&paragraph(&format!(
                "Alias: {}\nKeystore: {}",
                text(&signing, "key_alias"),
                text(&signing, "keystore")
            )));
            signing_toggle(ui, &row, project, enabled);
        }
        // Keep the form already shown, and whatever is typed into it.
        Some(Ok(_)) if signing_box.first_child().is_some_and(|row| row.widget_name() == "signing-form") => {}
        Some(Ok(_)) => {
            clear(&signing_box);
            let row = section(&signing_box, "Release signing");
            row.set_widget_name("signing-form");
            row.append(&paragraph(
                "Using the project's Gradle signing configuration.",
            ));
            signing_form(ui, &row, project);
        }
        // A profile that cannot be loaded refuses every release build, and its refusal says to
        // switch Relay signing off: that is this key (the profile is set aside, its key kept).
        Some(Err(error)) => {
            clear(&signing_box);
            let row = section(&signing_box, "Release signing");
            row.append(&paragraph(&error.to_string()));
            signing_toggle(ui, &row, project, true);
        }
    }
    clear(&history);
    match runs {
        None => {}
        Some(Ok(value)) => {
            let history = section(&history, "Build and run history");
            let runs = rows(&value, "runs");
            if runs.is_empty() {
                history.append(&paragraph("No builds or runs yet."));
            }
            for run in runs {
                let row = section(
                    &history,
                    &format!(
                        "#{} · {} · {}",
                        run["id"],
                        text(&run, "kind"),
                        text(&run, "state")
                    ),
                );
                row.append(&paragraph(&format!(
                    "{}\n{} · {}",
                    text(&run, "worktree"),
                    text(&run, "variant"),
                    text(&run, "started_at")
                )));
                if let Some(artifact) = run["artifact"].as_str() {
                    row.append(&paragraph(&format!(
                        "{}\nSigning: {}",
                        artifact,
                        text(&run, "signing")
                    )));
                    let copy = button("Copy artifact path", "quiet");
                    row.append(&copy);
                    let path = artifact.to_string();
                    copy.connect_clicked(move |b| b.clipboard().set_text(&path));
                }
                if is_active(&run) {
                    action(
                        ui,
                        &row,
                        "Stop",
                        "device.run.stop",
                        json!({"run_id":run["id"]}),
                    );
                }
            }
        }
        Some(Err(error)) => {
            history.append(&paragraph(&error.to_string()));
        }
    }
}

/// "Target device" and the Run button. The form outlives refreshes, so the device list
/// below and this picker are kept in step: a phone plugged in later can be chosen.
struct Targets {
    combo: glib::WeakRef<gtk::ComboBoxText>,
    shown: Vec<(String, String)>,
}

thread_local! {
    static TARGETS: std::cell::RefCell<Option<Targets>> = const { std::cell::RefCell::new(None) };
}

/// Connected devices a run can target, as (serial, title): a running AVD by its name, and a
/// leased device marked, since a run on it is refused until the lease goes. Both surfaces' pickers.
fn device_targets(devices: &[Value], avds: &[Value]) -> Vec<(String, String)> {
    devices
        .iter()
        .filter(|device| text(device, "state") == "device")
        .map(|device| {
            let serial = text(device, "serial");
            let name = avds.iter().find(|avd| avd["running_serial"] == serial).map_or(text(device, "model"), |avd| text(avd, "name"));
            let busy = if device["lease"]["action"].is_string() { " · in use" } else { "" };
            (serial.to_string(), format!("{} · {serial}{busy}", name.replace('_', " ")))
        })
        .collect()
}

/// The AVDs `avd.list` gave, or a line in `parent` saying why there are none.
fn listed_avds(parent: &gtk::Box, avds: Result<Vec<Value>, crate::client::Error>) -> Vec<Value> {
    match avds {
        Ok(avds) => avds,
        Err(error) => {
            parent.append(&paragraph(&format!("Virtual devices unavailable: {error}")));
            Vec::new()
        }
    }
}

/// A build or run that is still going, and so has a Stop button.
fn is_active(run: &Value) -> bool {
    matches!(text(run, "state"), "building" | "running" | "installing" | "launching")
}

/// Switches a project with a Relay key between it and the project's own Gradle signing.
fn signing_toggle(ui: &Rc<Ui>, parent: &gtk::Box, project: i64, enabled: bool) {
    action(
        ui,
        parent,
        if enabled { "Use Gradle signing" } else { "Use Relay signing" },
        "device.signing.set_enabled",
        json!({"project_id":project,"enabled":!enabled}),
    );
}

/// Refill "Target device" when the connected devices change, keeping the chosen one if it
/// is still there. Its `changed` handler recomputes the Run button.
fn sync_targets(devices: &[Value], avds: &[Value]) {
    let next = device_targets(devices, avds);
    let Some(combo) = TARGETS.with(|targets| {
        let mut targets = targets.borrow_mut();
        let targets = targets.as_mut()?;
        if targets.shown == next {
            return None;
        }
        targets.shown = next.clone();
        targets.combo.upgrade()
    }) else {
        return;
    };
    let selected = combo.active_id();
    combo.remove_all();
    for (serial, title) in &next {
        combo.append(Some(serial), title);
    }
    if !selected.is_some_and(|id| combo.set_active_id(Some(&id))) {
        combo.set_active(Some(0));
    }
}

/// One checkout the picker offers.
struct Checkout {
    branch: String,
    path: String,
    /// The project's own checkout: built with no `worktree` in the payload.
    primary: bool,
    session: Option<String>,
    /// Filled in by `mark_dirty` after the picker is shown.
    dirty: std::cell::Cell<bool>,
}

/// "Build from": a searchable list of the project's checkouts, branch first. The checkout of
/// the last build or run is preselected and tagged, else the project's own checkout, so a
/// rebuild is one click.
#[derive(Clone)]
struct WorktreePicker {
    widget: gtk::MenuButton,
    inner: Rc<Picker>,
}

/// The picker's state. Its own signal handlers hold it weakly, so it goes with its widgets.
struct Picker {
    widget: gtk::MenuButton,
    checkouts: Rc<Vec<Checkout>>,
    selected: std::cell::Cell<usize>,
    /// Index of the checkout the last build or run used, when it is still there.
    last: Option<usize>,
    face_branch: gtk::Label,
    face_path: gtk::Label,
    face_tag: gtk::Label,
    search: gtk::SearchEntry,
    rows: gtk::ListBox,
}

impl WorktreePicker {
    fn new(worktrees: &[Value], last: Option<&str>) -> Self {
        Picker::build(worktrees, last)
    }

    fn active_id(&self) -> Option<String> {
        self.inner.active_id()
    }

    fn choose(&self, index: i32) {
        self.inner.choose(index);
    }

    /// Tags the checkouts with uncommitted changes, from a `worktree.list` that reports them.
    fn mark_dirty(&self, worktrees: &[Value]) {
        let picker = &self.inner;
        for checkout in picker.checkouts.iter() {
            checkout.dirty.set(worktrees.iter().any(|tree| text(tree, "path") == checkout.path && tree["dirty"] == true));
        }
        picker.rows.remove_all();
        for index in 0..picker.checkouts.len() {
            picker.rows.append(&picker.row(index));
        }
        picker.show_selected();
    }
}

impl Picker {
    /// `worktrees` as `worktree.list` returns them (the project's own checkout first);
    /// `last` is the worktree path of the most recent build or run.
    fn build(worktrees: &[Value], last: Option<&str>) -> WorktreePicker {
        let mut checkouts: Vec<Checkout> = worktrees
            .iter()
            .enumerate()
            .map(|(index, tree)| Checkout {
                branch: text(tree, "branch").to_string(),
                path: text(tree, "path").to_string(),
                primary: index == 0,
                session: tree["session"].as_str().map(str::to_string),
                dirty: std::cell::Cell::new(tree["dirty"].as_bool() == Some(true)),
            })
            .collect();
        if checkouts.is_empty() {
            checkouts.push(Checkout { branch: "Project checkout".into(), path: String::new(), primary: true, session: None, dirty: Default::default() });
        }
        let last = last
            .filter(|path| !path.is_empty())
            .and_then(|path| checkouts.iter().position(|checkout| checkout.path.trim_end_matches('/') == path.trim_end_matches('/')));

        let face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        face.append(&crate::icons::image("branch", 14));
        let names = gtk::Box::new(gtk::Orientation::Vertical, 1);
        names.set_hexpand(true);
        let face_branch = label("", "build-from-branch");
        face_branch.set_ellipsize(gtk::pango::EllipsizeMode::End);
        face_branch.set_max_width_chars(30);
        let face_path = label("", "build-from-path");
        face_path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        face_path.set_max_width_chars(30);
        names.append(&face_branch);
        names.append(&face_path);
        face.append(&names);
        let face_tag = label("", "build-from-tag");
        face_tag.set_valign(gtk::Align::Center);
        face.append(&face_tag);
        face.append(&crate::icons::image("chevron-down", 12));
        let widget = gtk::MenuButton::new();
        widget.set_widget_name("device-worktree");
        widget.add_css_class("build-from");
        widget.set_child(Some(&face));
        widget.set_hexpand(true);

        let sheet = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sheet.add_css_class("build-from-sheet");
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search branches, paths or sessions"));
        sheet.append(&search);
        let rows = gtk::ListBox::new();
        rows.add_css_class("build-from-list");
        rows.set_selection_mode(gtk::SelectionMode::None);
        let empty = label("No branch matches.", "dim");
        empty.set_margin_top(10);
        empty.set_margin_bottom(10);
        rows.set_placeholder(Some(&empty));
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(340)
            .child(&rows)
            .build();
        sheet.append(&scroll);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&sheet));
        popover.set_has_arrow(false);
        widget.set_popover(Some(&popover));

        let picker = Rc::new(Self {
            widget: widget.clone(),
            checkouts: Rc::new(checkouts),
            selected: std::cell::Cell::new(last.unwrap_or(0)),
            last,
            face_branch,
            face_path,
            face_tag,
            search: search.clone(),
            rows,
        });
        for index in 0..picker.checkouts.len() {
            picker.rows.append(&picker.row(index));
        }
        picker.show_selected();

        let checkouts = picker.checkouts.clone();
        let query = search.downgrade();
        picker.rows.set_filter_func(move |row| {
            let needle = query.upgrade().map(|query| query.text().trim().to_lowercase()).unwrap_or_default();
            let Some(checkout) = usize::try_from(row.index()).ok().and_then(|index| checkouts.get(index)) else { return true };
            needle.is_empty()
                || checkout.branch.to_lowercase().contains(&needle)
                || checkout.path.to_lowercase().contains(&needle)
                || checkout.session.as_deref().is_some_and(|session| session.to_lowercase().contains(&needle))
        });
        let rows = picker.rows.downgrade();
        search.connect_search_changed(move |_| {
            if let Some(rows) = rows.upgrade() {
                rows.invalidate_filter();
            }
        });
        let weak = Rc::downgrade(&picker);
        picker.rows.connect_row_activated(move |_, row| {
            if let Some(picker) = weak.upgrade() {
                picker.choose(row.index());
            }
        });
        // Enter in the search box takes the first match.
        let weak = Rc::downgrade(&picker);
        search.connect_activate(move |_| {
            let Some(picker) = weak.upgrade() else { return };
            let mut index = 0;
            while let Some(row) = picker.rows.row_at_index(index) {
                if row.is_child_visible() {
                    picker.choose(index);
                    return;
                }
                index += 1;
            }
        });
        let close = popover.downgrade();
        search.connect_stop_search(move |_| {
            if let Some(popover) = close.upgrade() {
                popover.popdown();
            }
        });
        let focus = search.downgrade();
        popover.connect_show(move |_| {
            if let Some(search) = focus.upgrade() {
                search.set_text("");
                search.grab_focus();
            }
        });
        WorktreePicker { widget, inner: picker }
    }

    fn tag(&self, index: usize) -> Option<&'static str> {
        match self.last {
            Some(last) if last == index => Some("last build"),
            None if self.checkouts[index].primary => Some("checked out"),
            _ => None,
        }
    }

    fn row(&self, index: usize) -> gtk::ListBoxRow {
        let checkout = &self.checkouts[index];
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let mark = crate::icons::image("check", 14);
        mark.add_css_class("build-from-mark");
        mark.set_valign(gtk::Align::Center);
        body.append(&mark);
        let names = gtk::Box::new(gtk::Orientation::Vertical, 2);
        names.set_hexpand(true);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let branch = label(&checkout.branch, "build-from-branch");
        branch.set_ellipsize(gtk::pango::EllipsizeMode::End);
        branch.set_max_width_chars(34);
        branch.set_hexpand(true);
        top.append(&branch);
        if let Some(tag) = self.tag(index) {
            top.append(&label(tag, "build-from-tag"));
        }
        names.append(&top);
        let mut facts = vec![short_path(&checkout.path)];
        if let Some(session) = &checkout.session {
            facts.push(session.clone());
        }
        if checkout.dirty.get() {
            facts.push("uncommitted changes".into());
        }
        let path = label(&facts.join(" · "), "build-from-path");
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_max_width_chars(40);
        names.append(&path);
        body.append(&names);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&body));
        row.set_tooltip_text(Some(&checkout.path));
        row
    }

    fn choose(&self, index: i32) {
        let Ok(index) = usize::try_from(index) else { return };
        if index >= self.checkouts.len() {
            return;
        }
        self.selected.set(index);
        self.show_selected();
        self.widget.popdown();
    }

    fn show_selected(&self) {
        let index = self.selected.get();
        let checkout = &self.checkouts[index];
        self.face_branch.set_text(&checkout.branch);
        self.face_path.set_text(&short_path(&checkout.path));
        self.widget.set_tooltip_text(Some(&checkout.path));
        let tag = self.tag(index);
        self.face_tag.set_text(tag.unwrap_or(""));
        self.face_tag.set_visible(tag.is_some());
        let mut position = 0;
        while let Some(row) = self.rows.row_at_index(position) {
            if position as usize == index {
                row.add_css_class("chosen");
            } else {
                row.remove_css_class("chosen");
            }
            position += 1;
        }
    }

    /// The `worktree` to send: none for the project's own checkout.
    fn active_id(&self) -> Option<String> {
        let checkout = &self.checkouts[self.selected.get()];
        (!checkout.primary).then(|| checkout.path.clone())
    }
}

/// The tail of a checkout path: what tells two worktrees apart.
fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').rsplit('/').take(3).collect();
    if parts.len() < 3 {
        return path.to_string();
    }
    format!("…/{}", parts.into_iter().rev().collect::<Vec<_>>().join("/"))
}

/// The project's checkouts for "Build from", without the dirty flags: those cost a full
/// `git status` per checkout, so the picker is shown first and `mark_dirty` adds them.
async fn worktrees(ui: &Rc<Ui>, project: i64) -> Vec<Value> {
    match ui.call("worktree.list", json!({"project_id":project,"include_dirty":false})).await {
        Ok(v) => rows(&v, "worktrees"),
        Err(_) => Vec::new(),
    }
}

/// Marks the checkouts with uncommitted changes in `pickers` once the engine has looked.
fn mark_dirty(ui: &Rc<Ui>, project: i64, pickers: Vec<WorktreePicker>) {
    if project <= 0 || pickers.is_empty() {
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if let Ok(value) = ui.call("worktree.list", json!({"project_id":project,"include_dirty":true})).await {
            let trees = rows(&value, "worktrees");
            for picker in &pickers {
                picker.mark_dirty(&trees);
            }
        }
    });
}

/// The worktree of the project's most recent build or run, for the picker's default.
async fn last_worktree(ui: &Rc<Ui>, project: i64) -> Option<String> {
    if project <= 0 {
        return None;
    }
    let runs = ui.call("device.run.list", json!({"project_id":project})).await.ok()?;
    rows(&runs, "runs").first().map(|run| text(run, "worktree").to_string())
}

pub(crate) async fn verify_worktree_picker(ui: &Rc<Ui>) {
    assert_eq!(std::env::var("RELAY_INSTANCE").as_deref(), Ok("test"));
    let path = format!("/tmp/{}", "long-checkout-name/".repeat(30));
    let picker = WorktreePicker::new(
        &[
            json!({"branch":"main","path":"/tmp/project"}),
            json!({"branch":"very-long-branch-name/".repeat(30),"path":path}),
        ],
        Some(&path),
    );
    let window = gtk::Window::builder()
        .transient_for(&ui.window)
        .default_width(400)
        .default_height(80)
        .build();
    window.set_child(Some(&picker.widget));
    window.present();
    // The last build's checkout is preselected; the primary checkout builds with no worktree.
    assert_eq!(picker.active_id().as_deref(), Some(path.as_str()));
    picker.choose(0);
    assert_eq!(picker.active_id(), None);
    // Search filters by branch; Enter takes the first match.
    picker.inner.search.set_text("very-long");
    glib::timeout_future(std::time::Duration::from_millis(400)).await;
    let visible = |index| picker.inner.rows.row_at_index(index).unwrap().is_child_visible();
    assert!(!visible(0) && visible(1), "Search must hide branches that do not match");
    picker.inner.search.emit_activate();
    assert_eq!(picker.active_id().as_deref(), Some(path.as_str()));
    // Measure what the picker asks for: an unallocated window reports a width of 0 (RA-721).
    let minimum = picker.widget.measure(gtk::Orientation::Horizontal, -1).0;
    assert!(minimum <= 430, "Long branch and path must not expand the device surface: {minimum}px");
    window.close();
}

/// The target picker starts empty: `refresh` fills it through `sync_targets` right after.
fn build_form(ui: &Rc<Ui>, page: &gtk::Box, project: i64, worktrees: &[Value], last: Option<&str>) -> WorktreePicker {
    let form = section(page, "Build & run");
    let tree = WorktreePicker::new(worktrees, last);
    field("Build from", &tree.widget, &form);
    let target = gtk::ComboBoxText::new();
    TARGETS.with(|targets| *targets.borrow_mut() = Some(Targets { combo: target.downgrade(), shown: Vec::new() }));
    field("Target device", &target, &form);
    let variant = gtk::Entry::builder().text("debug").build();
    field("Gradle variant", &variant, &form);
    let format = gtk::ComboBoxText::new();
    format.append(Some("apk"), "APK");
    format.append(Some("bundle"), "Android App Bundle");
    format.set_active(Some(0));
    field("Artifact format", &format, &form);
    // Gradle Play Publisher's upload (D152): a published release cannot be recalled.
    let publish = gtk::CheckButton::with_label("Publish to Google Play (Gradle Play Publisher)");
    form.append(&publish);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    form.append(&controls);
    for (title, op) in [
        ("Run on device", "device.run"),
        ("Build artifact", "device.build"),
    ] {
        let key = button(title, "primary");
        key.set_sensitive(project > 0 && (op != "device.run" || target.active_id().is_some()));
        if op == "device.run" {
            let run = key.downgrade();
            target.connect_changed(move |target| {
                if let Some(run) = run.upgrade() {
                    run.set_sensitive(project > 0 && target.active_id().is_some());
                }
            });
        }
        controls.append(&key);
        let weak = Rc::downgrade(ui);
        let tree = tree.clone();
        let target = target.clone();
        let variant = variant.clone();
        let format = format.clone();
        let publish = publish.clone();
        let armed = publish.clone();
        let fire = move |_: &gtk::Button|{
            let Some(ui)=weak.upgrade()else{return;};let selected=tree.active_id().map(|v|v.to_string()).unwrap_or_default();let variant=variant.text().trim().to_string();
            if variant.is_empty(){ui.show_error("Enter the Gradle variant to build.");return;}
            let mut payload=json!({"project_id":project,"worktree":if selected.is_empty(){None}else{Some(selected)},"variant":variant});
            if op=="device.run"{let Some(device)=target.active_id()else{return;};payload["device"]=json!(device.as_str());}
            else{payload["format"]=json!(format.active_id().map(|v|v.to_string()).unwrap_or_else(||"apk".into()));payload["publish"]=json!(publish.is_active());}
            run_window(&ui,op,payload,title);
        };
        // A build that uploads to Play asks for a second click first.
        if op == "device.build" {
            crate::app::confirm_inline_if(&key, "Publish to Google Play?", move || armed.is_active(), fire);
        } else {
            key.connect_clicked(fire);
        }
    }
    tree
}

fn avd_form(ui: &Rc<Ui>, page: &gtk::Box) {
    let expander = gtk::Expander::new(Some("Create virtual device"));
    page.append(&expander);
    let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
    expander.set_child(Some(&form));
    let name = gtk::Entry::builder()
        .placeholder_text("Relay_API_35")
        .build();
    field("AVD name", &name, &form);
    let image = gtk::ComboBoxText::new();
    field("Installed system image", &image, &form);
    let profile = gtk::ComboBoxText::new();
    profile.append(Some(""), "SDK default");
    profile.set_active(Some(0));
    field("Device profile", &profile, &form);
    let load = button("Load installed images", "quiet");
    form.append(&load);
    let weak = Rc::downgrade(ui);
    let images = image.clone();
    let profiles = profile.clone();
    load.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        key.set_sensitive(false);
        let key = key.clone();
        let images = images.clone();
        let profiles = profiles.clone();
        glib::spawn_future_local(async move {
            match ui.call("avd.catalog", json!({})).await {
                Ok(value) => {
                    images.remove_all();
                    profiles.remove_all();
                    profiles.append(Some(""), "SDK default");
                    for item in rows(&value, "system_images") {
                        if let Some(id) = item.as_str() {
                            images.append(Some(id), id);
                        }
                    }
                    for item in rows(&value, "devices") {
                        if let Some(id) = item.as_str() {
                            profiles.append(Some(id), id);
                        }
                    }
                    images.set_active(Some(0));
                    profiles.set_active(Some(0));
                }
                Err(error) => ui.show_error(&error.to_string()),
            }
            key.set_sensitive(true);
        });
    });
    let create = button("Create AVD", "primary");
    form.append(&create);
    let weak = Rc::downgrade(ui);
    create.connect_clicked(move |key| {
        if let Some(ui) = weak.upgrade() {
            let title = name.text().trim().to_string();
            let Some(package) = image.active_id() else {
                ui.show_error("Load and select an installed system image first.");
                return;
            };
            if title.is_empty() {
                ui.show_error("Enter an AVD name.");
                return;
            }
            let device = profile
                .active_id()
                .map(|v| v.to_string())
                .filter(|v| !v.is_empty());
            ui.mutate(
                "avd.create",
                json!({"name":title,"package":package.as_str(),"device":device}),
                key,
            );
        }
    });
}

/// Phones and virtual devices in one list. A connected device opens in the mirror; an AVD that
/// is off boots headless straight into it, so both end up in the same window.
fn device_list(ui: &Rc<Ui>, parent: &gtk::Box, devices: &[Value], avds: &[Value]) {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    list.add_css_class("device-list");
    parent.append(&list);
    let stopped: Vec<&Value> = avds.iter().filter(|avd| !avd["running_serial"].is_string()).collect();
    if devices.is_empty() && stopped.is_empty() {
        let copy = paragraph("No Android device. Connect a phone with USB debugging enabled, or create a virtual device below.");
        list.append(&copy);
        return;
    }
    for device in devices {
        let serial = text(device, "serial").to_string();
        let state = text(device, "state");
        let avd = avds.iter().find(|avd| avd["running_serial"] == serial.as_str()).map(|avd| text(avd, "name"));
        let virtual_device = avd.is_some() || text(device, "kind") == "avd";
        let name = avd.unwrap_or(text(device, "model")).replace('_', " ");
        let mut facts = vec![serial.clone(), if virtual_device { "virtual".into() } else { "USB".into() }];
        if state != "device" {
            facts.push(state.to_string());
        }
        let row = device_row(&name, &facts.join(" · "), if state == "device" { "live" } else { "held" });
        if state == "device" {
            let mirror = button("Mirror", "primary");
            mirror.set_valign(gtk::Align::Center);
            mirror.set_tooltip_text(Some("Show and control this device's screen"));
            let weak = Rc::downgrade(ui);
            let target = serial.clone();
            mirror.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    crate::mirror::open(&ui, target.clone());
                }
            });
            row.append(&mirror);
        }
        // A virtual device runs headless; this is its off switch.
        if let Some(avd) = avd {
            let stop = button("Stop", "quiet");
            stop.set_valign(gtk::Align::Center);
            stop.set_tooltip_text(Some("Shut this virtual device down"));
            let weak = Rc::downgrade(ui);
            let payload = json!({"name":avd});
            stop.connect_clicked(move |key| {
                if let Some(ui) = weak.upgrade() {
                    ui.mutate("avd.stop", payload.clone(), key);
                }
            });
            row.append(&stop);
        }
        let copy = crate::app::icon_button("copy", "Copy device ID");
        copy.set_valign(gtk::Align::Center);
        copy.connect_clicked(move |key| {
            key.clipboard().set_text(&serial);
            key.set_tooltip_text(Some("Copied"));
        });
        row.append(&copy);
        list.append(&row);
        if let Some(lease) = lease_row(ui, device) {
            list.append(&lease);
        }
    }
    for avd in stopped {
        let name = text(avd, "name").to_string();
        let row = device_row(&name.replace('_', " "), "virtual · off", "");
        for (title, cold, style, tip) in [
            ("Start", false, "primary", "Boot this virtual device and open it in the mirror"),
            ("Cold", true, "quiet", "Cold boot: ignore the saved snapshot"),
        ] {
            let key = button(title, style);
            key.set_valign(gtk::Align::Center);
            key.set_tooltip_text(Some(tip));
            let weak = Rc::downgrade(ui);
            let name = name.clone();
            key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    crate::mirror::open_avd(&ui, name.clone(), cold);
                }
            });
            row.append(&key);
        }
        list.append(&row);
    }
}

/// A device row's frame: status lamp, name over facts; the caller appends the actions.
fn device_row(name: &str, facts: &str, lamp: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("device-row");
    let mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mark.add_css_class("lamp");
    if !lamp.is_empty() {
        mark.add_css_class(lamp);
    }
    mark.set_valign(gtk::Align::Center);
    row.append(&mark);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    let title = label(name, "device-row-name");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&title);
    let detail = label(facts, "device-row-facts");
    detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    words.append(&detail);
    row.append(&words);
    row
}

// A separate socket keeps noisy Gradle/logcat output away from the UI control plane.
fn run_window(ui: &Rc<Ui>, op: &'static str, payload: Value, title: &str) {
    let window = crate::panel::Panel::new(ui, title, 780);
    let page = window.body.clone();
    page.append(&paragraph("Closing this log panel leaves the build or run active. Use Stop in device history to stop it."));
    let output = gtk::TextView::new();
    output.set_editable(false);
    output.set_monospace(true);
    output.set_wrap_mode(gtk::WrapMode::WordChar);
    let scroll = crate::app::scrolled(&output);
    page.append(&scroll);
    // Right gravity: the mark stays after every line appended at the end.
    let bottom = output.buffer().create_mark(None, &output.buffer().end_iter(), false);
    let status = label("Connecting…", "dim");
    page.append(&status);
    let ui = ui.clone();
    let task = glib::spawn_future_local(async move {
        match Client::connect(&ui.rt, ui.path.clone()).await {
            Ok((client, notices)) => {
                // The run's stream carries its log lines only; its state arrives as run.changed.
                // Without the subscription the status keeps the first state, and the log runs.
                let _ = client.request(&ui.rt, "bus.subscribe", json!({"events":["run.changed"]})).await;
                let id = match client.request(&ui.rt, op, payload).await {
                    Ok(run) => {
                        status.set_text(&format!("Run #{} · {}", run["id"], text(&run, "state")));
                        run["id"].clone()
                    }
                    Err(error) => {
                        status.set_text(&error.to_string());
                        return;
                    }
                };
                ui.refresh_page();
                while let Ok(notice) = notices.recv().await {
                    match notice {
                        Notice::Event(event) if event.ev == "run.changed" && event.payload["id"] == id => {
                            status.set_text(&format!("Run #{id} · {}", text(&event.payload, "state")));
                        }
                        Notice::Frame(frame) if frame.stream == "logcat" => {
                            // Follow new output, unless the reader has scrolled up to read.
                            let at = scroll.vadjustment();
                            let follow = at.value() + at.page_size() >= at.upper() - 8.;
                            let buffer = output.buffer();
                            let mut end = buffer.end_iter();
                            buffer.insert(
                                &mut end,
                                &format!("{}\n", frame.data.as_str().unwrap_or("")),
                            );
                            if follow {
                                output.scroll_mark_onscreen(&bottom);
                            }
                            if buffer.line_count() > 2000 {
                                if let Some(mut trim) =
                                    buffer.iter_at_line(buffer.line_count() - 2000)
                                {
                                    buffer.delete(&mut buffer.start_iter(), &mut trim);
                                }
                            }
                        }
                        Notice::Disconnected(error) => {
                            status.set_text(&error.to_string());
                            break;
                        }
                        _ => {}
                    }
                }
                drop(client);
            }
            Err(error) => status.set_text(&error.to_string()),
        }
    });
    window.on_closed(move || task.abort());
    window.present();
}

fn signing_form(ui: &Rc<Ui>, row: &gtk::Box, project: i64) {
    let expander = gtk::Expander::new(Some("Create Relay signing key"));
    row.append(&expander);
    let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
    expander.set_child(Some(&form));
    let alias = gtk::Entry::builder().text("upload").build();
    field("Key alias", &alias, &form);
    let password = gtk::PasswordEntry::builder().show_peek_icon(true).build();
    let passwords = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    passwords.set_homogeneous(true);
    let left = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let right = gtk::Box::new(gtk::Orientation::Vertical, 6);
    field("Password", &password, &left);
    let confirmation = gtk::PasswordEntry::builder().show_peek_icon(true).build();
    field("Confirm", &confirmation, &right);
    passwords.append(&left);
    passwords.append(&right);
    form.append(&passwords);
    form.append(&paragraph("The key is stored privately and its password goes to Linux Secret Service. Export a backup before publishing releases with this key."));
    let create = button("Create signing key", "primary");
    form.append(&create);
    let weak = Rc::downgrade(ui);
    create.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let alias = alias.text().trim().to_string();
        let secret = password.text().to_string();
        if secret != confirmation.text() {
            ui.show_error("Passwords do not match.");
            return;
        }
        if alias.is_empty() || secret.len() < 6 {
            ui.show_error("Enter a key alias and a password of at least six characters.");
            return;
        }
        key.set_sensitive(false);
        let key = key.clone();
        let password = password.clone();
        let confirmation = confirmation.clone();
        glib::spawn_future_local(async move {
            match ui
                .call(
                    "device.signing.create",
                    json!({"project_id":project,"key_alias":alias,"password":secret}),
                )
                .await
            {
                Ok(_) => {
                    password.set_text("");
                    confirmation.set_text("");
                    ui.refresh_page();
                }
                Err(e) => ui.show_error(&e.to_string()),
            }
            key.set_sensitive(true);
        });
    });
}

/// "In use by brisk-otter: adb install app-debug.apk · since 14:02" for a leased device, with
/// a Release button unless a run holds it (a run's lease goes when the run is stopped).
fn lease_row(ui: &Rc<Ui>, device: &Value) -> Option<gtk::Box> {
    let lease = &device["lease"];
    let action = lease["action"].as_str()?;
    let holder = lease["session"].as_str().unwrap_or("you");
    let since = lease["since"].as_str()
        .and_then(|since| glib::DateTime::from_iso8601(since, None).ok())
        .and_then(|time| time.to_local().ok())
        .and_then(|time| time.format("%H:%M").ok())
        .map(|time| format!(" · since {time}"))
        .unwrap_or_default();
    let run = lease["run_id"].as_i64().map(|id| format!(" · run {id}")).unwrap_or_default();
    let every = if text(lease, "device") == "*" { " · every device" } else { "" };
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("device-lease");
    row.set_tooltip_text(Some("Another run or install on this device is refused until this one is released, so sessions cannot overwrite each other's builds."));
    let mark = gtk::Box::new(gtk::Orientation::Vertical, 0);
    mark.add_css_class("device-lease-mark");
    mark.set_valign(gtk::Align::Center);
    row.append(&mark);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
    words.set_hexpand(true);
    let who = label(&format!("In use by {holder}"), "device-lease-holder");
    words.append(&who);
    let what = label(&format!("{action}{run}{since}{every}"), "device-lease-action");
    what.set_wrap(true);
    words.append(&what);
    row.append(&words);
    if lease["kind"] != "run" {
        action_quiet(ui, &row, "Release", json!({"device": lease["device"]}));
    }
    Some(row)
}

fn action_quiet(ui: &Rc<Ui>, parent: &gtk::Box, title: &str, payload: Value) {
    let key = button(title, "quiet");
    key.set_valign(gtk::Align::Center);
    key.set_tooltip_text(Some("Free the device for other sessions now"));
    let weak = Rc::downgrade(ui);
    key.connect_clicked(move |b| {
        if let Some(ui) = weak.upgrade() {
            ui.mutate("device.release", payload.clone(), b);
        }
    });
    parent.append(&key);
}

/// The events that change what the footer panel shows, its own actions' included.
const PANEL_EVENTS: [&str; 6] = [
    "device.changed",
    "device.lease.acquired",
    "device.lease.released",
    "avd.changed",
    "run.changed",
    "device.signing.changed",
];

/// Which of the footer panel's parts an event changed.
#[derive(Clone, Copy, Default)]
struct Parts {
    devices: bool,
    runs: bool,
    signing: bool,
}

impl Parts {
    const ALL: Self = Self { devices: true, runs: true, signing: true };

    fn note(&mut self, ev: &str) {
        match ev {
            "device.changed" | "device.lease.acquired" | "device.lease.released" | "avd.changed" => self.devices = true,
            "run.changed" => self.runs = true,
            "device.signing.changed" => self.signing = true,
            _ => {}
        }
    }
}

/// The parts of the footer panel that follow the engine while it is open. The pickers, the
/// artifact format and a signing key being typed stay as they are.
struct Live {
    devices: gtk::Box,
    /// "Run on" and its devices: absent without a project, hidden with no device connected.
    target: Option<(gtk::Label, gtk::ComboBoxText)>,
    /// Release signing: absent without a project.
    signing: Option<gtk::Box>,
    run_states: gtk::Box,
    release_states: gtk::Box,
}

impl Live {
    /// Reads what `parts` needs, then redraws it in one go.
    async fn redraw(&self, ui: &Rc<Ui>, project: i64, parts: Parts) {
        let listed = if parts.devices {
            Some(tokio::join!(ui.call("device.list", json!({})), ui.call("avd.list", json!({}))))
        } else {
            None
        };
        let runs = if parts.runs && project > 0 {
            Some(ui.call("device.run.list", json!({"project_id":project})).await)
        } else {
            None
        };
        let signing = if parts.signing && project > 0 {
            Some(ui.call("device.signing.get", json!({"project_id":project})).await)
        } else {
            None
        };
        if let Some((devices, avds)) = listed {
            clear(&self.devices);
            let devices = match devices {
                Ok(v) => rows(&v, "devices"),
                Err(e) => {
                    self.devices.append(&paragraph(&e.to_string()));
                    Vec::new()
                }
            };
            let avds = listed_avds(&self.devices, avds.map(|value| rows(&value, "avds")));
            device_list(ui, &self.devices, &devices, &avds);
            self.sync_target(&devices, &avds);
        }
        if let Some(Ok(value)) = runs {
            self.draw_runs(ui, &value);
        }
        if let (Some(Err(error)), Some(form)) = (&signing, &self.signing) {
            // Broken: the way out its refusals name is switching Relay signing off.
            clear(form);
            form.append(&paragraph(&error.to_string()));
            signing_toggle(ui, form, project, true);
        }
        if let (Some(Ok(signing)), Some(form)) = (signing, &self.signing) {
            clear(form);
            if signing["configured"] == true {
                form.append(&label(
                    if signing["enabled"] == true {
                        "Relay signing configured"
                    } else {
                        "Gradle signing"
                    },
                    "body",
                ));
                signing_toggle(ui, form, project, signing["enabled"] == true);
            } else {
                form.append(&label("Gradle signing", "body"));
                signing_form(ui, form, project);
            }
        }
    }

    /// Refills "Run on", keeping the chosen device while it is still connected.
    fn sync_target(&self, devices: &[Value], avds: &[Value]) {
        let Some((caption, target)) = &self.target else { return };
        let selected = target.active_id();
        target.remove_all();
        for (serial, title) in device_targets(devices, avds) {
            target.append(Some(&serial), &title);
        }
        if !selected.is_some_and(|id| target.set_active_id(Some(&id))) {
            target.set_active(Some(0));
        }
        let any = target.active_id().is_some();
        caption.set_visible(any);
        target.set_visible(any);
    }

    /// Active runs with their Stop buttons, and the newest artifact.
    fn draw_runs(&self, ui: &Rc<Ui>, value: &Value) {
        clear(&self.run_states);
        clear(&self.release_states);
        let runs = rows(value, "runs");
        for current in runs.iter().filter(|r| is_active(r)) {
            let state = gtk::Box::new(gtk::Orientation::Horizontal, 9);
            state.add_css_class("device-release-state");
            let copy = label(
                &format!("{} · {}", text(current, "kind"), text(current, "state")),
                "body",
            );
            copy.set_hexpand(true);
            state.append(&copy);
            action(
                ui,
                &state,
                "Stop",
                "device.run.stop",
                json!({"run_id":current["id"]}),
            );
            if text(current, "kind") == "build" {
                self.release_states.append(&state);
            } else {
                self.run_states.append(&state);
            }
        }
        if let Some(last) = runs.iter().find(|r| r["artifact"].is_string()) {
            let output = gtk::Box::new(gtk::Orientation::Horizontal, 9);
            output.add_css_class("device-release-state");
            let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
            copy.set_hexpand(true);
            copy.append(&label(
                &format!("{} artifact", text(last, "signing")),
                "body",
            ));
            let artifact = text(last, "artifact").to_owned();
            let path = label(artifact.rsplit('/').next().unwrap_or(&artifact), "mono");
            path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            path.set_tooltip_text(Some(&artifact));
            copy.append(&path);
            output.append(&copy);
            let key = button("Copy path", "quiet");
            key.connect_clicked(move |key| {
                key.clipboard().set_text(&artifact);
                key.set_label("Copied");
            });
            output.append(&key);
            self.release_states.append(&output);
        }
    }
}

/// Footer utility. The full device page retains build history and advanced controls.
pub fn open(ui: &Rc<Ui>) {
    let Some(panel) = crate::panel::Panel::toggle(ui, "Device control", 430) else {
        return;
    };
    panel.bottom(340);
    panel.add_css_class("device-panel");
    let refresh = crate::app::icon_button("refresh", "Refresh devices");
    refresh.set_widget_name("device-refresh");
    let weak = Rc::downgrade(ui);
    refresh.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            if ui.dismiss_panels() {
                open(&ui);
            }
        }
    });
    panel.header_action(&refresh);
    let tabs = gtk::Stack::new();
    tabs.set_widget_name("device-tabs");
    let switch = gtk::StackSwitcher::new();
    switch.set_stack(Some(&tabs));
    switch.set_halign(gtk::Align::Fill);
    panel.body.append(&switch);
    let run = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let release = gtk::Box::new(gtk::Orientation::Vertical, 12);
    tabs.add_titled(&run, Some("run"), "RUN");
    tabs.add_titled(&release, Some("release"), "RELEASE");
    panel.body.append(&tabs);
    run.append(&paragraph("Loading devices…"));
    release.append(&paragraph("Loading worktrees…"));
    let ui = ui.clone();
    let task = glib::spawn_future_local(async move {
        let project = ui.project.get();
        let (trees, last) = tokio::join!(worktrees(&ui, project), last_worktree(&ui, project));
        clear(&run);
        clear(&release);
        let live = Live {
            devices: gtk::Box::new(gtk::Orientation::Vertical, 6),
            target: (project > 0).then(|| (label("Run on", "dim"), gtk::ComboBoxText::new())),
            signing: (project > 0).then(|| gtk::Box::new(gtk::Orientation::Vertical, 12)),
            run_states: gtk::Box::new(gtk::Orientation::Vertical, 12),
            release_states: gtk::Box::new(gtk::Orientation::Vertical, 12),
        };
        run.append(&label("Devices", "title"));
        live.devices.append(&paragraph("Loading devices…"));
        run.append(&live.devices);
        avd_form(&ui, &run);
        release.append(&label("Release artifact", "title"));
        release.append(&paragraph(
            "Build the selected checkout with its Gradle release configuration.",
        ));
        let mut pickers = Vec::new();
        for (form, is_release) in [(&run, false), (&release, true)] {
            let (Some((caption, target)), Some(signing)) = (&live.target, &live.signing) else {
                form.append(&paragraph("Select a project to build."));
                continue;
            };
            let tree = WorktreePicker::new(&trees, last.as_deref());
            field("Build from", &tree.widget, form);
            pickers.push(tree.clone());
            if !is_release {
                // Filled, and shown once a device is connected, by `Live::sync_target`.
                caption.set_visible(false);
                target.set_visible(false);
                form.append(caption);
                form.append(target);
            }
            let format = gtk::ComboBoxText::new();
            format.append(Some("bundle"), "Android App Bundle (.aab) · Google Play");
            format.append(Some("apk"), "Android package (.apk)");
            format.set_active(Some(0));
            if is_release {
                field("Artifact", &format, form);
                form.append(signing);
            }
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            form.append(&actions);
            for publish in [false, true] {
                if publish && !is_release {
                    continue;
                }
                let key = button(
                    if publish {
                        "Build and publish"
                    } else if is_release {
                        "Build release"
                    } else {
                        "Run debug"
                    },
                    if publish { "quiet" } else { "primary" },
                );
                key.set_sensitive(is_release);
                if !is_release {
                    let run = key.downgrade();
                    target.connect_changed(move |target| {
                        if let Some(run) = run.upgrade() {
                            run.set_sensitive(target.active_id().is_some());
                        }
                    });
                }
                actions.append(&key);
                let weak = Rc::downgrade(&ui);
                let tree = tree.clone();
                let target = target.clone();
                let format = format.clone();
                let fire = move |_: &gtk::Button| {
                    let Some(ui) = weak.upgrade() else { return; };
                    let path=tree.active_id().map(|s|s.to_string()).filter(|s|!s.is_empty());
                    let mut payload=json!({"project_id":project,"worktree":path,"variant":if is_release {"release"} else {"debug"}});
                    if is_release { payload["format"]=json!(format.active_id().map(|s|s.to_string())); payload["publish"]=json!(publish); }
                    else { let Some(serial)=target.active_id() else {return;}; payload["device"]=json!(serial.as_str()); }
                    run_window(&ui, if is_release {"device.build"} else {"device.run"}, payload, if is_release {"Release build"} else {"Run debug"});
                };
                // The Play upload is armed before it fires: a published release cannot be recalled (D152).
                if publish {
                    crate::app::confirm_inline(&key, "Publish to Google Play?", fire);
                } else {
                    key.connect_clicked(fire);
                }
            }
        }
        run.append(&live.run_states);
        release.append(&live.release_states);
        let history = button("Build history and advanced controls", "quiet");
        release.append(&history);
        let weak = Rc::downgrade(&ui);
        history.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.dismiss_panels();
                ui.navigate("devices");
            }
        });
        mark_dirty(&ui, project, pickers);
        // Subscribe before the first read, so nothing that changes in between is missed. The
        // panel's own Release, Stop, Boot and signing actions come back this way too.
        let mut events = Client::connect(&ui.rt, ui.path.clone()).await.ok();
        if let Some((client, _)) = &events {
            if client.request(&ui.rt, "bus.subscribe", json!({"events":PANEL_EVENTS})).await.is_err() {
                events = None;
            }
        }
        live.redraw(&ui, project, Parts::ALL).await;
        let Some((client, notices)) = events else { return };
        while let Ok(notice) = notices.recv().await {
            let mut parts = Parts::default();
            match notice {
                Notice::Event(event) => parts.note(&event.ev),
                Notice::Disconnected(_) => break,
                Notice::Frame(_) => continue,
            }
            // A boot or a build sends several in a row: take what has queued and redraw once.
            glib::timeout_future(std::time::Duration::from_millis(150)).await;
            while let Ok(notice) = notices.try_recv() {
                if let Notice::Event(event) = notice {
                    parts.note(&event.ev);
                }
            }
            live.redraw(&ui, project, parts).await;
        }
        drop(client);
    });
    panel.on_closed(move || task.abort());
    panel.present();
}
