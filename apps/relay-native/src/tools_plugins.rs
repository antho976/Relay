//! Plugins: bundled bundles of skills, agent rules, docs and MCP servers, switched on per
//! project (D159). The page is a searchable catalog beside a detail pane (`tools_market.rs`);
//! the project row in the sidebar opens the same switches for that project alone.
use super::market::{self, Market, Spec};
use super::paragraph;
use crate::app::{clear, label, rows, text, Ui};
use crate::client::Error;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::rc::Rc;

/// Every build of the engine bundles plugins, so an empty list or a refused `project_id` means
/// the engine that is running was started from an older build: `run.sh` reuses a running engine.
pub(crate) const STALE_ENGINE: &str = "The running Relay engine is older than this app and does not know about plugins yet. Restart it: when your agents are idle, run `target/debug/relay --instance dev q app.quit '{\"force\":true}'` (use your instance name), then ./run.sh again. Live agents come back as restorable; resume them from their tiles.";

/// What to say when a request fails: the engine's own sentence, never its kind and code
/// ("Conflict skill.name_exists: …"). An engine that predates plugins refuses a `plugin.*` op
/// or its payload, and the bus names that in `code`; its message may change and is not parsed.
pub(crate) fn explain_error(op: &str, error: &Error) -> String {
    match error {
        Error::Bus(bus)
            if op.starts_with("plugin.")
                && matches!(bus.code.as_str(), "bus.unknown_op" | "bus.schema" | "bus.not_implemented") =>
        {
            STALE_ENGINE.to_string()
        }
        // Its message names the other skill by id.
        Error::Bus(bus) if bus.code == "skill.name_exists" => "Another skill already has that name.".into(),
        Error::Bus(bus) => bus.message.clone(),
        _ => error.to_string(),
    }
}

pub async fn refresh(ui: &Rc<Ui>, project: i64) {
    market::refresh(ui, &SPEC, project).await
}

static SPEC: Spec = Spec {
    page: "plugins",
    title: "Plugins",
    subtitle: "Whole toolkits for a project's agents: skill folders, standing rules in every brief, and MCP tools. Switch one on per project.",
    icon: "plugins",
    noun: ("plugin", "plugins"),
    placeholder: "Search plugins, skills and tools",
    filters: &["All", "On", "Off", "Suggested"],
    list_op: "plugin.list",
    list_payload: || json!({}),
    list_key: "plugins",
    enable_op: "plugin.enable",
    id_key: "plugin_id",
    id: |plugin| text(plugin, "id").to_string(),
    haystack,
    passes: |plugin, filter, project| match filter {
        1 => enabled(plugin, project),
        2 => !enabled(plugin, project),
        3 => suggested(plugin, project),
        _ => true,
    },
    group: None,
    sort: |plugin| text(plugin, "name").to_lowercase(),
    card,
    detail,
    empty,
    setup: None,
};

fn haystack(plugin: &Value) -> String {
    let mut words = vec![
        text(plugin, "name").to_string(),
        text(plugin, "id").to_string(),
        text(plugin, "category").to_string(),
        text(plugin, "summary").to_string(),
        text(plugin, "description").to_string(),
    ];
    for skill in rows(plugin, "skills") {
        words.push(text(&skill, "name").to_string());
        words.push(text(&skill, "description").to_string());
    }
    for server in rows(plugin, "mcp_servers") {
        words.push(text(&server, "name").to_string());
        words.extend(tool_names(&server));
    }
    words.join("\n").to_lowercase()
}

fn tool_names(server: &Value) -> Vec<String> {
    rows(server, "tools")
        .iter()
        .filter_map(|t| t.as_str().map(str::to_string))
        .collect()
}

fn tool_count(plugin: &Value) -> usize {
    rows(plugin, "mcp_servers").iter().map(|s| rows(s, "tools").len()).sum()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn enabled(plugin: &Value, project: i64) -> bool {
    plugin["enabled_in"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(project)))
}

/// The engine leaves enabled projects out of `suggested_for`, but a switch flipped here patches
/// only `enabled_in`: a plugin switched on here stops being suggested, and switching it back off
/// restores that.
fn suggested(plugin: &Value, project: i64) -> bool {
    !enabled(plugin, project)
        && plugin["suggested_for"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(project)))
}

/// A switch that enables `plugin` for `project`, restoring itself when the engine refuses.
pub(crate) fn switch(
    ui: &Rc<Ui>,
    plugin: &Value,
    project: i64,
    feedback: Option<gtk::Label>,
) -> gtk::Switch {
    let toggle = gtk::Switch::new();
    toggle.add_css_class("settings-switch");
    toggle.set_valign(gtk::Align::Center);
    toggle.set_active(enabled(plugin, project));
    toggle.set_widget_name(&format!("plugin-{}-{project}", text(plugin, "id")));
    toggle.set_tooltip_text(Some(&format!(
        "Use {} in this project",
        text(plugin, "name")
    )));
    let weak = Rc::downgrade(ui);
    let id = text(plugin, "id").to_string();
    toggle.connect_state_set(move |key, on| {
        // A rejected write restores the switch while it is disabled.
        if !key.is_sensitive() {
            return glib::Propagation::Proceed;
        }
        if let Some(ui) = weak.upgrade() {
            let key = key.clone();
            let id = id.clone();
            let feedback = feedback.clone();
            key.set_sensitive(false);
            glib::spawn_future_local(async move {
                match ui
                    .call(
                        "plugin.enable",
                        json!({"plugin_id":id,"project_id":project,"enabled":on}),
                    )
                    .await
                {
                    Ok(_) => {
                        if let Some(feedback) = &feedback {
                            feedback.set_visible(false);
                        }
                    }
                    Err(error) => {
                        key.set_active(!on);
                        let message = explain_error("plugin.enable", &error);
                        match &feedback {
                            Some(feedback) => {
                                feedback.set_text(&message);
                                feedback.set_visible(true);
                            }
                            None => ui.show_error(&message),
                        }
                    }
                }
                key.set_sensitive(true);
            });
        }
        glib::Propagation::Proceed
    });
    toggle
}

fn card(ui: &Rc<Ui>, market: &Rc<Market>, plugin: &Value) -> gtk::Widget {
    let scope = market.scope.get();
    let id = text(plugin, "id");
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("ext-card");
    card.append(&market::plugin_mark(plugin, false));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_hexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = label(text(plugin, "name"), "ext-name");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    top.append(&name);
    let version = label(&format!("v{}", text(plugin, "version")), "ext-version");
    version.set_valign(gtk::Align::Baseline);
    top.append(&version);
    words.append(&top);
    let category = label(text(plugin, "category"), "ext-meta");
    category.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&category);
    let summary = label(text(plugin, "summary"), "ext-summary");
    summary.set_wrap(true);
    summary.set_lines(2);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.set_max_width_chars(44);
    words.append(&summary);
    // What it brings, leaving out what it has none of.
    let mut tags: Vec<gtk::Label> = [
        (rows(plugin, "skills").len(), "skill", "skills"),
        (tool_count(plugin), "MCP tool", "MCP tools"),
        (rows(plugin, "docs").len(), "doc", "docs"),
    ]
    .into_iter()
    .filter(|(n, _, _)| *n > 0)
    .map(|(n, one, many)| market::chip(&plural(n, one, many), ""))
    .collect();
    if suggested(plugin, scope) {
        let hint = market::chip("Suggested", "hint");
        hint.set_tooltip_text(Some("This project's files match what the plugin is for"));
        tags.push(hint);
    }
    if !tags.is_empty() {
        let flow = market::chips(&tags);
        flow.set_margin_top(2);
        words.append(&flow);
    }
    card.append(&words);
    let switch = market::toggle(ui, market, plugin, scope, format!("plugin-{id}-{scope}"));
    switch.set_valign(gtk::Align::Start);
    card.append(&switch);
    card.upcast()
}

fn detail(ui: &Rc<Ui>, market: &Rc<Market>, plugin: &Value, parent: &gtk::Box) {
    let id = text(plugin, "id").to_string();
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    head.append(&market::plugin_mark(plugin, true));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 4);
    words.set_valign(gtk::Align::Center);
    words.set_hexpand(true);
    let title = label(text(plugin, "name"), "ext-detail-title");
    title.set_wrap(true);
    words.append(&title);
    words.append(&label(
        &format!(
            "{} · v{} · bundled with Relay",
            text(plugin, "category"),
            text(plugin, "version")
        ),
        "ext-meta",
    ));
    head.append(&words);
    parent.append(&head);
    let lede = market::prose(text(plugin, "summary"));
    lede.add_css_class("ext-lede");
    parent.append(&lede);
    parent.append(&market::enable_block(
        ui,
        market,
        plugin,
        (
            "Agents launched here get its skills, rules and MCP tools.",
            "Switch it on to give this project's agents the whole toolkit.",
        ),
        format!("plugin-{id}-detail"),
    ));

    let skills = rows(plugin, "skills");
    let servers = rows(plugin, "mcp_servers");
    let tools = tool_count(plugin);
    let (projects, _) = market::reach(ui, plugin);
    let switch_name = {
        let id = id.clone();
        move |project: i64| format!("plugin-{id}-in-{project}")
    };
    parent.append(&market::tabs(
        market,
        vec![
            ("overview", "Overview", None, overview(plugin).upcast()),
            ("skills", "Skills", Some(skills.len()), skill_list(ui, market, &id, &skills).upcast()),
            ("tools", "MCP tools", Some(tools), tool_list(&servers).upcast()),
            // The rules every agent receives, then each document: what the tab shows.
            ("docs", "Rules & docs", Some(1 + rows(plugin, "docs").len()), documents(ui, market, &id).upcast()),
            (
                "projects",
                "Projects",
                Some(projects),
                market::project_switches(ui, market, plugin, &switch_name, Some(suggested)).upcast(),
            ),
        ],
    ));
}

fn overview(plugin: &Value) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 20);
    page.append(&market::prose(text(plugin, "description")));
    let reach = market::section("How it reaches agents");
    reach.append(&market::facts(&[
        (
            "Skills",
            "Written into every checkout as real skill folders. Running agents see them at once."
                .into(),
            false,
        ),
        (
            "Rules",
            "Added to each agent's brief when it starts or resumes.".into(),
            false,
        ),
        (
            "MCP tools",
            "Registered with the provider when an agent starts or resumes.".into(),
            false,
        ),
    ]));
    page.append(&reach);
    let servers: Vec<String> = rows(plugin, "mcp_servers")
        .iter()
        .map(|s| text(s, "name").to_string())
        .collect();
    let docs: Vec<String> = rows(plugin, "docs")
        .iter()
        .filter_map(|d| d.as_str().map(str::to_string))
        .collect();
    let about = market::section("Package");
    about.append(&market::facts(&[
        ("Identifier", text(plugin, "id").to_string(), true),
        ("Version", text(plugin, "version").to_string(), true),
        ("MCP servers", if servers.is_empty() { "None".into() } else { servers.join(", ") }, true),
        ("Documents", if docs.is_empty() { "None".into() } else { docs.join("\n") }, true),
    ]));
    page.append(&about);
    page
}

fn skill_list(ui: &Rc<Ui>, market: &Rc<Market>, plugin: &str, skills: &[Value]) -> gtk::Box {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("ext-entries");
    // The rounded card clips the rows' hover ground at its corners.
    list.set_overflow(gtk::Overflow::Hidden);
    if skills.is_empty() {
        list.append(&label("This plugin ships no skills.", "dim"));
    }
    for skill in skills {
        let name = text(skill, "name").to_string();
        let head = gtk::Box::new(gtk::Orientation::Vertical, 3);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = label(&name, "ext-entry-name");
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        top.append(&title);
        let files = skill["files"].as_u64().unwrap_or(0) as usize;
        if files > 0 {
            top.append(&label(&plural(files, "file", "files"), "ext-group-count"));
        }
        head.append(&top);
        let about = label(text(skill, "description"), "ext-entry-text");
        about.set_wrap(true);
        about.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        about.set_lines(3);
        about.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&about);
        // A toggle row over a body it shows, not a GtkExpander: an expander measures a wrapped
        // label in its title at the wrong width, so its rows come out short and overlap.
        let chevron = crate::icons::image("chevron-right", 12);
        chevron.add_css_class("ext-entry-chevron");
        chevron.set_valign(gtk::Align::Start);
        chevron.set_margin_top(3);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        line.append(&chevron);
        head.set_hexpand(true);
        line.append(&head);
        let key = gtk::ToggleButton::new();
        key.add_css_class("ext-entry-key");
        key.set_child(Some(&line));
        key.update_property(&[gtk::accessible::Property::Label(&name)]);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.add_css_class("ext-entry-body");
        body.set_visible(false);
        let entry = gtk::Box::new(gtk::Orientation::Vertical, 0);
        entry.add_css_class("ext-entry");
        entry.append(&key);
        entry.append(&body);
        let weak = Rc::downgrade(ui);
        let market = Rc::downgrade(market);
        let plugin = plugin.to_string();
        key.connect_toggled(move |key| {
            let open = key.is_active();
            body.set_visible(open);
            line.remove(&line.first_child().unwrap());
            let chevron = crate::icons::image(if open { "chevron-down" } else { "chevron-right" }, 12);
            chevron.add_css_class("ext-entry-chevron");
            chevron.set_valign(gtk::Align::Start);
            chevron.set_margin_top(3);
            line.prepend(&chevron);
            if !open || body.first_child().is_some() {
                return;
            }
            let (Some(ui), Some(market)) = (weak.upgrade(), market.upgrade()) else {
                return;
            };
            fetch(
                &ui,
                &market,
                &body,
                format!("{plugin}/skill/{name}"),
                json!({"plugin_id": plugin, "skill": name}),
                "Loading SKILL.md…",
                |ui, body, found| body.append(&market::markdown(ui, text(&found["skill"], "body"))),
            );
        });
        list.append(&entry);
    }
    list
}

fn tool_list(servers: &[Value]) -> gtk::Box {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
    if servers.is_empty() {
        list.append(&label("This plugin adds no MCP servers.", "dim"));
    }
    for server in servers {
        let block = gtk::Box::new(gtk::Orientation::Vertical, 10);
        block.add_css_class("ext-server");
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let name = label(text(server, "name"), "ext-server-name");
        name.set_hexpand(true);
        top.append(&name);
        let tools = tool_names(server);
        top.append(&label(&plural(tools.len(), "tool", "tools"), "ext-group-count"));
        block.append(&top);
        block.append(&market::prose(text(server, "description")));
        let tags: Vec<gtk::Label> = tools
            .iter()
            .map(|tool| market::chip(tool, "code"))
            .collect();
        block.append(&market::chips(&tags));
        list.append(&block);
    }
    let note = label(
        "Agents get these tools when they start or resume in a project with the plugin on.",
        "ext-hint",
    );
    note.set_wrap(true);
    list.append(&note);
    list
}

/// The rules every agent receives and the plugin's documents, fetched when the tab opens.
fn documents(ui: &Rc<Ui>, market: &Rc<Market>, plugin: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 20);
    let weak = Rc::downgrade(ui);
    let market = Rc::downgrade(market);
    let plugin = plugin.to_string();
    market::lazy(&page, move |page| {
        let (Some(ui), Some(market)) = (weak.upgrade(), market.upgrade()) else {
            return;
        };
        fetch(
            &ui,
            &market,
            page,
            plugin.clone(),
            json!({"plugin_id": plugin}),
            "Loading rules and documents…",
            fill_documents,
        );
    });
    page
}

/// Fills `container` from `plugin.get {payload}`, fetched once per engine connection and kept
/// under `key`. A failure leaves a way to ask again in place of the content.
fn fetch(
    ui: &Rc<Ui>,
    market: &Rc<Market>,
    container: &gtk::Box,
    key: String,
    payload: Value,
    loading: &'static str,
    fill: fn(&Ui, &gtk::Box, &Value),
) {
    let generation = ui.generation.get();
    let cached = market.documents(generation).get(&key).cloned();
    if let Some(found) = cached {
        fill(ui, container, &found);
        return;
    }
    container.append(&label(loading, "dim"));
    let (ui, market, container) = (ui.clone(), market.clone(), container.clone());
    glib::spawn_future_local(async move {
        let result = ui.call("plugin.get", payload.clone()).await;
        clear(&container);
        match result {
            Ok(found) => {
                fill(&ui, &container, &found);
                if ui.generation.get() == generation {
                    market.documents(generation).insert(key, found);
                }
            }
            Err(error) => {
                container.append(&paragraph(&explain_error("plugin.get", &error)));
                let again = crate::app::button("Try again", "quiet");
                again.set_halign(gtk::Align::Start);
                container.append(&again);
                let (weak, owner, target) = (Rc::downgrade(&ui), Rc::downgrade(&market), container.downgrade());
                again.connect_clicked(move |_| {
                    if let (Some(ui), Some(market), Some(container)) = (weak.upgrade(), owner.upgrade(), target.upgrade()) {
                        clear(&container);
                        fetch(&ui, &market, &container, key.clone(), payload.clone(), loading, fill);
                    }
                });
            }
        }
    });
}

fn fill_documents(ui: &Ui, page: &gtk::Box, found: &Value) {
    let rules = market::section("Rules every agent receives");
    rules.append(&market::markdown(ui, text(found, "instructions")));
    page.append(&rules);
    for doc in rows(found, "docs") {
        let block = market::section(text(&doc, "path"));
        block.append(&market::markdown(ui, text(&doc, "body")));
        page.append(&block);
    }
}

fn empty(ui: &Rc<Ui>, _market: &Rc<Market>) -> gtk::Widget {
    let retry = crate::app::button("Check again", "");
    let weak = Rc::downgrade(ui);
    retry.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.refresh_page();
        }
    });
    market::state_box(
        "plugins",
        "No plugins found",
        &label(STALE_ENGINE, "dim"),
        &[retry.upcast()],
    )
    .upcast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::BusError;

    #[test]
    fn a_stale_engine_is_read_from_the_error_code() {
        let unknown: Error = BusError::unknown_op("plugin.list").into();
        assert_eq!(explain_error("plugin.list", &unknown), STALE_ENGINE);
        assert_eq!(explain_error("skill.list", &unknown), "unknown op \"plugin.list\"");
        let worded: Error = BusError::invalid("plugin.not_found", "plugin. unknown field; not implemented").into();
        assert_eq!(explain_error("plugin.get", &worded), "plugin. unknown field; not implemented");
        assert_eq!(explain_error("plugin.get", &Error::Timeout), Error::Timeout.to_string());
    }

    #[test]
    fn a_refusal_says_the_engines_sentence_without_its_code() {
        let taken: Error = BusError::conflict("skill.name_exists", "that name is used by skill 4").into();
        assert_eq!(explain_error("skill.update", &taken), "Another skill already has that name.");
        let refused: Error = BusError::invalid("notify.not_found", "no such notification").into();
        assert!(refused.to_string().contains("notify.not_found"));
        assert_eq!(explain_error("notify.ack", &refused), "no such notification");
    }

    #[test]
    fn a_plugin_switched_on_is_no_longer_suggested() {
        assert!(suggested(&json!({"suggested_for":[3],"enabled_in":[]}), 3));
        assert!(!suggested(&json!({"suggested_for":[3],"enabled_in":[3]}), 3));
        assert!(!suggested(&json!({"suggested_for":[3],"enabled_in":[]}), 4));
    }
}
