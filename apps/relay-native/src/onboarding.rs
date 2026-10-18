use super::*;

struct Setup {
    ui: Rc<Ui>,
    panel: std::rc::Weak<crate::panel::Panel>,
    body: gtk::Box,
    workspace: RefCell<Value>,
    /// Relay has no project yet: the sheet is the first run, which ends in "Open Relay".
    first: bool,
    busy: Cell<bool>,
    message: gtk::Label,
    /// What the last scan of the workspace directory had to say, shown on the Local tab.
    scan_note: RefCell<String>,
    repos: RefCell<Vec<Value>>,
    choices: gtk::ComboBoxText,
    search: gtk::SearchEntry,
    connect: gtk::Button,
    add: gtk::Button,
    github: Cell<bool>,
    local: gtk::Entry,
    destination: gtk::Entry,
    workspace_path: gtk::Entry,
    workspace_name: gtk::Entry,
    project_name: gtk::Entry,
    import_path: gtk::Entry,
    created_project: RefCell<Option<Value>>,
    skip_import: gtk::Button,
    discovery_generation: Cell<u64>,
    /// The repository whose name `destination` was last filled from, so re-filtering the list
    /// keeps a folder name the user typed.
    destination_for: RefCell<Option<String>>,
}

pub fn open(ui: &Rc<Ui>, workspace: Option<Value>) {
    let first = ui.projects.borrow().is_empty();
    let title = if let Some(workspace) = &workspace {
        format!("Add project to {}", text(workspace, "name"))
    } else if first {
        "Build the first workspace".into()
    } else {
        "Add workspace".into()
    };
    let panel = if first {
        crate::panel::Panel::page(ui, &title)
    } else {
        let panel = crate::panel::Panel::new(ui, &title, 820);
        panel.centered(700);
        panel
    };
    panel.add_css_class("first-run");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
    body.set_halign(gtk::Align::Center);
    body.set_hexpand(true);
    body.set_size_request(
        if first {
            (ui.page_overlay.width() - 36).clamp(580, 844)
        } else {
            744
        },
        -1,
    );
    body.set_valign(gtk::Align::Center);
    body.add_css_class("setup-body");
    panel.body.append(&body);
    let setup = Rc::new(Setup {
        ui: ui.clone(),
        panel: Rc::downgrade(&panel),
        body,
        workspace: RefCell::new(Value::Null),
        first,
        busy: Cell::new(false),
        message: label("", "dim"),
        scan_note: RefCell::default(),
        repos: RefCell::default(),
        choices: gtk::ComboBoxText::new(),
        search: gtk::SearchEntry::new(),
        connect: button("Connect GitHub", "quiet"),
        add: button("Add project", "primary"),
        github: Cell::new(false),
        local: gtk::Entry::new(),
        destination: gtk::Entry::new(),
        workspace_path: gtk::Entry::builder()
            .text(
                std::env::current_dir()
                    .and_then(std::fs::canonicalize)
                    .map(|p| suggested_workspace(&p).to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
            .build(),
        workspace_name: gtk::Entry::new(),
        project_name: gtk::Entry::new(),
        import_path: gtk::Entry::new(),
        created_project: RefCell::new(None),
        skip_import: button("Skip import", "quiet"),
        discovery_generation: Cell::new(0),
        destination_for: RefCell::new(None),
    });
    setup.connect.set_widget_name("setup-connect");
    setup.message.set_wrap(true);
    setup.message.set_visible(false);
    setup
        .message
        .connect_label_notify(|message| message.set_visible(!message.text().is_empty()));
    setup.message.set_max_width_chars(65);
    let weak = Rc::downgrade(&setup);
    panel.set_guard(move || weak.upgrade().is_none_or(|s| !s.busy.get()));
    let keep = setup.clone();
    panel.on_closed(move || {
        clear(&keep.body);
    });
    if let Some(workspace) = workspace {
        *setup.workspace.borrow_mut() = workspace;
    }
    setup.project_step();
    panel.present();
}

impl Setup {
    fn project_step(self: &Rc<Self>) {
        clear(&self.body);
        let new_workspace = self.workspace.borrow()["id"].is_null();
        let intro = label(
            if new_workspace {
                "Relay detected the current directory. Pick an existing local repository, or connect GitHub and clone one into this workspace."
            } else {
                "Register another repository in this workspace. Pick one already on disk or clone it from GitHub."
            },
            "dim",
        );
        intro.set_wrap(true);
        self.body.append(&intro);
        let found = gtk::ComboBoxText::new();
        found.set_widget_name("setup-local-repos");
        if new_workspace {
            let providers = gtk::Box::new(gtk::Orientation::Horizontal, 14);
            providers.add_css_class("setup-providers");
            self.body.append(&providers);
            let ui = self.ui.clone();
            let target = providers.downgrade();
            glib::spawn_future_local(async move {
                if let Ok(value) = ui.call("provider.list", json!({})).await {
                    if let Some(target) = target.upgrade() {
                        for item in rows(&value, "providers") {
                            let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
                            row.append(&label(text(&item, "provider"), "body"));
                            row.append(&label(
                                item["signed_in_as"].as_str().unwrap_or(
                                    if item["installed"] == true {
                                        "installed, sign-in required"
                                    } else {
                                        "not installed"
                                    },
                                ),
                                "faint",
                            ));
                            target.append(&row);
                        }
                    }
                }
            });
            let fields = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            fields.set_valign(gtk::Align::End);
            let path = gtk::Box::new(gtk::Orientation::Vertical, 6);
            path.set_hexpand(true);
            self.workspace_path.set_widget_name("setup-path");
            field("Workspace directory", &self.workspace_path, &path);
            fields.append(&path);
            let scan = button("Scan", "quiet");
            scan.set_widget_name("setup-scan");
            scan.set_valign(gtk::Align::End);
            fields.append(&scan);
            let name = gtk::Box::new(gtk::Orientation::Vertical, 6);
            name.set_size_request(230, -1);
            self.workspace_name.set_widget_name("setup-name");
            self.workspace_name
                .set_placeholder_text(Some("Quiet Software"));
            field("Workspace name · optional", &self.workspace_name, &name);
            fields.append(&name);
            self.body.append(&fields);
            let weak = Rc::downgrade(self);
            let choices = found.clone();
            scan.connect_clicked(move |_| {
                if let Some(setup) = weak.upgrade() {
                    setup.discover(&choices);
                }
            });
            // Enter in the directory scans it, as Scan does.
            let scan = scan.downgrade();
            self.workspace_path.connect_activate(move |_| {
                if let Some(scan) = scan.upgrade().filter(|scan| scan.is_sensitive()) {
                    scan.emit_clicked();
                }
            });
        }
        let tabs = gtk::Stack::new();
        tabs.set_widget_name("setup-source");
        tabs.set_vhomogeneous(false);
        tabs.set_vexpand(false);
        let switch = gtk::StackSwitcher::new();
        switch.set_stack(Some(&tabs));
        switch.set_halign(gtk::Align::Start);
        self.body.append(&switch);
        let local = gtk::Box::new(gtk::Orientation::Vertical, 12);
        local.add_css_class("setup-repo-panel");
        self.local.set_widget_name("setup-local-path");
        let repository = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let choices = gtk::Box::new(gtk::Orientation::Vertical, 6);
        choices.set_hexpand(true);
        field("Repository", &found, &choices);
        repository.append(&choices);
        let project_name = gtk::Box::new(gtk::Orientation::Vertical, 6);
        project_name.set_size_request(230, -1);
        self.project_name
            .set_placeholder_text(Some("Uses repository name"));
        field("Project name · optional", &self.project_name, &project_name);
        repository.append(&project_name);
        local.append(&repository);
        let other = gtk::Expander::new(Some("Choose another folder"));
        let manual = gtk::Box::new(gtk::Orientation::Vertical, 6);
        field("Repository folder", &self.local, &manual);
        browse(&self.ui, &manual, &self.local, "Choose repository");
        other.set_child(Some(&manual));
        local.append(&other);
        let entry = self.local.clone();
        found.connect_changed(move |c| {
            if let Some(path) = c.active_id() {
                entry.set_text(&path);
            }
        });
        let github = gtk::Box::new(gtk::Orientation::Vertical, 12);
        github.add_css_class("setup-repo-panel");
        github.append(&self.connect);
        self.search
            .set_placeholder_text(Some("Search your repositories"));
        github.append(&self.search);
        self.choices.set_widget_name("setup-github-repos");
        field("Repository", &self.choices, &github);
        self.destination.set_widget_name("setup-destination");
        let advanced = gtk::Expander::new(Some("Clone folder name"));
        advanced.set_child(Some(&self.destination));
        github.append(&advanced);
        tabs.add_titled(&local, Some("local"), "Local");
        tabs.add_titled(&github, Some("github"), "GitHub");
        self.body.append(&tabs);
        self.message.set_text("");
        self.body.append(&self.message);
        if new_workspace {
            self.import_path
                .set_placeholder_text(Some("/home/you/dev/Relay/.relay"));
            field(
                "Relay v3 data directory · optional",
                &self.import_path,
                &self.body,
            );
        }
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let hint = label("Local choices must already contain .git. GitHub choices are cloned into this workspace.","dim");
        hint.set_wrap(true);
        hint.set_hexpand(true);
        footer.append(&hint);
        self.skip_import.set_visible(false);
        footer.append(&self.skip_import);
        footer.append(&self.add);
        self.add.set_valign(gtk::Align::Center);
        self.body.append(&footer);
        let weak = Rc::downgrade(self);
        self.skip_import.connect_clicked(move |_| {
            if let Some(setup) = weak.upgrade() {
                setup.import_path.set_text("");
                setup.submit();
            }
        });
        self.add.set_widget_name("setup-add");
        let weak = Rc::downgrade(self);
        tabs.connect_visible_child_name_notify(move |t| {
            let Some(setup) = weak.upgrade() else {
                return;
            };
            let gh = t.visible_child_name().as_deref() == Some("github");
            setup.github.set(gh);
            setup.update_action();
            if gh {
                setup.load_github();
            } else {
                setup.message.set_text(&setup.scan_note.borrow());
            }
        });
        let weak = Rc::downgrade(self);
        self.search.connect_search_changed(move |_| {
            if let Some(setup) = weak.upgrade() {
                setup.filter_repos();
            }
        });
        let weak = Rc::downgrade(self);
        self.choices.connect_changed(move |c| {
            let Some(setup) = weak.upgrade() else {
                return;
            };
            // Searching rebuilds the list and reselects the same repository; only a different
            // one replaces the folder name.
            let id = c.active_id().map(|id| id.to_string());
            if id.is_some() && *setup.destination_for.borrow() != id {
                let repo = id.as_deref().and_then(|id| {
                    setup
                        .repos
                        .borrow()
                        .iter()
                        .find(|r| text(r, "full_name") == id)
                        .cloned()
                });
                setup
                    .destination
                    .set_text(repo.as_ref().map(|r| text(r, "name")).unwrap_or(""));
                *setup.destination_for.borrow_mut() = id;
            }
            setup.update_action();
        });
        let weak = Rc::downgrade(self);
        self.local.connect_changed(move |_| {
            if let Some(setup) = weak.upgrade() {
                setup.update_action();
            }
        });
        let weak = Rc::downgrade(self);
        self.destination.connect_changed(move |_| {
            if let Some(setup) = weak.upgrade() {
                setup.update_action();
            }
        });
        let weak = Rc::downgrade(self);
        self.connect.connect_clicked(move |_| {
            if let Some(setup) = weak.upgrade() {
                setup.connect_github();
            }
        });
        let weak = Rc::downgrade(self);
        self.add.connect_clicked(move |_| {
            if let Some(setup) = weak.upgrade() {
                setup.submit();
            }
        });
        self.update_action();
        self.discover(&found);
    }

    fn discover(self: &Rc<Self>, found: &gtk::ComboBoxText) {
        found.remove_all();
        self.local.set_text("");
        let generation = self.discovery_generation.get().wrapping_add(1);
        self.discovery_generation.set(generation);
        // An earlier scan's error describes a directory that is no longer the one shown.
        self.scan_note.borrow_mut().clear();
        if !self.github.get() {
            self.message.set_text("");
        }
        let path = self.workspace.borrow()["path"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| self.workspace_path.text().trim().to_owned());
        let setup = self.clone();
        let found = found.clone();
        glib::spawn_future_local(async move {
            let result = setup
                .ui
                .call("workspace.discover", json!({"path":path}))
                .await;
            // A newer scan must not be replaced by a result, or an error, for an older directory.
            let current = setup.workspace.borrow()["path"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| setup.workspace_path.text().trim().to_owned());
            if current != path || setup.discovery_generation.get() != generation {
                return;
            }
            let note = match result {
                Ok(v) => {
                    let mut listed = 0;
                    for repo in rows(&v, "repositories").into_iter().filter(|r| {
                        !setup
                            .ui
                            .projects
                            .borrow()
                            .iter()
                            .any(|p| p["path"] == r["path"])
                    }) {
                        found.append(Some(text(&repo, "path")), text(&repo, "name"));
                        listed += 1;
                    }
                    found.set_active(Some(0));
                    // An empty picker and an Add key that stays off would say nothing.
                    if listed == 0 {
                        let place = if path.is_empty() { String::from("this directory") } else { path.clone() };
                        format!("No Git repositories found in {place} that are not already in Relay. Choose another folder, or clone one from GitHub.")
                    } else {
                        String::new()
                    }
                }
                Err(e) => e.to_string(),
            };
            if !setup.github.get() {
                setup.message.set_text(&note);
            }
            *setup.scan_note.borrow_mut() = note;
        });
    }

    fn update_action(&self) {
        self.add
            .set_label(if self.created_project.borrow().is_some() {
                "Retry import"
            } else if self.workspace.borrow()["id"].is_null() {
                if self.first { "Open Relay" } else { "Add workspace" }
            } else {
                "Add project"
            });
        self.add.set_sensitive(
            !self.busy.get()
                && if self.github.get() {
                    self.choices.active_id().is_some() && !self.destination.text().trim().is_empty()
                } else {
                    !self.local.text().trim().is_empty()
                },
        );
    }
    fn filter_repos(&self) {
        let previous = self.choices.active_id();
        self.choices.remove_all();
        let query = self.search.text().to_lowercase();
        for repo in self
            .repos
            .borrow()
            .iter()
            .filter(|r| text(r, "full_name").to_lowercase().contains(&query))
        {
            self.choices.append(
                Some(text(repo, "full_name")),
                &format!(
                    "{}{}{}",
                    text(repo, "full_name"),
                    if repo["private"] == true {
                        " · private"
                    } else {
                        ""
                    },
                    if repo["archived"] == true {
                        " · archived"
                    } else {
                        ""
                    }
                ),
            );
        }
        if !previous.is_some_and(|id| self.choices.set_active_id(Some(&id))) {
            self.choices.set_active(Some(0));
        }
        self.update_action();
    }
    fn load_github(self: &Rc<Self>) {
        self.connect.set_sensitive(false);
        self.message.set_text("Checking GitHub…");
        let setup = self.clone();
        glib::spawn_future_local(async move {
            let result=async {
                let status=setup.ui.call("github.status",json!({})).await?;
                if status["connected"]!=true {
                    setup.repos.borrow_mut().clear(); setup.filter_repos();
                    setup.message.set_text(if status["installed"]==true {"Connect GitHub to browse your repositories. Local repositories also work without an account."} else {"Install the GitHub CLI (gh), then connect your account. You can use a local repository now."});
                    setup.connect.set_label("Connect GitHub"); return Ok::<(),Error>(());
                }
                setup.connect.set_label("Refresh repositories");
                let repos=setup.ui.call("github.repo.list",json!({})).await?;
                *setup.repos.borrow_mut()=rows(&repos,"repositories"); setup.filter_repos();
                let count = setup.repos.borrow().len();
                setup.message.set_text(&format!("Connected as {} · {count} {}",text(&status,"login"),if count == 1 {"repository"} else {"repositories"})); Ok(())
            }.await;
            if let Err(e) = result {
                setup.message.set_text(&e.to_string());
            }
            setup.connect.set_sensitive(true);
        });
    }
    fn connect_github(self: &Rc<Self>) {
        if self.connect.label().as_deref() == Some("Refresh repositories") {
            self.load_github();
            return;
        }
        self.connect.set_sensitive(false);
        self.message.set_text(
            "Complete GitHub sign-in in your browser. The device code is copied to your clipboard.",
        );
        let setup = self.clone();
        let task = glib::spawn_future_local(async move {
            let result = async {
                let (client, notices) =
                    Client::connect(&setup.ui.rt, setup.ui.path.clone()).await?;
                client
                    .request(
                        &setup.ui.rt,
                        "bus.subscribe",
                        json!({"events":["github.changed"]}),
                    )
                    .await?;
                client
                    .request(&setup.ui.rt, "github.connect", json!({}))
                    .await?;
                while let Ok(notice) = notices.recv().await {
                    match notice {
                        Notice::Event(e) if e.ev == "github.changed" => {
                            setup.load_github();
                            return Ok::<(), Error>(());
                        }
                        Notice::Disconnected(e) => return Err(e),
                        _ => {}
                    }
                }
                Err(Error::Disconnected)
            }
            .await;
            if let Err(e) = result {
                setup.message.set_text(&e.to_string());
                setup.connect.set_sensitive(true);
            }
        });
        if let Some(panel) = self.panel.upgrade() {
            panel.on_closed(move || task.abort());
        }
    }
    fn submit(self: &Rc<Self>) {
        if self.busy.replace(true) {
            return;
        }
        self.update_action();
        self.body.set_sensitive(false);
        let gh = self.github.get();
        self.message.set_text(if gh {
            "Cloning repository…"
        } else {
            "Opening repository…"
        });
        let setup = self.clone();
        let selected = self.choices.active_id().and_then(|id| {
            self.repos
                .borrow()
                .iter()
                .find(|r| text(r, "full_name") == id)
                .cloned()
        });
        let mut payload = json!({});
        if gh {
            payload["url"] = selected.unwrap_or_default()["clone_url"].clone();
            payload["dest"] = json!(self.destination.text().trim());
        } else {
            payload["path"] = json!(self.local.text().trim());
            if !self.project_name.text().trim().is_empty() {
                payload["name"] = json!(self.project_name.text().trim());
            }
        }
        glib::spawn_future_local(async move {
            // A workspace this attempt created is kept only if its project is added too, so a
            // retry uses the directory and name as they are then, not as they were.
            let fresh = RefCell::new(None::<Value>);
            let result = async {
                if setup.workspace.borrow()["id"].is_null() {
                    let path = setup.workspace_path.text().trim().to_owned();
                    if !std::path::Path::new(&path).is_absolute() { return Err(Error::Protocol("Choose an absolute workspace directory.".into())); }
                    let known = setup.ui.workspaces.borrow().iter().find(|w| text(w,"path")==path).cloned();
                    let workspace = if let Some(known) = known { known } else {
                        match setup.ui.call("workspace.create",json!({"path":path,"name":if setup.workspace_name.text().trim().is_empty() {None} else {Some(setup.workspace_name.text().trim().to_owned())}})).await {
                            Ok(workspace) => { *fresh.borrow_mut() = Some(workspace["id"].clone()); workspace }
                            // The same directory spelled differently (a trailing slash, a symlink).
                            Err(Error::Bus(e)) if e.code == "workspace.exists" && e.details.as_ref().is_some_and(|d| d["workspace_id"].is_i64()) => {
                                json!({"id": e.details.as_ref().map(|d| d["workspace_id"].clone()), "path": path})
                            }
                            Err(e) => return Err(e),
                        }
                    };
                    *setup.workspace.borrow_mut() = workspace;
                }
                let saved = setup.created_project.borrow().clone();
                let project = if let Some(saved) = saved { saved } else {
                    payload["workspace_id"] = setup.workspace.borrow()["id"].clone();
                    let value = setup.ui.call(if gh {"project.clone"} else {"project.add"},payload).await?;
                    let project = if gh {value["project"].clone()} else {value};
                    *setup.created_project.borrow_mut() = Some(project.clone()); project
                };
                let source = setup.import_path.text().trim().to_owned();
                if !source.is_empty() { setup.ui.call("app.import.v3",json!({"source":source,"project_id":project["id"]})).await?; }
                Ok::<Value,Error>(project)
            }.await;
            if result.is_err() && setup.created_project.borrow().is_none() {
                if let Some(id) = fresh.take() {
                    // Empty, so this never removes a project; a refusal leaves it in the sidebar.
                    let _ = setup.ui.call("workspace.remove", json!({"workspace_id": id})).await;
                    *setup.workspace.borrow_mut() = Value::Null;
                }
            }
            setup.busy.set(false);
            setup.body.set_sensitive(true);
            setup.update_action();
            match result {
                Ok(v) => {
                    let project = &v;
                    if let Some(panel) = setup.panel.upgrade() {
                        panel.close();
                    }
                    setup.ui.registry_dirty.set(true);
                    setup
                        .ui
                        .open_project(project["id"].as_i64().unwrap_or(0), "agents");
                    setup.ui.refresh();
                }
                Err(e) => {
                    setup.message.set_text(&e.to_string());
                    setup
                        .skip_import
                        .set_visible(setup.created_project.borrow().is_some());
                }
            }
        });
    }
}

/// The workspace to offer for a GUI started in `cwd`: when that directory lies inside a Git
/// checkout, the checkout's parent, so discovery lists the repository instead of the workspace
/// becoming the repository itself (the engine applies the same rule to an empty path, but to
/// its own working directory, not this process's).
fn suggested_workspace(cwd: &std::path::Path) -> std::path::PathBuf {
    for ancestor in cwd.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor.parent().unwrap_or(ancestor).to_path_buf();
        }
    }
    cwd.to_path_buf()
}

fn browse(ui: &Rc<Ui>, body: &gtk::Box, entry: &gtk::Entry, title: &'static str) {
    let key = button("Browse…", "quiet");
    body.append(&key);
    let weak = Rc::downgrade(ui);
    let entry = entry.clone();
    key.connect_clicked(move |_| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let entry = entry.clone();
        glib::spawn_future_local(async move {
            let dialog = gtk::FileDialog::builder().title(title).build();
            if let Ok(file) = dialog.select_folder_future(Some(&ui.window)).await {
                if let Some(path) = file.path() {
                    entry.set_text(&path.to_string_lossy());
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::suggested_workspace;

    #[test]
    fn a_cwd_inside_a_checkout_suggests_the_checkout_parent() {
        let root = std::env::temp_dir().join(format!("relay-onboarding-{}", std::process::id()));
        let repo = root.join("repo");
        let deep = repo.join("target").join("debug");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        assert_eq!(suggested_workspace(&deep), root);
        assert_eq!(suggested_workspace(&repo), root);
        let _ = std::fs::remove_dir_all(&root);
    }
}
