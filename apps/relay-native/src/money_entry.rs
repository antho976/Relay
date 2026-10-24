//! The add / edit entry sheet, the add account sheet, and the record a value sheet.
use super::formatter;
use super::pages::{human_date, today};
use crate::app::{button, label, rows, text, Ui};
use crate::panel::Panel;
use gtk::prelude::*;
use gtk4 as gtk;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const KINDS: [(&str, &str); 3] = [("EXPENSE", "Expense"), ("INCOME", "Income"), ("TRANSFER", "Transfer")];

thread_local! {
    /// The account each kind of entry was last saved against (`EXPENSE`, `INCOME`, `TRANSFER`): a
    /// new one starts there. The category is never carried over to a new sheet, so nothing is
    /// filed where the person did not choose; Save and add another keeps it in the open sheet.
    static LAST_ACCOUNT: RefCell<HashMap<String, i64>> = RefCell::new(HashMap::new());
    /// Whether [`LAST_ACCOUNT`] was seeded from the ledger's newest entries yet.
    static SEEDED: Cell<bool> = const { Cell::new(false) };
}

/// Seeds [`LAST_ACCOUNT`] from a `money.tx.list` answer (newest first), once per run: before the
/// first save here, the sheet starts on the account each kind was last logged against anywhere.
fn seed_accounts(list: &Value) {
    SEEDED.with(|s| s.set(true));
    LAST_ACCOUNT.with(|last| {
        let mut last = last.borrow_mut();
        for tx in rows(list, "transactions") {
            if let Some(account) = tx["account_id"].as_i64() {
                last.entry(text(&tx, "type").to_string()).or_insert(account);
            }
        }
    });
}

fn last_account(kind: &str) -> Option<i64> {
    LAST_ACCOUNT.with(|last| last.borrow().get(kind).copied())
}

fn field(body: &gtk::Box, caption: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 6);
    block.append(&label(caption, "money-label"));
    block.append(widget);
    body.append(&block);
    block
}

/// A drop-down over `(id, name)` pairs, selecting `selected` when present. Typing in its list
/// finds a row, as in Entries' filters.
fn picker(items: &[(i64, String)], selected: Option<i64>) -> gtk::DropDown {
    let names: Vec<&str> = items.iter().map(|(_, n)| n.as_str()).collect();
    let dropdown = gtk::DropDown::from_strings(&names);
    dropdown.add_css_class("money-picker");
    dropdown.set_enable_search(true);
    // Search filters on this expression; without one it matches every row.
    dropdown.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
    if let Some(index) = selected.and_then(|id| items.iter().position(|(i, _)| *i == id)) {
        dropdown.set_selected(index as u32);
    }
    dropdown
}

/// The category key's face: the chosen category's badge and name, or what to do while there is
/// none.
fn show_choice(key: &gtk::MenuButton, choice: Option<&super::pages::Choice>) {
    let face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    face.set_hexpand(true);
    let name = match choice {
        Some(c) => {
            face.append(&super::pages::badge_sized(&c.icon, c.color, 20));
            label(&c.name, "")
        }
        None => label("Choose a category", "money-placeholder"),
    };
    name.set_hexpand(true);
    name.set_xalign(0.0);
    face.append(&name);
    // The drop-downs' chevron, beside them in the sheet.
    face.append(&crate::icons::image_with_stroke("chevron-down", 16, 2.0));
    key.set_child(Some(&face));
}

fn chosen(dropdown: &gtk::DropDown, items: &[(i64, String)]) -> Option<i64> {
    items.get(dropdown.selected() as usize).map(|(id, _)| *id)
}

/// "−$14.98", "+$2,150.00" or "$50.00": an entry's amount signed by its kind.
pub(super) fn signed(kind: &str, amount: i64) -> String {
    let fmt = formatter();
    match kind {
        "EXPENSE" => format!("−{}", fmt.format(amount)),
        "INCOME" => format!("+{}", fmt.format(amount)),
        _ => fmt.format(amount),
    }
}

/// Opens the entry sheet: a new entry, or `tx` to edit.
pub fn open(ui: &Rc<Ui>, tx: Option<Value>) {
    let title = if tx.is_some() { "Edit entry" } else { "Add entry" };
    let Some(panel) = Panel::toggle(ui, title, 460) else { return };
    panel.add_css_class("money-sheet");
    let loading = label("Reading accounts and categories…", "money-muted");
    panel.body.append(&loading);
    panel.present();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        // The first new entry of a run also reads the newest entries, for the account to start on.
        let lists = if tx.is_none() && !SEEDED.with(Cell::get) {
            let (lists, recent) = tokio::join!(ui.call("money.lists", json!({})), ui.call("money.tx.list", json!({"limit":60})));
            // Early in a period this one may hold nothing yet: the one before says where the
            // person last spent from.
            let empty = |page: &Value| page["transactions"].as_array().is_none_or(|rows| rows.is_empty());
            let recent = match recent {
                Ok(page) if empty(&page) => ui.call("money.tx.list", json!({"limit":60,"period_offset":-1})).await,
                other => other,
            };
            if let Ok(recent) = &recent {
                seed_accounts(recent);
            }
            lists
        } else {
            ui.call("money.lists", json!({})).await
        };
        loading.unparent();
        match lists {
            Ok(lists) => {
                super::remember_currency(&lists);
                form(&ui, &panel, &lists, tx);
            }
            Err(e) => {
                let l = label(&super::pages::unavailable(&e), "money-muted");
                l.set_wrap(true);
                panel.body.append(&l);
            }
        }
    });
}

fn form(ui: &Rc<Ui>, panel: &Rc<Panel>, lists: &Value, tx: Option<Value>) {
    let fmt = formatter();
    let body = &panel.body;
    let accounts: Vec<(i64, String)> = rows(lists, "accounts")
        .iter()
        .filter(|a| a["archived"] != true || tx.as_ref().is_some_and(|t| t["account_id"] == a["id"] || t["to_account_id"] == a["id"]))
        .map(|a| (a["id"].as_i64().unwrap_or(0), text(a, "name").to_string()))
        .collect();
    if accounts.is_empty() {
        let none = label(
            "There is no account to log against yet. Add one first; the first account also brings Tally's usual categories.",
            "money-muted",
        );
        none.set_wrap(true);
        body.append(&none);
        let add = button("Add an account", "primary");
        add.add_css_class("money-hero-action");
        let weak = Rc::downgrade(ui);
        let sheet = Rc::downgrade(panel);
        add.connect_clicked(move |_| {
            if let (Some(ui), Some(sheet)) = (weak.upgrade(), sheet.upgrade()) {
                sheet.close();
                account(&ui);
            }
        });
        body.append(&add);
        return;
    }
    let categories = rows(lists, "categories");
    let kind = Rc::new(RefCell::new(tx.as_ref().map_or("EXPENSE", |t| text(t, "type")).to_string()));

    // Expense, Income, Transfer.
    let segments = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    segments.add_css_class("money-segments");
    segments.set_homogeneous(true);
    let mut group: Option<gtk::ToggleButton> = None;
    let mut toggles = Vec::new();
    for (value, caption) in KINDS {
        let key = gtk::ToggleButton::with_label(caption);
        key.set_widget_name(&format!("money-kind-{}", value.to_lowercase()));
        key.set_group(group.as_ref());
        if group.is_none() {
            group = Some(key.clone());
        }
        key.set_active(*kind.borrow() == value);
        segments.append(&key);
        toggles.push((value, key));
    }
    body.append(&segments);

    // The amount, in the figure voice, with the currency beside it.
    let amount_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    amount_row.add_css_class("money-amount-row");
    let amount = gtk::Entry::new();
    amount.add_css_class("money-amount-field");
    amount.set_widget_name("money-entry-amount");
    amount.set_hexpand(true);
    amount.set_placeholder_text(Some(&fmt.format_input(0)));
    amount.set_input_purpose(gtk::InputPurpose::Number);
    amount.update_property(&[gtk::accessible::Property::Label("Amount")]);
    if let Some(t) = &tx {
        amount.set_text(&fmt.format_input(t["amount"].as_i64().unwrap_or(0)));
    }
    amount_row.append(&amount);
    let code = label(&fmt.currency, "money-suffix");
    code.set_valign(gtk::Align::Center);
    amount_row.append(&code);
    field(body, "Amount", &amount_row);
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);

    // Category, of the kind that matches; transfers take none. A new entry starts with none
    // chosen: the person picks it, so nothing is filed under the list's first row by default.
    // The key opens a list to type into (`pages::chooser`).
    let category_items: Rc<RefCell<Vec<super::pages::Choice>>> = Rc::default();
    let category_choice: Rc<Cell<Option<i64>>> = Rc::default();
    let category = gtk::MenuButton::new();
    category.add_css_class("money-picker");
    category.set_widget_name("money-entry-category");
    category.update_property(&[gtk::accessible::Property::Label("Category")]);
    let category_popover = gtk::Popover::new();
    category_popover.add_css_class("money-menu");
    category.set_popover(Some(&category_popover));
    {
        // Built each time it opens, so the tick follows the choice.
        let (items, choice) = (category_items.clone(), category_choice.clone());
        category.set_create_popup_func(move |key| {
            let Some(popover) = key.popover() else { return };
            let (items_for_pick, choice_for_pick, shown, sheet_popover) = (items.clone(), choice.clone(), key.downgrade(), popover.downgrade());
            let pick: Rc<dyn Fn(i64)> = Rc::new(move |id| {
                choice_for_pick.set(Some(id));
                if let Some(key) = shown.upgrade() {
                    show_choice(&key, items_for_pick.borrow().iter().find(|c| c.id == id));
                }
                if let Some(popover) = sheet_popover.upgrade() {
                    popover.popdown();
                }
            });
            popover.set_child(Some(&super::pages::chooser(&items.borrow(), choice.get(), pick)));
        });
    }
    let category_block = field(body, "Category", &category);
    let fill_categories = {
        let (items, choice, key, block, kind) = (category_items.clone(), category_choice.clone(), category.clone(), category_block.clone(), kind.clone());
        let selected = tx.as_ref().and_then(|t| t["category_id"].as_i64());
        move || {
            let kind = kind.borrow().clone();
            block.set_visible(kind != "TRANSFER");
            let list = super::pages::choices(&categories, &kind, selected);
            // The entry's own category, when it is of this kind; otherwise none until picked.
            let current = selected.filter(|id| list.iter().any(|c| c.id == *id));
            choice.set(current);
            show_choice(&key, list.iter().find(|c| Some(c.id) == current));
            *items.borrow_mut() = list;
        }
    };
    fill_categories();

    // The account: the entry's own, else the one this kind was last saved against, else the first.
    let first_kind = kind.borrow().clone();
    let start = tx.as_ref().and_then(|t| t["account_id"].as_i64()).or_else(|| last_account(&first_kind)).filter(|id| accounts.iter().any(|a| a.0 == *id));
    let from = picker(&accounts, start.or_else(|| accounts.first().map(|a| a.0)));
    from.set_widget_name("money-entry-account");
    from.update_property(&[gtk::accessible::Property::Label("Account")]);
    let from_block = field(body, "Account", &from);
    let to = picker(&accounts, tx.as_ref().and_then(|t| t["to_account_id"].as_i64()).or_else(|| accounts.iter().map(|a| a.0).find(|id| Some(*id) != start.or(accounts.first().map(|a| a.0)))));
    to.set_widget_name("money-entry-to-account");
    let to_block = field(body, "To account", &to);
    to_block.set_visible(*kind.borrow() == "TRANSFER");
    let from_caption = from_block.first_child().and_downcast::<gtk::Label>();
    let set_from_caption = move |transfer: bool| {
        if let Some(caption) = &from_caption {
            caption.set_text(if transfer { "FROM ACCOUNT" } else { "ACCOUNT" });
        }
    };
    set_from_caption(*kind.borrow() == "TRANSFER");

    // The date: today unless changed, picked on a calendar.
    let day = Rc::new(RefCell::new(tx.as_ref().map_or_else(today, |t| text(t, "date").to_string())));
    let date_key = day_picker(&day, "money-entry-date");
    field(body, "Date", &date_key);

    let note = gtk::Entry::new();
    note.add_css_class("money-field");
    note.set_widget_name("money-entry-note");
    note.set_placeholder_text(Some("What it was, if the category does not say"));
    if let Some(t) = &tx {
        note.set_text(text(t, "note"));
    }
    field(body, "Note", &note);

    // What Save and add another just saved, with its Undo: the sheet stays open for the next one.
    let added = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    added.add_css_class("money-added");
    added.set_visible(false);
    let added_text = label("", "money-added-text");
    added_text.set_hexpand(true);
    added_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    added.append(&added_text);
    let added_undo = button("Undo", "money-text-action");
    added_undo.set_widget_name("money-entry-added-undo");
    added.append(&added_undo);
    body.append(&added);
    let added_id: Rc<RefCell<Option<(Value, String)>>> = Rc::default();
    {
        let (ui, added_id, added_text) = (ui.clone(), added_id.clone(), added_text.clone());
        added_undo.connect_clicked(move |key| {
            let Some((id, said)) = added_id.borrow_mut().take() else { return };
            key.set_visible(false);
            let (ui, added_text) = (ui.clone(), added_text.clone());
            glib::spawn_future_local(async move {
                match ui.call("money.tx.delete", json!({"id":id})).await {
                    Ok(_) => added_text.set_text(&format!("Took back {said}")),
                    Err(e) => ui.show_error(&super::pages::unavailable(&e)),
                }
                ui.refresh_page();
            });
        });
    }

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let save = button("", "primary");
    save.add_css_class("money-hero-action");
    save.set_widget_name("money-entry-save");
    save.set_hexpand(true);
    let caption = move |kind: &str| match kind {
        "INCOME" => "Save income",
        "TRANSFER" => "Save transfer",
        _ => "Save expense",
    };
    save.set_label(caption(&kind.borrow()));
    // A new entry can be saved with the sheet kept open for the next: the same day, account and
    // category, the amount and note cleared.
    let another = button("Save and add another", "money-secondary");
    another.set_widget_name("money-entry-another");
    another.set_tooltip_text(Some("Save, then log the next one on the same day, account and category · Ctrl Enter"));
    if let Some(t) = &tx {
        let delete = button("Delete", "money-destructive");
        delete.set_widget_name("money-entry-delete");
        let weak = Rc::downgrade(ui);
        let sheet = Rc::downgrade(panel);
        let gone = t.clone();
        // The page's toast offers Undo, on Tally's pages and the Threads page alike, so it
        // deletes at once; a page with no toast would offer none, so there it asks first.
        let asking = Rc::downgrade(ui);
        let needed = move || asking.upgrade().is_some_and(|ui| !super::has_toast(&ui.page.borrow()));
        crate::app::confirm_inline_if(&delete, "Delete this entry?", needed, move |_| {
            if let (Some(ui), Some(sheet)) = (weak.upgrade(), sheet.upgrade()) {
                sheet.close();
                super::pages::delete(&ui, gone.clone());
            }
        });
        actions.append(&delete);
    } else {
        actions.append(&another);
    }
    actions.append(&save);
    body.append(&actions);

    for (value, key) in &toggles {
        let (kind, fill, to_block, save, value) = (kind.clone(), fill_categories.clone(), to_block.clone(), save.clone(), *value);
        let set_from_caption = set_from_caption.clone();
        key.connect_toggled(move |key| {
            if !key.is_active() || *kind.borrow() == value {
                return;
            }
            *kind.borrow_mut() = value.to_string();
            fill();
            to_block.set_visible(value == "TRANSFER");
            set_from_caption(value == "TRANSFER");
            save.set_label(caption(value));
        });
    }

    // Saves the sheet. With `another`, a new entry keeps the sheet open for the next one;
    // otherwise the sheet closes and the page's toast says what was saved, with Undo.
    let submit = {
        let ui = ui.clone();
        let sheet = Rc::downgrade(panel);
        // The keys and fields whose own handlers hold `submit` are held weakly here, or each
        // would keep the other alive and every sheet opened would stay in memory (fffb19e).
        let (amount_ref, note_ref, save_ref, another_ref) = (amount.downgrade(), note.downgrade(), save.downgrade(), another.downgrade());
        let (problem, kind, category, category_items, category_choice, from, to, day) =
            (problem.clone(), kind.clone(), category.clone(), category_items.clone(), category_choice.clone(), from.clone(), to.clone(), day.clone());
        let accounts = accounts.clone();
        let before = tx.clone();
        let (added, added_text, added_undo, added_id) = (added.clone(), added_text.clone(), added_undo.clone(), added_id.clone());
        move |keep_open: bool| {
            let (Some(amount), Some(note), Some(save), Some(another)) = (amount_ref.upgrade(), note_ref.upgrade(), save_ref.upgrade(), another_ref.upgrade()) else { return };
            let keep_open = keep_open && before.is_none();
            let refuse = |text: &str, culprit: &gtk::Widget| {
                problem.set_text(text);
                problem.set_visible(true);
                if culprit == amount.upcast_ref::<gtk::Widget>() {
                    amount.add_css_class("error");
                }
                culprit.grab_focus();
            };
            let Some(value) = formatter().parse(&amount.text()).filter(|v| *v > 0) else {
                refuse("Type an amount above zero, like 12.50.", amount.upcast_ref());
                return;
            };
            amount.remove_css_class("error");
            let kind = kind.borrow().clone();
            let Some(account) = chosen(&from, &accounts) else {
                refuse("Pick the account this came from.", from.upcast_ref());
                return;
            };
            let mut payload = json!({"type":kind,"amount":value,"date":*day.borrow(),"account_id":account});
            let what = if kind == "TRANSFER" {
                let target = chosen(&to, &accounts);
                if target.is_none_or(|t| t == account) {
                    refuse("A transfer goes to a different account.", to.upcast_ref());
                    return;
                }
                payload["to_account_id"] = json!(target);
                String::from("Transfer")
            } else {
                let Some(c) = category_choice.get() else {
                    refuse("Choose a category. Tally keeps every expense and income in one.", category.upcast_ref());
                    return;
                };
                payload["category_id"] = json!(c);
                category_items.borrow().iter().find(|item| item.id == c).map_or_else(|| super::pages::word(&kind), |item| item.name.clone())
            };
            payload["note"] = json!(note.text().trim());
            let said = format!("{what} {}", signed(&kind, value));
            let op = match &before {
                Some(t) => {
                    payload["id"] = t["id"].clone();
                    "money.tx.update"
                }
                None => "money.tx.add",
            };
            problem.set_visible(false);
            save.set_sensitive(false);
            another.set_sensitive(false);
            let (ui, sheet, problem, save, another, before) = (ui.clone(), sheet.clone(), problem.clone(), save.clone(), another.clone(), before.clone());
            let (amount, note, added, added_text, added_undo, added_id) = (amount.clone(), note.clone(), added.clone(), added_text.clone(), added_undo.clone(), added_id.clone());
            glib::spawn_future_local(async move {
                match ui.call(op, payload).await {
                    Ok(saved) => {
                        LAST_ACCOUNT.with(|last| last.borrow_mut().insert(kind.clone(), account));
                        ui.refresh_page();
                        if keep_open {
                            // The same sheet, ready for the next amount; what went in shows above
                            // the keys with its own Undo.
                            amount.set_text("");
                            note.set_text("");
                            save.set_sensitive(true);
                            another.set_sensitive(true);
                            *added_id.borrow_mut() = Some((saved["id"].clone(), said.clone()));
                            added_text.set_text(&format!("Added {said}"));
                            added_undo.set_visible(true);
                            added.set_visible(true);
                            amount.grab_focus();
                            return;
                        }
                        if let Some(sheet) = sheet.upgrade() {
                            sheet.close();
                        }
                        let undo = undo_save(&ui, &saved, before.as_ref());
                        super::toast(&ui, &format!("{} {said}", if before.is_some() { "Saved" } else { "Added" }), Some(undo));
                    }
                    Err(e) => {
                        problem.set_text(&super::pages::unavailable(&e));
                        problem.set_visible(true);
                        save.set_sensitive(true);
                        another.set_sensitive(true);
                    }
                }
            });
        }
    };
    let submit = Rc::new(submit);
    let run = submit.clone();
    save.connect_clicked(move |_| run(false));
    let run = submit.clone();
    another.connect_clicked(move |_| run(true));
    let run = submit.clone();
    amount.connect_activate(move |_| run(false));
    let run = submit.clone();
    note.connect_activate(move |_| run(false));
    // Ctrl Enter anywhere in the sheet: save and add another (on an edit, save).
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, mods| {
        use gtk::gdk::{Key, ModifierType};
        let mods = mods & (ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK | ModifierType::ALT_MASK | ModifierType::META_MASK);
        if mods == ModifierType::CONTROL_MASK && matches!(key, Key::Return | Key::KP_Enter | Key::ISO_Enter) {
            submit(true);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    body.add_controller(keys);
    amount.grab_focus();
}

/// The Undo for a save: a new entry is deleted; an edited one gets its fields back as they were.
fn undo_save(ui: &Rc<Ui>, saved: &Value, before: Option<&Value>) -> super::Action {
    let weak = Rc::downgrade(ui);
    let (op, payload) = match before {
        None => ("money.tx.delete", json!({"id":saved["id"]})),
        Some(t) => {
            let mut back = json!({"id":t["id"],"type":t["type"],"amount":t["amount"],"date":t["date"],"account_id":t["account_id"],"note":t["note"]});
            for key in ["to_account_id", "category_id"] {
                if !t[key].is_null() {
                    back[key] = t[key].clone();
                }
            }
            // An absent category keeps the one the edit gave it: say it goes.
            if t["category_id"].is_null() {
                back["clear_category"] = json!(true);
            }
            ("money.tx.update", back)
        }
    };
    Rc::new(move || {
        let Some(ui) = weak.upgrade() else { return };
        let payload = payload.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = ui.call(op, payload).await {
                ui.show_error(&super::pages::unavailable(&e));
            }
            ui.refresh_page();
        });
    })
}

fn parse_day(value: &str) -> Option<glib::DateTime> {
    let mut parts = value.split('-').map(|p| p.parse::<i32>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    glib::DateTime::from_local(y, m, d, 0, 0, 0.0).ok()
}

/// A key showing `day` ("Today", "Tue 6 Oct") that picks another on a calendar, into `day`.
fn day_picker(day: &Rc<RefCell<String>>, name: &str) -> gtk::MenuButton {
    let date_key = gtk::MenuButton::new();
    date_key.add_css_class("money-picker");
    date_key.set_widget_name(name);
    date_key.set_label(&human_date(&day.borrow()));
    let calendar = gtk::Calendar::new();
    if let Some(selected) = parse_day(&day.borrow()) {
        calendar.set_date(&selected);
    }
    let popover = gtk::Popover::new();
    popover.set_child(Some(&calendar));
    date_key.set_popover(Some(&popover));
    let (day, shown, popover) = (day.clone(), date_key.clone(), popover.clone());
    calendar.connect_day_selected(move |calendar| {
        let picked = calendar.date().format("%Y-%m-%d").map(|s| s.to_string()).unwrap_or_default();
        shown.set_label(&human_date(&picked));
        *day.borrow_mut() = picked;
        popover.popdown();
    });
    date_key
}

const ACCOUNT_TYPES: [(&str, &str); 5] = [
    ("CHEQUING", "Chequing"),
    ("SAVINGS", "Savings"),
    ("CASH", "Cash"),
    ("CREDIT", "Credit card"),
    ("INVESTMENT", "Investment"),
];

/// The add account sheet: a name, a type and what it holds today.
pub fn account(ui: &Rc<Ui>) {
    account_of(ui, None);
}

/// [`account`] with `kind` (an `AccountType` name) chosen to start with. An investment account
/// also takes its registration (TFSA, RRSP…) and where it is held.
pub fn account_of(ui: &Rc<Ui>, kind: Option<&str>) {
    let Some(panel) = Panel::toggle(ui, "Add an account", 460) else { return };
    panel.add_css_class("money-sheet");
    let body = &panel.body;
    let fmt = formatter();
    let name = gtk::Entry::new();
    name.add_css_class("money-field");
    name.set_widget_name("money-account-name");
    name.set_placeholder_text(Some("Everyday chequing"));
    field(body, "Name", &name);
    let kinds: Vec<&str> = ACCOUNT_TYPES.iter().map(|(_, caption)| *caption).collect();
    let picked = kind.and_then(|k| ACCOUNT_TYPES.iter().position(|(value, _)| *value == k)).unwrap_or(0);
    let kind = gtk::DropDown::from_strings(&kinds);
    kind.add_css_class("money-picker");
    kind.set_widget_name("money-account-type");
    kind.set_selected(picked as u32);
    field(body, "Type", &kind);
    // An investment account's registration and institution; hidden for the other types. A new
    // one starts non-registered: a wrong TFSA or RRSP would count its money against the room.
    let registrations: Vec<&str> = super::invest::REGISTRATIONS.iter().map(|(_, caption)| *caption).collect();
    let registration = gtk::DropDown::from_strings(&registrations);
    registration.add_css_class("money-picker");
    registration.set_widget_name("money-account-registration");
    let registration_block = field(body, "Registration", &registration);
    let institution = gtk::Entry::new();
    institution.add_css_class("money-field");
    institution.set_widget_name("money-account-institution");
    institution.set_placeholder_text(Some("Wealthsimple"));
    let institution_block = field(body, "Held at", &institution);
    let balance_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let balance = gtk::Entry::new();
    balance.add_css_class("money-field");
    balance.set_widget_name("money-account-balance");
    balance.set_hexpand(true);
    balance.set_placeholder_text(Some(&fmt.format_input(0)));
    balance_row.append(&balance);
    balance_row.append(&label(&fmt.currency, "money-suffix"));
    let balance_block = field(body, "Holds today", &balance_row);
    let owed = gtk::CheckButton::with_label("This is money owed, such as a card balance");
    owed.set_widget_name("money-account-owed");
    owed.set_active(ACCOUNT_TYPES[picked].0 == "CREDIT");
    body.append(&owed);
    let shape = {
        let (name, owed, registration_block, institution_block) = (name.downgrade(), owed.downgrade(), registration_block.downgrade(), institution_block.downgrade());
        let balance_caption = balance_block.first_child().and_downcast::<gtk::Label>();
        move |index: u32, changed: bool| {
            let value = ACCOUNT_TYPES.get(index as usize).map_or("", |(value, _)| *value);
            let investing = value == "INVESTMENT";
            if let Some(owed) = owed.upgrade() {
                owed.set_visible(!investing);
                if changed {
                    owed.set_active(value == "CREDIT");
                }
            }
            for block in [&registration_block, &institution_block] {
                if let Some(block) = block.upgrade() {
                    block.set_visible(investing);
                }
            }
            if let Some(caption) = &balance_caption {
                caption.set_text(if investing { "WORTH TODAY" } else { "HOLDS TODAY" });
            }
            if let Some(name) = name.upgrade() {
                name.set_placeholder_text(Some(if investing { "Wealthsimple TFSA" } else { "Everyday chequing" }));
            }
        }
    };
    shape(picked as u32, false);
    kind.connect_selected_notify(move |kind| shape(kind.selected(), true));
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);
    let save = button("Add account", "primary");
    save.add_css_class("money-hero-action");
    save.set_widget_name("money-account-save");
    body.append(&save);
    let submit = {
        let ui = ui.clone();
        let sheet = Rc::downgrade(&panel);
        let (name, kind, balance, owed, problem, save) = (name.clone(), kind.clone(), balance.clone(), owed.clone(), problem.clone(), save.clone());
        let (registration, institution) = (registration.clone(), institution.clone());
        move || {
            let title = name.text().trim().to_string();
            if title.is_empty() {
                problem.set_text("Give the account a name.");
                problem.set_visible(true);
                name.grab_focus();
                return;
            }
            let Some((value, _)) = ACCOUNT_TYPES.get(kind.selected() as usize) else {
                problem.set_text("Pick the account's type.");
                problem.set_visible(true);
                return;
            };
            let investing = *value == "INVESTMENT";
            let typed = balance.text();
            let opening = if typed.trim().is_empty() { Some(0) } else { formatter().parse(&typed) };
            let Some(opening) = opening else {
                problem.set_text(if investing {
                    "Type what it is worth as an amount, like 1250.00."
                } else {
                    "Type what it holds as an amount, like 1250.00. Tick the box below for money owed."
                });
                problem.set_visible(true);
                return;
            };
            let opening = if owed.is_active() && !investing { -opening } else { opening };
            let mut payload = json!({"name":title,"type":value,"opening_balance":opening});
            if investing {
                if let Some((registered, _)) = super::invest::REGISTRATIONS.get(registration.selected() as usize) {
                    payload["registration"] = json!(registered);
                }
                let held = institution.text().trim().to_string();
                if !held.is_empty() {
                    payload["institution"] = json!(held);
                }
            }
            save.set_sensitive(false);
            problem.set_visible(false);
            let (ui, sheet, problem, save) = (ui.clone(), sheet.clone(), problem.clone(), save.clone());
            glib::spawn_future_local(async move {
                match ui.call("money.account.add", payload).await {
                    Ok(v) => {
                        if let Some(sheet) = sheet.upgrade() {
                            sheet.close();
                        }
                        super::toast(&ui, &format!("Added {}.", text(&v, "name")), None);
                        ui.refresh_page();
                    }
                    Err(e) => {
                        problem.set_text(&super::pages::unavailable(&e));
                        problem.set_visible(true);
                        save.set_sensitive(true);
                    }
                }
            });
        }
    };
    let submit = Rc::new(submit);
    let run = submit.clone();
    save.connect_clicked(move |_| run());
    let run = submit.clone();
    name.connect_activate(move |_| run());
    let run = submit.clone();
    institution.connect_activate(move |_| run());
    balance.connect_activate(move |_| submit());
    panel.present();
    name.grab_focus();
}

/// The record a value sheet: what an investment account is worth on a day (`money.value.set`).
/// Tally reads an investment account as its newest value, plus the transfers after it. An account
/// its holdings fill (`account` is Investments' reading of it) is valued from them there whatever
/// is recorded, so the sheet says what the value changes: Overview and net worth.
pub fn value(ui: &Rc<Ui>, account: &Value) {
    let Some(panel) = Panel::toggle(ui, "Record a value", 460) else { return };
    panel.add_css_class("money-sheet");
    let body = &panel.body;
    let fmt = formatter();
    let name = text(account, "name").to_string();
    let about = if super::invest::by_hand(account) {
        format!("What {name} is worth on a day, from its statement or app. Tally counts from the newest value, and transfers after it add to it.")
    } else {
        format!("Investments values {name} from its holdings, whatever you record here. What you record is what Overview and net worth count for it, until a newer value: each holdings report records one.")
    };
    let about = label(&about, "money-muted");
    about.set_wrap(true);
    body.append(&about);
    let amount_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    amount_row.add_css_class("money-amount-row");
    let amount = gtk::Entry::new();
    amount.add_css_class("money-amount-field");
    amount.set_widget_name("money-value-amount");
    amount.set_hexpand(true);
    amount.set_placeholder_text(Some(&fmt.format_input(0)));
    amount.set_input_purpose(gtk::InputPurpose::Number);
    amount.update_property(&[gtk::accessible::Property::Label(&format!("What {name} is worth"))]);
    amount_row.append(&amount);
    let code = label(&fmt.currency, "money-suffix");
    code.set_valign(gtk::Align::Center);
    amount_row.append(&code);
    field(body, "Worth", &amount_row);
    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);
    let day = Rc::new(RefCell::new(today()));
    field(body, "Date", &day_picker(&day, "money-value-date"));
    let save = button("Record value", "primary");
    save.add_css_class("money-hero-action");
    save.set_widget_name("money-value-save");
    body.append(&save);
    let submit = {
        let ui = ui.clone();
        let sheet = Rc::downgrade(&panel);
        let (amount, problem, save, day) = (amount.clone(), problem.clone(), save.clone(), day.clone());
        let id = account["id"].clone();
        move || {
            let Some(worth) = formatter().parse(&amount.text()) else {
                problem.set_text("Type what it is worth, like 25072.31.");
                problem.set_visible(true);
                amount.add_css_class("error");
                amount.grab_focus();
                return;
            };
            amount.remove_css_class("error");
            problem.set_visible(false);
            save.set_sensitive(false);
            let payload = json!({"account_id":id,"date":*day.borrow(),"value":worth});
            let (ui, sheet, problem, save, name) = (ui.clone(), sheet.clone(), problem.clone(), save.clone(), name.clone());
            glib::spawn_future_local(async move {
                match ui.call("money.value.set", payload).await {
                    Ok(_) => {
                        if let Some(sheet) = sheet.upgrade() {
                            sheet.close();
                        }
                        super::toast(&ui, &format!("Recorded {name} at {}.", formatter().format(worth)), None);
                        ui.refresh_page();
                    }
                    Err(e) => {
                        problem.set_text(&super::pages::unavailable(&e));
                        problem.set_visible(true);
                        save.set_sensitive(true);
                    }
                }
            });
        }
    };
    let submit = Rc::new(submit);
    let run = submit.clone();
    save.connect_clicked(move |_| run());
    amount.connect_activate(move |_| submit());
    panel.present();
    amount.grab_focus();
}
