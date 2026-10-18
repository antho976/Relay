//! The sidebar's workspace and project tree, the project/workspace editor, and their removal.
//!
//! The tree scales by staying quiet: groups collapse (persisted), a filter appears once there
//! are enough projects to need one, each group shows its first few projects behind a
//! "Show N more" row, and row tools only appear under the pointer or keyboard focus.
use super::*;
use std::collections::BTreeSet;

/// Projects a group shows before the rest fold behind "Show N more". One extra is shown
/// rather than a "Show 1 more" row that saves nothing.
const GROUP_CAP: usize = 6;
/// The filter appears once the sidebar holds more projects than this.
const FILTER_AT: usize = 8;
const COLLAPSED_KEY: &str = "native.sidebar.collapsed_workspaces";

#[derive(Default)]
struct Sidebar {
    /// Workspace groups folded to their header. Persisted under [`COLLAPSED_KEY`].
    collapsed: BTreeSet<i64>,
    /// Groups showing every project past [`GROUP_CAP`]. Per run, not persisted.
    expanded: BTreeSet<i64>,
    loaded: bool,
    loading: bool,
    /// Whether the saved folding was read. A failed read never overwrites it.
    persist: bool,
    /// The project last seen active, so a switch into a collapsed group can reveal it.
    active: i64,
    /// The first project the filter matches, opened by Enter.
    first_match: Option<i64>,
    filter: Option<gtk::SearchEntry>,
    /// Hash of what the tree last rendered; `None` forces the next render to rebuild.
    rendered: Option<u64>,
}

thread_local! {
    static SIDEBAR: RefCell<Sidebar> = RefCell::default();
}

/// Borrow the sidebar state briefly. Never call back into GTK while holding it: a signal
/// handler may re-enter `render_projects`.
fn sidebar<R>(f: impl FnOnce(&mut Sidebar) -> R) -> R {
    SIDEBAR.with(|s| f(&mut s.borrow_mut()))
}

fn id_of(v: &Value) -> i64 {
    v["id"].as_i64().unwrap_or(0)
}

/// Open agents, and the lamp the sidebar shows for them.
struct Agents {
    open: usize,
    held: bool,
    running: bool,
}

fn agents_in(sessions: &[Value], projects: &[i64]) -> Agents {
    let mine = sessions.iter().filter(|s| {
        s["project_id"].as_i64().is_some_and(|id| projects.contains(&id)) && text(s, "state") != "closed"
    });
    let mut agents = Agents { open: 0, held: false, running: false };
    for s in mine {
        agents.open += 1;
        match text(s, "state") {
            "blocked" => agents.held = true,
            "running" | "spawning" => agents.running = true,
            _ => {}
        }
    }
    agents
}

fn lamp(agents: &Agents) -> gtk::Box {
    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    lamp.add_css_class("lamp");
    lamp.add_css_class("small-lamp");
    lamp.set_valign(gtk::Align::Center);
    if agents.held {
        lamp.add_css_class("held");
    } else if agents.running {
        lamp.add_css_class("live");
    }
    lamp
}

fn sorted_children<'a>(projects: &'a [Value], workspace: &Value) -> Vec<&'a Value> {
    let mut children: Vec<_> = projects.iter().filter(|p| p["workspace_id"] == workspace["id"]).collect();
    children.sort_by_key(|p| {
        (!p["pinned"].as_bool().unwrap_or(false), p["order"].as_i64().unwrap_or(0), id_of(p))
    });
    children
}

/// A quiet full-width row in the tree: "Show N more", "Show fewer", "Add a project…".
fn more_key(caption: &str) -> gtk::Button {
    let key = button(caption, "registry-more");
    if let Some(l) = key.child().and_downcast::<gtk::Label>() {
        l.set_xalign(0.0);
    }
    key
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Ui {
    pub(crate) fn render_projects(self: &Rc<Self>) {
        if !self.sidebar_ready() {
            return;
        }
        let filter = self.registry_filter();
        let projects = self.projects.borrow().clone();
        let workspaces = self.workspaces.borrow().clone();
        let query = filter.text().trim().to_lowercase();
        filter.set_visible(projects.len() > FILTER_AT || !query.is_empty());
        let active = self.project.get();
        // Switching into a folded group unfolds it; the first project seen at startup does not,
        // so a group folded last session stays folded.
        let previous = sidebar(|s| std::mem::replace(&mut s.active, active));
        if previous != 0 && previous != active {
            let home = projects.iter().find(|p| id_of(p) == active).and_then(|p| p["workspace_id"].as_i64());
            if home.is_some_and(|ws| sidebar(|s| s.collapsed.remove(&ws))) {
                self.save_collapsed();
            }
        }
        let (collapsed, expanded) = sidebar(|s| (s.collapsed.clone(), s.expanded.clone()));
        let sessions = self.sidebar_sessions.borrow().clone();
        // Only registry, lamp, folding or filter changes rebuild the tree: an agent going
        // idle in a project that still has one running leaves its lamp, and the tree, as is.
        let signature = {
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (active, &collapsed, &expanded, &query).hash(&mut hash);
            serde_json::to_string(&projects).unwrap_or_default().hash(&mut hash);
            serde_json::to_string(&workspaces).unwrap_or_default().hash(&mut hash);
            for project in &projects {
                let agents = agents_in(&sessions, &[id_of(project)]);
                (agents.open, agents.held, agents.running).hash(&mut hash);
            }
            hash.finish()
        };
        if sidebar(|s| s.rendered.replace(signature)) == Some(signature) {
            return;
        }
        clear(&self.projects_box);
        let mut first_match = None;
        for workspace in &workspaces {
            let ws_id = id_of(workspace);
            let mut children = sorted_children(&projects, workspace);
            let filtering = !query.is_empty();
            if filtering && !text(workspace, "name").to_lowercase().contains(&query) {
                children.retain(|p| {
                    text(p, "name").to_lowercase().contains(&query) || text(p, "path").to_lowercase().contains(&query)
                });
                if children.is_empty() {
                    continue;
                }
            }
            let folded = !filtering && collapsed.contains(&ws_id);
            let ids: Vec<i64> = children.iter().map(|p| id_of(p)).collect();
            self.projects_box.append(&self.workspace_header(workspace, &ids, folded, &sessions, active));
            if folded {
                continue;
            }
            if children.is_empty() {
                let add = more_key("Add a project…");
                add.set_focus_on_click(false);
                let weak = Rc::downgrade(self);
                let target = workspace.clone();
                add.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        ui.open_repository_in(Some(target.clone()));
                    }
                });
                self.projects_box.append(&add);
                continue;
            }
            let everything = filtering || expanded.contains(&ws_id) || children.len() <= GROUP_CAP + 1;
            let mut hidden = 0;
            for (i, project) in children.iter().enumerate() {
                let id = id_of(project);
                let agents = agents_in(&sessions, &[id]);
                // The active project and anything waiting on the user never fold away.
                if everything || i < GROUP_CAP || id == active || agents.held {
                    if filtering && first_match.is_none() {
                        first_match = Some(id);
                    }
                    self.projects_box.append(&self.project_row(project, &agents, active));
                } else {
                    hidden += 1;
                }
            }
            let more = if hidden > 0 {
                Some(format!("Show {hidden} more"))
            } else if !filtering && expanded.contains(&ws_id) && children.len() > GROUP_CAP + 1 {
                Some("Show fewer".to_string())
            } else {
                None
            };
            if let Some(caption) = more {
                let key = more_key(&caption);
                key.set_focus_on_click(false);
                let weak = Rc::downgrade(self);
                key.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        sidebar(|s| if !s.expanded.remove(&ws_id) {
                            s.expanded.insert(ws_id);
                        });
                        ui.render_projects();
                    }
                });
                self.projects_box.append(&key);
            }
        }
        sidebar(|s| s.first_match = first_match);
        if !query.is_empty() && first_match.is_none() {
            let none = label(&format!("No projects match “{}”", filter.text().trim()), "registry-empty");
            none.set_wrap(true);
            self.projects_box.append(&none);
        }
    }

    /// Load the folded groups once before the first render, so a folded sidebar never opens
    /// expanded and then snaps shut.
    fn sidebar_ready(self: &Rc<Self>) -> bool {
        let (loaded, loading) = sidebar(|s| (s.loaded, s.loading));
        if loaded {
            return true;
        }
        if !loading {
            sidebar(|s| s.loading = true);
            let ui = self.clone();
            glib::spawn_future_local(async move {
                let read = ui.call("settings.get", json!({"path": COLLAPSED_KEY})).await;
                let persist = read.is_ok();
                let ids: BTreeSet<i64> = read.ok()
                    .and_then(|v| v["value"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()))
                    .unwrap_or_default();
                sidebar(|s| {
                    s.collapsed = ids;
                    s.persist = persist;
                    s.loaded = true;
                    s.loading = false;
                });
                sidebar(|s| s.rendered = None);
                ui.render_projects();
            });
        }
        false
    }

    fn save_collapsed(self: &Rc<Self>) {
        let known: BTreeSet<i64> = self.workspaces.borrow().iter().map(id_of).collect();
        let Some(ids): Option<Vec<i64>> = sidebar(|s| {
            s.collapsed.retain(|id| known.contains(id));
            s.persist.then(|| s.collapsed.iter().copied().collect())
        }) else {
            return;
        };
        let ui = self.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("settings.set", json!({"path": COLLAPSED_KEY, "value": ids})).await {
                ui.show_error(&format!("Could not remember folded workspaces: {e}"));
            }
        });
    }

    /// The filter lives outside the rebuilt tree, between the heading and its scroller, so a
    /// rebuild never takes its focus.
    fn registry_filter(self: &Rc<Self>) -> gtk::SearchEntry {
        if let Some(entry) = sidebar(|s| s.filter.clone()) {
            return entry;
        }
        let entry = gtk::SearchEntry::new();
        entry.set_placeholder_text(Some("Filter projects"));
        entry.add_css_class("registry-filter");
        entry.set_widget_name("registry-filter");
        entry.set_visible(false);
        entry.update_property(&[gtk::accessible::Property::Label("Filter projects")]);
        let weak = Rc::downgrade(self);
        entry.connect_search_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.render_projects();
            }
        });
        let weak = Rc::downgrade(self);
        entry.connect_activate(move |entry| {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(id) = sidebar(|s| s.first_match) {
                entry.set_text("");
                ui.switch_project(id);
            }
        });
        entry.connect_stop_search(|entry| entry.set_text(""));
        if let Some(scroller) = self.projects_box.ancestor(gtk::ScrolledWindow::static_type()) {
            if let Some(parent) = scroller.parent().and_downcast::<gtk::Box>() {
                parent.insert_child_after(&entry, scroller.prev_sibling().as_ref());
            }
        }
        sidebar(|s| s.filter = Some(entry.clone()));
        entry
    }

    fn workspace_header(self: &Rc<Self>, workspace: &Value, projects: &[i64], folded: bool, sessions: &[Value], active: i64) -> gtk::Box {
        let ws_id = id_of(workspace);
        let name = text(workspace, "name");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.add_css_class("workspace-row");
        if folded {
            row.add_css_class("collapsed");
            if projects.contains(&active) {
                row.add_css_class("has-active");
            }
        }
        self.registry_drag(&row, workspace, true);
        let toggle = button("", "workspace-toggle");
        toggle.set_hexpand(true);
        toggle.set_focus_on_click(false);
        toggle.set_tooltip_text(Some(text(workspace, "path")));
        toggle.update_property(&[gtk::accessible::Property::Label(&format!(
            "{} workspace {name}", if folded { "Expand" } else { "Collapse" }
        ))]);
        toggle.update_state(&[gtk::accessible::State::Expanded(Some(!folded))]);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let chevron = crate::icons::image(if folded { "chevron-right" } else { "chevron-down" }, 12);
        chevron.add_css_class("workspace-chevron");
        line.append(&chevron);
        let caption = label(name, "workspace-name");
        caption.set_hexpand(true);
        caption.set_width_chars(1);
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        line.append(&caption);
        if folded {
            // A folded group still reports what is inside it: its size, and whether an agent
            // in it is working or waiting.
            let agents = agents_in(sessions, projects);
            if agents.held || agents.running {
                line.append(&lamp(&agents));
            }
            line.append(&label(&projects.len().to_string(), "workspace-count"));
        }
        toggle.set_child(Some(&line));
        let weak = Rc::downgrade(self);
        toggle.connect_clicked(move |_| {
            // While filtering every group is open; folding then would change nothing visible.
            if sidebar(|s| s.filter.as_ref().is_some_and(|f| !f.text().is_empty())) {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                sidebar(|s| if !s.collapsed.remove(&ws_id) {
                    s.collapsed.insert(ws_id);
                });
                ui.save_collapsed();
                ui.render_projects();
            }
        });
        row.append(&toggle);
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tools.add_css_class("workspace-tools");
        let add = icon_button("plus", "Add project to this workspace");
        add.add_css_class("small-key");
        add.add_css_class("add-project");
        add.set_focus_on_click(false);
        add.set_child(Some(&crate::icons::image("plus", 12)));
        add.set_widget_name(&format!("workspace-add-{ws_id}"));
        let weak = Rc::downgrade(self);
        let target = workspace.clone();
        add.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.open_repository_in(Some(target.clone()));
            }
        });
        tools.append(&add);
        let manage = self.registry_menu(workspace, true);
        manage.add_css_class("small-key");
        manage.add_css_class("manage");
        tools.append(&manage);
        row.append(&tools);
        row
    }

    fn project_row(self: &Rc<Self>, project: &Value, agents: &Agents, active: i64) -> gtk::Overlay {
        let id = id_of(project);
        let name = text(project, "name");
        let pinned = project["pinned"] == true;
        let row = gtk::Overlay::new();
        row.add_css_class("project-row");
        self.registry_drag(&row, project, false);
        let b = button("", "project");
        b.set_hexpand(true);
        b.set_focus_on_click(false);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.add_css_class("project-content");
        content.append(&lamp(agents));
        let title = label(name, "project-name");
        title.set_hexpand(true);
        title.set_width_chars(1);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        content.append(&title);
        if pinned {
            let mark = crate::icons::image("pin", 10);
            mark.add_css_class("project-pin-mark");
            content.append(&mark);
        }
        if agents.open > 0 {
            content.append(&label(&agents.open.to_string(), "project-count"));
        }
        b.set_child(Some(&content));
        let mut tip = format!("{name}\n{}\nNew agents branch from {}", text(project, "path"), text(project, "base_branch"));
        if agents.open > 0 {
            tip.push_str(&format!("\n{} open", plural(agents.open, "agent", "agents")));
        }
        // A repository that moved reads as gone here too, not only in its ⋯ menu (Relink…).
        if !std::path::Path::new(text(project, "path")).exists() {
            title.add_css_class("missing");
            tip.push_str("\nRepository not found at this path. Relink it from the ⋯ menu.");
        }
        b.set_tooltip_text(Some(&tip));
        if id == active {
            b.add_css_class("selected");
            row.add_css_class("selected");
        }
        if pinned {
            b.add_css_class("pinned");
        }
        let weak = Rc::downgrade(self);
        b.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.switch_project(id);
            }
        });
        row.set_child(Some(&b));
        let tools = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tools.add_css_class("project-tools");
        tools.set_halign(gtk::Align::End);
        tools.set_valign(gtk::Align::Center);
        let pin = icon_button("pin", if pinned { "Unpin project" } else { "Pin project" });
        pin.set_child(Some(&crate::icons::image("pin", 12)));
        pin.add_css_class("small-key");
        pin.add_css_class("manage");
        pin.set_focus_on_click(false);
        if pinned {
            pin.add_css_class("pinned");
        }
        let weak = Rc::downgrade(self);
        pin.connect_clicked(move |key| {
            if let Some(ui) = weak.upgrade() {
                ui.mutate("project.update", json!({"project_id": id, "pinned": !pinned}), key);
            }
        });
        tools.append(&pin);
        let menu = self.registry_menu(project, false);
        menu.add_css_class("small-key");
        menu.add_css_class("manage");
        tools.append(&menu);
        row.add_overlay(&tools);
        row
    }

    /// The ⋯ menu of a workspace or project row. Each entry is one line in `entries`; a
    /// destructive one goes below the separator.
    fn registry_menu(self: &Rc<Self>, value: &Value, workspace: bool) -> gtk::MenuButton {
        type Action = Rc<dyn Fn(&Rc<Ui>)>;
        let menu = gtk::MenuButton::new();
        menu.set_child(Some(&crate::icons::image("more", 13)));
        menu.set_focus_on_click(false);
        menu.set_tooltip_text(Some(if workspace { "Workspace menu" } else { "Project menu" }));
        let popover = gtk::Popover::new();
        popover.add_css_class("registry-menu");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.set_size_request(230, -1);
        let title = label(text(value, "name"), "title");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        body.append(&title);
        let path = label(text(value, "path"), "registry-menu-path");
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_max_width_chars(32);
        path.set_tooltip_text(Some(text(value, "path")));
        body.append(&path);
        let id = id_of(value);
        let v = value.clone();
        let mut entries: Vec<(&str, &str, bool, Action)> = if workspace {
            let (add, edit, remove) = (v.clone(), v.clone(), v.clone());
            vec![
                ("plus", "Add project…", false, Rc::new(move |ui: &Rc<Ui>| ui.open_repository_in(Some(add.clone())))),
                ("settings", "Workspace settings…", false, Rc::new(move |ui: &Rc<Ui>| ui.registry_editor(edit.clone(), true))),
                ("shield", "Guardrails…", false, Rc::new(move |ui: &Rc<Ui>| crate::pages::open_workspace_guardrails(ui, id))),
                ("trash", "Remove workspace…", true, Rc::new(move |ui: &Rc<Ui>| ui.confirm_registry_remove(&remove, true, None))),
            ]
        } else {
            let (edit, remove) = (v.clone(), v.clone());
            vec![
                ("terminal", "Open agents", false, Rc::new(move |ui: &Rc<Ui>| ui.open_project(id, "agents"))),
                ("files", "Files and Git", false, Rc::new(move |ui: &Rc<Ui>| ui.open_project(id, "code"))),
                ("settings", "Project settings…", false, Rc::new(move |ui: &Rc<Ui>| ui.registry_editor(edit.clone(), false))),
                ("shield", "Guardrails…", false, Rc::new(move |ui: &Rc<Ui>| crate::pages::open_project_guardrails(ui, id))),
                ("trash", "Remove project…", true, Rc::new(move |ui: &Rc<Ui>| ui.confirm_registry_remove(&remove, false, None))),
            ]
        };
        // A repository that moved: point the project at its new place instead of removing it.
        if !workspace && !std::path::Path::new(text(value, "path")).exists() {
            entries.push(("folder", "Relink…", false, Rc::new(move |ui: &Rc<Ui>| ui.relink_project(id))));
        }
        entries.sort_by_key(|entry| entry.2);
        let mut separated = false;
        for (icon, caption, danger, action) in entries {
            if !separated && danger {
                body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                separated = true;
            } else if body.last_child().is_some_and(|w| w.is::<gtk::Label>()) {
                body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            }
            let key = button("", "registry-action");
            if danger {
                key.add_css_class("danger");
            }
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 9);
            line.append(&crate::icons::image(icon, 15));
            line.append(&label(caption, ""));
            key.set_child(Some(&line));
            body.append(&key);
            let weak = Rc::downgrade(self);
            // Weak: the key lives inside the popover, so a strong capture is a cycle that keeps
            // every rebuilt row's menu alive.
            let pop = popover.downgrade();
            key.connect_clicked(move |_| {
                if let Some(pop) = pop.upgrade() {
                    pop.popdown();
                }
                if let Some(ui) = weak.upgrade() {
                    action(&ui);
                }
            });
        }
        popover.set_child(Some(&body));
        menu.set_popover(Some(&popover));
        menu
    }

    /// One confirmation removes a project or a workspace with everything Relay runs for it:
    /// the engine closes the agents (`force`), so nobody has to close them one by one first.
    fn confirm_registry_remove(self: &Rc<Self>, value: &Value, workspace: bool, removed: Option<Rc<dyn Fn()>>) {
        let id = id_of(value);
        let name = text(value, "name").to_string();
        let projects: Vec<i64> = if workspace {
            self.projects.borrow().iter().filter(|p| p["workspace_id"].as_i64() == Some(id)).map(id_of).collect()
        } else {
            vec![id]
        };
        let sessions = self.sidebar_sessions.borrow().clone();
        let agents: Vec<String> = sessions.iter()
            .filter(|s| s["project_id"].as_i64().is_some_and(|p| projects.contains(&p)) && text(s, "state") != "closed")
            .map(|s| text(s, "name").to_string())
            .collect();
        let kind = if workspace { "workspace" } else { "project" };
        let dialog = crate::panel::Panel::new(self, &format!("Remove {kind}"), 480);
        dialog.add_css_class("registry-confirm");
        let body = dialog.body.clone();
        body.set_spacing(12);
        let heading = label(&format!("Remove “{name}” from Relay?"), "registry-confirm-title");
        heading.set_wrap(true);
        body.append(&heading);
        let facts = gtk::Box::new(gtk::Orientation::Vertical, 6);
        if workspace {
            facts.append(&paragraph(&if projects.is_empty() {
                "It has no projects.".to_string()
            } else {
                format!("Its {} removed from Relay with it.", plural(projects.len(), "project is", "projects are"))
            }));
        }
        if agents.is_empty() {
            facts.append(&paragraph(if workspace { "No agents are open in it." } else { "It has no open agents." }));
        } else {
            facts.append(&paragraph(&format!(
                "{} will be stopped and closed.",
                plural(agents.len(), "agent", "agents")
            )));
            let mut names = agents.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
            if agents.len() > 4 {
                names.push_str(&format!(", +{} more", agents.len() - 4));
            }
            let list = label(&names, "registry-confirm-agents");
            list.set_wrap(true);
            list.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            facts.append(&list);
        }
        let keep = if agents.is_empty() {
            "Repository files are never touched."
        } else {
            "Their worktrees and branches stay on disk. Repository files are never touched."
        };
        // What goes for good, counted by the engine once the dialog is up.
        if !projects.is_empty() {
            let content = paragraph(if workspace { "Their tasks, notes and modules are deleted from Relay." } else { "Its tasks, notes and modules are deleted from Relay." });
            facts.append(&content);
            let weak = Rc::downgrade(self);
            let payload = if workspace { json!({"workspace_id": id}) } else { json!({"project_id": id}) };
            let content = content.downgrade();
            glib::spawn_future_local(async move {
                let Some(ui) = weak.upgrade() else { return };
                let Ok(counts) = ui.call("project.remove.preview", payload).await else { return };
                let Some(content) = content.upgrade() else { return };
                let n = |key: &str| counts[key].as_u64().unwrap_or(0) as usize;
                let (tasks, notes, modules) = (n("tasks"), n("notes"), n("modules"));
                content.set_text(&if tasks + notes + modules == 0 {
                    format!("{} no tasks, notes or modules.", if workspace { "They have" } else { "It has" })
                } else {
                    format!("{} {}, {} and {} are deleted from Relay.", if workspace { "Their" } else { "Its" },
                        plural(tasks, "task", "tasks"), plural(notes, "note", "notes"), plural(modules, "module", "modules"))
                });
            });
            facts.append(&paragraph("Relay backs up its store first, so they can be restored from that backup."));
        }
        let disk = paragraph(keep);
        facts.append(&disk);
        body.append(&facts);
        let worktrees = gtk::CheckButton::with_label("Also delete agent worktrees (Relay pool only; branches kept)");
        worktrees.add_css_class("registry-confirm-option");
        if !agents.is_empty() {
            body.append(&worktrees);
            let disk = disk.clone();
            worktrees.connect_toggled(move |check| {
                disk.set_text(if check.is_active() {
                    "Their Relay-pool worktrees are deleted; branches stay. Repository files are never touched."
                } else {
                    keep
                });
            });
        }
        let error = label("", "registry-error");
        error.set_wrap(true);
        error.set_selectable(true);
        error.set_visible(false);
        body.append(&error);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        actions.add_css_class("registry-confirm-actions");
        let cancel = button("Cancel", "quiet");
        let caption = format!("Remove {kind}");
        let accept = button(&caption, "danger");
        accept.set_widget_name("registry-remove-accept");
        actions.append(&cancel);
        actions.append(&accept);
        body.append(&actions);
        let weak = Rc::downgrade(&dialog);
        cancel.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.close();
            }
        });
        // Hold the dialog open while the engine closes the agents, so a stray click on the
        // scrim cannot hide the outcome.
        let busy = Rc::new(Cell::new(false));
        {
            let busy = busy.clone();
            dialog.set_guard(move || !busy.get());
        }
        // A worktree it would delete holds uncommitted work: the engine refuses `worktree.dirty`
        // and says which, and the next click deletes them anyway (RA-405). Keeping the
        // worktrees after all takes the question back.
        const DISCARD: &str = "Discard changes and remove";
        let discard = Rc::new(Cell::new(false));
        {
            let (discard, accept, caption, error) = (discard.clone(), accept.clone(), caption.clone(), error.clone());
            worktrees.connect_toggled(move |check| {
                if !check.is_active() && discard.replace(false) {
                    accept.set_label(&caption);
                    error.set_visible(false);
                }
            });
        }
        let weak = Rc::downgrade(self);
        let panel = Rc::downgrade(&dialog);
        let cancel_key = cancel.clone();
        accept.connect_clicked(move |key| {
            let cancel = &cancel_key;
            let (Some(ui), Some(panel)) = (weak.upgrade(), panel.upgrade()) else { return };
            let (op, mut payload) = if workspace {
                ("workspace.remove", json!({"workspace_id": id}))
            } else {
                ("project.remove", json!({"project_id": id}))
            };
            payload["force"] = json!(true);
            payload["remove_worktrees"] = json!(worktrees.is_active());
            payload["discard_changes"] = json!(discard.get() && worktrees.is_active());
            busy.set(true);
            key.set_sensitive(false);
            cancel.set_sensitive(false);
            key.set_label("Removing…");
            error.set_visible(false);
            let (key, cancel, error, caption) = (key.clone(), cancel.clone(), error.clone(), caption.clone());
            let (projects, removed, busy, discard) = (projects.clone(), removed.clone(), busy.clone(), discard.clone());
            glib::spawn_future_local(async move {
                let result = ui.call(op, payload).await;
                busy.set(false);
                match result {
                    Ok(_) => {
                        panel.close();
                        if let Some(done) = removed {
                            done();
                        }
                        ui.registry_removed(&projects, workspace.then_some(id));
                    }
                    Err(crate::client::Error::Bus(e)) if e.code == "worktree.dirty" => {
                        discard.set(true);
                        error.set_text(&format!(
                            "{}. Nothing was removed. Commit or stash those changes to keep them, or remove anyway to delete them for good.",
                            e.message
                        ));
                        error.set_visible(true);
                        key.set_label(DISCARD);
                        key.set_sensitive(true);
                        cancel.set_sensitive(true);
                    }
                    Err(e) => {
                        error.set_text(&e.to_string());
                        error.set_visible(true);
                        key.set_label(if discard.get() { DISCARD } else { &caption });
                        key.set_sensitive(true);
                        cancel.set_sensitive(true);
                    }
                }
            });
        });
        dialog.centered(420);
        dialog.present();
        cancel.grab_focus();
    }

    /// Every project's id in sidebar order.
    pub(crate) fn project_order(&self) -> Vec<i64> {
        let all = self.projects.borrow();
        self.workspaces.borrow().iter().flat_map(|w| sorted_children(&all, w)).map(id_of).collect()
    }

    /// Move the wall off the active project, which `self.projects` no longer lists: to its
    /// nearest remaining neighbour in `order` (the sidebar order from before it went), else
    /// the first project left, else none. A removal made here and one a refresh finds both
    /// come through here. False when unsaved work keeps the wall where it is.
    pub(crate) fn leave_removed_project(self: &Rc<Self>, order: &[i64]) -> bool {
        let active = self.project.get();
        if self.editor.is_dirty() {
            self.show_error("The selected project was removed. Save or copy your editor changes before selecting another project.");
            return false;
        }
        let live: Vec<i64> = self.projects.borrow().iter().map(id_of).collect();
        let at = order.iter().position(|&id| id == active).unwrap_or(0);
        let next = order[at..].iter().chain(order[..at].iter().rev()).copied().find(|id| live.contains(id));
        let page = self.page.borrow().clone();
        self.open_project(next.or_else(|| live.first().copied()).unwrap_or(0), &page);
        self.project.get() != active
    }

    /// Ask where the project's repository is now and relink it there (`project.relink`).
    fn relink_project(self: &Rc<Self>, id: i64) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Some(ui) = weak.upgrade() else { return };
            let dialog = gtk::FileDialog::builder().title("Where is this repository now?").build();
            let Ok(file) = dialog.select_folder_future(Some(&ui.window)).await else { return };
            let Some(path) = file.path() else { return };
            match ui.call("project.relink", json!({"project_id": id, "path": path})).await {
                Ok(_) => {
                    ui.registry_dirty.set(true);
                    ui.refresh();
                }
                Err(e) => ui.show_error(&e.to_string()),
            }
        });
    }

    /// Forget removed rows at once and move the wall off a removed project, to its nearest
    /// remaining neighbour in sidebar order.
    fn registry_removed(self: &Rc<Self>, projects: &[i64], workspace: Option<i64>) {
        let order = self.project_order();
        self.projects.borrow_mut().retain(|p| !projects.contains(&id_of(p)));
        if let Some(ws) = workspace {
            self.workspaces.borrow_mut().retain(|w| id_of(w) != ws);
            sidebar(|s| {
                s.collapsed.remove(&ws);
                s.expanded.remove(&ws);
            });
            self.save_collapsed();
        }
        self.registry_dirty.set(true);
        if projects.contains(&self.project.get()) {
            self.leave_removed_project(&order);
        }
        sidebar(|s| s.rendered = None);
        self.refresh();
    }

    /// Project or workspace settings: grouped sections, inline validation, and Save only
    /// when something changed and everything is valid.
    pub(crate) fn registry_editor(self: &Rc<Self>, value: Value, workspace: bool) {
        let title = if workspace { "Workspace settings" } else { "Project settings" };
        let Some((panel, body)) = self.sheet(title, 460) else {
            return;
        };
        panel.add_css_class("registry-editor");
        body.set_spacing(14);
        let id = id_of(&value);
        let projects: Vec<i64> = if workspace {
            self.projects.borrow().iter().filter(|p| p["workspace_id"].as_i64() == Some(id)).map(id_of).collect()
        } else {
            vec![id]
        };
        let agents = agents_in(&self.sidebar_sessions.borrow(), &projects).open;

        // Identity: what this is and where it lives.
        let head = gtk::Box::new(gtk::Orientation::Vertical, 4);
        head.add_css_class("registry-editor-head");
        let heading = label(text(&value, "name"), "registry-editor-name");
        heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&heading);
        let where_ = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let path = label(text(&value, "path"), "registry-path");
        path.set_selectable(true);
        path.set_hexpand(true);
        path.set_width_chars(1);
        path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path.set_tooltip_text(Some(text(&value, "path")));
        where_.append(&path);
        let copy = icon_button("copy", "Copy path");
        copy.add_css_class("small-key");
        copy.set_child(Some(&crate::icons::image("copy", 12)));
        let p = text(&value, "path").to_string();
        copy.connect_clicked(move |key| key.clipboard().set_text(&p));
        where_.append(&copy);
        head.append(&where_);
        let mut meta = vec![if agents == 0 { "No open agents".to_string() } else { format!("{} open", plural(agents, "agent", "agents")) }];
        if workspace {
            meta.insert(0, plural(projects.len(), "project", "projects"));
        } else if let Some(ws) = self.workspaces.borrow().iter().find(|w| w["id"] == value["workspace_id"]) {
            meta.push(format!("in {}", text(ws, "name")));
        }
        head.append(&label(&meta.join(" · "), "registry-meta"));
        body.append(&head);

        let general = section(&body, "General");
        let name = gtk::Entry::builder().text(text(&value, "name")).hexpand(true).build();
        let name_error = labeled(&general, "Name", &name, None);
        let pinned = gtk::Switch::new();
        let branch = gtk::Entry::builder().text(text(&value, "base_branch")).hexpand(true).build();
        let build = gtk::Entry::builder().text(text(&value, "build_cmd")).hexpand(true).placeholder_text("None").build();
        let run = gtk::Entry::builder().text(text(&value, "run_cmd")).hexpand(true).placeholder_text("Gradle wrapper install task").build();
        let mut branch_error = label("", "registry-field-error");
        if !workspace {
            pinned.set_active(value["pinned"] == true);
            pinned.set_valign(gtk::Align::Center);
            pinned.update_property(&[gtk::accessible::Property::Label("Pin to the top of its workspace")]);
            let pin_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            pin_row.add_css_class("registry-switch-row");
            let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
            copy.set_hexpand(true);
            copy.append(&label("Pin to the top", "registry-field-label"));
            copy.append(&paragraph("Pinned projects sort first in their workspace."));
            pin_row.append(&copy);
            pin_row.append(&pinned);
            general.append(&pin_row);

            let branches = section(&body, "Branches");
            branch.add_css_class("registry-mono");
            branch_error = labeled(&branches, "Base branch", &branch,
                Some("New agents branch from this base. Existing agent branches stay where they are."));

            let commands = section(&body, "Commands");
            build.add_css_class("registry-mono");
            run.add_css_class("registry-mono");
            labeled(&commands, "Build command", &build,
                Some("Runs in the merged checkout when an integration builds. Empty means no build step."));
            labeled(&commands, "Run command", &run,
                Some("Installs the build on a device for runs and deploys. Empty uses the Gradle wrapper's install task."));
        }

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.add_css_class("registry-editor-actions");
        let status = label("", "registry-status");
        status.set_hexpand(true);
        status.set_wrap(true);
        let cancel = button("Cancel", "quiet");
        let save = button("Save changes", "primary");
        save.set_widget_name("registry-save");
        save.set_sensitive(false);
        actions.append(&status);
        actions.append(&cancel);
        actions.append(&save);
        body.append(&actions);

        // Danger zone: one confirmation, which closes the agents itself.
        let danger = section(&body, "Danger zone");
        danger.add_css_class("registry-danger");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
        copy.set_hexpand(true);
        copy.append(&label(if workspace { "Remove workspace" } else { "Remove project" }, "registry-field-label"));
        let what = if workspace { "the workspace and its projects" } else { "the project" };
        copy.append(&paragraph(&if agents == 0 {
            format!("Forgets {what}. Repository files and branches stay on disk.")
        } else {
            format!("Stops and closes {}, then forgets {what}. Repository files and branches stay on disk.", plural(agents, "agent", "agents"))
        }));
        row.append(&copy);
        let remove = button("Remove…", "danger");
        remove.set_valign(gtk::Align::Center);
        remove.set_widget_name("registry-remove");
        row.append(&remove);
        danger.append(&row);

        // Trimmed as the form compares them: a value stored with spaces around it (from the CLI)
        // must not read as an unsaved change that keeps the sheet from closing.
        let original = (
            text(&value, "name").trim().to_string(),
            text(&value, "base_branch").to_string(),
            text(&value, "build_cmd").trim().to_string(),
            text(&value, "run_cmd").trim().to_string(),
            value["pinned"] == true,
        );
        let original = Rc::new(original);
        let discard = Rc::new(Cell::new(false));
        // Validate on every edit: errors sit under their field, Save waits for a valid change.
        let check: Rc<dyn Fn() -> bool> = {
            // Weak: the inputs' own handlers hold this closure, and Save's handler holds the
            // inputs, so a strong Save here would keep the whole form alive after close.
            let (name, branch, build, run, pinned) = (name.downgrade(), branch.downgrade(), build.downgrade(), run.downgrade(), pinned.downgrade());
            let (name_error, branch_error, status, save, original) = (name_error.clone(), branch_error.clone(), status.clone(), save.downgrade(), original.clone());
            Rc::new(move || {
                let (Some(name), Some(branch), Some(build), Some(run), Some(pinned)) =
                    (name.upgrade(), branch.upgrade(), build.upgrade(), run.upgrade(), pinned.upgrade())
                else {
                    return false;
                };
                let show = |entry: &gtk::Entry, error: &gtk::Label, message: Option<&str>| {
                    error.set_text(message.unwrap_or(""));
                    error.set_visible(message.is_some());
                    if message.is_some() {
                        entry.add_css_class("invalid");
                    } else {
                        entry.remove_css_class("invalid");
                    }
                    message.is_none()
                };
                let n = name.text();
                let mut valid = show(&name, &name_error, n.trim().is_empty().then_some("Enter a name."));
                let mut dirty = n.trim() != original.0;
                if !workspace {
                    let b = branch.text();
                    let problem = if b.trim().is_empty() {
                        Some("Enter a base branch.")
                    } else if b.as_str() != b.trim() {
                        Some("Remove the spaces around the branch name.")
                    } else if b.chars().any(char::is_whitespace) {
                        Some("Branch names cannot contain spaces.")
                    } else {
                        None
                    };
                    valid &= show(&branch, &branch_error, problem);
                    dirty |= b.as_str() != original.1
                        || build.text().trim() != original.2
                        || run.text().trim() != original.3
                        || pinned.is_active() != original.4;
                }
                status.remove_css_class("registry-status-warn");
                status.set_text(if !valid { "Fix the highlighted field to save." } else if dirty { "Unsaved changes" } else { "" });
                if let Some(save) = save.upgrade() {
                    save.set_sensitive(valid && dirty);
                }
                dirty
            })
        };
        for entry in [&name, &branch, &build, &run] {
            let check = check.clone();
            entry.connect_changed(move |_| {
                check();
            });
            let save = save.downgrade();
            entry.connect_activate(move |_| {
                if let Some(save) = save.upgrade().filter(|save| save.is_sensitive()) {
                    save.emit_clicked();
                }
            });
        }
        {
            let check = check.clone();
            pinned.connect_active_notify(move |_| {
                check();
            });
        }
        {
            let (check, discard, status) = (check.clone(), discard.clone(), status.clone());
            panel.set_guard(move || {
                if discard.get() || !check() {
                    return true;
                }
                status.set_text("Save or cancel your changes first.");
                status.add_css_class("registry-status-warn");
                false
            });
        }
        {
            let (panel, discard) = (Rc::downgrade(&panel), discard.clone());
            cancel.connect_clicked(move |_| {
                discard.set(true);
                if let Some(panel) = panel.upgrade() {
                    panel.close();
                }
            });
        }
        {
            let weak = Rc::downgrade(self);
            let (panel, discard, status) = (Rc::downgrade(&panel), discard.clone(), status.clone());
            let (name, branch, build, run, pinned, original) = (name.clone(), branch.clone(), build.clone(), run.clone(), pinned.clone(), original.clone());
            save.connect_clicked(move |key| {
                let Some(ui) = weak.upgrade() else { return };
                let n = name.text().trim().to_string();
                let (op, payload) = if workspace {
                    ("workspace.update", json!({"workspace_id": id, "name": n}))
                } else {
                    let mut payload = json!({"project_id": id});
                    if n != original.0 {
                        payload["name"] = json!(n);
                    }
                    if branch.text().as_str() != original.1 {
                        payload["base_branch"] = json!(branch.text().as_str());
                    }
                    // An empty command clears it (null); "" would be stored and run as one.
                    for (key, entry, before) in [("build_cmd", &build, &original.2), ("run_cmd", &run, &original.3)] {
                        let now = entry.text().trim().to_string();
                        if &now != before {
                            payload[key] = if now.is_empty() { Value::Null } else { json!(now) };
                        }
                    }
                    if pinned.is_active() != original.4 {
                        payload["pinned"] = json!(pinned.is_active());
                    }
                    ("project.update", payload)
                };
                key.set_sensitive(false);
                status.set_text("Saving…");
                let (panel, discard, status, key) = (panel.clone(), discard.clone(), status.clone(), key.clone());
                glib::spawn_future_local(async move {
                    match ui.call(op, payload).await {
                        Ok(_) => {
                            discard.set(true);
                            ui.registry_dirty.set(true);
                            ui.refresh();
                            if let Some(panel) = panel.upgrade() {
                                panel.close();
                            }
                        }
                        Err(e) => {
                            status.set_text(&e.to_string());
                            status.add_css_class("registry-status-warn");
                            key.set_sensitive(true);
                        }
                    }
                });
            });
        }
        {
            let weak = Rc::downgrade(self);
            let close = Rc::downgrade(&panel);
            remove.connect_clicked(move |_| {
                let Some(ui) = weak.upgrade() else { return };
                let (close, discard) = (close.clone(), discard.clone());
                let done: Rc<dyn Fn()> = Rc::new(move || {
                    discard.set(true);
                    if let Some(panel) = close.upgrade() {
                        panel.close();
                    }
                });
                ui.confirm_registry_remove(&value, workspace, Some(done));
            });
        }
        panel.present();
        name.grab_focus();
    }
}

/// A grouped settings section with its heading.
fn section(parent: &gtk::Box, title: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    section.add_css_class("registry-section");
    section.append(&label(&title.to_uppercase(), "section-label"));
    parent.append(&section);
    section
}

/// A labelled field with an inline error slot and optional help; returns the error label.
fn labeled(parent: &gtk::Box, caption: &str, widget: &impl IsA<gtk::Widget>, help: Option<&str>) -> gtk::Label {
    let field = gtk::Box::new(gtk::Orientation::Vertical, 5);
    field.add_css_class("registry-field");
    field.append(&label(caption, "registry-field-label"));
    widget.as_ref().update_property(&[gtk::accessible::Property::Label(caption)]);
    field.append(widget);
    let error = label("", "registry-field-error");
    error.set_wrap(true);
    error.set_visible(false);
    field.append(&error);
    if let Some(help) = help {
        field.append(&paragraph(help));
    }
    parent.append(&field);
    error
}

fn paragraph(copy: &str) -> gtk::Label {
    let l = label(copy, "registry-help");
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l
}
