//! Opt-in checks against the isolated native smoke engine, never installed providers.
use crate::app::Ui;
use crate::smoke::util::{choose, chosen, click, clickable, named, press, wait_for};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::rc::Rc;
use std::time::Duration;

/// Waits until the engine holds `expected` at settings `path`: Settings saves on its own, a
/// moment after the control changes.
async fn persisted(ui: &Rc<Ui>, path: &str, expected: &Value, reason: &str) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if call(ui, "settings.get", json!({"path":path})).await?["value"] == *expected {
            return Ok(());
        }
        if std::time::Instant::now() > deadline {
            return Err(format!("{reason}: {path} was not saved"));
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
}

async fn call(ui: &Rc<Ui>, op: &str, payload: Value) -> Result<Value, String> {
    ui.call(op, payload).await.map_err(|error| format!("{op}: {error}"))
}

fn selected_tasks(root: &gtk::Widget) -> usize {
    let own = usize::from(
        root.has_css_class("launch-task")
            && root
                .downcast_ref::<gtk::ToggleButton>()
                .is_some_and(|key| key.is_active()),
    );
    let mut count = own;
    let mut child = root.first_child();
    while let Some(widget) = child {
        count += selected_tasks(&widget);
        child = widget.next_sibling();
    }
    count
}

pub(crate) async fn run(ui: &Rc<Ui>) -> Result<(), String> {
    let project = ui.project.get();
    let workspace = ui
        .workspaces
        .borrow()
        .first()
        .cloned()
        .expect("Fixture workspace");
    let root = std::path::PathBuf::from(workspace["path"].as_str().unwrap())
        .join(format!("roadmap-{}", uuid::Uuid::new_v4()));
    assert!(
        root.starts_with(std::env::temp_dir()),
        "Fixture repository must be temporary"
    );
    std::fs::create_dir_all(&root).unwrap();
    for args in [
        vec!["init", "-b", "main"],
        vec![
            "-c",
            "user.name=Native Smoke",
            "-c",
            "user.email=smoke@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "Fixture",
        ],
    ] {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Fixture git: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let other = call(
        ui,
        "project.add",
        json!({"workspace_id":workspace["id"],"path":root,"name":"Roadmap scope with a deliberately very long project name that must remain inside its sidebar"}),
    )
    .await?;
    let other = other["id"].as_i64().unwrap();
    ui.refresh();
    wait_for(
        || {
            ui.projects
                .borrow()
                .iter()
                .any(|p| p["id"].as_i64() == Some(other))
        },
        "second fixture project",
    )
    .await?;
    let task = call(
        ui,
        "task.create",
        json!({"project_id":project,"title":"Roadmap launch selection","column":"ready"}),
    )
    .await?;
    ui.navigate("agents");
    ui.show_launch(task["id"].as_i64());
    wait_for(
        || named(&ui.window, "launch-start").is_some_and(|w| w.is_sensitive()),
        "launch tasks",
    )
    .await?;
    for index in 0..6 {
        let profile = named(&ui.window, &format!("launch-profile-{index}")).unwrap();
        assert_eq!(
            selected_tasks(&profile),
            usize::from(index == 0),
            "Only agent1 gets an initial assignment"
        );
        assert_eq!(chosen(&profile, &format!("launch-provider-{index}"))?, 0, "Every agent defaults to Claude");
    }
    // Pressed as a person would: max is offered for Claude only and minimal for Codex only.
    choose(&ui.window, "launch-effort-0", 5)?;
    choose(&ui.window, "launch-provider-0", 1)?;
    assert_eq!(chosen(&ui.window, "launch-effort-0")?, 3, "Codex cannot inherit max effort");
    choose(&ui.window, "launch-effort-0", 0)?;
    choose(&ui.window, "launch-provider-0", 0)?;
    assert_eq!(chosen(&ui.window, "launch-effort-0")?, 3, "Claude cannot inherit minimal effort");
    let model = named(&ui.window, "launch-model-0")
        .unwrap()
        .downcast::<gtk::Entry>()
        .unwrap();
    assert!(
        model.is_mapped(),
        "Model selection is visible without expanding advanced controls"
    );
    model.set_text("fixture-model-id");
    let stale_submit = named(&ui.window, "launch-start")
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    let before = call(ui, "session.list", json!({"project_id":project})).await?["sessions"]
        .as_array()
        .unwrap()
        .len();
    ui.open_project(other, "agents");
    assert!(ui.sessions.borrow().is_empty(), "Old project sessions must disappear before the next bus response");
    ui.verify_shell();
    // Deliberately raw: this replays a click that was already in flight when the project
    // switched, which no sensitivity check could have stopped.
    stale_submit.emit_clicked();
    glib::timeout_future(Duration::from_millis(120)).await;
    assert_eq!(
        call(ui, "session.list", json!({"project_id":project})).await?["sessions"]
            .as_array()
            .unwrap()
            .len(),
        before,
        "Old launch callback cannot create a session after switching projects"
    );
    assert!(
        call(ui, "session.list", json!({"project_id":other})).await?["sessions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    ui.open_project(project, "agents");

    // Add an agent with the rail's key after show_launch has returned: the crew must reach
    // the launch, not just the rail (checked by the session count below).
    ui.show_launch(None);
    wait_for(|| named(&ui.window, "launch-start").is_some_and(|w| w.is_sensitive()), "multi-agent form").await?;
    click(&ui.window, "launch-add")?;
    let submit = named(&ui.window, "launch-start").unwrap().downcast::<gtk::Button>().unwrap();
    let started = std::time::Instant::now();
    press(&submit, "launch-start")?;
    wait_for(|| submit.is_sensitive(), "two-agent launch").await?;
    let all = call(ui, "session.list", json!({"project_id":project})).await?;
    assert_eq!(all["sessions"].as_array().unwrap().len(), before + 2, "The add key must launch two agents");
    // Both new sessions are running even if event refresh already cached them.
    assert!(all["sessions"].as_array().unwrap().iter().all(|s| s["state"] != "created"));
    println!("TWO_AGENT_LAUNCH_MS={}", started.elapsed().as_millis());
    let before = before + 2;

    // Revisit unchanged sessions: their widgets were destroyed while the render signatures
    // used to survive. A fresh pane must initialize its header and dismiss the blank slate.
    for _ in 0..3 {
        wait_for(|| ui.sessions.borrow().len() == before, "returning session panels").await?;
        ui.verify_shell();
        ui.open_project(other, "agents");
        wait_for(|| ui.sessions.borrow().is_empty(), "empty project panels").await?;
        ui.open_project(project, "agents");
    }
    wait_for(|| ui.sessions.borrow().len() == before, "final session panels").await?;
    ui.verify_shell();
    wait_for(|| ui.terminal_contents_contain("RELAY NATIVE VERIFICATION"), "terminal output after project switches").await?;
    println!("TERMINAL_PROJECT_ROUNDTRIPS=4");

    call(ui, "git.branch.create", json!({"project_id":other,"name":"switch-fixture","checkout":false})).await?;
    // An explicit destination survives the project's layout restore; the Git panel is drawn
    // only while it is shown.
    ui.open_project(other, "code");
    let git_hidden = ui.editor.layout_state()["git"] != true;
    if git_hidden {
        ui.editor.toggle_git();
    }
    // The switch keys live in the branch picker's popover: open it, as a person would.
    wait_for(|| named(&ui.window, "git-branch-picker").is_some_and(|w| w.is_mapped()), "branch picker").await?;
    named(&ui.window, "git-branch-picker")
        .and_then(|w| w.downcast::<gtk::MenuButton>().ok())
        .ok_or("Branch picker type")?
        .popup();
    wait_for(|| clickable(&ui.window, "branch-switch-switch-fixture"), "branch switch control").await?;
    assert!(!ui.editor.agents_visible(), "Files and Git must stay open after the layout restore");
    click(&ui.window, "branch-switch-switch-fixture")?;
    let mut switched = false;
    for _ in 0..100 {
        if call(ui, "git.status", json!({"project_id":other})).await?["branch"] == "switch-fixture" {
            switched = true;
            break;
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    assert!(switched, "Switch button must check out its branch");
    if git_hidden {
        ui.editor.toggle_git();
    }
    println!("BRANCH_SWITCH_CONTROL=ok");
    ui.open_project(project, "agents");

    crate::tools::devices::verify_worktree_picker(ui).await;
    let skill = call(
        ui,
        "skill.create",
        json!({"name":"Roadmap scope fixture","body":"# Scope fixture\nInstructions."}),
    )
    .await?;
    for project_id in [project, other] {
        call(
            ui,
            "skill.enable",
            json!({"skill_id":skill["id"],"project_id":project_id,"enabled":true}),
        )
        .await?;
    }
    ui.navigate("skills");
    wait_for(
        || named(&ui.window, "skills-project").is_some(),
        "Skills project picker",
    )
    .await?;
    let split = named(&ui.window, "skills-split")
        .unwrap()
        .downcast::<gtk::Paned>()
        .unwrap();
    let position = split.position();
    split.set_position(position + 40);
    // Read back the layout, not the property just set: the library really is that wide.
    let library = split.start_child().ok_or("Skills split has no library")?;
    wait_for(|| library.width() == position + 40, "Skills split resized").await?;
    assert!(split.vexpands());
    let picker = named(&ui.window, "skills-project")
        .unwrap()
        .downcast::<gtk::DropDown>()
        .unwrap();
    let other_index = ui
        .projects
        .borrow()
        .iter()
        .position(|p| p["id"].as_i64() == Some(other))
        .unwrap();
    picker.set_selected(other_index as u32);
    let picker = named(&ui.window, "skills-project")
        .unwrap()
        .downcast::<gtk::DropDown>()
        .unwrap();
    assert!(
        picker.measure(gtk::Orientation::Horizontal, -1).0 < 220,
        "Long project names must ellipsize inside the Skills library"
    );
    let toggle_name = format!("skill-enabled-{}", skill["id"]);
    let toggle = named(&ui.window, &toggle_name)
        .unwrap()
        .downcast::<gtk::Switch>()
        .unwrap();
    assert!(toggle.is_active());
    toggle.set_active(false);
    wait_for(|| toggle.is_sensitive(), "Skills enablement save").await?;
    let listed = call(ui, "skill.list", json!({})).await?;
    let listed = listed["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == skill["id"])
        .unwrap();
    assert!(listed["enabled_in"]
        .as_array()
        .unwrap()
        .contains(&json!(project)));
    assert!(
        !listed["enabled_in"]
            .as_array()
            .unwrap()
            .contains(&json!(other)),
        "Project picker must bind the toggle to its chosen project"
    );

    let paths = [
        "appearance.wallpapers",
        "appearance.wallpaper",
        "appearance.wallpaper_rotation",
        "appearance.panel_alpha",
    ];
    let mut saved = Vec::new();
    for path in paths {
        saved.push((
            path,
            call(ui, "settings.get", json!({"path":path})).await?["value"].clone(),
        ));
    }
    let library = crate::wallpaper_rotation::library_or_defaults(&Value::Null);
    let presets = library.as_array().unwrap();
    assert_eq!(presets.len(), 3);
    let mut decoded = Vec::new();
    for preset in presets {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(
                preset["image"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("data:image/png;base64,")
                    .unwrap(),
            )
            .unwrap();
        assert!(
            !decoded.contains(&bytes),
            "Bundled wallpapers must be distinct"
        );
        let texture = gtk::gdk::Texture::from_bytes(&glib::Bytes::from(bytes.as_slice())).unwrap();
        assert_eq!((texture.width(), texture.height()), (384, 240));
        decoded.push(bytes);
    }
    let second = presets[1]["image"].clone();
    call(
        ui,
        "settings.set",
        json!({"path":"appearance.wallpapers","value":null}),
    )
    .await?;
    call(
        ui,
        "settings.set",
        json!({"path":"appearance.wallpaper","value":null}),
    )
    .await?;
    call(ui, "settings.set", json!({"path":"appearance.wallpaper_rotation","value":{"enabled":false,"interval_minutes":15}})).await?;
    ui.page_projects.borrow_mut().remove("settings");
    ui.navigate("settings");
    wait_for(
        || named(&ui.window, "settings-wallpaper-pick-2").is_some(),
        "Settings offers three bundled wallpapers for an unset library",
    )
    .await?;
    assert!(named(&ui.window, "settings-wallpaper-pick-3").is_none());
    assert!(
        call(ui, "settings.get", json!({"path":"appearance.wallpapers"})).await?["value"].is_null()
    );
    assert!(
        call(ui, "settings.get", json!({"path":"appearance.wallpaper"})).await?["value"].is_null(),
        "Opening Settings keeps an unset background plain"
    );
    let search = named(&ui.window, "settings-search")
        .unwrap()
        .downcast::<gtk::SearchEntry>()
        .unwrap();
    search.set_text("Android SDK");
    wait_for(
        || named(&ui.window, "setting:device.sdk_path").is_some_and(|w| w.is_mapped()),
        "Settings search finds Android field",
    )
    .await?;
    search.set_text("wallpaper");
    wait_for(
        || named(&ui.window, "settings-wallpaper-pick-1").is_some_and(|w| w.is_mapped()),
        "Settings search returns Appearance",
    )
    .await?;
    let alpha = named(&ui.window, "setting:appearance.panel_alpha")
        .unwrap()
        .downcast::<gtk::Scale>()
        .unwrap();
    alpha.set_value(0.81);
    click(&ui.window, "settings-wallpaper-pick-1")?;
    assert_eq!(
        alpha.value(),
        0.81,
        "Wallpaper selection preserves another staged field"
    );
    persisted(ui, "appearance.wallpaper", &second, "Picking a wallpaper saves it").await?;
    assert!(named(&ui.window, "settings-wallpaper-preview").is_some());
    click(&ui.window, "settings-wallpaper-open")?;
    let preview = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == presets[1]["name"].as_str())
        .expect("Full wallpaper preview window");
    let picture = preview.child().unwrap().downcast::<gtk::Picture>().unwrap();
    assert_eq!(
        picture.content_fit(),
        gtk::ContentFit::Contain,
        "Full preview must show the entire image"
    );
    assert!(picture.paintable().is_some());
    preview.close();
    let rotation = named(&ui.window, "setting:appearance.wallpaper_rotation.enabled")
        .unwrap()
        .downcast::<gtk::Switch>()
        .unwrap();
    rotation.set_active(true);
    persisted(ui, "appearance.wallpaper_rotation.enabled", &json!(true), "Rotation switch").await?;
    persisted(ui, "appearance.wallpapers", &library, "Saving persists the offered preset library").await?;
    persisted(ui, "appearance.wallpaper", &second, "Wallpaper pick").await?;
    persisted(ui, "appearance.panel_alpha", &json!(0.81), "Panel opacity").await?;
    glib::timeout_future(Duration::from_millis(120)).await;
    assert!(
        crate::wallpaper_rotation::rotate_once(ui).await.unwrap(),
        "Enabled rotation must select a distinct next image"
    );
    // A rotation paints this window only; the saved wallpaper, and the audit table, are untouched.
    let rotated = crate::wallpaper_rotation::shown(ui, &second);
    assert_ne!(rotated, second);
    assert!(
        presets.iter().any(|preset| preset["image"] == rotated),
        "Rotation chooses another bundled wallpaper"
    );
    assert_eq!(
        call(ui, "settings.get", json!({"path":"appearance.wallpaper"})).await?["value"],
        second,
        "Rotation must not rewrite the saved wallpaper"
    );
    assert_eq!(
        call(ui, "settings.get", json!({"path":"appearance.panel_alpha"})).await?["value"],
        0.81,
        "Rotation must not overwrite appearance edits"
    );
    for (path, value) in saved {
        call(ui, "settings.set", json!({"path":path,"value":value})).await?;
    }
    call(ui, "skill.delete", json!({"skill_id":skill["id"]})).await?;
    call(ui, "project.remove", json!({"project_id":other})).await?;
    std::fs::remove_dir_all(root).unwrap();
    ui.page_projects.borrow_mut().remove("settings");
    ui.open_project(project, "agents");
    println!("ROADMAP_TOOLS_OK: launch scope, all Claude, provider effort, device selector, skills project, settings search, wallpaper autosave, presets and rotation");
    Ok(())
}
