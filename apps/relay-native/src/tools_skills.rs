//! Skills: reusable agent instructions installed from GitHub or written here, switched on or
//! off per project. The page is a searchable catalog beside a detail pane (`tools_market.rs`).
use super::market::{self, Market, Spec};
use super::paragraph;
use super::plugins::explain_error;
use crate::app::{button, field, label, rows, text, Ui};
use crate::client::Error;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub async fn refresh(ui: &Rc<Ui>, project: i64) {
    market::refresh(ui, &SPEC, project).await
}

static SPEC: Spec = Spec {
    page: "skills",
    title: "Skills",
    subtitle: "Reusable agent instructions, written into every checkout and provider home as real skill folders. A new skill is on everywhere; switch it off where it does not belong.",
    icon: "skills",
    noun: ("skill", "skills"),
    placeholder: "Search skills by name, description or source",
    filters: &["All", "On", "Off"],
    list_op: "skill.list",
    // Bodies are cut to their opening, which holds the description; the detail and the
    // editor fetch the whole body with skill.get.
    list_payload: || json!({"summary": true}),
    list_key: "skills",
    enable_op: "skill.enable",
    id_key: "skill_id",
    id: |skill| skill["id"].to_string(),
    haystack: |skill| {
        format!(
            "{}\n{}\n{}\n{}",
            text(skill, "name"),
            description(skill),
            source(skill),
            text(skill, "source_path")
        )
        .to_lowercase()
    },
    passes: |skill, filter, project| match filter {
        1 => market::enabled(skill, project),
        2 => !market::enabled(skill, project),
        _ => true,
    },
    group: Some(source),
    sort: |skill| text(skill, "name").to_lowercase(),
    card,
    detail,
    empty,
    setup: Some(setup),
};

/// How much of a body `skill.list {summary: true}` keeps (the engine's SKILL_SUMMARY_BYTES).
const SUMMARY_BYTES: usize = 4096;

/// Whether a listed body may have been cut: a char boundary can land up to 3 bytes short.
fn maybe_cut(skill: &Value) -> bool {
    text(skill, "body").len() + 4 > SUMMARY_BYTES
}

/// The skill with its whole body: the listed row when it cannot have been cut, else skill.get.
async fn whole(ui: &Rc<Ui>, skill: &Value) -> Result<Value, Error> {
    if maybe_cut(skill) {
        ui.call("skill.get", json!({"skill_id": skill["id"]})).await
    } else {
        Ok(skill.clone())
    }
}

/// What the skill is for. The engine reads it from the SKILL.md front matter (or the first
/// prose line) with the one parser it also uses for names and plugin skills.
fn description(skill: &Value) -> &str {
    text(skill, "description")
}

/// `owner/repository` for an installed skill, or "Local skills".
fn source(skill: &Value) -> String {
    match skill["source_url"].as_str() {
        Some(url) => {
            let url = url.trim_end_matches('/').trim_end_matches(".git");
            let parts: Vec<&str> = url.rsplit('/').take(2).collect();
            match parts.as_slice() {
                [repo, owner] => format!("{owner}/{repo}"),
                _ => url.to_string(),
            }
        }
        None => "Local skills".into(),
    }
}

fn web_url(skill: &Value) -> Option<String> {
    skill["source_url"]
        .as_str()
        .map(|url| url.trim_end_matches('/').trim_end_matches(".git").to_string())
}

fn card(ui: &Rc<Ui>, market: &Rc<Market>, skill: &Value) -> gtk::Widget {
    let scope = market.scope.get();
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("ext-card");
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_hexpand(true);
    let name = label(text(skill, "name"), "ext-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let about = description(skill);
    if !about.is_empty() {
        let summary = label(about, "ext-summary");
        summary.set_wrap(true);
        summary.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        summary.set_lines(2);
        summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
        summary.set_max_width_chars(44);
        words.append(&summary);
    }
    let path = label(
        skill["source_path"].as_str().unwrap_or("Written in Relay"),
        "ext-path",
    );
    path.set_ellipsize(gtk::pango::EllipsizeMode::Start);
    path.set_tooltip_text(skill["source_path"].as_str());
    words.append(&path);
    card.append(&words);
    let switch = market::toggle(ui, market, skill, scope, format!("skill-enabled-{}", skill["id"]));
    switch.set_valign(gtk::Align::Start);
    card.append(&switch);
    card.upcast()
}

fn detail(ui: &Rc<Ui>, market: &Rc<Market>, skill: &Value, parent: &gtk::Box) {
    let id = skill["id"].clone();
    let head = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let title = label(text(skill, "name"), "ext-detail-title");
    title.set_wrap(true);
    head.append(&title);
    head.append(&label(
        &match skill["source_url"].as_str() {
            Some(_) => format!("From {} on GitHub", source(skill)),
            None => "Written in Relay".to_string(),
        },
        "ext-meta",
    ));
    parent.append(&head);
    let about = description(skill);
    if !about.is_empty() {
        let lede = market::prose(about);
        lede.add_css_class("ext-lede");
        parent.append(&lede);
    }
    parent.append(&market::enable_block(
        ui,
        market,
        skill,
        (
            "Agents in this project find it in their skills folder.",
            "Agents in this project do not see it.",
        ),
        format!("skill-{id}-scope"),
    ));

    // Actions.
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.add_css_class("ext-action-bar");
    if let Some(url) = web_url(skill) {
        let open = button("Open on GitHub", "");
        open.set_tooltip_text(Some(&url));
        let weak = Rc::downgrade(ui);
        open.connect_clicked(move |key| {
            if let Some(ui) = weak.upgrade() {
                ui.mutate("os.open_url", json!({"url": url}), key);
            }
        });
        bar.append(&open);
        let update = button("Update from GitHub", "");
        // The engine keeps the repository but not the branch a skill came from (RA-256), so
        // an update can only read the default branch. Say so before it replaces anything.
        update.set_tooltip_text(Some("Download the current version of this skill from the repository's default branch. A skill installed from another branch is replaced by the default branch's copy; reinstall it from its branch URL instead."));
        let weak = Rc::downgrade(ui);
        let payload = json!({"url": skill["source_url"], "subdir": skill["source_path"]});
        let name = text(skill, "name").to_string();
        // Called directly, not through Ui::mutate, so a success says so.
        update.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            key.set_sensitive(false);
            let (key, payload, name) = (key.clone(), payload.clone(), name.clone());
            glib::spawn_future_local(async move {
                match ui.call("skill.install", payload).await {
                    Ok(_) => ui.show_info(&format!("Updated {name} from GitHub")),
                    Err(error) => ui.show_error(&explain_error("skill.install", &error)),
                }
                key.set_sensitive(true);
                ui.refresh_page();
            });
        });
        bar.append(&update);
    } else {
        let edit_key = button("Edit", "");
        let weak = Rc::downgrade(ui);
        let editing = skill.clone();
        edit_key.connect_clicked(move |key| {
            let Some(ui) = weak.upgrade() else { return };
            // The editor saves what it shows, so it opens only on the whole body.
            let (editing, key) = (editing.clone(), key.clone());
            key.set_sensitive(false);
            glib::spawn_future_local(async move {
                let result = whole(&ui, &editing).await;
                key.set_sensitive(true);
                match result {
                    Ok(skill) => edit(&ui, Some(skill)),
                    Err(error) => ui.show_error(&explain_error("skill.get", &error)),
                }
            });
        });
        bar.append(&edit_key);
    }
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bar.append(&spacer);
    let remove = button("Remove", "quiet");
    remove.add_css_class("ext-danger");
    remove.set_tooltip_text(Some("Remove this skill from every project"));
    let weak = Rc::downgrade(ui);
    crate::app::confirm_inline(&remove, "Click again to remove", move |key| {
        if let Some(ui) = weak.upgrade() {
            ui.mutate("skill.delete", json!({"skill_id": id}), key);
        }
    });
    bar.append(&remove);
    parent.append(&bar);

    let mut facts = Vec::new();
    if let Some(url) = web_url(skill) {
        facts.push(("Source", url, true));
    }
    if let Some(path) = skill["source_path"].as_str() {
        facts.push(("Path", path.to_string(), true));
    }
    if let Some(revision) = skill["revision"].as_str() {
        facts.push(("Revision", revision.chars().take(8).collect(), true));
    }
    // "3 h ago", "Oct 4", as times read everywhere else.
    let date = |key: &str| {
        crate::relative::ago(text(skill, key), crate::relative::Form::Long)
            .unwrap_or_else(|| text(skill, key).chars().take(10).collect())
    };
    facts.push(("Installed", date("created_at"), false));
    facts.push(("Updated", date("updated_at"), false));
    parent.append(&market::facts(&facts));

    let instructions = gtk::Box::new(gtk::Orientation::Vertical, 0);
    instructions.append(&market::markdown(ui, text(skill, "body")));
    if maybe_cut(skill) {
        // Show the opening at once, then the whole body when it arrives.
        let (ui, skill, instructions) = (ui.clone(), skill.clone(), instructions.downgrade());
        glib::spawn_future_local(async move {
            let Ok(full) = whole(&ui, &skill).await else { return };
            let Some(instructions) = instructions.upgrade() else { return };
            while let Some(child) = instructions.first_child() {
                instructions.remove(&child);
            }
            instructions.append(&market::markdown(&ui, text(&full, "body")));
        });
    }
    let skill_id = skill["id"].clone();
    let switch_name = move |project: i64| format!("skill-{skill_id}-in-{project}");
    parent.append(&market::tabs(
        market,
        vec![
            ("instructions", "Instructions", None, instructions.upcast()),
            (
                "projects",
                "Projects",
                Some(market::reach(ui, skill).0),
                market::project_switches(ui, market, skill, &switch_name, None).upcast(),
            ),
        ],
    ));
}

fn empty(ui: &Rc<Ui>, market: &Rc<Market>) -> gtk::Widget {
    let install = button("Install from GitHub", "primary");
    let weak = Rc::downgrade(market);
    install.connect_clicked(move |_| {
        if let Some(market) = weak.upgrade() {
            market.reveal_banner();
        }
    });
    let create = button("Write a local skill", "");
    let weak = Rc::downgrade(ui);
    create.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            edit(&ui, None);
        }
    });
    market::state_box(
        "skills",
        "No skills installed",
        &label(
            "Paste a GitHub repository and Relay finds its SKILL.md files, or write instructions of your own.",
            "dim",
        ),
        &[install.upcast(), create.upcast()],
    )
    .upcast()
}

/// Header actions and the installer banner.
fn setup(ui: &Rc<Ui>, market: &Rc<Market>) {
    let create = button("New local skill", "");
    let weak = Rc::downgrade(ui);
    create.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            edit(&ui, None);
        }
    });
    market.actions.append(&create);
    let install = gtk::ToggleButton::with_label("Install from GitHub");
    install.add_css_class("primary");
    install.add_css_class("ext-install");
    market.actions.append(&install);
    let head = gtk::Box::new(gtk::Orientation::Vertical, 3);
    head.append(&label("Install from GitHub", "ext-banner-title"));
    let copy = label(
        "Relay finds every SKILL.md in the repository. Give a folder to install only the skills under it.",
        "dim",
    );
    copy.set_wrap(true);
    head.append(&copy);
    market.banner.append(&head);
    let source = install_form(ui, market);
    // Opened, the banner takes the typing: its repository field.
    let banner = market.banner.clone();
    install.connect_toggled(move |key| {
        banner.set_visible(key.is_active());
        if key.is_active() {
            source.grab_focus();
        }
    });
    *market.banner_key.borrow_mut() = Some(install);
}

/// The installer's fields and keys, in the banner. Returns the repository field.
fn install_form(ui: &Rc<Ui>, market: &Rc<Market>) -> gtk::Entry {
    let page = &market.banner;
    let source = gtk::Entry::builder()
        .placeholder_text("https://github.com/owner/repository")
        .hexpand(true)
        .build();
    let installer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    installer.add_css_class("skills-installer");
    installer.set_valign(gtk::Align::End);
    let repository = gtk::Box::new(gtk::Orientation::Vertical, 6);
    repository.set_hexpand(true);
    field("GitHub repository or SKILL.md URL", &source, &repository);
    installer.append(&repository);
    let subdir = gtk::Entry::builder()
        .placeholder_text("skills/review")
        .hexpand(false)
        .build();
    let folder = gtk::Box::new(gtk::Orientation::Vertical, 6);
    folder.set_size_request(240, -1);
    folder.set_hexpand(false);
    field("Folder · optional", &subdir, &folder);
    installer.append(&folder);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let install = button("Install", "primary");
    let replace = button("Replace conflicting skill", "quiet");
    replace.set_visible(false);
    controls.append(&install);
    controls.append(&replace);
    controls.set_valign(gtk::Align::End);
    installer.append(&controls);
    page.append(&installer);
    let feedback = paragraph("");
    feedback.set_visible(false);
    page.append(&feedback);
    // Enter in either field installs.
    for input in [&source, &subdir] {
        let key = install.downgrade();
        input.connect_activate(move |_| {
            if let Some(key) = key.upgrade().filter(|key| key.is_sensitive()) {
                key.emit_clicked();
            }
        });
    }
    let conflict = Rc::new(RefCell::new(None::<(i64, String, String)>));
    for (key, replacing) in [(install.clone(), false), (replace.clone(), true)] {
        let weak = Rc::downgrade(ui);
        let shown = Rc::downgrade(market);
        let source = source.clone();
        let subdir = subdir.clone();
        let install = install.clone();
        let replace = replace.clone();
        let feedback = feedback.clone();
        let conflict = conflict.clone();
        key.connect_clicked(move |_|{
            let Some(ui)=weak.upgrade()else{return;};
            feedback.set_visible(true);
            let url=source.text().trim().to_string();let dir=subdir.text().trim().to_string();
            if url.is_empty(){feedback.set_text("Enter a GitHub repository URL.");return;}
            let mut payload=json!({"url":url,"subdir":if dir.is_empty(){None}else{Some(&dir)}});
            if replacing {
                let saved=conflict.borrow();
                let Some((id,old_url,old_dir))=saved.as_ref()else{return;};
                if old_url!=&url || old_dir!=&dir{feedback.set_text("Source changed. Install again to check the current conflict.");replace.set_visible(false);return;}
                payload["replace_skill_id"]=json!(id);
            }
            install.set_sensitive(false);replace.set_sensitive(false);
            // A clone can take a while: say it started.
            feedback.set_text("Installing from GitHub…");
            let (source,subdir,install,replace,feedback,conflict,shown)=(source.clone(),subdir.clone(),install.clone(),replace.clone(),feedback.clone(),conflict.clone(),shown.clone());
            glib::spawn_future_local(async move{
                match ui.call("skill.install",payload).await {
                    Ok(result)=>{
                        let installed=rows(&result,"skills");
                        feedback.set_text(&format!("Installed {} skill{}.",installed.len(),if installed.len()==1{""}else{"s"}));replace.set_visible(false);conflict.borrow_mut().take();
                        if source.text().trim()==url && subdir.text().trim()==dir{source.set_text("");subdir.set_text("");}
                        // The catalog shows what came in.
                        if let (Some(market),Some(first))=(shown.upgrade(),installed.first()){market.select(first["id"].to_string());}
                        ui.refresh_page();
                    }
                    Err(error)=>{
                        feedback.set_text(&explain_error("skill.install",&error));replace.set_visible(false);conflict.borrow_mut().take();
                        if let Error::Bus(error)=error {
                            if error.code=="skill.name_exists" {
                                if let Some(details)=error.details {
                                    if details["can_replace"].as_bool()==Some(true) {
                                        if let Some(id)=details["installed_skill"]["id"].as_i64(){
                                            feedback.set_text(&format!("A skill named '{}' already exists. Replace it with the downloaded instructions?",text(&details["installed_skill"],"name")));
                                            *conflict.borrow_mut()=Some((id,url,dir));replace.set_visible(true);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                install.set_sensitive(true);replace.set_sensitive(true);
            });
        });
    }
    source
}

fn edit(ui: &Rc<Ui>, skill: Option<Value>) {
    let window = crate::panel::Panel::new(
        ui,
        if skill.is_some() {
            "Edit skill"
        } else {
            "New skill"
        },
        700,
    );
    let page = window.body.clone();
    let name = gtk::Entry::builder()
        .text(skill.as_ref().map(|v| text(v, "name")).unwrap_or(""))
        .build();
    field("Name", &name, &page);
    let focus_name = name.clone();
    let body = gtk::TextView::new();
    body.set_monospace(true);
    body.set_wrap_mode(gtk::WrapMode::WordChar);
    body.buffer()
        .set_text(skill.as_ref().map(|v| text(v, "body")).unwrap_or(""));
    let scroll = crate::app::scrolled(&body);
    field("Instructions", &scroll, &page);
    let status = paragraph("");
    page.append(&status);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let save = button("Save skill", "primary");
    let discard = button("Discard and close", "quiet");
    controls.append(&save);
    controls.append(&discard);
    page.append(&controls);
    let dirty = Rc::new(Cell::new(false));
    let busy = Rc::new(Cell::new(false));
    let changed = dirty.clone();
    name.connect_changed(move |_| changed.set(true));
    let changed = dirty.clone();
    body.buffer().connect_changed(move |_| changed.set(true));
    let changed = dirty.clone();
    let running = busy.clone();
    let message = status.clone();
    window.set_guard(move || {
        if changed.get() || running.get() {
            message.set_text("Save your changes or choose Discard and close.");
            false
        } else {
            true
        }
    });
    let weak_window = Rc::downgrade(&window);
    let changed = dirty.clone();
    discard.connect_clicked(move |_| {
        changed.set(false);
        if let Some(window) = weak_window.upgrade() {
            window.close();
        }
    });
    let weak = Rc::downgrade(ui);
    let weak_window = Rc::downgrade(&window);
    save.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let title = name.text().trim().to_string();
        let buffer = body.buffer();
        let content = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if title.is_empty() || content.trim().is_empty() {
            status.set_text("A name and instructions are required.");
            return;
        }
        let (op, payload) = match &skill {
            Some(skill) => (
                "skill.update",
                json!({"skill_id":skill["id"],"name":title,"body":content}),
            ),
            None => ("skill.create", json!({"name":title,"body":content})),
        };
        busy.set(true);
        key.set_sensitive(false);
        discard.set_sensitive(false);
        name.set_sensitive(false);
        body.set_editable(false);
        let (key, discard, name, body, status, dirty, busy, weak_window) = (
            key.clone(),
            discard.clone(),
            name.clone(),
            body.clone(),
            status.clone(),
            dirty.clone(),
            busy.clone(),
            weak_window.clone(),
        );
        glib::spawn_future_local(async move {
            match ui.call(op, payload).await {
                Ok(_) => {
                    dirty.set(false);
                    busy.set(false);
                    if let Some(window) = weak_window.upgrade() {
                        window.close();
                    }
                    ui.refresh_page();
                }
                Err(error) => status.set_text(&explain_error(op, &error)),
            }
            busy.set(false);
            key.set_sensitive(true);
            discard.set_sensitive(true);
            name.set_sensitive(true);
            body.set_editable(true);
        });
    });
    // Ctrl+Enter saves from either field, as the task form's does. Capture: the instructions
    // would otherwise take it as a new line.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let key = save.downgrade();
    keys.connect_key_pressed(move |_, pressed, _, mods| {
        if matches!(pressed, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) && mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if let Some(save) = key.upgrade().filter(|save| save.is_sensitive()) {
                save.emit_clicked();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    page.add_controller(keys);
    window.present();
    // An edit keeps the name as it is rather than selecting it to be typed over.
    focus_name.grab_focus_without_selecting();
}
