//! One opt-in update attempt per application launch, with engine-owned execution.
use crate::app::{text, Ui};
use crate::tools::plugins::explain_error;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::rc::Rc;

/// A provider id as the rest of the app names it in a sentence.
fn name(provider: &str) -> &str {
    match provider {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    }
}

pub(crate) fn startup(ui: &Rc<Ui>) {
    if ui.provider_updates_checked.replace(true) {
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let Ok(config) = ui.call("settings.get", json!({"path":"providers"})).await else {
            // Nothing was attempted (the engine dropped right after connect); the next connect
            // tries again rather than skipping the update for the rest of the launch.
            ui.provider_updates_checked.set(false);
            return;
        };
        // One notice for both providers: a second message would replace the first unread.
        let mut outcomes = Vec::new();
        for provider in ["claude", "codex"] {
            if config["value"][provider]["auto_update"] == true {
                match ui
                    .call(
                        "provider.update",
                        json!({"provider":provider,"automatic":true}),
                    )
                    .await
                {
                    Ok(result) if result["started"] != true => {
                        outcomes.push(format!("{}: {}", name(provider), text(&result, "message")))
                    }
                    Err(error) => outcomes.push(format!("{} update: {}", name(provider), explain_error("provider.update", &error))),
                    _ => {}
                }
            }
        }
        if !outcomes.is_empty() {
            ui.show_error(&outcomes.join(" · "));
        }
    });
}

/// Settings' "Update now": `key` stays off until the engine answers. An update that started
/// is news; one the engine would not start (a live session, an installer it does not own) and
/// a refusal are not.
pub(crate) fn update(ui: &Rc<Ui>, provider: &'static str, key: &gtk::Button) {
    key.set_sensitive(false);
    let (ui, key) = (ui.clone(), key.clone());
    glib::spawn_future_local(async move {
        match ui
            .call("provider.update", json!({"provider":provider}))
            .await
        {
            Ok(result) if result["started"] == true => ui.show_info(text(&result, "message")),
            Ok(result) => ui.show_error(text(&result, "message")),
            Err(error) => ui.show_error(&format!("{} update: {}", name(provider), explain_error("provider.update", &error))),
        }
        key.set_sensitive(true);
    });
}

/// The engine sends `notify.new` with every finished update, and that repaints the Settings
/// page's provider cards; nothing else on screen shows a provider version. "updating" follows
/// the start that already said so, and would cover it in red.
pub(crate) fn event(ui: &Rc<Ui>, value: &Value) {
    let message = format!("{}: {}", name(text(value, "provider")), text(value, "message"));
    match text(value, "state") {
        "updating" => {}
        "complete" => ui.show_info(&message),
        _ => ui.show_error(&message),
    }
}
