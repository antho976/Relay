//! The shared shell of the Plugins and Skills pages: a header with the project the switches
//! apply to, a searchable and filterable catalog beside a detail pane, and the per-project
//! switches both pages use.
//!
//! A page is mounted once and keeps its own state (data, scope, selection, search, filter,
//! the open detail tab and lazily fetched documents), because engine events re-read it through
//! `tools::refresh`; a re-read that brings the same catalog, projects and palette repaints
//! nothing. Project and workspace events do not reach the page (app.rs only reloads the
//! registry), so the next re-read, including the one on returning to the page, picks up a
//! rename or a new project. `skill.changed` and `plugin.changed` for a project other than the
//! active one never reach the page (app.rs drops them), so a switch patches local data from the
//! engine's answer instead of waiting for an event.
use super::plugins::explain_error;
use crate::app::{button, clear, label, rows, text, Ui};
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

type Setup = fn(&Rc<Ui>, &Rc<Market>);

/// What differs between the Plugins and Skills pages.
pub(crate) struct Spec {
    pub page: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub icon: &'static str,
    /// Singular and plural, lower case.
    pub noun: (&'static str, &'static str),
    pub placeholder: &'static str,
    pub filters: &'static [&'static str],
    pub list_op: &'static str,
    pub list_payload: fn() -> Value,
    pub list_key: &'static str,
    pub enable_op: &'static str,
    pub id_key: &'static str,
    pub id: fn(&Value) -> String,
    /// Lower-case text the search words must all appear in.
    pub haystack: fn(&Value) -> String,
    /// Whether an item passes filter `index` for the scoped project.
    pub passes: fn(&Value, usize, i64) -> bool,
    pub group: Option<fn(&Value) -> String>,
    pub sort: fn(&Value) -> String,
    pub card: fn(&Rc<Ui>, &Rc<Market>, &Value) -> gtk::Widget,
    pub detail: fn(&Rc<Ui>, &Rc<Market>, &Value, &gtk::Box),
    pub empty: fn(&Rc<Ui>, &Rc<Market>) -> gtk::Widget,
    /// Fills the header actions and the banner once, when the page is built.
    pub setup: Option<Setup>,
}

pub(crate) struct Market {
    pub spec: &'static Spec,
    ui: Weak<Ui>,
    pub root: gtk::Box,
    pub actions: gtk::Box,
    pub banner: gtk::Box,
    pub banner_key: RefCell<Option<gtk::ToggleButton>>,
    notice: gtk::Box,
    notice_text: gtk::Label,
    body: gtk::Stack,
    error_text: gtk::Label,
    picker: gtk::DropDown,
    picker_ids: RefCell<Vec<(i64, String)>>,
    split: gtk::Paned,
    search: gtk::SearchEntry,
    filter_keys: Vec<(gtk::ToggleButton, gtk::Label)>,
    hint: gtk::Label,
    list_stack: gtk::Stack,
    list: gtk::ListBox,
    list_scroll: gtk::ScrolledWindow,
    nomatch_text: gtk::Label,
    detail: gtk::Box,
    detail_scroll: gtk::ScrolledWindow,
    pub data: RefCell<Vec<Value>>,
    loaded: Cell<bool>,
    pub scope: Cell<i64>,
    base: Cell<i64>,
    selected: RefCell<Option<String>>,
    filter: Cell<usize>,
    pub tab: RefCell<String>,
    /// Documents fetched on demand, and the engine connection they came from: see `documents`.
    cache: RefCell<HashMap<String, Value>>,
    cache_generation: Cell<u64>,
    quiet: Cell<bool>,
    rows: RefCell<Vec<(gtk::ListBoxRow, String)>>,
    /// What the list was last drawn against besides the catalog: see `context`.
    drawn: RefCell<Value>,
    /// The detail pane's item, scope and context, so a repaint keeps an unchanged pane.
    shown: RefCell<Option<(Value, i64, Value)>>,
}

/// Everything a page reads besides its catalog: the active project, project and workspace
/// names, and the palette the documents are highlighted in.
fn context(ui: &Ui) -> Value {
    json!([ui.project.get(), *ui.projects.borrow(), *ui.workspaces.borrow(), *ui.palette.borrow()])
}

thread_local! {
    static MARKETS: RefCell<HashMap<&'static str, Rc<Market>>> = RefCell::new(HashMap::new());
}

/// Loads the page's catalog and repaints it, building the page on first use.
pub(crate) async fn refresh(ui: &Rc<Ui>, spec: &'static Spec, project: i64) {
    let generation = ui.generation.get();
    let market = mount(ui, spec);
    if !market.loaded.get() {
        market.body.set_visible_child_name("loading");
    }
    let result = ui.call(spec.list_op, (spec.list_payload)()).await;
    if !super::current(ui, spec.page, project, generation) {
        return;
    }
    match result {
        Ok(value) => {
            let data = rows(&value, spec.list_key);
            let first = !market.loaded.replace(true);
            if first {
                // Named once there is data behind them: callers wait on these names.
                market.picker.set_widget_name(&format!("{}-project", spec.page));
                market.split.set_widget_name(&format!("{}-split", spec.page));
            }
            // Most events that reach the page changed nothing on it. Rebuilding every row
            // anyway drops focus and hover, fights the scroll and can swallow a switch press.
            if first || *market.data.borrow() != data || *market.drawn.borrow() != context(ui) {
                *market.data.borrow_mut() = data;
                market.render(ui);
            }
        }
        Err(error) => {
            let message = explain_error(spec.list_op, &error);
            if market.loaded.get() {
                market.say(&message);
            } else {
                market.error_text.set_text(&message);
                market.body.set_visible_child_name("error");
            }
        }
    }
}

fn mount(ui: &Rc<Ui>, spec: &'static Spec) -> Rc<Market> {
    let page = ui.pages[spec.page].clone();
    if let Some(market) = MARKETS.with(|m| m.borrow().get(spec.page).cloned()) {
        if market.root.parent().as_ref() == Some(page.upcast_ref::<gtk::Widget>()) {
            return market;
        }
    }
    let market = build(ui, spec);
    clear(&page);
    page.add_css_class("ext-page");
    page.set_spacing(0);
    page.append(&market.root);
    if let Some(setup) = spec.setup {
        setup(ui, &market);
    }
    MARKETS.with(|m| m.borrow_mut().insert(spec.page, market.clone()));
    market
}

fn build(ui: &Rc<Ui>, spec: &'static Spec) -> Rc<Market> {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("ext-root");
    root.set_vexpand(true);
    root.set_hexpand(true);

    // Header: identity, the project the switches apply to, and page actions.
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    head.add_css_class("ext-head");
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tile.add_css_class("ext-tile");
    // A box packs its child at the top of the min-height; a bin layout centres it both ways.
    tile.set_layout_manager(Some(gtk::BinLayout::new()));
    tile.set_valign(gtk::Align::Center);
    let glyph = crate::icons::image(spec.icon, 20);
    glyph.set_valign(gtk::Align::Center);
    glyph.set_halign(gtk::Align::Center);
    tile.append(&glyph);
    head.append(&tile);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    words.append(&label(spec.title, "ext-title"));
    let subtitle = label(spec.subtitle, "ext-subtitle");
    subtitle.set_wrap(true);
    subtitle.set_max_width_chars(88);
    words.append(&subtitle);
    head.append(&words);
    let scope = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    scope.add_css_class("ext-scope");
    scope.set_valign(gtk::Align::Center);
    scope.append(&label("Project", "ext-scope-label"));
    let picker = gtk::DropDown::from_strings(&[]);
    picker.set_enable_search(true);
    // Search filters on this expression; without one it matches every project.
    picker.set_expression(Some(gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    )));
    picker.add_css_class("settings-select");
    picker.set_tooltip_text(Some("The project every switch on this page applies to"));
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let name = label("", "");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_width_chars(1);
        name.set_max_width_chars(24);
        item.downcast_ref::<gtk::ListItem>()
            .unwrap()
            .set_child(Some(&name));
    });
    factory.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let value = item.item().and_downcast::<gtk::StringObject>().unwrap();
        let name = item.child().and_downcast::<gtk::Label>().unwrap();
        name.set_text(&value.string());
        name.set_tooltip_text(Some(&value.string()));
    });
    picker.set_factory(Some(&factory));
    picker.set_list_factory(Some(&factory));
    scope.append(&picker);
    head.append(&scope);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("ext-actions");
    actions.set_valign(gtk::Align::Center);
    head.append(&actions);
    root.append(&head);

    let banner = gtk::Box::new(gtk::Orientation::Vertical, 10);
    banner.add_css_class("ext-banner");
    banner.set_visible(false);
    root.append(&banner);

    let notice = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    notice.add_css_class("ext-notice");
    notice.set_visible(false);
    let notice_text = label("", "ext-notice-text");
    notice_text.set_wrap(true);
    notice_text.set_selectable(true);
    notice_text.set_hexpand(true);
    notice.append(&notice_text);
    let dismiss = crate::app::icon_button("close", "Dismiss");
    dismiss.set_valign(gtk::Align::Start);
    notice.append(&dismiss);
    root.append(&notice);

    // Catalog: search, filters, list.
    let catalog = gtk::Box::new(gtk::Orientation::Vertical, 0);
    catalog.add_css_class("ext-catalog");
    catalog.set_size_request(300, -1);
    let tools = gtk::Box::new(gtk::Orientation::Vertical, 8);
    tools.add_css_class("ext-tools");
    let search = gtk::SearchEntry::new();
    search.add_css_class("ext-search");
    search.set_placeholder_text(Some(spec.placeholder));
    search.set_hexpand(true);
    tools.append(&search);
    let filters = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    filters.add_css_class("ext-filters");
    filters.set_homogeneous(true);
    let mut filter_keys: Vec<(gtk::ToggleButton, gtk::Label)> = Vec::new();
    for caption in spec.filters {
        let key = gtk::ToggleButton::new();
        key.add_css_class("ext-filter");
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        inner.set_halign(gtk::Align::Center);
        inner.append(&label(caption, "ext-filter-name"));
        let count = label("0", "ext-count");
        inner.append(&count);
        key.set_child(Some(&inner));
        if let Some((first, _)) = filter_keys.first() {
            key.set_group(Some(first));
        }
        filters.append(&key);
        filter_keys.push((key, count));
    }
    if let Some((first, _)) = filter_keys.first() {
        first.set_active(true);
    }
    tools.append(&filters);
    let hint = label("", "ext-hint");
    hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
    tools.append(&hint);
    catalog.append(&tools);
    let list = gtk::ListBox::new();
    list.add_css_class("ext-list");
    list.set_selection_mode(gtk::SelectionMode::Single);
    let list_scroll = crate::app::scrolled(&list);
    list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let list_stack = gtk::Stack::new();
    list_stack.set_vexpand(true);
    list_stack.add_named(&list_scroll, Some("list"));
    let nomatch_text = label("", "dim");
    let clear_key = button("Clear search and filters", "");
    let nomatch = state_box(
        "search",
        &format!("No {} match", spec.noun.1),
        &nomatch_text,
        &[clear_key.clone().upcast()],
    );
    list_stack.add_named(&nomatch, Some("nomatch"));
    catalog.append(&list_stack);

    // Detail.
    let detail = gtk::Box::new(gtk::Orientation::Vertical, 18);
    detail.add_css_class("ext-detail");
    let detail_scroll = crate::app::scrolled(&detail);
    detail_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    detail_scroll.add_css_class("ext-detail-scroll");
    detail_scroll.set_size_request(380, -1);

    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.add_css_class("ext-split");
    split.set_vexpand(true);
    split.set_hexpand(true);
    split.set_resize_start_child(false);
    split.set_resize_end_child(true);
    split.set_shrink_start_child(false);
    split.set_shrink_end_child(false);
    split.set_start_child(Some(&catalog));
    split.set_end_child(Some(&detail_scroll));
    split.set_position(((ui.window.width() - 240) / 3).clamp(330, 440));

    let body = gtk::Stack::new();
    body.set_vexpand(true);
    body.set_hexpand(true);
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(20, 20);
    let loading = gtk::Box::new(gtk::Orientation::Vertical, 10);
    loading.add_css_class("ext-state");
    loading.set_valign(gtk::Align::Center);
    loading.set_halign(gtk::Align::Center);
    loading.append(&spinner);
    loading.append(&label(&format!("Loading {}…", spec.noun.1), "dim"));
    body.add_named(&loading, Some("loading"));
    let error_text = label("", "dim");
    let retry = button("Try again", "");
    let error = state_box(
        "refresh",
        &format!("Could not load {}", spec.noun.1),
        &error_text,
        &[retry.clone().upcast()],
    );
    body.add_named(&error, Some("error"));
    body.add_named(&split, Some("catalog"));
    body.set_visible_child_name("loading");
    root.append(&body);

    let market = Rc::new(Market {
        spec,
        ui: Rc::downgrade(ui),
        root,
        actions,
        banner,
        banner_key: RefCell::new(None),
        notice,
        notice_text,
        body,
        error_text,
        picker,
        picker_ids: RefCell::new(Vec::new()),
        split,
        search,
        filter_keys,
        hint,
        list_stack,
        list,
        list_scroll,
        nomatch_text,
        detail,
        detail_scroll,
        data: RefCell::new(Vec::new()),
        loaded: Cell::new(false),
        scope: Cell::new(0),
        base: Cell::new(i64::MIN),
        selected: RefCell::new(None),
        filter: Cell::new(0),
        tab: RefCell::new(String::new()),
        cache: RefCell::new(HashMap::new()),
        cache_generation: Cell::new(0),
        quiet: Cell::new(false),
        rows: RefCell::new(Vec::new()),
        drawn: RefCell::new(Value::Null),
        shown: RefCell::new(None),
    });
    market
        .body
        .add_named(&(spec.empty)(ui, &market), Some("empty"));
    market.search.set_key_capture_widget(Some(&market.root));

    let weak = Rc::downgrade(&market);
    dismiss.connect_clicked(move |_| {
        if let Some(market) = weak.upgrade() {
            market.notice.set_visible(false);
        }
    });
    let weak = Rc::downgrade(ui);
    retry.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.refresh_page();
        }
    });
    let weak = Rc::downgrade(&market);
    market.picker.connect_selected_notify(move |picker| {
        let Some(market) = weak.upgrade() else { return };
        if market.quiet.get() {
            return;
        }
        let chosen = market
            .picker_ids
            .borrow()
            .get(picker.selected() as usize)
            .map(|p| p.0);
        if let (Some(project), Some(ui)) = (chosen, market.ui.upgrade()) {
            market.scope.set(project);
            market.render(&ui);
        }
    });
    let weak = Rc::downgrade(&market);
    market.search.connect_search_changed(move |_| {
        if let Some(market) = weak.upgrade() {
            if let Some(ui) = market.ui.upgrade() {
                if !market.quiet.get() {
                    market.render(&ui);
                }
            }
        }
    });
    for (index, (key, _)) in market.filter_keys.iter().enumerate() {
        let weak = Rc::downgrade(&market);
        key.connect_toggled(move |key| {
            let Some(market) = weak.upgrade() else { return };
            if !key.is_active() || market.quiet.get() {
                return;
            }
            market.filter.set(index);
            if let Some(ui) = market.ui.upgrade() {
                market.render(&ui);
            }
        });
    }
    let weak = Rc::downgrade(&market);
    clear_key.connect_clicked(move |_| {
        let Some(market) = weak.upgrade() else { return };
        market.quiet.set(true);
        market.search.set_text("");
        market.filter.set(0);
        if let Some((first, _)) = market.filter_keys.first() {
            first.set_active(true);
        }
        market.quiet.set(false);
        if let Some(ui) = market.ui.upgrade() {
            market.render(&ui);
        }
    });
    let weak = Rc::downgrade(&market);
    market.list.connect_row_selected(move |_, row| {
        let Some(market) = weak.upgrade() else { return };
        if market.quiet.get() {
            return;
        }
        let Some(row) = row else { return };
        let id = market
            .rows
            .borrow()
            .iter()
            .find(|(r, _)| r == row)
            .map(|(_, id)| id.clone());
        let Some(id) = id else { return };
        *market.selected.borrow_mut() = Some(id.clone());
        let item = market.find(&id);
        if let Some(ui) = market.ui.upgrade() {
            market.show_detail(&ui, item.as_ref());
        }
    });
    market
}

impl Market {
    pub(crate) fn find(&self, id: &str) -> Option<Value> {
        self.data
            .borrow()
            .iter()
            .find(|item| (self.spec.id)(item) == id)
            .cloned()
    }

    /// Documents fetched for engine connection `generation`. A reconnect, perhaps to an engine
    /// rebuilt with newer bundled plugins, starts empty.
    pub(crate) fn documents(&self, generation: u64) -> std::cell::RefMut<'_, HashMap<String, Value>> {
        if self.cache_generation.replace(generation) != generation {
            self.cache.borrow_mut().clear();
        }
        self.cache.borrow_mut()
    }

    /// Shows a dismissible message above the catalog.
    pub(crate) fn say(&self, message: &str) {
        self.notice_text.set_text(message);
        self.notice.set_visible(true);
    }

    /// Opens the banner (the skill installer) and moves focus into it.
    pub(crate) fn reveal_banner(&self) {
        match self.banner_key.borrow().as_ref() {
            Some(key) => key.set_active(true),
            None => self.banner.set_visible(true),
        }
        self.banner.child_focus(gtk::DirectionType::TabForward);
    }

    /// Patches an item's projects from the engine's answer and repaints.
    pub(crate) fn set_enabled(self: &Rc<Self>, ui: &Rc<Ui>, id: &str, enabled_in: Value) {
        if !enabled_in.is_array() {
            return;
        }
        for item in self.data.borrow_mut().iter_mut() {
            if (self.spec.id)(item) == id {
                item["enabled_in"] = enabled_in.clone();
            }
        }
        self.render(ui);
    }

    fn sync_picker(&self, ui: &Ui) {
        let projects: Vec<(i64, String)> = ui
            .projects
            .borrow()
            .iter()
            .map(|p| (p["id"].as_i64().unwrap_or(0), text(p, "name").to_string()))
            .collect();
        // The scope follows the active project only when that changes, so a project picked
        // here survives every repaint.
        let active = ui.project.get();
        if active != self.base.get() {
            self.base.set(active);
            self.scope.set(active);
        }
        if !projects.iter().any(|p| p.0 == self.scope.get()) {
            let fallback = if projects.iter().any(|p| p.0 == active) {
                active
            } else {
                projects.first().map(|p| p.0).unwrap_or(0)
            };
            self.scope.set(fallback);
        }
        if *self.picker_ids.borrow() != projects {
            let names: Vec<&str> = projects.iter().map(|p| p.1.as_str()).collect();
            self.picker.set_model(Some(&gtk::StringList::new(&names)));
            *self.picker_ids.borrow_mut() = projects.clone();
        }
        let position = projects
            .iter()
            .position(|p| p.0 == self.scope.get())
            .map(|i| i as u32)
            .unwrap_or(gtk::INVALID_LIST_POSITION);
        if self.picker.selected() != position {
            self.picker.set_selected(position);
        }
        self.picker.set_sensitive(!projects.is_empty());
    }

    pub(crate) fn render(self: &Rc<Self>, ui: &Rc<Ui>) {
        let spec = self.spec;
        if !self.loaded.get() {
            return;
        }
        self.quiet.set(true);
        *self.drawn.borrow_mut() = context(ui);
        self.sync_picker(ui);
        let scope = self.scope.get();
        let data = self.data.borrow().clone();

        for (index, (key, count)) in self.filter_keys.iter().enumerate() {
            let n = data.iter().filter(|v| (spec.passes)(v, index, scope)).count();
            count.set_text(&n.to_string());
            key.set_sensitive(index == 0 || n > 0 || index == self.filter.get());
        }
        let project = project_name(ui, scope);
        self.hint.set_text(&if scope > 0 {
            format!("Switches apply to {project}")
        } else {
            "Add a project to switch these on".to_string()
        });
        self.hint.set_tooltip_text(Some(&self.hint.text()));

        let query = self.search.text().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let filter = self.filter.get();
        let mut shown: Vec<Value> = data
            .iter()
            .filter(|v| (spec.passes)(v, filter, scope))
            .filter(|v| {
                let hay = (spec.haystack)(v);
                words.iter().all(|w| hay.contains(w))
            })
            .cloned()
            .collect();
        let group_of = |v: &Value| spec.group.map(|g| g(v)).unwrap_or_default();
        shown.sort_by_key(|v| (group_of(v).to_lowercase(), (spec.sort)(v)));

        let offset = self.list_scroll.vadjustment().value();
        self.list.remove_all();
        self.rows.borrow_mut().clear();
        let mut current_group: Option<String> = None;
        for item in &shown {
            if spec.group.is_some() {
                let group = group_of(item);
                if current_group.as_deref() != Some(group.as_str()) {
                    let members = shown.iter().filter(|v| group_of(v) == group).count();
                    let on = shown
                        .iter()
                        .filter(|v| group_of(v) == group && enabled(v, scope))
                        .count();
                    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let name = label(&group, "ext-group-name");
                    name.set_hexpand(true);
                    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    head.append(&name);
                    head.append(&label(&format!("{on} of {members} on"), "ext-group-count"));
                    let row = gtk::ListBoxRow::new();
                    row.add_css_class("ext-group");
                    row.set_selectable(false);
                    row.set_activatable(false);
                    row.set_focusable(false);
                    row.set_child(Some(&head));
                    self.list.append(&row);
                    current_group = Some(group);
                }
            }
            let row = gtk::ListBoxRow::new();
            row.add_css_class("ext-row");
            row.set_child(Some(&(spec.card)(ui, self, item)));
            self.list.append(&row);
            self.rows.borrow_mut().push((row, (spec.id)(item)));
        }
        self.body
            .set_visible_child_name(if data.is_empty() { "empty" } else { "catalog" });
        let visible = if shown.is_empty() {
            self.nomatch_text.set_text(&if words.is_empty() {
                format!("Nothing passes the {} filter for {project}.", spec.filters[filter])
            } else {
                format!("Nothing matches \u{201c}{}\u{201d}.", self.search.text())
            });
            "nomatch"
        } else {
            "list"
        };
        self.list_stack.set_visible_child_name(visible);

        let wanted = self.selected.borrow().clone();
        let chosen = wanted
            .filter(|id| shown.iter().any(|v| &(spec.id)(v) == id))
            .or_else(|| shown.first().map(|v| (spec.id)(v)));
        *self.selected.borrow_mut() = chosen.clone();
        if let Some(id) = &chosen {
            if let Some((row, _)) = self.rows.borrow().iter().find(|(_, r)| r == id) {
                self.list.select_row(Some(row));
            }
        }
        self.quiet.set(false);
        let adjustment = self.list_scroll.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(offset));
        let item = chosen.and_then(|id| shown.into_iter().find(|v| (spec.id)(v) == id));
        self.show_detail(ui, item.as_ref());
    }

    fn show_detail(self: &Rc<Self>, ui: &Rc<Ui>, item: Option<&Value>) {
        let scope = self.scope.get();
        // The detail also shows project names and switches, and documents in the palette.
        let next = item.map(|v| (v.clone(), scope, context(ui)));
        if *self.shown.borrow() == next {
            return;
        }
        let same = match (self.shown.borrow().as_ref(), item) {
            (Some((old, _, _)), Some(new)) => (self.spec.id)(old) == (self.spec.id)(new),
            _ => false,
        };
        let adjustment = self.detail_scroll.vadjustment();
        let offset = if same { adjustment.value() } else { 0. };
        clear(&self.detail);
        match item {
            Some(item) => (self.spec.detail)(ui, self, item, &self.detail),
            None => {
                let (one, _) = self.spec.noun;
                let text = label(
                    "Pick one from the list to see what it adds and where it is on.",
                    "dim",
                );
                let placeholder =
                    state_box(self.spec.icon, &format!("No {one} selected"), &text, &[]);
                placeholder.set_vexpand(true);
                self.detail.append(&placeholder);
            }
        }
        *self.shown.borrow_mut() = next;
        glib::idle_add_local_once(move || adjustment.set_value(offset));
    }
}

pub(crate) fn enabled(item: &Value, project: i64) -> bool {
    item["enabled_in"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(project)))
}

pub(crate) fn project_name(ui: &Ui, project: i64) -> String {
    ui.projects
        .borrow()
        .iter()
        .find(|p| p["id"].as_i64() == Some(project))
        .map(|p| text(p, "name").to_owned())
        .unwrap_or_else(|| "no project".into())
}

/// Projects the item is on, counting only projects that still exist.
pub(crate) fn reach(ui: &Ui, item: &Value) -> (usize, usize) {
    let projects = ui.projects.borrow();
    let on = projects
        .iter()
        .filter(|p| p["id"].as_i64().is_some_and(|id| enabled(item, id)))
        .count();
    (on, projects.len())
}

/// A switch that turns `item` on or off for `project`. A refusal restores it; success patches
/// the page's data from the engine's answer.
pub(crate) fn toggle(
    ui: &Rc<Ui>,
    market: &Rc<Market>,
    item: &Value,
    project: i64,
    name: String,
) -> gtk::Switch {
    let spec = market.spec;
    let switch = gtk::Switch::new();
    switch.add_css_class("ext-switch");
    switch.add_css_class("settings-switch");
    switch.set_valign(gtk::Align::Center);
    switch.set_widget_name(&name);
    switch.set_active(enabled(item, project));
    switch.set_sensitive(project > 0);
    let title = text(item, "name").to_string();
    let tip = if project > 0 {
        format!("Use {title} in {}", project_name(ui, project))
    } else {
        "Add a project first".to_string()
    };
    switch.set_tooltip_text(Some(&tip));
    switch.update_property(&[gtk::accessible::Property::Label(&tip)]);
    let market = Rc::downgrade(market);
    let key = (spec.id)(item);
    enable_switch(ui, &switch, spec.enable_op, spec.id_key, item["id"].clone(), project, move |ui, result| {
        let Some(market) = market.upgrade() else { return };
        match result {
            Ok(item) => market.set_enabled(ui, &key, item["enabled_in"].clone()),
            Err(error) => market.say(&error),
        }
    });
    switch
}

/// Makes `switch` turn `id` on or off for `project` through `op` (`{id_key: id, project_id,
/// enabled}`), then hands `done` the engine's answer, a refusal as the sentence to show. A
/// refusal flips the switch back while it is insensitive, and an insensitive switch sends
/// nothing: without that guard the flip back would send the opposite request, fail the same
/// way, and loop for as long as it fails.
pub(crate) fn enable_switch(
    ui: &Rc<Ui>,
    switch: &gtk::Switch,
    op: &'static str,
    id_key: &'static str,
    id: Value,
    project: i64,
    done: impl Fn(&Rc<Ui>, Result<Value, String>) + 'static,
) {
    let weak = Rc::downgrade(ui);
    let done = Rc::new(done);
    switch.connect_state_set(move |switch, on| {
        if !switch.is_sensitive() {
            return glib::Propagation::Proceed;
        }
        let Some(ui) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        switch.set_sensitive(false);
        let switch = switch.clone();
        let mut payload = json!({"project_id": project, "enabled": on});
        payload[id_key] = id.clone();
        let done = done.clone();
        glib::spawn_future_local(async move {
            let result = ui.call(op, payload).await.map_err(|error| explain_error(op, &error));
            if result.is_err() {
                switch.set_active(!on);
            }
            done(&ui, result);
            switch.set_sensitive(true);
        });
        glib::Propagation::Proceed
    });
}

/// The big "On in X" panel at the top of a detail pane, with its lamp and switch.
pub(crate) fn enable_block(
    ui: &Rc<Ui>,
    market: &Rc<Market>,
    item: &Value,
    effect: (&str, &str),
    name: String,
) -> gtk::Box {
    let scope = market.scope.get();
    let on = scope > 0 && enabled(item, scope);
    let block = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    block.add_css_class("ext-enable");
    let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    lamp.add_css_class("ext-lamp");
    lamp.set_valign(gtk::Align::Center);
    if on {
        block.add_css_class("on");
        lamp.add_css_class("on");
    }
    block.append(&lamp);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 3);
    words.set_hexpand(true);
    let project = project_name(ui, scope);
    let title = if scope <= 0 {
        "No project to switch it on for".to_string()
    } else if on {
        format!("On in {project}")
    } else {
        format!("Off in {project}")
    };
    let heading = label(&title, "ext-enable-title");
    heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&heading);
    let (count, total) = reach(ui, item);
    let line = label(
        &format!(
            "{} On in {count} of {total} project{}.",
            if on { effect.0 } else { effect.1 },
            if total == 1 { "" } else { "s" }
        ),
        "ext-enable-text",
    );
    line.set_wrap(true);
    words.append(&line);
    block.append(&words);
    block.append(&toggle(ui, market, item, scope, name));
    block
}

/// Every project with its own switch for `item`, grouped by workspace.
pub(crate) fn project_switches(
    ui: &Rc<Ui>,
    market: &Rc<Market>,
    item: &Value,
    name: &dyn Fn(i64) -> String,
    suggested: Option<fn(&Value, i64) -> bool>,
) -> gtk::Box {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 22);
    list.add_css_class("ext-projects");
    let projects = ui.projects.borrow().clone();
    let workspaces = ui.workspaces.borrow().clone();
    if projects.is_empty() {
        list.append(&label("Add a project to switch this on for it.", "dim"));
        return list;
    }
    let mut groups: Vec<(String, Vec<Value>)> = workspaces
        .iter()
        .map(|w| {
            let members = projects
                .iter()
                .filter(|p| p["workspace_id"] == w["id"])
                .cloned()
                .collect();
            (text(w, "name").to_string(), members)
        })
        .filter(|(_, members): &(String, Vec<Value>)| !members.is_empty())
        .collect();
    let others: Vec<Value> = projects
        .iter()
        .filter(|p| !workspaces.iter().any(|w| w["id"] == p["workspace_id"]))
        .cloned()
        .collect();
    if !others.is_empty() {
        groups.push(("Other projects".to_string(), others));
    }
    let scope = market.scope.get();
    // One section per workspace: its name over a rounded card of project rows, as in Settings.
    for (caption, members) in groups {
        let group = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("ext-projects-head");
        let title = label(&caption, "ext-projects-name");
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        head.append(&title);
        let on = members
            .iter()
            .filter(|p| p["id"].as_i64().is_some_and(|id| enabled(item, id)))
            .count();
        head.append(&label(&format!("{on} of {} on", members.len()), "ext-group-count"));
        group.append(&head);
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("ext-card-list");
        group.append(&card);
        list.append(&group);
        for project in members {
            let id = project["id"].as_i64().unwrap_or(0);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("ext-project");
            if id == scope {
                row.add_css_class("scoped");
            }
            let lamp = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            lamp.add_css_class("ext-lamp");
            lamp.add_css_class("small");
            lamp.set_valign(gtk::Align::Center);
            if enabled(item, id) {
                lamp.add_css_class("on");
            }
            row.append(&lamp);
            let title = label(text(&project, "name"), "ext-project-name");
            title.set_hexpand(true);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_tooltip_text(Some(text(&project, "name")));
            row.append(&title);
            if suggested.is_some_and(|f| f(item, id)) {
                let hint = chip("Suggested", "hint");
                hint.set_tooltip_text(Some("This project's files match what the plugin is for"));
                hint.set_valign(gtk::Align::Center);
                row.append(&hint);
            }
            row.append(&toggle(ui, market, item, id, name(id)));
            card.append(&row);
        }
    }
    list
}

/// A pill-segmented tab bar over a stack. The open tab is remembered across items.
pub(crate) fn tabs(
    market: &Rc<Market>,
    pages: Vec<(&'static str, &str, Option<usize>, gtk::Widget)>,
) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar.add_css_class("ext-tabs");
    // Pill-segmented, as in the usage card: only as wide as its keys.
    bar.set_halign(gtk::Align::Start);
    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    stack.set_hhomogeneous(false);
    let wanted = market.tab.borrow().clone();
    let initial = if pages.iter().any(|p| p.0 == wanted) {
        wanted
    } else {
        pages.first().map(|p| p.0.to_string()).unwrap_or_default()
    };
    let mut first: Option<gtk::ToggleButton> = None;
    for (id, caption, count, widget) in pages {
        stack.add_named(&widget, Some(id));
        let key = gtk::ToggleButton::new();
        key.add_css_class("ext-tab");
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inner.append(&label(caption, "ext-tab-name"));
        if let Some(count) = count {
            inner.append(&label(&count.to_string(), "ext-count"));
        }
        key.set_child(Some(&inner));
        match &first {
            Some(first) => key.set_group(Some(first)),
            None => first = Some(key.clone()),
        }
        key.set_active(id == initial);
        let weak = Rc::downgrade(market);
        let target = stack.downgrade();
        key.connect_toggled(move |key| {
            if !key.is_active() {
                return;
            }
            if let (Some(market), Some(stack)) = (weak.upgrade(), target.upgrade()) {
                stack.set_visible_child_name(id);
                *market.tab.borrow_mut() = id.to_string();
            }
        });
        bar.append(&key);
    }
    if !initial.is_empty() {
        stack.set_visible_child_name(&initial);
    }
    root.append(&bar);
    root.append(&stack);
    root
}

/// Calls `fill` the first time `container` is shown, so a tab's documents load on demand.
pub(crate) fn lazy(container: &gtk::Box, fill: impl Fn(&gtk::Box) + 'static) {
    container.connect_map(move |container| {
        if container.first_child().is_none() {
            fill(container);
        }
    });
}

pub(crate) fn chip(caption: &str, class: &str) -> gtk::Label {
    let chip = label(caption, "ext-chip");
    if !class.is_empty() {
        chip.add_css_class(class);
    }
    chip
}

pub(crate) fn chips(items: &[gtk::Label]) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.add_css_class("ext-chips");
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_row_spacing(4);
    flow.set_column_spacing(4);
    flow.set_max_children_per_line(64);
    flow.set_valign(gtk::Align::Start);
    for item in items {
        flow.append(item);
    }
    flow
}

/// A centered state: an icon tile, a title, an explanation and actions.
pub(crate) fn state_box(
    icon: &str,
    title: &str,
    text: &gtk::Label,
    actions: &[gtk::Widget],
) -> gtk::Box {
    let state = gtk::Box::new(gtk::Orientation::Vertical, 10);
    state.add_css_class("ext-state");
    state.set_valign(gtk::Align::Center);
    state.set_halign(gtk::Align::Center);
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tile.add_css_class("ext-tile");
    tile.set_layout_manager(Some(gtk::BinLayout::new()));
    tile.set_halign(gtk::Align::Center);
    let glyph = crate::icons::image(icon, 18);
    glyph.set_valign(gtk::Align::Center);
    glyph.set_halign(gtk::Align::Center);
    tile.append(&glyph);
    state.append(&tile);
    let heading = label(title, "ext-state-title");
    heading.set_xalign(0.5);
    heading.set_justify(gtk::Justification::Center);
    state.append(&heading);
    text.set_wrap(true);
    text.set_max_width_chars(54);
    text.set_xalign(0.5);
    text.set_justify(gtk::Justification::Center);
    text.set_selectable(true);
    state.append(text);
    if !actions.is_empty() {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 8);
        row.set_halign(gtk::Align::Center);
        row.set_margin_top(6);
        for action in actions {
            row.append(action);
        }
        state.append(&row);
    }
    state
}

/// A titled block inside a detail tab.
pub(crate) fn section(title: &str) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 8);
    block.add_css_class("ext-section");
    block.append(&label(title, "ext-section-title"));
    block
}

pub(crate) fn prose(value: &str) -> gtk::Label {
    let prose = label(value, "ext-prose");
    prose.set_wrap(true);
    prose.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    prose.set_selectable(true);
    prose.set_max_width_chars(84);
    prose
}

/// Read-only Markdown in the editor's palette.
pub(crate) fn markdown(ui: &Ui, body: &str) -> sourceview5::View {
    use sourceview5::prelude::*;
    let buffer = sourceview5::Buffer::new(None);
    if let Some(language) = sourceview5::LanguageManager::default().language("markdown") {
        buffer.set_language(Some(&language));
    }
    let manager = sourceview5::StyleSchemeManager::default();
    if let Some(scheme) = manager
        .scheme(&format!("relay-{}", ui.palette.borrow()))
        .or_else(|| manager.scheme("relay-matte"))
    {
        buffer.set_style_scheme(Some(&scheme));
    }
    buffer.set_highlight_matching_brackets(false);
    buffer.set_text(body.trim_end());
    let view = sourceview5::View::with_buffer(&buffer);
    view.add_css_class("ext-source");
    // Rounded like the cards around it; the corners clip the text view's own ground.
    view.set_overflow(gtk::Overflow::Hidden);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_hexpand(true);
    view.set_top_margin(12);
    view.set_bottom_margin(12);
    view.set_left_margin(14);
    view.set_right_margin(14);
    view
}

/// A key/value grid of facts.
pub(crate) fn facts(rows: &[(&str, String, bool)]) -> gtk::Grid {
    let grid = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(7)
        .build();
    grid.add_css_class("ext-facts");
    for (line, (key, value, mono)) in rows.iter().enumerate() {
        let key = label(key, "ext-fact-key");
        key.set_valign(gtk::Align::Start);
        grid.attach(&key, 0, line as i32, 1, 1);
        let value = label(value, if *mono { "ext-fact-mono" } else { "ext-fact" });
        value.set_hexpand(true);
        value.set_wrap(true);
        value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        value.set_selectable(true);
        grid.attach(&value, 1, line as i32, 1, 1);
    }
    grid
}

/// A rounded monogram plate for items without their own artwork.
pub(crate) fn monogram(name: &str, large: bool) -> gtk::Label {
    let letters: String = name
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    let mark = gtk::Label::new(Some(if letters.is_empty() { "?" } else { &letters }));
    mark.add_css_class("ext-mark");
    if large {
        mark.add_css_class("large");
    }
    mark.set_valign(gtk::Align::Start);
    mark.set_halign(gtk::Align::Center);
    mark
}
