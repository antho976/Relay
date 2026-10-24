//! The sidebar's usage card, above you at the foot: one provider at a time, its plan, and the
//! 5-hour and weekly meters as bars, each with when it resets under it.
//!
//! It reads the rows and preferences `UsageState` keeps; which provider it shows and whether
//! it is folded live under `usage.card` in settings. Its sliders key opens the usage panel
//! beside it.
use super::*;

/// The card's widgets, built once: the tabs keep their focus across refreshes, and only the
/// body under them is rebuilt.
pub(super) struct Card {
    pub(super) root: gtk::Box,
    tabs: Vec<(gtk::ToggleButton, &'static str)>,
    options: gtk::Button,
    /// Re-reads the limits, as the panel's key does, without opening it.
    pub(super) refresh: gtk::Button,
    fold: gtk::Button,
    body: gtk::Box,
    /// Set while a render moves the tabs, so their handlers write nothing.
    syncing: Cell<bool>,
}

impl Card {
    pub(super) fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("usage-card");
        root.set_widget_name("sidebar-usage");
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        head.add_css_class("usage-card-head");
        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        strip.add_css_class("usage-card-tabs");
        strip.set_hexpand(true);
        strip.set_halign(gtk::Align::Start);
        let mut tabs: Vec<(gtk::ToggleButton, &'static str)> = Vec::new();
        for (provider, name) in PROVIDERS {
            let tab = gtk::ToggleButton::with_label(name);
            tab.set_widget_name(&format!("sidebar-usage-{provider}"));
            if let Some((first, _)) = tabs.first() {
                tab.set_group(Some(first));
            }
            strip.append(&tab);
            tabs.push((tab, provider));
        }
        head.append(&strip);
        let options = icon_button("sliders", "Usage details and display options");
        options.set_child(Some(&crate::icons::image("sliders", 15)));
        options.set_widget_name("sidebar-usage-options");
        options.set_valign(gtk::Align::Center);
        head.append(&options);
        let refresh = icon_button("refresh", "Re-read usage limits");
        refresh.set_widget_name("sidebar-usage-refresh");
        refresh.set_valign(gtk::Align::Center);
        head.append(&refresh);
        let fold = icon_button("chevron-up", "Fold usage");
        fold.set_widget_name("sidebar-usage-fold");
        fold.set_valign(gtk::Align::Center);
        head.append(&fold);
        root.append(&head);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("usage-card-body");
        root.append(&body);
        Self { root, tabs, options, refresh, fold, body, syncing: Cell::new(false) }
    }
}

/// The card's own names for the meters; the panel's are too terse for a sentence-case row.
fn title(meter: Meter) -> &'static str {
    match meter {
        Meter::FiveHour => "5 hour window",
        Meter::Weekly => "This week",
        Meter::Fable => "Fable this week",
    }
}

/// "Resets in 3h 41m", without the panel's wall clock: the card has no room for it.
fn reset_line(window: &Window, now: u64) -> String {
    if window.stale(now) {
        return "Reset since the last report".into();
    }
    match (window.resets_at, &window.resets_in) {
        (Some(at), _) => format!("Resets in {}", crate::relative::span(at - now)),
        (None, Some(label)) => format!("Resets in {label}"),
        (None, None) => "Reset time not reported".into(),
    }
}

/// The plan as the provider names it ("Claude Max"), when the engine reports one, and how old
/// the report is: a plan name alone never said whether the bars were current.
fn caption(item: Option<&Value>) -> String {
    let Some(item) = item else {
        return "Nothing reported yet".into();
    };
    let plan = text(item, "plan").trim();
    let age = parse_time(text(item, "taken_at")).map(ago);
    match (plan.is_empty(), age) {
        (false, Some(age)) => format!("{plan} · reported {age}"),
        (false, None) => plan.to_string(),
        (true, Some(age)) => format!("Reported {age}"),
        (true, None) => String::new(),
    }
}

fn card_row(meter: Meter, window: &Window, now: u64) -> gtk::Box {
    let stale = window.stale(now);
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.add_css_class("usage-card-row");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = label(title(meter), "usage-card-title");
    name.set_hexpand(true);
    name.set_xalign(0.0);
    top.append(&name);
    let pct = label(&format!("{:.0}%", window.pct), "usage-card-pct");
    if stale {
        pct.add_css_class("stale");
    } else if let Some(level) = level(window.pct) {
        pct.add_css_class(level);
    }
    top.append(&pct);
    row.append(&top);
    row.append(&bar(window.pct, stale, "usage-card-bar"));
    // Under its own bar, so the 5-hour reset never reads as the weekly one.
    let reset = label(&reset_line(window, now), "usage-card-reset");
    reset.set_xalign(0.0);
    reset.set_tooltip_text(Some(&reset_text(window, now)));
    if stale {
        reset.add_css_class("stale");
    }
    row.append(&reset);
    row
}

impl Ui {
    /// The tabs, the options key and the fold key. Called once, from `install_usage`.
    pub(super) fn install_usage_card(self: &Rc<Self>) {
        let card = &self.usage.card;
        for (tab, provider) in &card.tabs {
            let weak = Rc::downgrade(self);
            let provider = *provider;
            tab.connect_toggled(move |tab| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if tab.is_active() && !ui.usage.card.syncing.get() {
                    ui.set_card_pref("provider", json!(provider));
                }
            });
        }
        let weak = Rc::downgrade(self);
        card.options.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.usage_panel();
            }
        });
        let weak = Rc::downgrade(self);
        card.refresh.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.refresh_usage(true);
            }
        });
        let weak = Rc::downgrade(self);
        card.fold.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let folded = ui.usage.prefs.borrow()["card"]["folded"].as_bool().unwrap_or(false);
                ui.set_card_pref("folded", json!(!folded));
            }
        });
    }

    /// Writes `usage.card.<key>` unless it already holds `value`, then shows it.
    fn set_card_pref(self: &Rc<Self>, key: &str, value: Value) {
        if self.usage.prefs.borrow()["card"][key] == value {
            return;
        }
        put(&mut self.usage.prefs.borrow_mut(), &format!("card.{key}"), value.clone());
        self.render_usage_card();
        let ui = self.clone();
        let payload = json!({"path": format!("usage.card.{key}"), "value": value});
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call("settings.set", payload).await {
                ui.show_error(&e.to_string());
                ui.reload_usage_prefs();
            }
        });
    }

    pub(super) fn render_usage_card(&self) {
        let now = now();
        let state = &self.usage;
        let card = &state.card;
        let prefs = state.prefs.borrow().clone();
        let visible = state.visible();
        let chosen = prefs["card"]["provider"].as_str().unwrap_or("");
        let shown = visible
            .iter()
            .position(|(id, _, _)| *id == chosen)
            .or((!visible.is_empty()).then_some(0));
        // A hidden or unreported provider falls back to the first shown one without writing
        // that fallback down: the choice comes back when its provider does.
        card.syncing.set(true);
        for (tab, provider) in &card.tabs {
            tab.set_visible(visible.iter().any(|(id, _, _)| id == provider));
            if shown.is_some_and(|i| visible[i].0 == *provider) && !tab.is_active() {
                tab.set_active(true);
            }
        }
        card.syncing.set(false);
        let folded = prefs["card"]["folded"].as_bool().unwrap_or(false);
        card.fold.set_child(Some(&crate::icons::image(if folded { "chevron-down" } else { "chevron-up" }, 14)));
        card.fold.set_tooltip_text(Some(if folded { "Unfold usage" } else { "Fold usage" }));
        card.body.set_visible(!folded);
        clear(&card.body);
        let Some(index) = shown else {
            let note = label("Limits appear after an agent's next reply in a Relay session.", "usage-card-caption");
            note.set_wrap(true);
            note.set_xalign(0.0);
            card.body.append(&note);
            return;
        };
        let (provider, _, item) = &visible[index];
        let caption = label(&caption(item.as_ref()), "usage-card-caption");
        caption.set_xalign(0.0);
        // A refresh that failed says so where the plan goes; the meters keep the last read.
        if let Some(error) = state.failed.borrow().as_ref() {
            caption.set_text("Couldn't re-read usage");
            caption.add_css_class("failed");
            caption.set_tooltip_text(Some(&format!("The last refresh failed: {error}")));
        }
        card.body.append(&caption);
        let found = item.as_ref().map(windows).unwrap_or_default();
        let mut shown_any = false;
        for &meter in meters(provider) {
            if !enabled(&prefs, provider, meter.key()) {
                continue;
            }
            let Some(window) = found.iter().find(|w| classify(&w.name) == Some(meter)) else {
                continue;
            };
            card.body.append(&card_row(meter, window, now));
            shown_any = true;
        }
        if !shown_any && item.is_some() {
            let note = label("No shown meter reported", "usage-card-caption");
            note.set_xalign(0.0);
            card.body.append(&note);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_names_the_plan_or_the_report_age() {
        assert_eq!(caption(None), "Nothing reported yet");
        assert!(caption(Some(&json!({"plan": "Claude Max", "taken_at": "2026-10-08T01:26:55Z"}))).starts_with("Claude Max · reported "));
        assert_eq!(caption(Some(&json!({"plan": "Claude Max"}))), "Claude Max");
        assert!(caption(Some(&json!({"taken_at": "2026-10-08T01:26:55Z"}))).starts_with("Reported "));
        assert_eq!(caption(Some(&json!({}))), "");
    }

    #[test]
    fn the_card_reset_line_drops_the_wall_clock() {
        let window = |resets_at| Window { name: "five_hour".into(), pct: 12., resets_at, resets_in: None };
        assert_eq!(reset_line(&window(Some(100 + 3 * 3600 + 41 * 60)), 100), "Resets in 3h 41m");
        assert_eq!(reset_line(&window(Some(90)), 100), "Reset since the last report");
    }
}
