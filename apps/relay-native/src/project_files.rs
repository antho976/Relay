use super::*;

impl Editor {
    pub fn mount_agents(&self, agents: &impl IsA<gtk::Widget>) {
        self.content_stack.add_named(agents, Some("agents"));
        self.show_agents();
    }
    pub fn show_agents(&self) {
        if self.content_stack.child_by_name("agents").is_some() {
            self.content_stack.set_visible_child_name("agents");
        }
    }
    pub fn agents_visible(&self) -> bool {
        self.content_stack.visible_child_name().as_deref() == Some("agents")
    }
    /// Mount the Design view (`design.rs`) beside the agent wall.
    pub fn mount_design(&self, design: &impl IsA<gtk::Widget>) {
        self.content_stack.add_named(design, Some("design"));
    }
    pub fn show_design(&self) {
        if self.content_stack.child_by_name("design").is_some() {
            self.content_stack.set_visible_child_name("design");
        }
    }
    pub fn design_visible(&self) -> bool {
        self.content_stack.visible_child_name().as_deref() == Some("design")
    }
    pub fn connect_view_changed(&self, callback: impl Fn() + 'static) {
        self.content_stack
            .connect_visible_child_name_notify(move |_| callback());
    }
    pub fn show_files(&self) {
        self.file_sidebar.set_visible(true);
        self.content_stack
            .set_visible_child_name(if self.path.borrow().is_empty() {
                "empty"
            } else if self.image_mode.get() {
                "image"
            } else {
                "source"
            });
    }
    pub fn toggle_files(&self) {
        self.file_sidebar
            .set_visible(!self.file_sidebar.is_visible());
    }
    pub fn toggle_git(&self) {
        self.git.set_visible(!self.git.is_visible());
    }
    pub fn layout_state(&self) -> Value {
        json!({"files":self.file_sidebar.is_visible(),"git":self.git.is_visible(),
            "files_width":self.files_split.position().max(260),"git_width":self.git.width().max(320)})
    }
    pub fn apply_layout(&self, state: &Value) {
        if let Some(visible) = state["files"].as_bool() {
            self.file_sidebar.set_visible(visible);
        }
        if let Some(visible) = state["git"].as_bool() {
            self.git.set_visible(visible);
        }
        let files_width = state["files_width"].as_i64().unwrap_or(260).clamp(180, 440) as i32;
        let git_width = state["git_width"].as_i64().unwrap_or(320).clamp(260, 520) as i32;
        self.files_split.set_position(files_width);
        self.git_split.set_position(
            (self.root.width().max(900)
                - if self.file_sidebar.is_visible() {
                    files_width
                } else {
                    0
                }
                - git_width)
                .max(200),
        );
    }
    pub fn prepare_project(self: &Rc<Self>, ui: &Rc<Ui>) {
        self.load_tree(ui);
        self.refresh_scopes(ui);
        self.refresh_git(ui);
        self.update_scope_label(ui);
    }
    pub(super) fn update_scope_label(self: &Rc<Self>, ui: &Rc<Ui>) {
        // The viewed checkout is gone from a loaded list (its session closed, or branch cleanup
        // removed it): every scope is picked from that list, so it was removed, not unknown.
        let current = self.worktree.borrow().clone();
        let removed = !current.is_empty() && {
            let worktrees = self.worktrees.borrow();
            !worktrees.is_empty() && !worktrees.iter().any(|w| text(w, "path") == current)
        };
        if removed && !self.is_dirty() {
            ui.show_error(&format!("The checkout {current} was removed; showing the primary checkout."));
            self.select_checkout(ui, "");
            return;
        }
        let projects = ui.projects.borrow();
        let project = projects.iter().find(|p| p["id"] == ui.project.get());
        let project_name = project.map(|p| text(p, "name")).unwrap_or("Project");
        let spaces = ui.workspaces.borrow();
        let space = project.and_then(|p| spaces.iter().find(|w| w["id"] == p["workspace_id"]));
        let workspace_name = space.map(|w| text(w, "name")).unwrap_or("Workspace");
        let worktrees = self.worktrees.borrow();
        let selected = worktrees
            .iter()
            .find(|w| text(w, "path") == current)
            .or_else(|| worktrees.first());
        // Unsaved edits keep a removed checkout open; say so rather than name the primary.
        let branch = if removed {
            format!("{} (removed)", current.rsplit('/').next().unwrap_or(&current))
        } else {
            selected
                .map(|w| text(w, "branch"))
                .unwrap_or("Primary checkout")
                .to_owned()
        };
        self.scope_label
            .set_text(&format!("{workspace_name} / {project_name} · {branch}"));
    }
    pub(super) fn select_checkout(self: &Rc<Self>, ui: &Rc<Ui>, path: &str) {
        if self.worktree.borrow().as_str() == path {
            return;
        }
        if self.is_dirty() {
            ui.show_error("Save or discard your changes before switching checkouts.");
            return;
        }
        let agents = self.agents_visible();
        self.clear_document();
        self.commit_message.buffer().set_text("");
        self.branch_name.set_text("");
        self.branch_start.set_text("");
        self.selected_path.borrow_mut().clear();
        self.expanded.borrow_mut().clear();
        *self.worktree.borrow_mut() = path.into();
        self.search.set_text("");
        self.load_tree(ui);
        self.refresh_git(ui);
        self.update_scope_label(ui);
        if agents {
            self.show_agents();
        }
    }
    pub(super) fn scope_picker(self: &Rc<Self>, ui: &Rc<Ui>, button: &gtk::MenuButton) {
        let popover = gtk::Popover::new();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        body.set_size_request(350, -1);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search projects, branches, or agents"));
        body.append(&search);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut searchable: Vec<(gtk::Widget, String)> = Vec::new();
        list.append(&label("PROJECTS", "section-label"));
        for project in ui.projects.borrow().iter() {
            let spaces = ui.workspaces.borrow();
            let space = spaces.iter().find(|w| w["id"] == project["workspace_id"]);
            let title = format!(
                "{} / {}",
                space.map(|w| text(w, "name")).unwrap_or("Workspace"),
                text(project, "name")
            );
            let key = button_row(&title, "folder");
            let id = project["id"].as_i64().unwrap_or(0);
            if id == ui.project.get() {
                key.add_css_class("selected");
            }
            let weak = Rc::downgrade(ui);
            let pop = popover.downgrade();
            key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.open_project(id, "agents");
                }
                if let Some(pop) = pop.upgrade() {
                    pop.popdown();
                }
            });
            searchable.push((key.clone().upcast(), title.to_lowercase()));
            list.append(&key);
        }
        list.append(&label("CHECKOUTS & BRANCHES", "section-label"));
        let mut choices = vec![(String::new(), String::from("Primary checkout"))];
        // worktree.list names the primary checkout first; it is the entry above, under "".
        choices.extend(self.worktrees.borrow().iter().skip(1).map(|w| {
            (
                text(w, "path").to_string(),
                format!(
                    "{} · {}",
                    text(w, "branch"),
                    w["session"].as_str().unwrap_or("Checkout")
                ),
            )
        }));
        for (path, title) in choices {
            let key = button_row(&title, "branch");
            key.set_tooltip_text(Some(&path));
            if path == *self.worktree.borrow() {
                key.add_css_class("selected");
            }
            let ed = self.clone();
            let weak = Rc::downgrade(ui);
            let pop = popover.downgrade();
            key.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ed.select_checkout(&ui, &path);
                }
                if let Some(pop) = pop.upgrade() {
                    pop.popdown();
                }
            });
            searchable.push((key.clone().upcast(), title.to_lowercase()));
            list.append(&key);
        }
        let branch_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        branch_list.append(&label("LOCAL BRANCHES", "section-label"));
        list.append(&branch_list);
        let ed = self.clone();
        let weak = Rc::downgrade(ui);
        let pop = popover.downgrade();
        let query = search.clone();
        let project = ui.project.get();
        let worktree = self.worktree.borrow().clone();
        let payload = self.payload(ui, json!({}));
        glib::spawn_future_local(async move {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = ui.call("git.branches", payload).await;
            // Not scope_revision: every invalidate bumps it, and the branches are still those of
            // this project and checkout.
            if !ed.matches(&ui, project, &worktree) {
                return;
            }
            match result {
                Ok(value) => {
                    for branch in rows(&value, "branches") {
                        let name = text(&branch, "name").to_owned();
                        let current = branch["current"] == true;
                        let holder = if current { None } else { ed.branch_holder(&ui, &name) };
                        let key = button_row(&name, if current { "check" } else { "branch" });
                        key.set_widget_name(&format!("scope-branch-{name}"));
                        key.set_sensitive(!current);
                        key.set_tooltip_text(Some(&match &holder {
                            _ if current => String::from("The branch this checkout is on"),
                            Some((path, _)) if path.is_empty() => format!("{name} is checked out in the primary checkout; open it"),
                            Some((path, _)) => format!("{name} is checked out in {path}; open that checkout"),
                            None => format!("Switch this checkout to {name}"),
                        }));
                        let needle = name.to_lowercase();
                        key.set_visible(needle.contains(query.text().to_lowercase().as_str()));
                        let row = key.downgrade();
                        query.connect_search_changed(move |search| {
                            if let Some(row) = row.upgrade() {
                                row.set_visible(
                                    needle.contains(search.text().to_lowercase().as_str()),
                                );
                            }
                        });
                        let editor = ed.clone();
                        let weak = Rc::downgrade(&ui);
                        let pop = pop.clone();
                        key.connect_clicked(move |_| {
                            if let Some(pop) = pop.upgrade() {
                                pop.popdown();
                            }
                            if let Some(ui) = weak.upgrade() {
                                match &holder {
                                    Some((path, _)) => editor.select_checkout(&ui, path),
                                    None => editor.git_action(
                                        &ui,
                                        "git.branch.switch",
                                        json!({"name":name}),
                                        None,
                                    ),
                                }
                            }
                        });
                        branch_list.append(&key);
                    }
                }
                Err(error) => branch_list.append(&label(&error.to_string(), "dim")),
            }
        });
        search.connect_search_changed(move |search| {
            let query = search.text().to_lowercase();
            for (row, title) in &searchable {
                row.set_visible(title.contains(&query));
            }
        });
        // Enter takes the first match, as the Git panel's branch picker does.
        let rows = list.downgrade();
        search.connect_activate(move |search| {
            if search.text().trim().is_empty() {
                return;
            }
            // The filter runs a moment after typing; bring it up to date first.
            search.emit_by_name::<()>("search-changed", &[]);
            if let Some(key) = rows.upgrade().and_then(|list| first_choice(list.upcast_ref())) {
                key.emit_clicked();
            }
        });
        let scroll = scrolled(&list);
        scroll.set_max_content_height(400);
        scroll.set_propagate_natural_height(true);
        body.append(&scroll);
        popover.set_child(Some(&body));
        button.set_popover(Some(&popover));
        popover.connect_show(move |_| {
            search.grab_focus();
        });
    }
    pub(super) fn bind_tree_row(
        self: &Rc<Self>,
        ui: &Rc<Ui>,
        row: &impl IsA<gtk::Widget>,
        path: &str,
        directory: bool,
    ) {
        row.set_widget_name(&format!("project-file:{path}"));
        let mark = gtk::GestureClick::new();
        mark.set_button(1);
        let weak = Rc::downgrade(self);
        let selected = path.to_owned();
        mark.connect_pressed(move |_, _, _, _| {
            if let Some(editor) = weak.upgrade() {
                *editor.selected_path.borrow_mut() = selected.clone();
                editor.selected_directory.set(directory);
            }
        });
        row.add_controller(mark);
        let select = gtk::GestureClick::new();
        select.set_button(3);
        let ed = self.clone();
        let weak = Rc::downgrade(ui);
        let path_owned = path.to_owned();
        let target = row.as_ref().downgrade();
        select.connect_pressed(move |_, _, x, y| {
            let (Some(ui), Some(target)) = (weak.upgrade(), target.upgrade()) else {
                return;
            };
            *ed.selected_path.borrow_mut() = path_owned.clone();
            ed.selected_directory.set(directory);
            let pop = gtk::Popover::new();
            pop.set_parent(&target);
            pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            let actions = gtk::Box::new(gtk::Orientation::Vertical, 2);
            for (title, op) in [
                ("New file or folder", "file.create"),
                ("Rename", "file.rename"),
                ("Move to trash", "file.delete"),
            ] {
                let key = button(title, "quiet");
                let ed = ed.clone();
                let ui = Rc::downgrade(&ui);
                let close = pop.downgrade();
                key.connect_clicked(move |_| {
                    if let Some(pop) = close.upgrade() {
                        pop.popdown();
                    }
                    if let Some(ui) = ui.upgrade() {
                        ed.file_action(&ui, op);
                    }
                });
                actions.append(&key);
            }
            pop.set_child(Some(&actions));
            pop.connect_closed(|pop| pop.unparent());
            pop.popup();
        });
        row.add_controller(select);
        let source = gtk::DragSource::new();
        source.set_actions(gtk::gdk::DragAction::MOVE);
        let weak = Rc::downgrade(ui);
        let ed = self.clone();
        if directory {
            self.bind_folder_drop(ui, row, path);
        }
        let path = path.to_owned();
        source.connect_prepare(move |_, _, _| {
            let ui = weak.upgrade()?;
            let data =
                json!({"project":ui.project.get(),"worktree":*ed.worktree.borrow(),"path":path})
                    .to_string();
            Some(gtk::gdk::ContentProvider::for_value(&data.to_value()))
        });
        row.add_controller(source);
    }
}
/// The first shown key under `parent` that can be pressed, in the order the list reads.
fn first_choice(parent: &gtk::Widget) -> Option<gtk::Button> {
    let mut child = parent.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if !widget.is_visible() {
            continue;
        }
        match widget.downcast_ref::<gtk::Button>() {
            Some(key) if key.is_sensitive() => return Some(key.clone()),
            Some(_) => {}
            None if widget.is::<gtk::Box>() => {
                if let Some(key) = first_choice(&widget) {
                    return Some(key);
                }
            }
            None => {}
        }
    }
    None
}

fn button_row(title: &str, icon: &str) -> gtk::Button {
    let row = button("", "quiet");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&crate::icons::image(icon, 13));
    let name = label(title, "body");
    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    name.set_max_width_chars(42);
    content.append(&name);
    row.set_child(Some(&content));
    row
}
/// The explorer glyph for a path and the CSS class that tints it.
pub(super) fn file_glyph(path: &str) -> (&'static str, &'static str) {
    let file = std::path::Path::new(path);
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match name.as_str() {
        "cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lockb"
        | "flake.lock" | "poetry.lock" | "composer.lock" | "gemfile.lock" | "uv.lock" => {
            return ("file-lock", "ft-lock");
        }
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitkeep" | ".ignore" => {
            return ("file-git", "ft-git");
        }
        "cargo.toml" | "rust-toolchain" | "rust-toolchain.toml" => {
            return ("file-config", "ft-rust");
        }
        "package.json" | "tsconfig.json" => return ("file-json", "ft-js"),
        "dockerfile" | "makefile" | "justfile" | "containerfile" => {
            return ("file-shell", "ft-shell");
        }
        "license" | "licence" | "copying" | "notice" => return ("file-text", "ft-text"),
        _ if name.starts_with(".env") || (name.ends_with("rc") && name.starts_with('.')) => {
            return ("file-config", "ft-config");
        }
        _ => {}
    }
    let extension = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "rs" => ("file-rust", "ft-rust"),
        "ts" | "mts" | "cts" => ("file-ts", "ft-ts"),
        "tsx" | "jsx" => ("file-react", "ft-react"),
        "js" | "mjs" | "cjs" => ("file-js", "ft-js"),
        "json" | "jsonc" | "json5" | "jsonl" => ("file-json", "ft-json"),
        "md" | "mdx" | "markdown" => ("file-md", "ft-md"),
        "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "env" | "properties" | "editorconfig" => {
            ("file-config", "ft-config")
        }
        "css" | "scss" | "sass" | "less" => ("file-css", "ft-css"),
        "html" | "htm" | "xml" | "svelte" | "vue" | "xhtml" | "ui" => ("file-html", "ft-html"),
        "py" | "pyi" | "pyw" => ("file-py", "ft-py"),
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "nu" => ("file-shell", "ft-shell"),
        "lock" => ("file-lock", "ft-lock"),
        "txt" | "rst" | "log" | "csv" | "tsv" => ("file-text", "ft-text"),
        "zip" | "gz" | "tar" | "xz" | "7z" | "zst" | "bz2" | "tgz" | "rar" => ("modules", "ft-archive"),
        "go" => ("code", "ft-go"),
        "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "hh" | "m" | "mm" => ("code", "ft-c"),
        "java" | "kt" | "kts" | "scala" | "groovy" | "gradle" => ("code", "ft-java"),
        "cs" | "fs" | "swift" | "rb" | "php" | "lua" | "zig" | "dart" | "ex" | "exs" | "hs"
        | "ml" | "nim" | "r" | "jl" | "pl" => ("code", "ft-code"),
        "sql" | "graphql" | "gql" | "proto" | "prisma" => ("file-config", "ft-data"),
        "glsl" | "wgsl" | "hlsl" | "frag" | "vert" | "shader" => ("code", "ft-shader"),
        _ if super::image_preview::is_image(path) => ("file-image", "ft-image"),
        _ => ("file", "ft-plain"),
    }
}

/// A tinted file-type icon, as the explorer and the Git panel draw it.
pub(super) fn file_image(path: &str, size: i32) -> gtk::Image {
    let (glyph, tint) = file_glyph(path);
    let image = crate::icons::image(glyph, size);
    image.add_css_class("file-icon");
    image.add_css_class(tint);
    image.set_valign(gtk::Align::Center);
    image
}

// Whether a porcelain entry is unmerged (DD, AU, UD, UA, DU, AA or UU), as the Git panel reads it.
use relay_client::git_view::is_conflict as is_unmerged;

/// One porcelain status code as VS Code letters it, with its colour class.
pub(super) fn status_letter(code: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match code.trim() {
        "M" | "T" => ("M", "git-modified", "Modified"),
        "A" => ("A", "git-added", "Added"),
        "?" => ("U", "git-untracked", "Untracked"),
        "D" => ("D", "git-deleted", "Deleted"),
        "R" => ("R", "git-renamed", "Renamed"),
        "C" => ("C", "git-renamed", "Copied"),
        "U" => ("!", "git-conflict", "Conflict"),
        "!" => ("I", "git-ignored", "Ignored"),
        _ => return None,
    })
}

thread_local! {
    /// The explorer row last selected, so a new selection can clear it.
    static SELECTED_ROW: RefCell<Option<glib::WeakRef<gtk::Widget>>> = const { RefCell::new(None) };
    /// Each folder holding a change from the last `git.status`, with the strongest status
    /// under it (`FOLDER_CLASSES` order), so tinting a folder is one lookup.
    static CHANGED: RefCell<std::collections::HashMap<String, usize>> = RefCell::default();
}

const FOLDER_CLASSES: [&str; 3] = ["git-untracked", "git-modified", "git-conflict"];

pub(super) fn mark_selected(row: &impl IsA<gtk::Widget>) {
    let row = row.as_ref();
    SELECTED_ROW.with(|selected| {
        if let Some(previous) = selected.borrow().as_ref().and_then(|weak| weak.upgrade()) {
            previous.remove_css_class("selected");
        }
        row.add_css_class("selected");
        *selected.borrow_mut() = Some(row.downgrade());
    });
}

/// The strongest status among the changes under a folder, if any.
pub(super) fn folder_status(path: &str) -> Option<&'static str> {
    CHANGED.with(|changed| changed.borrow().get(path).map(|rank| FOLDER_CLASSES[*rank]))
}

const STATUS_CLASSES: [&str; 7] = [
    "git-modified", "git-added", "git-untracked", "git-deleted", "git-renamed", "git-conflict", "git-ignored",
];

impl Editor {
    /// Record the latest status and re-tint the folders already drawn in the explorer.
    pub(super) fn note_changes(&self, files: &[Value]) {
        CHANGED.with(|changed| {
            let mut folders = std::collections::HashMap::new();
            for file in files {
                let code = if !text(file, "worktree").trim().is_empty() {
                    text(file, "worktree")
                } else {
                    text(file, "index")
                };
                let rank = match code.trim() {
                    _ if is_unmerged(file) => 2,
                    "!" => continue,
                    "?" => 0,
                    _ => 1,
                };
                let mut path = text(file, "path");
                while let Some((folder, _)) = path.rsplit_once('/') {
                    let strongest = folders.entry(folder.to_owned()).or_insert(rank);
                    *strongest = (*strongest).max(rank);
                    path = folder;
                }
            }
            *changed.borrow_mut() = folders;
        });
        fn walk(widget: &gtk::Widget) {
            if widget.has_css_class("code-folder") {
                if let Some(path) = widget.widget_name().strip_prefix("project-file:") {
                    let status = folder_status(path);
                    if let Some(name) = tree_name(widget) {
                        for class in STATUS_CLASSES {
                            name.remove_css_class(class);
                        }
                        if let Some(class) = status {
                            name.add_css_class(class);
                        }
                    }
                }
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                walk(&next);
                child = next.next_sibling();
            }
        }
        walk(self.tree.upcast_ref());
    }
}

fn tree_name(widget: &gtk::Widget) -> Option<gtk::Label> {
    let mut child = widget.first_child();
    while let Some(next) = child {
        if next.has_css_class("tree-name") {
            return next.downcast().ok();
        }
        if let Some(found) = tree_name(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Vertical guides at each ancestor's chevron, as VS Code's explorer draws them.
pub(super) fn indent_guides(depth: usize) -> gtk::DrawingArea {
    let guides = gtk::DrawingArea::new();
    guides.set_content_width(depth as i32 * INDENT);
    guides.set_content_height(1);
    guides.add_css_class("tree-guides");
    guides.set_draw_func(move |widget, cr, _, height| {
        let color = widget.color();
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            color.alpha() as f64,
        );
        cr.set_line_width(1.0);
        for level in 0..depth {
            let x = (level as i32 * INDENT + INDENT / 2) as f64 + 0.5;
            cr.move_to(x, 0.0);
            cr.line_to(x, height as f64);
        }
        let _ = cr.stroke();
    });
    guides
}
pub(super) const INDENT: i32 = 12;
