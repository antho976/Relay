//! Opt-in screenshot run. The only clock in the client is this verification path.
use crate::app::Ui;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::json;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::time::{Duration, Instant};
use util::{click, clickable, named, require, wait_for, wait_within};
#[path = "smoke_notes.rs"]
mod notes;
#[path = "smoke_util.rs"]
pub(crate) mod util;

thread_local! {
    /// Checks still running in detached futures. The capture waits for them, so a check
    /// that hangs fails the run instead of being dropped when the window closes (RA-718).
    static PENDING: RefCell<Vec<&'static str>> = RefCell::default();
}

/// Runs `check` beside the page, and holds the capture until it has finished.
fn track(name: &'static str, check: impl Future<Output = Result<(), String>> + 'static) {
    PENDING.with(|pending| pending.borrow_mut().push(name));
    glib::spawn_future_local(async move {
        if let Err(error) = check.await {
            panic!("{name}: {error}");
        }
        PENDING.with(|pending| {
            let mut pending = pending.borrow_mut();
            if let Some(index) = pending.iter().position(|n| *n == name) {
                pending.remove(index);
            }
        });
    });
}

/// A control whose pointer target can be measured: shown, enabled and allocated.
fn allocated(ui: &Ui, name: &str) -> bool {
    named(&ui.window, name).is_some_and(|w| w.is_mapped() && w.is_sensitive() && w.width() > 0)
}

fn record_geometry(ui: &Ui, screenshot: &str) {
    if let Ok(requested) = std::env::var("RELAY_NATIVE_PAGE") {
        if matches!(
            requested.as_str(),
            "board" | "modules" | "settings" | "skills"
        ) {
            assert_eq!(
                *ui.page.borrow(),
                requested,
                "Capture must show the requested page"
            );
        }
    }
    fn collect(
        widget: &gtk::Widget,
        window: &gtk::ApplicationWindow,
        rows: &mut Vec<serde_json::Value>,
    ) {
        if !widget.is_mapped() {
            return;
        }
        if let Some(bounds) = widget.compute_bounds(window) {
            rows.push(json!({
                "type": widget.type_().name(), "name": widget.widget_name().to_string(),
                "classes": widget.css_classes().iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                "text": widget.downcast_ref::<gtk::Label>().map(|l| l.text().to_string()),
                "x": bounds.x(), "y": bounds.y(), "width": bounds.width(), "height": bounds.height()
            }));
        }
        let mut child = widget.first_child();
        while let Some(item) = child {
            child = item.next_sibling();
            collect(&item, window, rows);
        }
    }
    let context = ui.window.pango_context();
    let font = context
        .load_font(&gtk::pango::FontDescription::from_string("Fira Mono 10"))
        .expect("Bundled monospace font must load");
    assert_eq!(
        font.describe().family().as_deref(),
        Some("Fira Mono"),
        "Do not silently substitute a different terminal font"
    );
    let mut widgets = Vec::new();
    collect(ui.window.upcast_ref(), &ui.window, &mut widgets);
    // DESIGN.md's shell: each bar's height and its 1px rule.
    for (class, dimension, expected) in [
        ("topbar", "height", 53.),
        ("statusbar", "height", 31.),
        ("umd", "height", 26.),
    ] {
        for widget in &widgets {
            if widget["classes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c == class)
            {
                assert_eq!(
                    widget[dimension].as_f64(),
                    Some(expected),
                    "Reference shell geometry: {class}"
                );
            }
        }
    }
    if *ui.page.borrow() == "settings" {
        assert!(
            !widgets.iter().any(|w| w["classes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c == "sidebar")),
            "Settings owns the full workspace"
        );
    }
    let path = std::path::Path::new(screenshot).with_extension("geometry.json");
    std::fs::write(path, serde_json::to_vec_pretty(&widgets).unwrap())
        .expect("Save widget geometry");
}

/// The stack of settings categories, found from one of its controls.
fn categories_stack(ui: &Ui) -> Option<gtk::Stack> {
    let save = named(&ui.window, "guardrail-save")?;
    std::iter::successors(save.parent(), |w| w.parent())
        .filter_map(|w| w.downcast::<gtk::Stack>().ok())
        .find(|stack| stack.child_by_name("safety").is_some())
}

async fn verify_settings_save(ui: Rc<Ui>) -> Result<(), String> {
    const THRESHOLD: &str = "guardrail-field:destructive_write.min_removed_pct";
    wait_for(
        || {
            named(&ui.window, "setting:appearance.panel_alpha").is_some()
                && named(&ui.window, THRESHOLD).is_some()
        },
        "Settings and guardrail fields loaded",
    )
    .await?;
    // The font size is a stepper: − value +, in steps of a quarter point.
    let font = named(&ui.window, "setting:terminal.font_size").ok_or("Font size field missing")?;
    let more = font.last_child().and_downcast::<gtk::Button>().ok_or("Font size stepper has no + key")?;
    let opacity = named(&ui.window, "setting:appearance.panel_alpha")
        .unwrap()
        .downcast::<gtk::Scale>()
        .map_err(|_| "Opacity field type")?;
    let threshold = named(&ui.window, THRESHOLD)
        .unwrap()
        .downcast::<gtk::SpinButton>()
        .map_err(|_| "Guardrail threshold type")?;
    // From 9.75, the default, which a save would rightly leave alone, to 10.25.
    more.emit_clicked();
    more.emit_clicked();
    opacity.set_value(0.96);
    // The layered editor keeps its fields off until the layers are read; a value set before
    // that is overwritten by the fill.
    wait_for(|| threshold.is_sensitive(), "Guardrail layers loaded").await?;
    threshold.set_value(62.5);
    // Guardrails save on their own: the layered editor sends only the fields that changed.
    // Its key stays disabled until a field differs, so this fails if that wiring breaks.
    // It lives on the Guardrails category, so show that first, as its category key does.
    let category = |name: &str| -> Result<(), String> {
        named(&ui.window, &format!("settings-category-{name}"))
            .and_downcast::<gtk::ToggleButton>()
            .ok_or(format!("Missing settings category: {name}"))?
            .set_active(true);
        Ok(())
    };
    category("safety")?;
    wait_for(|| clickable(&ui.window, "guardrail-save"), "Guardrail save shown and enabled").await?;
    click(&ui.window, "guardrail-save")?;
    let read = |op: &'static str, payload: serde_json::Value| {
        let ui = ui.clone();
        async move { ui.call(op, payload).await.map_err(|e| format!("{op}: {e}")) }
    };
    // Every other control saves itself a moment after it changes.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let font = read("settings.get", json!({"path":"terminal.font_size"})).await?;
        let opacity = read("settings.get", json!({"path":"appearance.panel_alpha"})).await?;
        if font["value"].as_f64() == Some(10.25) && opacity["value"].as_f64() == Some(0.96) {
            break;
        }
        require(Instant::now() < deadline, "Font size and opacity were not saved on their own")?;
        glib::timeout_future(Duration::from_millis(50)).await;
    }
    // The guardrail save runs on its own call; read until it lands, within the same budget.
    let deadline = Instant::now() + util::WAIT;
    loop {
        let guardrails = read("guardrail.config.get", json!({})).await?;
        if guardrails["destructive_write"]["min_removed_pct"].as_f64() == Some(62.5) {
            break;
        }
        require(Instant::now() < deadline, "Guardrail threshold was not saved")?;
        glib::timeout_future(Duration::from_millis(50)).await;
    }
    // The capture shows Appearance, or the category RELAY_NATIVE_SETTINGS_CATEGORY names.
    let shown = std::env::var("RELAY_NATIVE_SETTINGS_CATEGORY").unwrap_or_else(|_| "appearance".into());
    category(&shown)?;
    // RELAY_NATIVE_SETTINGS_SCROLL scrolls that category's column down by so many pixels.
    if let Some(offset) = std::env::var("RELAY_NATIVE_SETTINGS_SCROLL").ok().and_then(|v| v.parse::<f64>().ok()) {
        glib::timeout_future(Duration::from_millis(300)).await;
        let column = categories_stack(&ui).and_then(|stack| stack.child_by_name(&shown)).and_downcast::<gtk::ScrolledWindow>();
        if let Some(column) = column {
            column.vadjustment().set_value(offset);
        }
    }
    println!("Settings save verified across categories, including fractional values");
    Ok(())
}

fn verify_pointer_target(ui: &Ui, name: &str) {
    let target = named(&ui.window, name).unwrap();
    assert!(
        target.is_sensitive() && target.is_mapped(),
        "{name} must be interactive"
    );
    let bounds = target.compute_bounds(&ui.window).unwrap();
    let hit = ui
        .window
        .pick(
            (bounds.x() + bounds.width() / 2.0) as f64,
            (bounds.y() + bounds.height() / 2.0) as f64,
            gtk::PickFlags::DEFAULT,
        )
        .expect("Control center must hit a widget");
    assert!(
        hit == target || hit.is_ancestor(&target),
        "{name} is blocked by {} ({})",
        hit.type_().name(),
        hit.widget_name()
    );
    println!("Pointer target verified: {name}");
}

fn click_control(ui: &Ui, name: &str) {
    verify_pointer_target(ui, name);
    let target = named(&ui.window, name).unwrap();
    if let Some(driver) = std::env::var_os("RELAY_NATIVE_POINTER_DRIVER") {
        let bounds = target.compute_bounds(&ui.window).unwrap();
        let status = std::process::Command::new("python3")
            .arg(driver)
            .arg(ui.window.title().unwrap())
            .arg(((bounds.x() + bounds.width() / 2.0) as i32).to_string())
            .arg(((bounds.y() + bounds.height() / 2.0) as i32).to_string())
            .status()
            .unwrap();
        assert!(status.success(), "Pointer injection failed for {name}");
        println!("Mouse click sent: {name}");
    } else {
        click(&ui.window, name).unwrap_or_else(|error| panic!("{error}"));
    }
}

fn verify_control_contrast(root: &impl IsA<gtk::Widget>) {
    fn walk(widget: &gtk::Widget, highlighted: bool, light_tabs: bool) {
        let light_tabs = light_tabs
            || widget.has_css_class("utility-panel")
            || widget.has_css_class("setup-body");
        let highlighted = highlighted
            || (widget.is::<gtk::Button>() && widget.has_css_class("primary"))
            || (light_tabs
                && widget.is::<gtk::Button>()
                && widget.state_flags().contains(gtk::StateFlags::CHECKED)
                && widget
                    .parent()
                    .is_some_and(|p| p.is::<gtk::StackSwitcher>()));
        if highlighted && (widget.is::<gtk::Label>() || widget.is::<gtk::Image>()) {
            let original = widget.state_flags();
            for state in [
                gtk::StateFlags::NORMAL,
                gtk::StateFlags::BACKDROP,
                gtk::StateFlags::PRELIGHT,
                gtk::StateFlags::ACTIVE,
                gtk::StateFlags::INSENSITIVE,
            ] {
                widget.set_state_flags(original | state, true);
                let color = widget.color();
                assert!(
                    color.red().max(color.green()).max(color.blue()) < 0.3 && color.alpha() > 0.95,
                    "Light button content must stay dark in {state:?}: {color:?}"
                );
            }
            widget.set_state_flags(original, true);
        }
        let mut child = widget.first_child();
        while let Some(item) = child {
            walk(&item, highlighted, light_tabs);
            child = item.next_sibling();
        }
    }
    walk(root.as_ref(), false, false);
    println!("Primary and selected-tab contrast verified in normal, inactive, hover, pressed and disabled states");
}

fn verify_panel_toggles(ui: &Rc<Ui>) {
    fn click(ui: &Ui, name: &str) {
        util::click(&ui.window, name).unwrap_or_else(|error| panic!("{error}"));
    }
    for control in [
        "sidebar-usage-options",
        "status-resources",
        "status-devices",
        "command-palette",
        "window-presets",
    ] {
        for index in 0..50 {
            click(ui, control);
            assert_eq!(
                ui.panels.borrow().len(),
                (index + 1) % 2,
                "{control} must toggle, never stack"
            );
        }
        click(ui, control);
        click(ui, "panel-close");
        assert!(ui.panels.borrow().is_empty(), "One X closes {control}");
    }
    click(ui, "sidebar-usage-options");
    click(ui, "status-resources");
    assert_eq!(
        ui.panels.borrow().len(),
        1,
        "Different controls replace the old panel"
    );
    let guarded = ui.panels.borrow().last().unwrap().clone();
    guarded.set_guard(|| false);
    click(ui, "status-resources");
    click(ui, "window-presets");
    assert_eq!(
        ui.panels.borrow().len(),
        1,
        "Busy or dirty panels cannot be replaced"
    );
    assert!(Rc::ptr_eq(ui.panels.borrow().last().unwrap(), &guarded));
    guarded.set_guard(|| true);
    guarded.close();
    click(ui, "nav-board");
    assert_eq!(ui.page.borrow().as_str(), "board");
    click(ui, "nav-board");
    assert_eq!(ui.page.borrow().as_str(), "agents");
    println!("Panel toggles verified: 50 clicks per control, one-close, switching, guards and page toggle");
}

async fn edit_fixture(ui: Rc<Ui>, page: String) -> Result<(), String> {
    let project = ui.project.get();
    let call = |op: &'static str, payload: serde_json::Value| {
        let ui = ui.clone();
        async move { ui.call(op, payload).await.map_err(|e| format!("{op}: {e}")) }
    };
    let mut note = 0;
    if page == "board" {
        let data = call("task.list", json!({"project_id":project})).await?;
        crate::pages::open_task(&ui, data["tasks"][0]["id"].as_i64().ok_or("No fixture task")?);
    } else if page == "notes" {
        let data = call("notes.list", json!({"project_id":project})).await?;
        note = data["notes"][0]["id"].as_i64().ok_or("No fixture note")?;
        crate::pages::open_note(&ui, data["notes"][0].clone());
    } else if page == "launch" {
        let task = call(
            "task.create",
            json!({"project_id":project,"title":"Native launch queue","column":"ready"}),
        )
        .await?;
        ui.show_launch(task["id"].as_i64());
        // The form enables Start only once it has loaded; a person cannot submit it sooner.
        wait_for(|| clickable(&ui.window, "launch-start"), "Launch form loaded").await?;
        // A review group with two builders, chosen with the keys a person would press: the
        // group brings its reviewer, and the add key a second builder.
        util::choose(&ui.window, "launch-mode", 1)?;
        util::click(&ui.window, "launch-add")?;
        return click(&ui.window, "launch-start");
    }
    let field = if page == "board" { "task-title" } else { "note-body" };
    let mut found = None;
    wait_for(
        || {
            found = gtk::Window::list_toplevels()
                .into_iter()
                .find_map(|window| named(&window, field).map(|widget| (window, widget)));
            found.is_some()
        },
        &format!("{page} editor field {field}"),
    )
    .await?;
    let (window, widget) = found.unwrap();
    if page == "board" {
        // The title edits in place behind its pencil, as on a GitHub issue.
        click(&window, "task-title-edit")?;
        widget
            .downcast::<gtk::Entry>()
            .map_err(|_| "Task title type")?
            .set_text("Unsaved task fixture");
        let panel = ui
            .panels
            .borrow()
            .last()
            .cloned()
            .ok_or("Task editor is in the main window")?;
        panel.close();
        require(!ui.panels.borrow().is_empty(), "A dirty task must not close")?;
        named(&window, "task-title")
            .ok_or("Task title missing after a refused close")?
            .downcast::<gtk::Entry>()
            .map_err(|_| "Task title type")?
            .set_text("Native task edit verified");
        click(&window, "draft-save")
    } else {
        widget
            .downcast::<gtk::TextView>()
            .map_err(|_| "Note body type")?
            .buffer()
            .set_text("Native note save verified.");
        // The Notes editor saves through its window action, not Draft's key. The action is
        // enabled once the editor has seen the change, and does nothing while the note is
        // still loading, so ask again until the engine holds the new body.
        let deadline = Instant::now() + util::WAIT;
        loop {
            window
                .activate_action("notes.save", None)
                .map_err(|_| "Notes window has no save action")?;
            glib::timeout_future(Duration::from_millis(100)).await;
            let stored = call("notes.get", json!({"note_id":note})).await?;
            if stored["body"] == "Native note save verified." {
                return Ok(());
            }
            require(Instant::now() < deadline, "Notes save never reached the engine")?;
        }
    }
}

async fn verify_confirmations(ui: Rc<Ui>) -> Result<(), String> {
    for accept in [false, true] {
        let panel = crate::panel::Panel::new(&ui, "Confirmation fixture", 480);
        panel
            .body
            .append(&gtk::Label::new(Some("Fixture only. No mutation.")));
        let pending = panel.clone();
        glib::idle_add_local_once(move || {
            if accept {
                click(&pending.body, "panel-accept").unwrap_or_else(|error| panic!("{error}"));
            } else {
                pending.close();
            }
        });
        require(
            panel.response("Confirm").await == accept,
            "Confirmation returned the wrong answer",
        )?;
        require(ui.panels.borrow().is_empty(), "Confirmation left its panel open")?;
    }
    println!("In-app confirmation accept and cancel verified");
    Ok(())
}

/// Decides two held writes from their cards on the Guardrails page, as a person would
/// (RA-719): Allow once stays disabled until the exact action has been reviewed, then
/// confirms the hold; Reject… closes the other without running it.
async fn verify_guardrail_decisions(ui: Rc<Ui>) -> Result<(), String> {
    let project = ui.project.get();
    let call = |op: &'static str, payload: serde_json::Value| {
        let ui = ui.clone();
        async move { ui.call(op, payload).await.map_err(|e| format!("{op}: {e}")) }
    };
    let open = |list: &serde_json::Value| -> Vec<i64> {
        list["holds"].as_array().into_iter().flatten().filter_map(|h| h["id"].as_i64()).collect()
    };
    let sessions = call("session.list", json!({"project_id":project})).await?;
    let session = sessions["sessions"]
        .as_array()
        .and_then(|all| all.iter().find(|s| s["role"] == "builder"))
        .and_then(|s| s["name"].as_str())
        .ok_or("No fixture builder to hold")?
        .to_owned();
    call("guardrail.config.set", json!({"project_id":project,"patch":{"protected_paths":["secret/*"]}})).await?;
    let mut holds = Vec::new();
    for decision in ["allow", "reject"] {
        let path = format!("secret/native-{decision}-{}", std::process::id());
        let before = open(&call("guardrail.holds.list", json!({"project_id":project})).await?);
        let gate = json!({"session":session,"kind":"write","path":path,"new_text":"fixture"});
        require(ui.call("guardrail.gate", gate).await.is_err(), &format!("{path} was not held"))?;
        let after = open(&call("guardrail.holds.list", json!({"project_id":project})).await?);
        let id = after.into_iter().find(|id| !before.contains(id)).ok_or("The held write made no hold")?;
        holds.push((path, id));
    }
    let page = ui.pages.get("guardrails").ok_or("No Guardrails page")?.clone().upcast::<gtk::Widget>();
    ui.refresh_page();
    // A card to decide: its subject is shown, and the page is not ignoring the pointer
    // while cards settle after a move.
    let card = |path: &str| {
        util::find(&page, &|w| {
            w.has_css_class("guardrail-card")
                && util::find(w, &|l| l.downcast_ref::<gtk::Label>().is_some_and(|l| l.text() == path)).is_some()
        })
        .filter(|card| {
            let mut ancestor = Some(card.clone());
            while let Some(widget) = ancestor {
                if !widget.can_target() {
                    return false;
                }
                ancestor = widget.parent();
            }
            card.is_mapped()
        })
    };
    let key = |card: &gtk::Widget, label: &str| {
        util::find(card, &|w| w.downcast_ref::<gtk::Button>().is_some_and(|b| b.label().as_deref() == Some(label)))
            .and_then(|w| w.downcast::<gtk::Button>().ok())
            .ok_or_else(|| format!("No {label} key on the hold card"))
    };
    let resolved = |id: i64, state: &'static str| {
        async move {
            let deadline = Instant::now() + util::WAIT;
            loop {
                let hold = call("guardrail.hold.get", json!({"hold_id":id})).await?;
                if hold["hold"]["state"] == state {
                    return Ok::<(), String>(());
                }
                require(Instant::now() < deadline, &format!("Hold {id} did not become {state}"))?;
                glib::timeout_future(Duration::from_millis(50)).await;
            }
        }
    };

    let (path, id) = &holds[0];
    wait_for(|| card(path).is_some(), "Held write shown on the Guardrails page").await?;
    let allow_card = card(path).unwrap();
    let allow = key(&allow_card, "Allow once")?;
    require(
        allow.is_mapped() && !allow.is_sensitive(),
        "Allow once must stay disabled until the exact action is reviewed",
    )?;
    util::press(&key(&allow_card, "Review exact action")?, "Review exact action")?;
    wait_for(|| allow.is_sensitive(), "Reviewing the exact action enables Allow once").await?;
    util::press(&allow, "Allow once")?;
    resolved(*id, "confirmed").await?;

    let (path, id) = &holds[1];
    wait_for(|| card(path).is_some(), "Second held write shown").await?;
    let reject_card = card(path).unwrap();
    util::press(&key(&reject_card, "Reject…")?, "Reject…")?;
    let send = key(&reject_card, "Send denial")?;
    wait_for(|| send.is_mapped(), "Denial form shown").await?;
    util::press(&send, "Send denial")?;
    resolved(*id, "rejected").await?;
    println!("Guardrail decisions verified: review before Allow once, Allow confirms, Reject closes");
    verify_exception_requests(&ui, &session).await
}

/// Approve once, Approve for this session, Deny and Revoke on exception requests (RA-719).
/// Only an agent may call `guardrail.request`, so the request comes from the live fixture
/// session itself: native-smoke.py's provider turns a `native-request kind value scope` line
/// on its terminal into that call, with its own RELAY_SESSION and RELAY_TOKEN.
async fn verify_exception_requests(ui: &Rc<Ui>, session: &str) -> Result<(), String> {
    let project = ui.project.get();
    let call = |op: &'static str, payload: serde_json::Value| {
        let ui = ui.clone();
        async move { ui.call(op, payload).await.map_err(|e| format!("{op}: {e}")) }
    };
    let mut requests = Vec::new();
    for (decision, scope) in [("once", "once"), ("session", "session"), ("deny", "once")] {
        let value = format!("secret/native-request-{decision}-{}", std::process::id());
        call("session.input", json!({"session":session,"data":format!("native-request path {value} {scope}\n")})).await?;
        let deadline = Instant::now() + util::WAIT;
        let id = loop {
            let open = call("guardrail.requests.list", json!({"project_id":project,"state":"open"})).await?;
            let found = open["requests"].as_array().into_iter().flatten().find(|r| r["value"] == value.as_str());
            if let Some(id) = found.and_then(|r| r["id"].as_i64()) {
                break id;
            }
            if Instant::now() >= deadline {
                let tail = call("session.scrollback", json!({"session":session})).await?;
                let tail = tail["text"].as_str().unwrap_or_default();
                let tail: String = tail.chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
                return Err(format!("{session} made no request for {value}; its terminal ends: {tail:?}"));
            }
            glib::timeout_future(Duration::from_millis(50)).await;
        };
        requests.push((value, id));
    }
    let page = ui.pages.get("guardrails").ok_or("No Guardrails page")?.clone().upcast::<gtk::Widget>();
    ui.refresh_page();
    // The page's own card, not the prompt in the tray, and only once the pointer may reach it.
    let card = |id: i64| {
        let name = format!("guardrail-request-{id}");
        util::find(&page, &|w| w.widget_name() == name.as_str()).filter(|card| {
            let mut ancestor = Some(card.clone());
            while let Some(widget) = ancestor {
                if !widget.can_target() {
                    return false;
                }
                ancestor = widget.parent();
            }
            card.is_mapped()
        })
    };
    let key = |root: &gtk::Widget, label: &str| {
        util::find(root, &|w| w.downcast_ref::<gtk::Button>().is_some_and(|b| b.label().as_deref() == Some(label)))
            .and_then(|w| w.downcast::<gtk::Button>().ok())
            .ok_or_else(|| format!("No {label} key"))
    };
    let settled = |id: i64, test: fn(&serde_json::Value) -> bool, what: &'static str| async move {
        let deadline = Instant::now() + util::WAIT;
        loop {
            let request = call("guardrail.request.get", json!({"request_id":id})).await?;
            if test(&request) {
                return Ok::<(), String>(());
            }
            require(Instant::now() < deadline, &format!("Request {id} {what}: {request}"))?;
            glib::timeout_future(Duration::from_millis(50)).await;
        }
    };
    let answer = |id: i64, label: &'static str| async move {
        wait_for(|| card(id).is_some(), &format!("Request {id} shown on the Guardrails page")).await?;
        util::press(&key(&card(id).unwrap(), label)?, label)
    };

    let (_, once) = requests[0];
    answer(once, "Approve once").await?;
    settled(once, |r| r["state"] == "confirmed" && r["scope"] == "once" && r["active"] == true, "approved once").await?;

    let (ref value, always) = requests[1];
    answer(always, "Approve for this session").await?;
    settled(always, |r| r["state"] == "confirmed" && r["scope"] == "session" && r["active"] == true, "approved for the session").await?;

    let (_, denied) = requests[2];
    answer(denied, "Deny…").await?;
    let send = key(&card(denied).ok_or("Denied request's card gone")?, "Send denial")?;
    wait_for(|| send.is_mapped(), "Denial form shown").await?;
    util::press(&send, "Send denial")?;
    settled(denied, |r| r["state"] == "rejected", "denied").await?;

    // The session grant is listed under ACTIVE EXCEPTIONS with its own Revoke key.
    ui.refresh_page();
    let grant = || {
        util::find(&page, &|w| {
            w.has_css_class("guardrail-grant")
                && util::find(w, &|l| l.downcast_ref::<gtk::Label>().is_some_and(|l| l.text() == value.as_str())).is_some()
        })
        .filter(|row| row.is_mapped())
    };
    wait_for(|| grant().is_some(), "Session grant listed under active exceptions").await?;
    util::press(&key(&grant().unwrap(), "Revoke")?, "Revoke")?;
    settled(always, |r| r["active"] == false && !r["revoked_at"].is_null(), "revoked").await?;
    println!("Exception requests verified: Approve once, Approve for this session, Deny, Revoke");
    Ok(())
}

pub fn install(ui: &Rc<Ui>) {
    let Ok(path) = std::env::var("RELAY_NATIVE_SCREENSHOT") else {
        return;
    };
    let fixture = std::env::var("RELAY_NATIVE_FIXTURE").as_deref() == Ok("1");
    // A fixture, a page or a roadmap part writes through the engine: it types into
    // terminals, edits notes, tasks, files and settings, and launches sessions. Arm any
    // of them only against a disposable engine the driver started under the temp
    // directory, never the live one an inherited variable happens to reach. A bare
    // screenshot (with VERIFY_CONNECTION or VERIFY_CONTRAST) only reads, so it may
    // look at any engine.
    let acts = fixture
        || std::env::var_os("RELAY_NATIVE_PAGE").is_some()
        || std::env::var_os("RELAY_NATIVE_ROADMAP").is_some();
    let isolated = ui.path.starts_with(std::env::temp_dir())
        && !ui.path.components().any(|c| c == std::path::Component::ParentDir);
    if acts && !isolated {
        eprintln!(
            "Smoke harness refused: RELAY_NATIVE_FIXTURE/PAGE/ROADMAP act on the engine, and {} is not under {}. Point RELAY_NATIVE_SOCKET at a disposable engine there.",
            ui.path.display(),
            std::env::temp_dir().display()
        );
        std::process::exit(1);
    }
    // A panic in a detached GLib future must fail the fixture, rather than leave
    // an idle window until the harness timeout.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        std::process::exit(1);
    }));
    if std::env::var_os("RELAY_NATIVE_POINTER_DRIVER").is_some() {
        ui.window
            .set_title(Some(&format!("Relay pointer smoke {}", std::process::id())));
    }
    if let Ok(size) = std::env::var("RELAY_NATIVE_SIZE") {
        if let Some((w, h)) = size.split_once(',') {
            if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                ui.window.set_default_size(w, h);
            }
        }
    }
    let ui = ui.clone();
    if let Ok(part) = std::env::var("RELAY_NATIVE_ROADMAP") {
        assert!(fixture, "Roadmap smoke parts run with RELAY_NATIVE_FIXTURE=1");
        glib::spawn_future_local(async move {
            // Every part starts from the fixture project the first refresh selects.
            let ready = wait_within(
                Duration::from_secs(20),
                || {
                    ui.project.get() != 0
                        && !ui.projects.borrow().is_empty()
                        && !ui.workspaces.borrow().is_empty()
                },
                "Fixture project and workspace loaded",
            )
            .await;
            let result = match (ready, part.as_str()) {
                (Err(error), _) => Err(error),
                (_, "notes") => notes::run(&ui).await,
                (_, "files") => crate::smoke_project_files::run(&ui).await,
                (_, "lifecycle") => crate::smoke_project_files::profile_lifecycle(&ui).await,
                (_, "registry") => crate::smoke_registry::run(&ui).await,
                (_, "tools") => crate::roadmap_smoke::run(&ui).await,
                _ => Err(format!("Unknown roadmap smoke part: {part}")),
            };
            if let Err(error) = result {
                panic!("Roadmap regression {part}: {error}");
            }
            ui.window.close();
            assert!(
                !ui.window.is_visible(),
                "Roadmap test left a dirty draft: {}",
                ui.notice.text()
            );
            println!("ROADMAP_OK={part}");
        });
        return;
    }
    if fixture {
        crate::pages::verify_note_tools();
        let input = ui.clone();
        glib::timeout_add_local_once(Duration::from_secs(1), move || {
            input.verify_terminal_input()
        });
    }
    let duration = std::env::var("RELAY_NATIVE_SMOKE_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let navigate = ui.clone();
    glib::timeout_add_local_once(Duration::from_secs(2), move || {
        if fixture {
            // In Settings this control is Back. Enter the wall before testing
            // the sidebar toggle, then navigate to the requested capture page.
            navigate.navigate("agents");
            navigate.verify_shell();
            if std::env::var("RELAY_NATIVE_PAGE").as_deref() == Ok("agents") {
                track("confirmations", verify_confirmations(navigate.clone()));
            }
            let visible = navigate.sidebar.is_visible();
            click(&navigate.window, "sidebar-toggle").unwrap();
            click(&navigate.window, "sidebar-toggle").unwrap();
            assert_eq!(navigate.sidebar.is_visible(), visible);
        }
        if let Ok(page) = std::env::var("RELAY_NATIVE_PAGE") {
            if page == "workspace-project-submit" {
                click(&navigate.window, "new-session").unwrap();
                assert!(
                    named(&navigate.window, "setup-local-repos").is_some(),
                    "New Session must guide an empty workspace to project selection"
                );
                if let Ok(path) = std::env::var("RELAY_NATIVE_SETUP_PATH") {
                    named(&navigate.window, "setup-path")
                        .unwrap()
                        .downcast::<gtk::Entry>()
                        .unwrap()
                        .set_text(&path);
                }
                let ui = navigate.clone();
                track("workspace project submit", async move {
                    // Let GTK allocate the newly mounted setup form before pointer checks.
                    wait_for(|| allocated(&ui, "setup-scan"), "Setup form allocated").await?;
                    click_control(&ui, "setup-scan");
                    wait_for(
                        || clickable(&ui.window, "setup-add"),
                        "The discovered local project must be actionable",
                    )
                    .await?;
                    click_control(&ui, "setup-add");
                    Ok(())
                });
                return;
            }
            if page == "project-launch" {
                click(&navigate.window, "new-session").unwrap();
                let ui = navigate.clone();
                track("project launch", async move {
                    wait_for(|| allocated(&ui, "launch-start"), "Launch form loaded").await?;
                    click_control(&ui, "launch-start");
                    Ok(())
                });
                return;
            }

            if page == "toggles" {
                verify_panel_toggles(&navigate);
                return;
            }

            if page == "mirror" {
                // No adb in the fixture: the docked mirror shows its rail and a problem card.
                crate::mirror::prefer_dock();
                if let Ok(avd) = std::env::var("RELAY_NATIVE_MIRROR_AVD") {
                    crate::mirror::open_avd(&navigate, avd, false);
                } else {
                    let device = std::env::var("RELAY_NATIVE_MIRROR_DEVICE").unwrap_or_else(|_| "fixture-serial".into());
                    crate::mirror::open(&navigate, device);
                }
                return;
            }

            if matches!(page.as_str(), "device-run" | "device-release" | "resources") {
                if page == "resources" {
                    navigate.resources();
                } else {
                    crate::tools::devices::open(&navigate);
                    if page == "device-run" {
                        let ui = navigate.clone();
                        track("device refresh", async move {
                            wait_for(|| clickable(&ui.window, "device-refresh"), "Device panel loaded")
                                .await?;
                            click(&ui.window, "device-refresh")?;
                            require(
                                ui.panels.borrow().len() == 1,
                                "Device refresh must keep a panel open",
                            )
                        });
                    }
                    if page == "device-release" {
                        named(&navigate.window, "device-tabs")
                            .unwrap()
                            .downcast::<gtk::Stack>()
                            .unwrap()
                            .set_visible_child_name("release");
                    }
                }
                return;
            }
            if page.starts_with("setup") {
                if named(&navigate.window, "setup-add").is_none() {
                    navigate.open_repository();
                }
                if page == "setup" {
                    return;
                }
                let ui = navigate.clone();
                track("setup", async move {
                    wait_for(|| allocated(&ui, "setup-scan"), "Setup form allocated").await?;
                    if let Ok(path) = std::env::var("RELAY_NATIVE_SETUP_PATH") {
                        named(&ui.window, "setup-path")
                            .ok_or("Setup path missing")?
                            .downcast::<gtk::Entry>()
                            .map_err(|_| "Setup path type")?
                            .set_text(&path);
                    }
                    click_control(&ui, "setup-scan");
                    if page.starts_with("setup-github") {
                        // Nothing below depends on the scan's result, which only fills the
                        // Local tab and never switches tabs, so the switch need not wait for it.
                        named(&ui.window, "setup-source")
                            .ok_or("Setup source tabs missing")?
                            .downcast::<gtk::Stack>()
                            .map_err(|_| "Setup source type")?
                            .set_visible_child_name("github");
                        wait_for(
                            || named(&ui.window, "setup-github-repos").is_some_and(|w| w.is_mapped()),
                            "GitHub tab shown",
                        )
                        .await?;
                    }
                    if page.contains("connect") {
                        wait_for(|| clickable(&ui.window, "setup-connect"), "GitHub connect shown")
                            .await?;
                        click(&ui.window, "setup-connect")?;
                    }
                    if page.ends_with("submit") {
                        wait_within(
                            Duration::from_secs(10),
                            || clickable(&ui.window, "setup-add"),
                            "A selected repository must be actionable",
                        )
                        .await?;
                        click_control(&ui, "setup-add");
                    }
                    Ok(())
                });
                return;
            }

            // The palette as it is used: typed into, Enter runs the highlighted match.
            if page == "palette-run" {
                let ui = navigate.clone();
                track("palette run", async move {
                    click(&ui.window, "command-palette")?;
                    glib::timeout_future(Duration::from_millis(300)).await;
                    let panel = util::named(&ui.window, "app-panel").ok_or("no palette panel")?;
                    let search = util::find(&panel, &|w| w.is::<gtk::SearchEntry>())
                        .and_downcast::<gtk::SearchEntry>()
                        .ok_or("no search in the palette")?;
                    search.set_text("boa");
                    glib::timeout_future(Duration::from_millis(200)).await;
                    search.emit_activate();
                    wait_for(|| *ui.page.borrow() == "board", "Board opened from the palette").await?;
                    println!("Command palette run verified: typed, Enter, Board");
                    Ok(())
                });
                return;
            }
            if matches!(page.as_str(), "palette" | "layouts" | "usage") {
                let control = match page.as_str() {
                    "palette" => "command-palette",
                    "layouts" => "window-presets",
                    _ => "sidebar-usage-options",
                };
                click(&navigate.window, control).unwrap();
                return;
            }
            if page == "launch-preview" {
                navigate.show_launch(None);
                return;
            }
            // The agent settings popover of the pane nearest the middle of a full wall, where it
            // has the least room on either side; the capture is the popover itself.
            if page == "agent-settings" {
                navigate.navigate("agents");
                let ui = navigate.clone();
                track("agent settings", async move {
                    wait_for(|| tightest_settings_key(&ui).is_some(), "Agent settings key shown").await?;
                    tightest_settings_key(&ui).unwrap().emit_clicked();
                    wait_for(|| agent_settings_popover(&ui).is_some(), "Agent settings open").await?;
                    glib::timeout_future(Duration::from_millis(600)).await;
                    let popover = agent_settings_popover(&ui).unwrap();
                    let key = tightest_settings_key(&ui).unwrap();
                    let bounds = key.compute_bounds(&ui.window).unwrap();
                    println!("AGENT_SETTINGS window={} key_y={} popover={}", ui.window.height(), bounds.y(), popover.height());
                    require(popover.height() <= ui.window.height(), "Agent settings popover fits the window")?;
                    // The smaller window captures the tallest state: Close's confirmation open.
                    if ui.window.height() < 800 {
                        let close = util::find(&popover, &|w| w.has_css_class("agent-menu-danger")).ok_or("Close key")?;
                        close.downcast::<gtk::Button>().unwrap().emit_clicked();
                        glib::timeout_future(Duration::from_millis(400)).await;
                        let go = util::find(&popover, &|w| w.has_css_class("agent-menu-confirm-go") && w.is_mapped()).ok_or("Close confirmation shown")?;
                        let bounds = go.compute_bounds(&popover).ok_or("Close confirmation placed")?;
                        println!("AGENT_SETTINGS confirm popover={} go_bottom={}", popover.height(), bounds.y() + bounds.height());
                        require(popover.height() <= ui.window.height(), "Agent settings popover fits with Close's confirmation open")?;
                    }
                    Ok(())
                });
                return;
            }
            // The agents bar's Agent skills and Plugins sheets, over the agents page.
            if matches!(page.as_str(), "project-skills" | "project-plugins") {
                navigate.navigate("agents");
                if page == "project-skills" {
                    navigate.project_skills();
                } else {
                    navigate.project_plugins();
                }
                return;
            }
            // The start screen and the entry sheet, over Money's Home.
            if matches!(page.as_str(), "start" | "money-entry") {
                navigate.navigate("money-home");
                if page == "start" {
                    crate::money::show_start(&navigate);
                } else {
                    crate::money::add_entry(&navigate);
                }
                return;
            }
            // A task's page (`task:<id>`) or the New task page, over the board.
            if let Some(task) = page.strip_prefix("task:").and_then(|id| id.parse::<i64>().ok()) {
                navigate.navigate("board");
                crate::pages::open_task(&navigate, task);
                return;
            }
            if let Some(module) = page.strip_prefix("module:").and_then(|id| id.parse::<i64>().ok()) {
                crate::pages::open_module_board(&navigate, module);
                return;
            }
            if page == "board-view" {
                navigate.navigate("board");
                let window = navigate.window.clone();
                glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                    if let Some(menu) = named(&window, "board-view").and_downcast::<gtk::MenuButton>() {
                        menu.popup();
                    }
                });
                return;
            }
            if page == "module-new" {
                navigate.navigate("modules");
                crate::pages::new_module(&navigate, navigate.project.get());
                return;
            }
            if page == "task-new" {
                navigate.navigate("board");
                crate::pages::new_task(&navigate, navigate.project.get(), "backlog");
                return;
            }
            if page == "design" {
                crate::design::smoke_open(&navigate);
                return;
            }
            // Threads: the newest thread (scripts/threads-smoke.py seeds it), or a new one.
            if page == "thread" || page == "threads" {
                crate::money::threads::smoke_open(&navigate, page == "thread");
                return;
            }
            navigate.navigate(if page == "launch" { "agents" } else { &page });
            if fixture && matches!(page.as_str(), "board" | "notes" | "launch") {
                track("fixture edit", edit_fixture(navigate.clone(), page.clone()));
            }
            if fixture && page == "guardrails" {
                track("guardrail decisions", verify_guardrail_decisions(navigate.clone()));
            }
            if fixture && std::env::var("RELAY_NATIVE_BURST").as_deref() == Ok("1") {
                let ui = navigate.clone();
                // Layout checks reconnect hidden panes. Wait for their fixture sockets
                // before feeding input, just as a user waits for a ready terminal.
                track("burst input", async move {
                    wait_for(|| ui.burst_ready(), "Every burst pane attached").await?;
                    ui.verify_burst(false);
                    Ok(())
                });
            }
            if page == "code" && fixture {
                navigate.editor.verify_open(&navigate);
                let ui = navigate.clone();
                track("code save", async move {
                    // verify_save expects the README read to have finished: the editor is
                    // neither busy nor modified, and the source view accepts typing.
                    wait_for(
                        || {
                            !ui.editor.is_dirty()
                                && named(&ui.window, "project-source")
                                    .and_then(|w| w.downcast::<gtk::TextView>().ok())
                                    .is_some_and(|view| view.is_editable())
                        },
                        "README.md opened in the editor",
                    )
                    .await?;
                    ui.editor.verify_save(&ui);
                    Ok(())
                });
            }
            if page == "settings" && fixture {
                track("settings save", verify_settings_save(navigate.clone()));
            }
        }
    });
    glib::timeout_add_local_once(Duration::from_secs(duration), move || {
        glib::spawn_future_local(async move {
            // The window closes after this; a check still running would be dropped unseen.
            if let Err(error) = wait_within(
                Duration::from_secs(15),
                || PENDING.with(|pending| pending.borrow().is_empty()),
                "fixture checks",
            )
            .await
            {
                panic!("{error}: {:?} never finished", PENDING.with(|p| p.borrow().clone()));
            }
            capture(ui, path, fixture);
        });
    });
}

fn tightest_settings_key(ui: &Ui) -> Option<gtk::Button> {
    fn walk(widget: &gtk::Widget, root: &gtk::Widget, middle: f32, best: &mut Option<(f32, gtk::Button)>) {
        if widget.has_css_class("agent-settings-key") && widget.is_mapped() {
            if let (Some(key), Some(bounds)) = (widget.downcast_ref::<gtk::Button>(), widget.compute_bounds(root)) {
                let off = (bounds.y() - middle).abs();
                if best.as_ref().is_none_or(|(o, _)| off < *o) {
                    *best = Some((off, key.clone()));
                }
            }
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            walk(&next, root, middle, best);
            child = next.next_sibling();
        }
    }
    let root: gtk::Widget = ui.window.clone().upcast();
    let mut best = None;
    walk(&root, &root, ui.window.height() as f32 / 2.0, &mut best);
    best.map(|(_, key)| key)
}

fn agent_settings_popover(ui: &Ui) -> Option<gtk::Widget> {
    util::find(&ui.window.clone().upcast(), &|w| w.has_css_class("agent-settings") && w.is_mapped())
}

fn capture(ui: Rc<Ui>, path: String, fixture: bool) {
    if fixture && *ui.page.borrow() == "board" {
        assert!(
            ui.dismiss_panels(),
            "Saved task should close back to the board"
        );
    }
    for name in ["setup-scan", "setup-path", "setup-local-path", "setup-add"] {
        if named(&ui.window, name).is_some_and(|w| w.is_mapped() && w.is_sensitive()) {
            verify_pointer_target(&ui, name);
        }
    }
    if std::env::var_os("RELAY_NATIVE_VERIFY_CONTRAST").is_some() {
        verify_control_contrast(&ui.window);
    }

    if std::env::var("RELAY_NATIVE_VERIFY_CONNECTION").as_deref() == Ok("1") {
        assert!(
            ui.client.borrow().is_some() && !ui.notice.is_visible(),
            "Native engine connection failed: {}",
            ui.notice.text()
        );
        println!("Native engine connection verified: {}", ui.path.display());
    }
    if fixture && std::env::var("RELAY_NATIVE_BURST").as_deref() == Ok("1") {
        ui.verify_burst(true);
    }
    assert_eq!(
        gtk::Window::list_toplevels()
            .iter()
            .filter(|w| w.is_visible())
            .count(),
        1 + usize::from(
            ui.notes_window
                .borrow()
                .as_ref()
                .is_some_and(|w| w.window.is_visible())
        ),
        "Only the main workspace and retained Notes window may be visible"
    );
    // WidgetPaintable can have no node between invalidation and GTK's next
    // frame (notably after a Code save). Capture a rendered frame, bounded.
    let mut attempts = 0;
    glib::timeout_add_local(Duration::from_millis(50), move || {
        attempts += 1;
        let target: gtk::Widget =
            if std::env::var("RELAY_NATIVE_PAGE").as_deref() == Ok("agent-settings") {
                agent_settings_popover(&ui).expect("Agent settings popover")
            } else if std::env::var("RELAY_NATIVE_PAGE").as_deref() == Ok("notes") {
                ui.notes_window
                    .borrow()
                    .as_ref()
                    .expect("Notes window")
                    .window
                    .clone()
                    .upcast()
            } else {
                ui.window.clone().upcast()
            };
        let paintable = gtk::WidgetPaintable::new(Some(&target));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, target.width() as f64, target.height() as f64);
        let (Some(node), Some(renderer)) = (snapshot.to_node(), target.native().and_then(|n| n.renderer())) else {
            assert!(
                attempts < 40,
                "Screenshot failed: no rendered frame after two seconds"
            );
            ui.window.queue_draw();
            return glib::ControlFlow::Continue;
        };
        renderer
            .render_texture(&node, None)
            .save_to_png(&path)
            .expect("Save screenshot");
        record_geometry(&ui, &path);
        println!("Screenshot saved: {path}");
        if fixture && std::env::var("RELAY_NATIVE_PAGE").as_deref() == Ok("launch-preview") {
            ui.verify_launch();
        }
        if std::env::var("RELAY_NATIVE_PAGE").as_deref() == Ok("start") {
            // Leaving the start screen gives the bars their keys and readings back.
            assert!(ui.window.has_css_class("starting"), "The start screen should have quieted the bars");
            assert!(crate::money::escape_start(&ui), "Escape should close the start screen");
            assert!(!ui.window.has_css_class("starting"));
            for part in &ui.start_chrome {
                assert!(part.opacity() == 1.0 && part.is_sensitive(), "{} still quiet", part.css_name());
            }
            println!("Start screen dismiss verified");
        }
        ui.window.close();
        assert!(
            !ui.window.is_visible(),
            "Native close blocked: {}",
            ui.notice.text()
        );
        glib::ControlFlow::Break
    });
}
