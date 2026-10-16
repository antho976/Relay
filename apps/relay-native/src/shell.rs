use super::*;
use vte4::prelude::*;
#[path = "shell/registry.rs"]
mod registry;
#[path = "agent_menu.rs"]
mod agent_menu;
pub(super) use agent_menu::is_closing;

thread_local! {
    /// The bell's notify.list is in flight; see refresh_notification_count.
    static BELL_LOADING: Cell<bool> = const { Cell::new(false) };
}

impl Ui {
    /// Open `project` on `page`: an explicit destination, which the project's saved layout
    /// does not override.
    pub fn open_project(self: &Rc<Self>, project: i64, page: &str) {
        self.enter_project(project, Some(page));
    }
    /// Switch to `project` on the page it was last left on.
    pub fn switch_project(self: &Rc<Self>, project: i64) {
        self.enter_project(project, None);
    }
    fn enter_project(self: &Rc<Self>, project: i64, page: Option<&str>) {
        if project != self.project.get() {
            if !self.dismiss_panels() {
                return;
            }
            if self.editor.is_dirty() {
                self.show_error("Save or discard editor changes before switching projects.");
                return;
            }
            if self.launch_busy.get() {
                self.show_error("Wait for agent creation to finish before switching projects.");
                return;
            }
            self.launch.set_reveal_child(false);
            self.project.set(project);
            // Switch the visible wall synchronously. The registry already contains
            // all projects' sessions; a slow refresh must never leave the old CLI here.
            *self.sessions.borrow_mut() = self.sidebar_sessions.borrow().iter()
                .filter(|session| session["project_id"].as_i64() == Some(project))
                .cloned().collect();
            self.ordered.borrow_mut().clear();
            self.reconcile();
            self.editor.reset();
            self.restored_project.set(0);
            self.explicit_page.set(if page.is_some() { project } else { 0 });
            self.refresh();
        }
        self.navigate(page.unwrap_or("agents"));
    }
    fn registry_drag(
        self: &Rc<Self>,
        widget: &impl IsA<gtk::Widget>,
        value: &Value,
        workspace: bool,
    ) {
        let id = value["id"].as_i64().unwrap_or(0);
        let kind = if workspace { "workspace" } else { "project" };
        widget.set_widget_name(&format!("registry-{kind}-{id}"));
        let token = format!("relay-{kind}:{id}");
        let source = gtk::DragSource::new();
        source.set_actions(gtk::gdk::DragAction::MOVE);
        source.connect_prepare(move |_, _, _| {
            Some(gtk::gdk::ContentProvider::for_value(&token.to_value()))
        });
        widget.add_controller(source);
        let drop = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        let target = value.clone();
        drop.connect_drop(move |drop, v, _, y| {
            let Ok(token) = v.get::<String>() else {
                return false;
            };
            let Some(from) = token
                .strip_prefix(&format!("relay-{kind}:"))
                .and_then(|s| s.parse::<i64>().ok())
            else {
                return false;
            };
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            if from == id {
                return false;
            }
            let mut items = if workspace {
                ui.workspaces.borrow().clone()
            } else {
                ui.projects
                    .borrow()
                    .iter()
                    .filter(|p| {
                        p["workspace_id"] == target["workspace_id"]
                            && p["pinned"] == target["pinned"]
                    })
                    .cloned()
                    .collect()
            };
            items.sort_by_key(|p| {
                (
                    p["order"].as_i64().unwrap_or(0),
                    p["id"].as_i64().unwrap_or(0),
                )
            });
            let Some(index) = items.iter().position(|p| p["id"] == from) else {
                ui.show_error("Reorder projects within the same workspace and pinned group.");
                return false;
            };
            let item = items.remove(index);
            let Some(index) = items.iter().position(|p| p["id"] == id) else {
                return false;
            };
            let after = drop
                .widget()
                .is_some_and(|w| y > f64::from(w.height()) / 2.0);
            items.insert(index + usize::from(after), item);
            ui.projects_box.set_sensitive(false);
            glib::spawn_future_local(async move {
                // One reorder call: all orders in one transaction, one undo entry, one event.
                // Only the items whose order changed (the moved range) are sent.
                let key = if workspace { "workspace_id" } else { "project_id" };
                let orders: Vec<Value> = items
                    .iter()
                    .enumerate()
                    .filter(|(order, item)| item["order"].as_i64() != Some(*order as i64))
                    .map(|(order, item)| json!({key:item["id"],"order":order}))
                    .collect();
                if !orders.is_empty() {
                    let op = if workspace { "workspace.reorder" } else { "project.reorder" };
                    if let Err(e) = ui.call(op, json!({"orders":orders})).await {
                        ui.show_error(&e.to_string());
                    }
                }
                ui.projects_box.set_sensitive(true);
                ui.registry_dirty.set(true);
                ui.refresh();
            });
            true
        });
        widget.add_controller(drop);
    }
    pub(super) fn sheet(
        &self,
        title: &str,
        width: i32,
    ) -> Option<(Rc<crate::panel::Panel>, gtk::Box)> {
        let panel = crate::panel::Panel::toggle(self, title, width)?;
        let body = panel.body.clone();
        Some((panel, body))
    }
    pub(super) fn layout(self: &Rc<Self>) {
        let started = std::time::Instant::now();
        let names: Vec<_> = self.ordered.borrow().iter().cloned().collect();
        if self
            .focused
            .borrow()
            .as_ref()
            .is_none_or(|n| !names.contains(n))
        {
            *self.focused.borrow_mut() = names.first().cloned();
        }
        let focus = self.focused.borrow().clone();
        let mode = self.mode.borrow().clone();
        self.wall_right.set_visible(
            names.len() > 1
                && (matches!(mode.as_str(), "review" | "mosaic")
                    || (mode == "grid" && self.columns.get() == 2)),
        );
        clear(&self.focus_tabs);
        self.focus_tabs
            .set_visible(mode == "focus" && names.len() > 1);
        let mut targets: Vec<(Rc<Pane>, gtk::Grid, i32, i32)> = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if mode == "focus" {
                let b = button("", "focus-tab");
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 7);
                let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                lamp.add_css_class("lamp");
                lamp.add_css_class("small-lamp");
                lamp.set_valign(gtk::Align::Center);
                if let Some(session) = self
                    .sessions
                    .borrow()
                    .iter()
                    .find(|s| text(s, "name") == name)
                {
                    tab_lamp(&lamp, text(session, "state"));
                }
                row.append(&lamp);
                row.append(&label(name, ""));
                b.set_child(Some(&row));
                b.set_widget_name(&format!("focus-tab-{name}"));
                if focus.as_ref() == Some(name) {
                    b.add_css_class("selected");
                }
                let weak = Rc::downgrade(self);
                let name = name.clone();
                b.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        *ui.focused.borrow_mut() = Some(name.clone());
                        ui.layout();
                        ui.save_layout();
                    }
                });
                self.focus_tabs.append(&b);
            }
            let Some(p) = self.panes.borrow().get(name).cloned() else {
                continue;
            };
            if mode == "focus" && focus.as_ref() != Some(name) {
                continue;
            }
            let (grid, column, row) = if matches!(mode.as_str(), "review" | "mosaic") && names.len() > 1 {
                if focus.as_ref() == Some(name) {
                    (&self.wall, 0, 0)
                } else {
                    let offset = names[..i]
                        .iter()
                        .filter(|n| Some(*n) != focus.as_ref())
                        .count() as i32;
                    let columns = if mode == "mosaic" { 2 } else { 1 };
                    (&self.wall_right, offset % columns, offset / columns)
                }
            } else if mode == "grid" && self.columns.get() == 2 && names.len() > 1 {
                let grid = if i % 2 == 0 {
                    &self.wall
                } else {
                    &self.wall_right
                };
                (grid, 0, (i / 2) as i32)
            } else {
                let cols = if mode == "focus" {
                    1
                } else {
                    self.columns.get()
                };
                (&self.wall, i as i32 % cols, if mode == "focus" { 0 } else { i as i32 / cols })
            };
            targets.push((p, grid.clone(), column, row));
        }
        // Incremental: a pane already in its grid moves in place. Re-parenting a VTE unrealizes
        // and re-realizes it, so detaching the whole wall on every open or close redrew every
        // terminal. Only panes that change grid, or leave the wall, are removed.
        for grid in [&self.wall, &self.wall_right] {
            let mut child = grid.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if !targets.iter().any(|(p, g, ..)| g == grid && p.root.upcast_ref::<gtk::Widget>() == &widget) {
                    grid.remove(&widget);
                }
            }
        }
        let mut moved = 0;
        for (p, grid, column, row) in targets {
            // Sizes can change without a move (a column appears or goes); the resize check
            // itself is a no-op when the grid did not change.
            p.schedule_resize();
            if p.root.parent().as_ref() == Some(grid.upcast_ref::<gtk::Widget>()) {
                let Some(cell) = grid
                    .layout_manager()
                    .and_then(|manager| manager.layout_child(&p.root).downcast::<gtk::GridLayoutChild>().ok())
                else {
                    continue;
                };
                if cell.column() == column && cell.row() == row {
                    continue;
                }
                cell.set_column(column);
                cell.set_row(row);
            } else {
                if let Some(parent) = p.root.parent() {
                    match parent.downcast::<gtk::Grid>() {
                        Ok(other) => other.remove(&p.root),
                        Err(_) => p.root.unparent(),
                    }
                }
                grid.attach(&p.root, column, row, 1, 1);
            }
            moved += 1;
        }
        self.sync_focus_keys();
        tracing::debug!(panes = names.len(), moved, elapsed_us = started.elapsed().as_micros() as u64, "wall layout");
        self.update_attachments();
    }
    pub(super) fn set_mode(self: &Rc<Self>, mode: &str) {
        *self.mode.borrow_mut() = mode.into();
        self.layout();
        self.save_layout();
    }
    pub(super) fn save_layout(self: &Rc<Self>) {
        self.layout_revision
            .set(self.layout_revision.get().wrapping_add(1));
        let project = self.project.get();
        if project == 0 || self.client.borrow().is_none() || self.restored_project.get() != project
        {
            return;
        }
        self.layout_saves
            .borrow_mut()
            .insert(project, self.layout_state());
        if self.layout_saving.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            loop {
                let next = ui.layout_saves.borrow_mut().pop_first();
                let Some((project, state)) = next else { break };
                if let Err(e) = ui
                    .call(
                        "settings.set",
                        json!({"path":format!("native.layout.current.{project}"),"value":state}),
                    )
                    .await
                {
                    ui.show_error(&e.to_string());
                }
            }
            ui.layout_saving.set(false);
        });
    }
    fn layout_state(&self) -> Value {
        // "agents" is both the agent wall and Files and Git; keep which one was showing.
        let page = match self.page.borrow().as_str() {
            "agents" if !self.editor.agents_visible() => String::from("code"),
            // Money is not a project's layout: Dev comes back on the agent wall.
            page if crate::money::is_page(page) => String::from("agents"),
            page => page.to_owned(),
        };
        json!({"page":page,"agent_layout":*self.mode.borrow(),"columns":self.columns.get(),"focused":*self.focused.borrow(),"order":*self.ordered.borrow(),"sidebar":if self.page.borrow().as_str() == "settings" { self.settings_sidebar.get() } else { self.sidebar.is_visible() },"split":self.wall_split.position(),"width":self.window.width(),"height":self.window.height(),"project_tools":self.editor.layout_state()})
    }
    /// `project` and `revision` are what the refresh saw when it began: an iteration that
    /// outlived a project switch must not mark, or save over, the new project's layout.
    pub(super) async fn restore_layout(self: &Rc<Self>, project: i64, revision: u64) {
        if self.project.get() != project || self.restored_project.get() == project {
            return;
        }
        self.restored_project.set(project);
        // Opened on an explicit page: restore the rest of the project's layout, never its
        // saved page, and never save the previous project's layout over it.
        let explicit = self.explicit_page.replace(0) == project;
        if self.layout_revision.get() != revision && !explicit {
            self.save_layout();
            return;
        }
        let result = self
            .call(
                "settings.get",
                json!({"path":format!("native.layout.current.{project}")}),
            )
            .await;
        // A pending startup read must not undo navigation or a layout edit.
        if self.project.get() != project || (self.layout_revision.get() != revision && !explicit) {
            return;
        }
        match result {
            Ok(v) if !v["value"].is_null() => {
                let applying = self.applying_ui.replace(true);
                self.apply_layout_keeping(&v["value"], explicit);
                self.applying_ui.set(applying);
            }
            Ok(_) => {}
            // Unread is not restored: retry on the next refresh rather than let a save
            // replace a layout that was never applied.
            Err(_) => self.restored_project.set(0),
        }
        self.refresh_page();
    }
    pub(super) fn apply_layout(self: &Rc<Self>, state: &Value) {
        self.apply_layout_keeping(state, false);
    }
    /// Apply a saved layout; with `keep_page`, everything but its page.
    fn apply_layout_keeping(self: &Rc<Self>, state: &Value, keep_page: bool) {
        let files_shown = *self.page.borrow() == "agents" && !self.editor.agents_visible();
        self.editor.apply_layout(&state["project_tools"]);
        if keep_page && files_shown {
            // Files and Git was asked for: the saved panes must not hide its tree.
            self.editor.show_files();
        }
        if let Some(mode) = state["agent_layout"]
            .as_str()
            .filter(|s| matches!(*s, "grid" | "focus" | "review" | "mosaic"))
        {
            *self.mode.borrow_mut() = mode.into();
        }
        if let Some(n) = state["columns"].as_i64() {
            self.columns.set(n.clamp(1, 3) as i32);
        }
        if let Some(n) = state["focused"].as_str() {
            *self.focused.borrow_mut() = Some(n.into());
        }
        if let Some(order) = state["order"].as_array() {
            let current = self.ordered.borrow().clone();
            let mut order: Vec<String> = order
                .iter()
                .filter_map(|n| n.as_str().map(str::to_owned))
                .filter(|n| current.contains(n))
                .collect();
            order.dedup();
            for n in current {
                if !order.contains(&n) {
                    order.push(n);
                }
            }
            *self.ordered.borrow_mut() = order;
        }
        if let Some(split) = state["split"].as_i64() {
            let saved_width = state["width"].as_i64().unwrap_or(1440) as f64;
            let current_width = self.window.width().max(self.window.default_width());
            let sidebar = if self.sidebar.is_visible() {
                200.0
            } else {
                0.0
            };
            let available = (current_width as f64 - sidebar - 32.0).max(560.0);
            let ratio =
                (split as f64 / (saved_width - sidebar - 32.0).max(560.0)).clamp(0.25, 0.75);
            self.wall_split.set_position((available * ratio) as i32);
        }
        if let Some(visible) = state["sidebar"].as_bool() {
            self.settings_sidebar.set(visible);
            if self.page.borrow().as_str() != "settings" {
                self.sidebar.set_visible(visible);
            }
        }
        if let Some(page) = state["page"]
            .as_str()
            .filter(|_| !keep_page)
            .filter(|p| *p == "agents" || *p == "code" || self.pages.contains_key(*p))
        {
            self.navigate(page);
        } else {
            self.layout();
        }
    }
    pub(super) fn sync_navigation(self: &Rc<Self>, page: &str) {
        *self.navigation_pending.borrow_mut() = Some((self.project.get(), page.into()));
        if self.navigation_sending.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            // Coalesce rapid clicks and keep the engine's final page in click order.
            loop {
                let next = ui.navigation_pending.borrow_mut().take();
                let Some((project, page)) = next else { break };
                let client = ui.client.borrow().clone();
                let Some(client) = client else { break };
                let id = uuid::Uuid::new_v4();
                ui.navigation_echoes.borrow_mut().insert(id);
                let result = client
                    .request_with_id(
                        &ui.rt,
                        "ui.page.switch",
                        json!({"page":page,"project_id":if project>0 {Some(project)} else {None}}),
                        id,
                    )
                    .await;
                if let Err(error) = result {
                    ui.navigation_echoes.borrow_mut().remove(&id);
                    ui.show_error(&error.to_string());
                }
            }
            ui.navigation_sending.set(false);
        });
    }
    pub(super) fn apply_ui_event(self: &Rc<Self>, state: &Value) {
        self.applying_ui.set(true);
        if let Some(page) = state["page"].as_str() {
            let project = state["project_id"].as_i64().unwrap_or(self.project.get());
            if *self.page.borrow() != page || self.project.get() != project {
                self.open_project(project, page);
            }
        }
        if let Some(panes) = state["panes"].as_array() {
            for pane in panes {
                if pane["focused"] == true {
                    if let Some(session) = pane["target"]["session"].as_str() {
                        let changed = self.focused.borrow().as_deref() != Some(session);
                        *self.focused.borrow_mut() = Some(session.into());
                        // ui.changed carries all panes even when switching pages.
                        // Only a new terminal focus on Agents should change its layout.
                        if changed && state["page"].as_str().is_none_or(|page| page == "agents") {
                            self.set_mode("focus");
                            self.navigate("agents");
                        }
                    }
                }
            }
        }
        self.applying_ui.set(false);
    }
    pub(super) fn layout_menu(self: &Rc<Self>) {
        let Some((window, body)) = self.sheet("Window presets", 390) else {
            return;
        };
        window.compact(false, 620);
        body.append(&label(
            "Choose a starting layout, then save your arrangement.",
            "dim",
        ));
        let modes = gtk::Grid::builder()
            .column_spacing(8)
            .row_spacing(8)
            .column_homogeneous(true)
            .build();
        body.append(&modes);
        for (i, (mode, caption, columns)) in [
            ("grid", "Two columns", 2),
            ("focus", "Focus + tabs", 1),
            ("review", "Focus + stack", 2),
            ("mosaic", "Focus + grid", 2),
            ("grid", "Three columns", 3),
            ("grid", "Vertical stack", 1),
        ]
        .into_iter()
        .enumerate()
        {
            let b = button("", "layout-preset");
            let card = gtk::Box::new(gtk::Orientation::Vertical, 7);
            let preview = gtk::DrawingArea::new();
            preview.set_content_height(50);
            preview.set_draw_func(move |_, cr, width, height| {
                let w = f64::from(width);
                let h = f64::from(height);
                cr.set_source_rgb(0.35, 0.37, 0.39);
                let rect = |x: f64, y: f64, rw: f64, rh: f64| {
                    cr.rectangle(x * w + 2., y * h + 2., rw * w - 4., rh * h - 4.);
                    let _ = cr.fill();
                };
                match mode {
                    "focus" => rect(0., 0., 1., 1.),
                    "review" => {
                        rect(0., 0., 0.6, 1.);
                        rect(0.6, 0., 0.4, 0.5);
                        rect(0.6, 0.5, 0.4, 0.5);
                    }
                    "mosaic" => {
                        rect(0., 0., 0.5, 1.);
                        for j in 0..4 {
                            rect(
                                0.5 + f64::from(j % 2) * 0.25,
                                f64::from(j / 2) * 0.5,
                                0.25,
                                0.5,
                            );
                        }
                    }
                    _ => {
                        for j in 0..columns * 2 {
                            rect(
                                f64::from(j % columns) / f64::from(columns),
                                f64::from(j / columns) * 0.5,
                                1. / f64::from(columns),
                                0.5,
                            );
                        }
                    }
                }
            });
            card.append(&preview);
            card.append(&label(caption, ""));
            b.set_child(Some(&card));
            if *self.mode.borrow() == mode && (mode != "grid" || self.columns.get() == columns) {
                b.add_css_class("selected");
            }
            modes.attach(&b, (i % 2) as i32, (i / 2) as i32, 1, 1);
            let weak = Rc::downgrade(self);
            let grid = modes.downgrade();
            b.connect_clicked(move |key| {
                if let Some(ui) = weak.upgrade() {
                    // Columns are the grid's alone; focus must not leave a one-column grid behind.
                    if mode == "grid" {
                        ui.columns.set(columns);
                    }
                    ui.set_mode(mode);
                    if let Some(grid) = grid.upgrade() {
                        let mut child = grid.first_child();
                        while let Some(item) = child {
                            item.remove_css_class("selected");
                            child = item.next_sibling();
                        }
                    }
                    key.add_css_class("selected");
                }
            });
        }
        let sidebar = gtk::CheckButton::with_label("Show sidebar");
        sidebar.set_active(self.sidebar.is_visible());
        body.append(&sidebar);
        let weak = Rc::downgrade(self);
        sidebar.connect_toggled(move |b| {
            if let Some(ui) = weak.upgrade() {
                ui.sidebar.set_visible(b.is_active());
                ui.save_layout();
            }
        });
        body.append(&label("Saved presets", "title"));
        let saved = gtk::Box::new(gtk::Orientation::Vertical, 4);
        body.append(&saved);
        let name = gtk::Entry::builder()
            .placeholder_text("Preset name")
            .max_length(64)
            .build();
        body.append(&name);
        let save = button("Save current", "primary");
        body.append(&save);
        let weak = Rc::downgrade(self);
        let win = window.clone();
        let project = self.project.get();
        save.connect_clicked(move |key| {
            if let Some(ui) = weak.upgrade() {
                let name = name.text().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let state = ui.layout_state();
                let win = win.clone();
                let key = key.clone();
                key.set_sensitive(false);
                glib::spawn_future_local(async move {
                    match ui
                        .call(
                            "ui.layout.save",
                            json!({"project_id":project,"name":name,"state":state}),
                        )
                        .await
                    {
                        Ok(_) => win.close(),
                        Err(e) => ui.show_error(&e.to_string()),
                    }
                    key.set_sensitive(true);
                });
            }
        });
        let ui = self.clone();
        glib::spawn_future_local(async move {
            match ui
                .call("ui.layout.list", json!({"project_id":project}))
                .await
            {
                Ok(v) => {
                    for name in rows(&v, "layouts").iter().filter_map(Value::as_str) {
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                        let apply = button(name, "quiet");
                        apply.set_hexpand(true);
                        row.append(&apply);
                        let delete = icon_button("edit-delete-symbolic", "Delete preset");
                        row.append(&delete);
                        saved.append(&row);
                        let weak = Rc::downgrade(&ui);
                        let preset = name.to_string();
                        apply.connect_clicked(move |b| {
                            if let Some(ui) = weak.upgrade() {
                                ui.mutate("ui.layout.apply", json!({"project_id":project,"name":preset}), b);
                            }
                        });
                        // The list is read once, so a deleted preset leaves it here.
                        let weak = Rc::downgrade(&ui);
                        let preset = name.to_string();
                        let row = row.downgrade();
                        crate::app::confirm_inline(&delete, "Delete", move |key| {
                            let Some(ui) = weak.upgrade() else { return };
                            let payload = json!({"project_id":project,"name":preset});
                            let (key, row) = (key.clone(), row.clone());
                            key.set_sensitive(false);
                            glib::spawn_future_local(async move {
                                match ui.call("ui.layout.delete", payload).await {
                                    Ok(_) => {
                                        if let Some(row) = row.upgrade() {
                                            if let Some(list) = row.parent().and_downcast::<gtk::Box>() {
                                                list.remove(&row);
                                            }
                                        }
                                        ui.refresh();
                                    }
                                    Err(e) => ui.show_error(&e.to_string()),
                                }
                                key.set_sensitive(true);
                                ui.refresh_page();
                            });
                        });
                    }
                }
                Err(e) => ui.show_error(&e.to_string()),
            }
        });
        window.present();
    }
    pub(super) fn install_pane_controls(self: &Rc<Self>, pane: &Rc<Pane>, name: &str) {
        let drag = gtk::DragSource::new();
        drag.set_actions(gtk::gdk::DragAction::MOVE);
        let name = name.to_string();
        drag.connect_prepare(move |_, _, _| {
            Some(gtk::gdk::ContentProvider::for_value(&name.to_value()))
        });
        pane.header.add_controller(drag);
        let drop_target = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        let target = pane.name().to_string();
        drop_target.connect_drop(move |_, v, _, y| {
            let Ok(name) = v.get::<String>() else {
                return false;
            };
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let mut order = ui.ordered.borrow_mut();
            let Some(from) = order.iter().position(|n| *n == name) else {
                return false;
            };
            if name == target {
                return false;
            }
            order.remove(from);
            let Some(to) = order.iter().position(|n| *n == target) else {
                return false;
            };
            order.insert(to + usize::from(y > 100.0), name);
            drop(order);
            ui.layout();
            ui.save_layout();
            true
        });
        pane.root.add_controller(drop_target);
    }
    pub(super) fn session_actions(self: &Rc<Self>, pane: &Rc<Pane>, session: &Value) {
        // reconcile lays out only when the order changes, so a state change alone reaches the
        // focus tabs here; an agent turning blocked must not keep its running lamp.
        let tab = format!("focus-tab-{}", text(session, "name"));
        let mut child = self.focus_tabs.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if widget.widget_name() == tab {
                if let Some(lamp) = widget.first_child().and_then(|row| row.first_child()) {
                    tab_lamp(&lamp, text(session, "state"));
                }
            }
        }
        self.clear_agent_actions(pane);
        clear(&pane.slate_actions);
        if self.agent_overrides(pane, session) {
            if session["placeholder"] != true {
                self.agent_controls(pane, session);
            }
            return;
        }
        let state = text(session, "state");
        let name = text(session, "name");
        let (caption, icon, op, tooltip) = match state {
            "created" => ("Start", "media-playback-start-symbolic", "session.spawn", "Start the agent"),
            "parked" => ("Wake", "media-playback-start-symbolic", "session.wake", "Wake: start the provider again and resume its conversation"),
            "restorable" | "exited" => {
                ("Resume", "media-playback-start-symbolic", "session.resume", "Resume where it left off")
            }
            _ => ("Park", "park", "session.park", "Park: release the process, keep the worktree and scrollback"),
        };
        let action = icon_button(icon, tooltip);
        action.set_child(Some(&crate::icons::image(icon, 15)));
        if matches!(state, "created" | "parked" | "restorable" | "exited") {
            let slate_action = button(
                if state == "created" {
                    "Start session"
                } else {
                    caption
                },
                "",
            );
            let weak = Rc::downgrade(self);
            let n = name.to_string();
            slate_action.connect_clicked(move |key| {
                if let Some(ui) = weak.upgrade() {
                    ui.agent_op(op, &n, Some(key));
                }
            });
            pane.slate_actions.append(&slate_action);
        }
        let weak = Rc::downgrade(self);
        let n = name.to_string();
        action.connect_clicked(move |b| {
            if let Some(ui) = weak.upgrade() {
                ui.agent_op(op, &n, Some(b));
            }
        });
        pane.actions.append(&action);
        if matches!(state, "restorable" | "exited") {
            // Confirmed in place: the key itself turns into "Confirm …" for a second click.
            for (caption, icon, op, armed, tip) in [
                ("Clear context", "refresh", "session.clear_restorable", "Confirm clear", "Start fresh in this session and worktree; saved provider conversation context is cleared"),
                ("Discard session", "close", "session.close", "Confirm discard", "Remove this session from the wall; its worktree is kept, and so is its branch unless its work is already merged"),
            ] {
                for key in [button(caption, "quiet"), icon_button(icon, caption)] {
                    let slate = key.label().is_some();
                    key.set_tooltip_text(Some(if slate { tip } else { caption }));
                    let weak = Rc::downgrade(self);
                    let n = name.to_string();
                    crate::app::confirm_inline(&key, if slate { armed } else { "Confirm" }, move |key| {
                        if let Some(ui) = weak.upgrade() {
                            let payload = if op == "session.close" { json!({"session":n,"remove_worktree":false,"purge_build":false}) } else { json!({"session":n}) };
                            ui.mutate(op, payload, key);
                        }
                    });
                    if slate { pane.slate_actions.append(&key); } else { pane.actions.append(&key); }
                }
            }
        }
        if matches!(state, "created" | "parked" | "restorable" | "exited") {
            self.describe_stopped_session(pane, session);
        } else {
            pane.begin_context();
        }
        self.agent_controls(pane, session);
    }
    /// Every notify event lands here. A burst of them shares one count, as refresh() does: the
    /// count in flight runs once more when it answers if the revision moved meanwhile. Only
    /// counts are asked for (`count_only`): all unread, and the unread held ones that tint it.
    pub(super) fn refresh_notification_count(self: &Rc<Self>) {
        self.notification_revision
            .set(self.notification_revision.get().wrapping_add(1));
        if BELL_LOADING.with(|loading| loading.replace(true)) {
            return;
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let (result, generation) = loop {
                let revision = ui.notification_revision.get();
                let generation = ui.generation.get();
                let result = tokio::join!(
                    ui.call("notify.list", json!({"count_only":true})),
                    ui.call("notify.list", json!({"count_only":true,"category":"agent_blocked"})),
                    ui.call("notify.list", json!({"count_only":true,"category":"guardrail"}))
                );
                if ui.notification_revision.get() == revision {
                    break (result, generation);
                }
            };
            BELL_LOADING.with(|loading| loading.set(false));
            if let (Ok(all), Ok(blocked), Ok(guardrail)) = result {
                if ui.generation.get() != generation {
                    return;
                }
                let count = |v: &Value| v["unread"].as_u64().unwrap_or(0);
                let unread = count(&all);
                ui.notification_count.set_visible(unread > 0);
                ui.notification_count.set_text(&if unread > 99 {
                    String::from("99+")
                } else {
                    unread.to_string()
                });
                if count(&blocked) + count(&guardrail) > 0 {
                    ui.notification_count.add_css_class("held");
                } else {
                    ui.notification_count.remove_css_class("held");
                }
            }
        });
    }
    /// The frame the Agent skills and Plugins sheets share, presented as a rounded floating
    /// sheet in Settings' vocabulary: the project it applies to over an intro, a section whose
    /// heading carries `noun` and a count, a rounded card of rows showing `loading` until it is
    /// filled, a hidden feedback line, and a footer key that closes the sheet and opens `page`.
    /// Returns the project's name, the card, the count beside the heading and the line.
    fn switch_sheet(
        self: &Rc<Self>,
        panel: &Rc<crate::panel::Panel>,
        body: &gtk::Box,
        project: i64,
        intro: &str,
        loading: &str,
        (noun, manage, page): (&str, &str, &'static str),
    ) -> (String, gtk::Box, gtk::Label, gtk::Label) {
        panel.add_css_class("agent-skills-popover");
        panel.float();
        let name = self
            .projects
            .borrow()
            .iter()
            .find(|p| p["id"].as_i64() == Some(project))
            .map(|p| text(p, "name").to_owned())
            .unwrap_or_else(|| "No project".into());
        body.set_spacing(0);
        let head = gtk::Box::new(gtk::Orientation::Vertical, 4);
        head.add_css_class("agent-sheet-head");
        let scope = label(&name, "agent-sheet-project");
        scope.set_ellipsize(gtk::pango::EllipsizeMode::End);
        scope.set_tooltip_text(Some(&name));
        head.append(&scope);
        let intro = label(intro, "agent-skills-intro");
        intro.set_wrap(true);
        intro.set_xalign(0.);
        head.append(&intro);
        body.append(&head);
        let section = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        section.add_css_class("agent-sheet-section");
        let title = label(noun, "agent-sheet-section-title");
        title.set_hexpand(true);
        section.append(&title);
        let count = label("", "agent-sheet-count");
        section.append(&count);
        body.append(&section);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("agent-sheet-card");
        list.append(&label(loading, "agent-sheet-empty"));
        body.append(&list);
        let feedback = label("", "agent-sheet-feedback");
        feedback.set_wrap(true);
        feedback.set_xalign(0.);
        feedback.set_visible(false);
        body.append(&feedback);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("agent-skills-footer");
        let key = button("", "settings-link");
        let caption = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        caption.append(&label(manage, ""));
        caption.append(&crate::icons::image("chevron-right", 12));
        key.set_child(Some(&caption));
        key.set_hexpand(true);
        key.set_halign(gtk::Align::End);
        footer.append(&key);
        body.append(&footer);
        let weak = Rc::downgrade(self);
        let panel_weak = Rc::downgrade(panel);
        key.connect_clicked(move |_| {
            if let Some(panel) = panel_weak.upgrade() {
                panel.close();
            }
            if let Some(ui) = weak.upgrade() {
                ui.navigate(page);
            }
        });
        panel.present();
        (name, list, count, feedback)
    }
    /// One row of a switch sheet: a title over a quieter line, and the switch.
    fn sheet_row(title: &str, detail: &str, wrap: bool, toggle: &gtk::Switch) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        row.add_css_class("agent-skill-row");
        let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        let name = label(title, "skill-title");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&name);
        let line = label(detail, "skill-source");
        if wrap {
            line.set_wrap(true);
            line.set_xalign(0.);
        } else {
            line.set_ellipsize(gtk::pango::EllipsizeMode::End);
            line.set_tooltip_text(Some(detail));
        }
        line.set_visible(!detail.is_empty());
        words.append(&line);
        row.append(&words);
        toggle.add_css_class("settings-switch");
        toggle.set_valign(gtk::Align::Center);
        row.append(toggle);
        row
    }
    pub(crate) fn project_skills(self: &Rc<Self>) {
        let project = self.project.get();
        let Some((panel, body)) = self.sheet("Agent skills", 400) else {
            return;
        };
        panel.top(560);
        let (name, list, count, feedback) = self.switch_sheet(
            &panel,
            &body,
            project,
            "Switch the skills agents here read when they start or resume. Running agents keep the context already loaded.",
            "Loading skills…",
            ("Skills", "Open skill library", "skills"),
        );
        let ui = self.clone();
        let panel = Rc::downgrade(&panel);
        glib::spawn_future_local(async move {
            match ui.call("skill.list", json!({"project_id":project,"summary":true})).await {
                Ok(result) => {
                    clear(&list);
                    let skills = rows(&result, "skills");
                    count.set_text(&format!("{} installed", skills.len()));
                    if skills.is_empty() {
                        list.append(&label("No skills installed yet.", "agent-sheet-empty"));
                    }
                    for skill in skills {
                        let source = skill["source_path"].as_str().unwrap_or(
                            if skill["source_url"].is_string() {
                                "Installed from GitHub"
                            } else {
                                "Written in Relay"
                            },
                        );
                        let toggle = gtk::Switch::new();
                        let enabled = skill["enabled_in"]
                            .as_array()
                            .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(project)));
                        toggle.set_active(enabled);
                        toggle.set_sensitive(project > 0);
                        toggle.set_tooltip_text(Some(&format!(
                            "Enable {} for {name}",
                            text(&skill, "name")
                        )));
                        list.append(&Self::sheet_row(text(&skill, "name"), source, false, &toggle));
                        let feedback = feedback.clone();
                        crate::tools::enable_switch(&ui, &toggle, "skill.enable", "skill_id", skill["id"].clone(), project, move |_, result| {
                            match result {
                                Ok(_) => feedback.set_visible(false),
                                Err(error) => { feedback.set_text(&error); feedback.set_visible(true); }
                            }
                        });
                    }
                }
                Err(error) => {
                    clear(&list);
                    list.set_visible(false);
                    feedback.set_text(&error.to_string());
                    feedback.set_visible(true);
                }
            }
            if let Some(panel) = panel.upgrade() {
                panel.fit();
            }
        });
    }
    /// The plugin switches of the project in view, from the agents toolbar's Plugins key.
    pub(crate) fn project_plugins(self: &Rc<Self>) {
        let project = self.project.get();
        let Some((panel, body)) = self.sheet("Plugins", 440) else {
            return;
        };
        panel.top(640);
        let (_, list, count, feedback) = self.switch_sheet(
            &panel,
            &body,
            project,
            "A plugin that is on gives every agent here its skills, standing rules and MCP tools. Skills reach running agents now; rules and tools apply when an agent starts or resumes.",
            "Loading plugins…",
            ("Plugins", "All plugins", "plugins"),
        );
        let ui = self.clone();
        let panel = Rc::downgrade(&panel);
        glib::spawn_future_local(async move {
            match ui.call("plugin.list", json!({"project_id":project})).await {
                Ok(result) => {
                    clear(&list);
                    let plugins = rows(&result, "plugins");
                    // A total, not "n on": the switches below change that without a reload.
                    count.set_text(&format!("{} available", plugins.len()));
                    if plugins.is_empty() {
                        let stale = label(crate::tools::plugins::STALE_ENGINE, "agent-sheet-empty");
                        stale.set_wrap(true);
                        stale.set_xalign(0.);
                        list.append(&stale);
                    }
                    for plugin in plugins {
                        let suggested = plugin["suggested_for"]
                            .as_array()
                            .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(project)));
                        let detail = if suggested {
                            format!("Suggested for this project · {}", text(&plugin, "summary"))
                        } else {
                            text(&plugin, "summary").to_string()
                        };
                        let toggle = crate::tools::plugins::switch(
                            &ui,
                            &plugin,
                            project,
                            Some(feedback.clone()),
                        );
                        toggle.set_sensitive(project > 0);
                        list.append(&Self::sheet_row(text(&plugin, "name"), &detail, true, &toggle));
                    }
                }
                Err(error) => {
                    clear(&list);
                    list.set_visible(false);
                    feedback.set_text(&crate::tools::plugins::explain_error("plugin.list", &error));
                    feedback.set_visible(true);
                }
            }
            // Filled asynchronously: grow to the rows rather than keep the "Loading" height.
            if let Some(panel) = panel.upgrade() {
                panel.fit();
            }
        });
    }
    pub(super) fn command_palette(self: &Rc<Self>) {
        let Some((window, body)) = self.sheet("Command palette", 560) else {
            return;
        };
        window.compact(true, 400);
        let search = gtk::SearchEntry::new();
        body.append(&search);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.append(&list);
        let mut entries = Vec::new();
        for (page, name) in [
            ("agents", "Agents"),
            ("code", "Code"),
            ("board", "Board"),
            ("modules", "Modules"),
            ("notes", "Notes"),
            ("skills", "Skills"),
            ("plugins", "Plugins"),
            ("settings", "Settings"),
            ("mailbox", "Mailbox"),
            ("guardrails", "Guardrails"),
            ("devices", "Devices"),
            ("notifications", "Notifications"),
        ] {
            let b = button(name, "nav");
            list.append(&b);
            entries.push((name.to_lowercase(), b.clone()));
            let weak = Rc::downgrade(self);
            let w = window.clone();
            b.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.navigate(page);
                    w.close();
                }
            });
        }
        for (key, caption) in crate::money::palette_entries() {
            let b = button(&caption, "nav");
            list.append(&b);
            entries.push((caption.to_lowercase(), b.clone()));
            let weak = Rc::downgrade(self);
            let w = window.clone();
            b.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    w.close();
                    crate::money::run_palette(&ui, key);
                }
            });
        }
        for s in self.sessions.borrow().iter() {
            let name = text(s, "name").to_string();
            let b = button(&format!("Focus {name}"), "nav");
            list.append(&b);
            entries.push((name.to_lowercase(), b.clone()));
            let weak = Rc::downgrade(self);
            let w = window.clone();
            b.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    *ui.focused.borrow_mut() = Some(name.clone());
                    ui.set_mode("focus");
                    ui.navigate("agents");
                    w.close();
                }
            });
        }
        search.connect_search_changed(move |s| {
            let q = s.text().to_lowercase();
            for (name, b) in &entries {
                b.set_visible(name.contains(&q));
            }
        });
        window.present();
        search.grab_focus();
    }
    pub(super) fn install_shortcuts(self: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, keycode, mods| {
            let Some(ui) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !ui.panels.borrow().is_empty() {
                return glib::Propagation::Proceed;
            }
            // Capture runs before the focused terminal sees the key.
            let in_terminal = GtkWindowExt::focus(&ui.window).is_some_and(|w| {
                w.is::<vte4::Terminal>() || w.ancestor(vte4::Terminal::static_type()).is_some()
            });
            let key = crate::shortcuts::latin(key, keycode);
            if key == gtk::gdk::Key::Escape && crate::money::escape_start(&ui) {
                return glib::Propagation::Stop;
            }
            for (action, _, fallback) in crate::shortcuts::DEFAULTS {
                let bindings = ui.keybindings.borrow();
                let chord = bindings[action].as_str().unwrap_or(fallback);
                if !crate::shortcuts::matches(key, mods, chord)
                    || (in_terminal && crate::shortcuts::terminal_owns(chord))
                {
                    continue;
                }
                drop(bindings);
                match action {
                    "palette" => ui.command_palette(),
                    "agents" | "code" | "board" | "settings" => ui.navigate(action),
                    "space" => crate::money::toggle_space(&ui),
                    // In Money the same chord logs an entry.
                    "new_session" if crate::money::active() => crate::money::new_thread(&ui),
                    // Rebuilding an open sheet would discard what is being typed into it.
                    "new_session" if ui.launch.reveals_child() => {}
                    "new_session" => ui.show_launch(None),
                    "sidebar" => {
                        ui.sidebar.set_visible(!ui.sidebar.is_visible());
                        ui.save_layout();
                    }
                    _ => continue,
                }
                return glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Escape && ui.launch.reveals_child() && !ui.launch_busy.get() {
                ui.launch.set_reveal_child(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(keys);
    }
    pub(super) fn load_keybindings(self: &Rc<Self>) {
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let generation = ui.generation.get();
            match ui.call("settings.get", json!({"path":"keybindings"})).await {
                Ok(v) if generation == ui.generation.get() => {
                    *ui.keybindings.borrow_mut() = v["value"].clone()
                }
                Err(e) => ui.show_error(&e.to_string()),
                _ => {}
            }
        });
    }
    pub(crate) fn load_appearance(self: &Rc<Self>) {
        /// What the last reload applied. Each runs 150 ms after any appearance or font-size
        /// save, slider drags included, and repeats only what changed: a stylesheet reload
        /// restyles every widget in both windows, and a wallpaper decode is a 4K JPEG.
        #[derive(Default)]
        struct Applied {
            /// The provider below was loaded into; another `Ui` starts over.
            provider: usize,
            css: String,
            /// The wallpaper value shown (or being decoded), `None` before the first.
            wallpaper: Option<Value>,
            /// Counts decodes, so only the newest one lands.
            decode: u64,
            /// Counts reloads, so one that waited on a decode does not undo a newer one.
            reload: u64,
            /// The mode and font size the editor and every pane last took.
            panes: Option<(String, f64)>,
        }
        thread_local! {
            static APPLIED: RefCell<Applied> = RefCell::new(Applied::default());
        }
        let ui = self.clone();
        glib::spawn_future_local(async move {
            let generation = ui.generation.get();
            let (mode, size, alpha, contrast, image, dim) = tokio::join!(
                ui.call("settings.get", json!({"path":"appearance.mode"})),
                ui.call("settings.get", json!({"path":"terminal.font_size"})),
                ui.call("settings.get", json!({"path":"appearance.panel_alpha"})),
                ui.call(
                    "settings.get",
                    json!({"path":"appearance.content_contrast"})
                ),
                ui.call("settings.get", json!({"path":"appearance.wallpaper"})),
                ui.call("settings.get", json!({"path":"appearance.wallpaper_dim"}))
            );
            if generation != ui.generation.get() {
                return;
            }
            let provider = ui.appearance.as_ptr() as usize;
            let reload = APPLIED.with(|a| {
                let mut a = a.borrow_mut();
                if a.provider != provider {
                    *a = Applied { provider, ..Applied::default() };
                }
                a.reload += 1;
                a.reload
            });
            if let Ok(value) = mode {
                *ui.palette.borrow_mut() = value["value"].as_str().unwrap_or("matte").into();
            }
            if let Ok(value) = size {
                ui.font_size
                    .set(value["value"].as_f64().unwrap_or(9.75).clamp(8.0, 24.0));
            }
            let colors = crate::fonts::palette(&ui.palette.borrow());
            let css: [String; 10] = std::array::from_fn(|i| {
                format!("@define-color {} {};", crate::fonts::TOKENS[i], colors[i])
            });
            let mut css = css.join("\n");
            let alpha = alpha
                .ok()
                .and_then(|v| v["value"].as_f64())
                .unwrap_or(1.)
                .clamp(0.5, 1.);
            let contrast = contrast
                .ok()
                .and_then(|v| v["value"].as_f64())
                .unwrap_or(0.)
                .clamp(0., 1.);
            let alpha = alpha + (1. - alpha) * contrast;
            for ((name, color), offset) in ["wall", "console", "slab", "wash"]
                .iter()
                .zip(colors.iter())
                .zip([0.02_f64, 0.0, 0.04, 0.08])
            {
                css += &format!(
                    "\n@define-color {name} alpha({color},{});",
                    (alpha + offset).min(1.)
                );
                // Windows with no wallpaper behind them (Notes) and menus draw these, so
                // panel opacity never shows the desktop through them.
                css += &format!("\n@define-color {name}_solid {color};");
            }
            // Content is as see-through as a terminal: `page` coverage, slightly denser than the
            // chrome (the bars and sidebars, at `alpha`). A terminal plate sits on one @wall
            // layer and @plate tops that up to it; every other page paints @page_fill, the same
            // coverage in one layer (theme.css, .page-stack). Matte's opaque default keeps the
            // opaque screen.
            let page = (alpha + 0.05).min(1.);
            let top_up = |under: f64| if under >= 1. { 1. } else { ((page - under) / (1. - under)).clamp(0., 1.) };
            let plate = top_up((alpha + 0.02).min(1.));
            css += &format!("\n@define-color plate alpha({},{plate:.3});", colors[crate::fonts::SCREEN]);
            css += &format!("\n@define-color page_fill alpha({},{page});", colors[0]);
            // The Notes paper is a terminal in one layer, for a window with no page stack
            // (css/notes.css): the @plate's screen over its @wall, each weighted by what it
            // covers, at the same `page` coverage. A tab over the chrome's @console tops that
            // up to the same coverage.
            let screen_share = if page > 0. { plate / page } else { 1. };
            let paper = format!("mix({},{},{screen_share:.3})", colors[0], colors[crate::fonts::SCREEN]);
            css += &format!("\n@define-color notes_paper alpha({paper},{page});");
            css += &format!("\n@define-color notes_paper_top alpha({paper},{:.3});", top_up(alpha));
            css += &format!("\n@define-color backbox_chrome alpha({},{});", colors[1], alpha.max(0.84));
            let dim = dim.ok().and_then(|v| v["value"].as_f64()).unwrap_or(0.28).clamp(0., 0.85);
            // The picture, decoded off the GTK thread (base64 and a full-size JPEG or PNG) and
            // only when it changed. `None`: keep what is shown; `Some(Err)`: it would not decode.
            let mut picture: Option<Result<Option<gtk::gdk::Texture>, ()>> = None;
            if let Ok(v) = image {
                // Settings keeps showing the saved choice; a rotation only paints this window.
                crate::tools::settings::sync_wallpaper(&ui, &v["value"]);
                let image = crate::wallpaper_rotation::shown(&ui, &v["value"]);
                let decode = APPLIED.with(|a| {
                    let mut a = a.borrow_mut();
                    if a.wallpaper.as_ref() == Some(&image) {
                        return None;
                    }
                    a.wallpaper = Some(image.clone());
                    a.decode += 1;
                    Some(a.decode)
                });
                if let Some(decode) = decode {
                    let data = image.as_str().and_then(|s| {
                        s.strip_prefix("data:image/jpeg;base64,")
                            .or_else(|| s.strip_prefix("data:image/png;base64,"))
                    });
                    let decoded = match data {
                        Some(data) => {
                            let data = data.to_owned();
                            ui.rt
                                .spawn_blocking(move || {
                                    use base64::Engine;
                                    base64::engine::general_purpose::STANDARD.decode(data).ok().and_then(|bytes| {
                                        gtk::gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)).ok()
                                    })
                                })
                                .await
                                .ok()
                                .flatten()
                                .map(Some)
                                .ok_or(())
                        }
                        None => Ok(None),
                    };
                    // A newer reload chose another picture meanwhile.
                    if APPLIED.with(|a| a.borrow().decode) == decode {
                        picture = Some(decoded);
                    }
                }
            }
            // The look lands in one go, picture included, as it did when the decode held the
            // thread; a reload that started while this one decoded has applied its own.
            if APPLIED.with(|a| a.borrow().reload) == reload {
                // The font size is not in it: a size change leaves the stylesheet alone.
                if APPLIED.with(|a| a.borrow().css != css) {
                    ui.appearance.load_from_string(&css);
                    APPLIED.with(|a| a.borrow_mut().css = css);
                    restyle_hidden();
                }
                ui.wallpaper_dim.set_opacity(dim);
            }
            match picture {
                Some(Ok(texture)) => ui.wallpaper.set_paintable(texture.as_ref()),
                Some(Err(())) => {
                    // Not remembered as shown, so the next reload says so again.
                    APPLIED.with(|a| a.borrow_mut().wallpaper = None);
                    ui.show_error("The saved wallpaper could not be decoded.");
                }
                None => {}
            }
            // The editor and the terminals: only a mode or font size they have not taken.
            let applied = (ui.palette.borrow().clone(), ui.font_size.get());
            if APPLIED.with(|a| a.borrow().panes.as_ref() != Some(&applied)) {
                ui.editor.set_palette(&ui.palette.borrow());
                for p in ui.panes.borrow().values() {
                    p.apply_appearance(&ui.palette.borrow(), ui.font_size.get());
                    p.schedule_resize();
                }
                APPLIED.with(|a| a.borrow_mut().panes = Some(applied));
            }
        });
    }
}

/// A focus tab's lamp shows its session's state.
fn tab_lamp(lamp: &impl IsA<gtk::Widget>, state: &str) {
    for class in ["live", "held", "waiting"] {
        lamp.remove_css_class(class);
    }
    match state {
        "running" | "spawning" => lamp.add_css_class("live"),
        "blocked" => lamp.add_css_class("held"),
        "restorable" => lamp.add_css_class("waiting"),
        _ => (),
    }
}

/// GTK keeps the named colours a widget resolved while it is hidden at a stylesheet reload (a
/// page not on screen, a closed panel) and shows it later with the old ones: every page built
/// before panel opacity was read stayed at the opaque defaults. A class that flips with each
/// reload gives every widget a new declaration, so each resolves its colours afresh when shown.
fn restyle_hidden() {
    thread_local! {
        static FLIP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    fn walk(widget: &gtk::Widget, on: &str, off: &str) {
        widget.remove_css_class(off);
        widget.add_css_class(on);
        let mut child = widget.first_child();
        while let Some(next) = child {
            walk(&next, on, off);
            child = next.next_sibling();
        }
    }
    let flip = FLIP.with(|f| {
        f.set(!f.get());
        f.get()
    });
    let (on, off) = if flip { ("restyle-a", "restyle-b") } else { ("restyle-b", "restyle-a") };
    for window in gtk::Window::list_toplevels() {
        walk(&window, on, off);
    }
}
