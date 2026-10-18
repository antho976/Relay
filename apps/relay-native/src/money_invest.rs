//! The Investments page (`money-invest`), and the Wealthsimple import flow it shares with a
//! thread's import card (docs/INVESTMENTS.md). Like Tally's other pages it reads and writes the
//! engine's ledger through the bus only (`money.invest.*`, `money.value.set`, `money.fx.fetch`):
//! positions, values, gains, returns and room are the engine's readings, never worked out here.
use super::pages::{
    allocation_bar, badge_sized, card, card_asking, cell, clear_rebuilding, columns, date, dot, flex, human_date, icon_row, meter_in, n,
    plural, rebuilding, retype, rgb, row_key, signed_whole, strip, tile, today, typing, unavailable, word, Flex, Tone,
};
use super::{formatter, message, remember_currency, Page};
use crate::app::{button, clear, label, rows, text, Ui};
use crate::client::Error;
use crate::panel::Panel;
use gtk::prelude::*;
use gtk4 as gtk;
use relay_money::copy;
use relay_money::money::{Locale, MoneyFormatter};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

/// `Registration`'s values, in the contract's order, and what the pages call them.
pub(crate) const REGISTRATIONS: [(&str, &str); 8] = [
    ("NON_REGISTERED", "Non-registered"),
    ("TFSA", "TFSA"),
    ("RRSP", "RRSP"),
    ("FHSA", "FHSA"),
    ("RESP", "RESP"),
    ("LIRA", "LIRA"),
    ("RRIF", "RRIF"),
    ("OTHER", "Other"),
];

/// What a registration is called; an investment account given none reads "Not set".
pub(crate) fn registration_label(registration: &str) -> &'static str {
    REGISTRATIONS.iter().find(|(value, _)| *value == registration).map_or("Not set", |(_, caption)| *caption)
}

/// A registration's hue, an index into `HUES`, for its badge, bar segment and legend dot, never
/// its text: sky TFSA, orchid RRSP, sand FHSA, slate non-registered, lavender RESP, olive the
/// rest. They keep clear of the greens (money in), amber (waiting) and the reds (over).
pub(crate) fn registration_hue(registration: &str) -> i64 {
    match registration {
        "TFSA" => 2,
        "RRSP" => 4,
        "FHSA" => 9,
        "NON_REGISTERED" => 10,
        "RESP" => 3,
        _ => 8,
    }
}

/// Units held and prices are fixed point at 1e-8 (`QTY_SCALE`, `PRICE_SCALE`).
const SCALE: u64 = 100_000_000;

/// "41.2", "0.0153": units held, with no trailing zeros.
fn units(quantity: i64) -> String {
    let (whole, fraction) = (quantity.unsigned_abs() / SCALE, quantity.unsigned_abs() % SCALE);
    let mut out = whole.to_string();
    if fraction > 0 {
        out.push('.');
        out.push_str(format!("{fraction:08}").trim_end_matches('0'));
    }
    if quantity < 0 { format!("\u{2212}{out}") } else { out }
}

/// A price per unit in its own currency, to its cents: "$38.12", "US$187.20".
fn price_text(price: i64, currency: &str) -> String {
    let fmt = MoneyFormatter::new(currency, Locale::from_env());
    let step = 10_i64.pow(8_u32.saturating_sub(fmt.fraction_digits));
    fmt.format((price + step / 2) / step)
}

/// Basis points to one decimal: "+14.5%" and "−3.2%" when `signed`, else "12.3%".
fn percent(bps: i64, signed: bool) -> String {
    let body = format!("{:.1}%", bps.unsigned_abs() as f64 / 100.0);
    match bps.signum() {
        1 if signed => format!("+{body}"),
        -1 => format!("\u{2212}{body}"),
        _ => body,
    }
}

/// A legend's share in whole percent: "52%", or "<1%" for a sliver. The Tally panel's too.
pub(super) fn share(bps: i64) -> String {
    if bps > 0 && bps < 50 { String::from("<1%") } else { format!("{}%", (bps + 50) / 100) }
}

/// A gain reads in the state colours, and always with its sign, so colour is never the only cue.
fn gain_class(gain: i64) -> Option<&'static str> {
    match gain.signum() {
        1 => Some("money-in"),
        -1 => Some("money-over"),
        _ => None,
    }
}

/// "a, b and c".
fn join_and<S: AsRef<str>>(parts: &[S]) -> String {
    let words: Vec<&str> = parts.iter().map(|part| part.as_ref()).collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

static MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// A `YYYY-MM` month's index, 0 for January.
fn month_of(month: &str) -> Option<usize> {
    month.get(5..7)?.parse::<usize>().ok().filter(|m| (1..=12).contains(m)).map(|m| m - 1)
}

fn days_since(day: &str) -> Option<i64> {
    let (then, now) = (date(day)?, date(&today())?);
    Some(now.difference(&then).as_days())
}

/// "today", "yesterday" or "Fri 8 May", to follow "as of".
pub(super) fn as_of(day: &str) -> String {
    match human_date(day) {
        said if said == "Today" || said == "Yesterday" => said.to_lowercase(),
        said => said,
    }
}

thread_local! {
    /// The page's head, built once so its keys keep their focus across reads: the context line,
    /// and the Update rates key, shown while a holding is valued at an estimated rate.
    static HEAD: RefCell<Option<(gtk::Label, gtk::Button)>> = const { RefCell::new(None) };
    /// Whether Holdings shows past its largest [`HOLDINGS_SHOWN`].
    static ALL_HOLDINGS: Cell<bool> = const { Cell::new(false) };
    /// Whether Activity shows past its newest [`ACTIVITY_SHOWN`].
    static ALL_ACTIVITY: Cell<bool> = const { Cell::new(false) };
}

const HOLDINGS_SHOWN: usize = 15;
const ACTIVITY_SHOWN: usize = 8;

/// How many lines the page reads for Activity (`money.invest.list`): one past those it shows,
/// so it knows there are more, or the engine's most once the person asked for them.
pub(super) fn activity_limit() -> usize {
    if ALL_ACTIVITY.with(|a| a.get()) { 200 } else { ACTIVITY_SHOWN + 1 }
}

/// Builds the page's head: the title with its keys (Update rates, Ask about this, Import…), and
/// the context line under it. Ask about this is the page's one way to ask; only Needs a look adds
/// its own, about what it lists.
pub(super) fn head(ui: &Rc<Ui>, page: &Page) {
    clear(&page.head);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let title = label("Investments", "money-title");
    title.set_hexpand(true);
    top.append(&title);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    keys.set_halign(gtk::Align::Start);
    keys.set_valign(gtk::Align::Center);
    let rates = button("Update rates", "quiet");
    rates.set_widget_name("money-invest-rates");
    rates.set_tooltip_text(Some(
        "Some holdings are in US dollars at an estimated rate. This fetches the day's rate from the Bank of Canada; nothing about you is sent.",
    ));
    rates.set_visible(false);
    keys.append(&rates);
    let ask = super::pages::bare("quiet");
    ask.set_widget_name("money-invest-ask");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let mark = crate::icons::image("claude", 14);
    mark.set_valign(gtk::Align::Center);
    row.append(&mark);
    row.append(&label("Ask about this", ""));
    ask.set_child(Some(&row));
    ask.set_tooltip_text(Some("Ask in a thread: \u{201c}How are my investments doing?\u{201d}"));
    ask.update_property(&[gtk::accessible::Property::Label("Ask in a thread: How are my investments doing?")]);
    keys.append(&ask);
    let import = button("Import…", "");
    import.set_widget_name("money-invest-import-key");
    import.set_tooltip_text(Some("A Wealthsimple holdings report, activities export or monthly statement (.csv)"));
    keys.append(&import);
    top.append(&keys);
    page.head.append(&top);
    flex(&top, Flex::Controls);
    let context = label("", "money-context");
    context.set_wrap(true);
    page.head.append(&context);
    let weak = Rc::downgrade(ui);
    rates.connect_clicked(move |key| {
        let Some(ui) = weak.upgrade() else { return };
        key.set_sensitive(false);
        let key = key.clone();
        glib::spawn_future_local(async move {
            // The fetch runs on the engine's own thread and ends in `money.changed`, which redraws.
            match ui.call("money.fx.fetch", json!({})).await {
                Ok(v) if v["started"] == false => super::toast(&ui, "The rate is already on its way.", None),
                Ok(_) => super::toast(&ui, "Fetching the day's US dollar rate from the Bank of Canada…", None),
                Err(e) => ui.show_error(&refusal(&e)),
            }
            key.set_sensitive(true);
        });
    });
    let weak = Rc::downgrade(ui);
    ask.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::threads::ask(&ui, "How are my investments doing?");
        }
    });
    let weak = Rc::downgrade(ui);
    import.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import_from_page(&ui);
        }
    });
    HEAD.with(|h| *h.borrow_mut() = Some((context, rates)));
}

/// The context line ("3 accounts at Wealthsimple · as of Fri 8 May") and the Update rates key,
/// from a reading. Values over a month old say so, in the line's own quiet ink: stale is not a
/// waiting state.
fn show_head(p: &Value, accounts: &[Value]) {
    let Some((context, rates)) = HEAD.with(|h| h.borrow().clone()) else { return };
    rates.set_visible(rows(p, "holdings").iter().any(|h| h["fx_estimated"] == true));
    if p["empty"] == true || accounts.is_empty() {
        context.set_text("TFSA, RRSP, FHSA and the rest, side by side");
        return;
    }
    let mut parts = vec![plural(accounts.len(), "account")];
    let mut places: Vec<&str> = accounts.iter().map(|a| text(a, "institution")).filter(|place| !place.is_empty()).collect();
    places.sort_unstable();
    places.dedup();
    if let [place] = places.as_slice() {
        parts[0] = format!("{} at {place}", parts[0]);
    }
    match p["as_of"].as_str() {
        Some(day) => {
            parts.push(format!("as of {}", as_of(day)));
            if days_since(day).is_some_and(|days| days > 31) {
                parts.push(String::from("over a month old: import a fresh holdings report to bring it up to date"));
            }
        }
        None => parts.push(String::from("no holdings report yet")),
    }
    context.set_text(&parts.join(" · "));
}

/// A failed read. An engine from before investments says so; anything else offers Try again.
pub(super) fn failed(ui: &Rc<Ui>, page: &Page, error: &Error) {
    if let Some((context, rates)) = HEAD.with(|h| h.borrow().clone()) {
        context.set_text("");
        rates.set_visible(false);
    }
    match error {
        Error::Bus(e) if matches!(e.code.as_str(), "bus.unknown_op" | "bus.not_implemented") => {
            message(&page.body, "This engine predates investments. Update Relay's engine to see them here.");
        }
        _ => super::pages::failed(ui, page, "", error),
    }
}

/// The room worth a row: a registration the person holds, has typed room for, or has put money
/// into this year.
fn room_shown(room: &[Value], accounts: &[Value]) -> Vec<Value> {
    room.iter()
        .filter(|r| {
            let registration = text(r, "registration");
            accounts.iter().any(|a| text(a, "registration") == registration) || r["room"].as_i64().is_some() || n(r, "contributed") > 0
        })
        .cloned()
        .collect()
}

/// The page: a strip of figures with the allocation under it, Accounts beside Room, Holdings,
/// then Income beside Activity. `activity` is `money.invest.list`, `lists` `money.lists` (the
/// value of an account recorded by hand); either is absent when its read failed.
pub(super) fn draw(ui: &Rc<Ui>, page: &Page, p: &Value, activity: Option<&Value>, lists: Option<&Value>) {
    remember_currency(p);
    let fmt = formatter();
    let accounts = rows(p, "accounts");
    show_head(p, &accounts);
    let typed = typing(ui, "money-room-");
    clear_rebuilding(&page.body);
    if p["empty"] == true || accounts.is_empty() {
        empty_state(ui, &page.body, text(p, "currency"));
        return;
    }
    let holdings = rows(p, "holdings");
    let room = room_shown(&rows(p, "room"), &accounts);
    let recorded = lists.map(|l| rows(l, "accounts")).unwrap_or_default();
    let balance = |account: &Value| recorded.iter().find(|a| a["id"] == account["id"]).map(|a| n(a, "balance"));
    // Accounts valued by hand are outside the engine's portfolio; the strip counts them, and says so.
    let hand: i64 = accounts.iter().filter(|a| by_hand(a)).filter_map(&balance).sum();
    let year = room.first().map(|r| n(r, "year")).unwrap_or_else(|| glib::DateTime::now_local().map(|now| i64::from(now.year())).unwrap_or(0));
    // In RRSP season the RRSP row is last year's: rows of two years leave the year out of the
    // captions, and each row says its own.
    let one_year = room.iter().all(|r| n(r, "year") == year);
    let dated = |caption: &str| if one_year { format!("{caption} · {year}") } else { caption.to_string() };

    // The strip: value, gain, income and room, then where the money sits.
    let (strip_box, cells) = strip(&page.body);
    let value = n(p, "value");
    let mut held = format!("{} · {}", plural(accounts.len(), "account"), plural(holdings.len(), "holding"));
    if hand != 0 {
        held = format!("{held} · {} of it recorded by hand", fmt.format_whole(hand));
    }
    cell(&cells, "chart", "Portfolio value", &fmt.format_whole(value + hand), &held, None);
    let gain = n(p, "gain");
    let gain_line = match p["gain_bps"].as_i64() {
        Some(bps) => format!("{} on a cost of {}", percent(bps, true), fmt.format_whole(n(p, "book"))),
        None => String::from("no cost recorded to measure it on"),
    };
    cell(&cells, "spark", "Gain", &signed_whole(&fmt, gain), &gain_line, gain_class(gain));
    cell(&cells, "income", "Income · 12 months", &fmt.format_whole(n(p, "income_12m")), "dividends and interest", None);
    let known: Vec<&Value> = room.iter().filter(|r| r["left"].as_i64().is_some()).collect();
    let over: Vec<&str> = room.iter().filter(|r| n(r, "over") > 0).map(|r| registration_label(text(r, "registration"))).collect();
    let (figure, line, class) = if room.is_empty() {
        (String::from("—"), String::from("no TFSA, RRSP or FHSA yet"), None)
    } else if known.is_empty() {
        (String::from("—"), String::from("type your room from CRA below"), None)
    } else {
        let left: i64 = known.iter().map(|r| n(r, "left")).sum();
        let names: Vec<&str> = known.iter().map(|r| registration_label(text(r, "registration"))).collect();
        if over.is_empty() {
            (fmt.format_whole(left), format!("in {}", join_and(&names)), None)
        } else {
            (fmt.format_whole(left), format!("over in {}", join_and(&over)), Some("money-over"))
        }
    };
    cell(&cells, "plan", &dated("Room left"), &figure, &line, class);
    let allocation = rows(p, "allocation");
    if value > 0 && !allocation.is_empty() {
        let block = gtk::Box::new(gtk::Orientation::Vertical, 10);
        block.add_css_class("tally-pace");
        let parts: Vec<(i64, f64)> = allocation.iter().map(|a| (registration_hue(text(a, "registration")), n(a, "share_bps") as f64 / 10_000.0)).collect();
        let bar = allocation_bar(&parts, 10);
        let spoken: Vec<String> = allocation.iter().map(|a| format!("{} {}", registration_label(text(a, "registration")), share(n(a, "share_bps")))).collect();
        bar.update_property(&[gtk::accessible::Property::Label(&format!("By registration: {}", spoken.join(", ")))]);
        block.append(&bar);
        // Four to a line, fewer on a narrow page: the scroller clips what does not fit, never
        // scrolls to it.
        let legend = gtk::FlowBox::new();
        legend.add_css_class("tally-legend");
        legend.set_selection_mode(gtk::SelectionMode::None);
        legend.set_homogeneous(false);
        legend.set_column_spacing(18);
        legend.set_row_spacing(6);
        legend.set_max_children_per_line(4);
        legend.set_halign(gtk::Align::Start);
        for a in &allocation {
            let item = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            item.append(&dot(registration_hue(text(a, "registration"))));
            item.append(&label(&format!("{} {}", registration_label(text(a, "registration")), share(n(a, "share_bps"))), "tally-figures"));
            item.set_tooltip_text(Some(&fmt.format_whole(n(a, "value"))));
            let child = gtk::FlowBoxChild::new();
            child.set_child(Some(&item));
            child.set_focusable(false);
            legend.insert(&child, -1);
        }
        block.append(&legend);
        strip_box.append(&block);
    }

    // What the engine could not square, said plainly: a sale of more than the ledger holds.
    let issues: Vec<String> = p["issues"].as_array().map(|list| list.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if !issues.is_empty() {
        let question = format!("Tally says: {}. What does that mean, and how do I fix it?", issues.join("; "));
        let list = card_asking(&page.body, "Needs a look", None, Some((ui, &question)));
        for issue in &issues {
            list.append(&icon_row(&tile("spark"), issue, "", "", ""));
        }
    }

    // Accounts beside room.
    let pair = columns(&page.body, 2);
    let add: super::Action = {
        let weak = Rc::downgrade(ui);
        Rc::new(move || {
            if let Some(ui) = weak.upgrade() {
                super::add_investment_account(&ui);
            }
        })
    };
    let list = card(&pair[0], "Accounts", Some(("Add an account", add)));
    for account in &accounts {
        list.append(&account_row(ui, account, balance(account)));
    }
    let list = card(&pair[1], &dated("Contribution room"), None);
    if room.is_empty() {
        list.append(&icon_row(&tile("plan"), "No TFSA, RRSP or FHSA yet", "Give an investment account its registration to follow its room", "", ""));
    }
    for r in &room {
        list.append(&room_row(ui, r, !one_year));
    }
    if let Some(rrsp) = room.iter().find(|r| text(r, "registration") == "RRSP") {
        if let Some(deadline) = rrsp["deadline"].as_str() {
            let detail = format!("Contributions until {} count toward {}", human_date(deadline), n(rrsp, "year"));
            list.append(&icon_row(&tile("calendar"), "RRSP deadline", &detail, "", ""));
        }
    }
    let note = label("Your room is on CRA My Account, and an RRSP's on your Notice of Assessment too. Tally subtracts what it sees going in.", "money-row-detail");
    note.set_wrap(true);
    note.add_css_class("tally-note");
    list.append(&note);

    // Holdings, largest first.
    let list = card(&page.body, "Holdings", None);
    if holdings.is_empty() {
        list.append(&icon_row(&tile("chart"), "No holdings yet", "They come in with a Wealthsimple holdings report: Import… above", "", ""));
    } else {
        list.append(&holdings_head());
        let all = ALL_HOLDINGS.with(|a| a.get());
        for h in holdings.iter().take(if all { holdings.len() } else { HOLDINGS_SHOWN }) {
            list.append(&holding_row(h));
        }
        if holdings.len() > HOLDINGS_SHOWN {
            let caption = if all { format!("Show the largest {HOLDINGS_SHOWN}") } else { format!("Show all {}", holdings.len()) };
            let more = button(&caption, "money-text-action");
            more.set_widget_name("money-invest-more");
            more.set_halign(gtk::Align::Start);
            more.add_css_class("tally-save");
            let weak = Rc::downgrade(ui);
            more.connect_clicked(move |_| {
                ALL_HOLDINGS.with(|a| a.set(!a.get()));
                if let Some(ui) = weak.upgrade() {
                    ui.refresh_page();
                }
            });
            list.append(&more);
        }
    }

    // Income beside activity.
    let pair = columns(&page.body, 2);
    let income = rows(p, "income");
    let paid = n(p, "income_12m");
    let list = card(&pair[0], "Income · 12 months", None);
    let months = rows(p, "income_by_month");
    if paid > 0 && !months.is_empty() {
        list.append(&income_bars(&months));
    }
    if income.is_empty() {
        list.append(&icon_row(&tile("income"), "No dividends or interest in the last year", "They come in with an activities export", "", ""));
    }
    for payout in income.iter().take(5) {
        let what = match text(payout, "type") {
            "DIVIDEND" => String::from("Dividend"),
            "INTEREST" => String::from("Interest"),
            "REINVEST" => String::from("Reinvested dividend"),
            other => word(other),
        };
        let title = match payout["symbol"].as_str().filter(|s| !s.is_empty()) {
            Some(symbol) => format!("{what} · {symbol}"),
            None => what,
        };
        let account = accounts.iter().find(|a| a["id"] == payout["account_id"]).map_or("", |a| text(a, "name"));
        let detail = format!("{account} · {}", human_date(text(payout, "date")));
        list.append(&icon_row(&tile("income"), &title, &detail, &fmt.format_signed(n(payout, "amount")), "money-in"));
    }
    let list = card(&pair[1], "Activity", None);
    match activity.map(|a| rows(a, "activities")) {
        None => list.append(&icon_row(&tile("list"), "Activity could not be read", "", "", "")),
        Some(items) if items.is_empty() => {
            list.append(&icon_row(&tile("list"), "No activity yet", "Buys, sells, contributions and dividends come in with an activities export", "", ""));
        }
        Some(items) => {
            // Older lines too, on asking: a line imported into the wrong account may be one.
            let all = ALL_ACTIVITY.with(|a| a.get());
            for item in items.iter().take(if all { items.len() } else { ACTIVITY_SHOWN }) {
                list.append(&activity_row(ui, item));
            }
            if items.len() > ACTIVITY_SHOWN {
                let caption = if all { format!("Show the newest {ACTIVITY_SHOWN}") } else { String::from("Show more") };
                let more = button(&caption, "money-text-action");
                more.set_widget_name("money-invest-activity-more");
                more.set_halign(gtk::Align::Start);
                more.add_css_class("tally-save");
                let weak = Rc::downgrade(ui);
                more.connect_clicked(move |_| {
                    ALL_ACTIVITY.with(|a| a.set(!a.get()));
                    if let Some(ui) = weak.upgrade() {
                        ui.refresh_page();
                    }
                });
                list.append(&more);
            }
        }
    }
    retype(&page.body, typed);
}

/// No investment account yet: what the page is for, and the three ways in. Wealthsimple's files
/// are in Canadian dollars and the engine reads them into no other ledger, so a ledger kept in
/// `currency` other than CAD is told so and offered only an account by hand.
fn empty_state(ui: &Rc<Ui>, body: &gtk::Box, currency: &str) {
    let wealthsimple = currency == "CAD";
    let block = gtk::Box::new(gtk::Orientation::Vertical, 14);
    block.add_css_class("tally-card");
    block.add_css_class("money-empty");
    block.append(&label("No investments yet", "money-empty-title"));
    let about = if wealthsimple {
        String::from(
            "Bring in your Wealthsimple accounts to see your TFSA, RRSP and FHSA side by side: what they are worth, what they have earned, and the room left this year. Tally reads the files Wealthsimple lets you download; the files stay on this PC.",
        )
    } else {
        let keeps = if currency.is_empty() { String::new() } else { format!(", and this one keeps {currency}") };
        format!(
            "Wealthsimple's files need a ledger kept in Canadian dollars{keeps}, so Tally cannot read them here. Add your investment accounts by hand and record what each is worth from its statement."
        )
    };
    let about = label(&about, "money-muted");
    about.set_wrap(true);
    about.set_max_width_chars(70);
    block.append(&about);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let connect = button("Connect Wealthsimple", "primary");
    connect.add_css_class("money-hero-action");
    connect.set_widget_name("money-invest-connect");
    connect.set_tooltip_text(Some("A thread walks you through downloading your files and bringing them in"));
    let import = button("Import a file…", "");
    import.set_widget_name("money-invest-empty-import");
    let account = button("Add an account by hand…", if wealthsimple { "" } else { "primary" });
    account.set_widget_name("money-invest-empty-account");
    if wealthsimple {
        actions.append(&connect);
        actions.append(&import);
    } else {
        account.add_css_class("money-hero-action");
    }
    actions.append(&account);
    block.append(&actions);
    flex(&actions, Flex::Controls);
    if wealthsimple {
        let hint = label("Connect opens a thread that guides you, step by step, to your holdings report and activities on my.wealthsimple.com.", "money-row-detail");
        hint.set_wrap(true);
        block.append(&hint);
    }
    body.append(&block);
    let weak = Rc::downgrade(ui);
    connect.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::threads::ask(&ui, "Help me connect my Wealthsimple accounts");
        }
    });
    let weak = Rc::downgrade(ui);
    import.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            import_from_page(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    account.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            super::add_investment_account(&ui);
        }
    });
}

/// An account no file has filled: nothing held, no cash, no cost. It is valued by hand, so what it
/// is worth is Tally's balance for it, its newest recorded value.
pub(crate) fn by_hand(a: &Value) -> bool {
    n(a, "holdings") == 0 && n(a, "book") == 0 && n(a, "value") == 0 && n(a, "cash") == 0
}

/// "Return +6.2% a year since Mar 2024, money-weighted": the account's return when the engine
/// can measure it, with its method and its start, as the contract asks.
fn return_line(a: &Value) -> Option<String> {
    let bps = a["return_bps"].as_i64()?;
    let annual = a["return_annual"] == true;
    let since = a["return_since"]
        .as_str()
        .and_then(date)
        .and_then(|day| day.format(if annual { "%b %Y" } else { "%-d %b" }).ok())
        .map(|day| format!(" since {day}"))
        .unwrap_or_default();
    Some(format!("Return {}{}{since}, money-weighted", percent(bps, true), if annual { " a year" } else { "" }))
}

/// An account: its registration's badge, its name over where it is held and what it holds (and
/// its return), and its value over its gain. A key: it records a value, as its statement shows.
/// An account valued by hand ([`by_hand`]) shows `balance`, Tally's balance for it.
fn account_row(ui: &Rc<Ui>, a: &Value, balance: Option<i64>) -> gtk::Box {
    let fmt = formatter();
    let registration = text(a, "registration");
    let count = n(a, "holdings");
    let by_hand = by_hand(a);
    let mut detail = vec![registration_label(registration).to_string()];
    if !text(a, "institution").is_empty() {
        detail.push(text(a, "institution").to_string());
    }
    if by_hand {
        detail.push(String::from("value recorded by hand"));
    } else {
        detail.push(plural(count.max(0) as usize, "holding"));
        if n(a, "cash") != 0 {
            detail.push(format!("{} cash", fmt.format_whole(n(a, "cash"))));
        }
        if let Some(day) = a["valued_on"].as_str() {
            detail.push(format!("valued {}", as_of(day)));
        }
    }
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    inner.append(&badge_sized("chart", registration_hue(registration), 28));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let name = label(text(a, "name"), "money-row-title");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let said = label(&detail.join(" · "), "money-row-detail");
    said.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&said);
    if let Some(line) = return_line(a) {
        let returned = label(&line, "money-row-detail");
        returned.set_ellipsize(gtk::pango::EllipsizeMode::End);
        words.append(&returned);
    }
    inner.append(&words);
    let figures = gtk::Box::new(gtk::Orientation::Vertical, 1);
    figures.set_valign(gtk::Align::Center);
    let worth = label(&fmt.format(if by_hand { balance.unwrap_or(0) } else { n(a, "value") }), "money-amount");
    worth.set_xalign(1.0);
    figures.append(&worth);
    if !by_hand {
        let gain = n(a, "gain");
        let mut line = fmt.format_signed(gain);
        if let Some(bps) = a["gain_bps"].as_i64() {
            line = format!("{line} · {}", percent(bps, true));
        }
        let gained = label(&line, "tally-figures");
        gained.set_xalign(1.0);
        if let Some(class) = gain_class(gain) {
            gained.add_css_class(class);
        }
        figures.append(&gained);
    }
    inner.append(&figures);
    let weak = Rc::downgrade(ui);
    let account = a.clone();
    let run: super::Action = Rc::new(move || {
        if let Some(ui) = weak.upgrade() {
            super::record_value(&ui, &account);
        }
    });
    row_key(&inner, &format!("Record what {} is worth", text(a, "name")), run)
}

/// A registration's room this year: its badge and name (with its year, when `dated`) over what
/// went in of the room, the room field (the CRA figure), a meter of the room used in its hue (red
/// over it: no pace tick, room has no pace), and what being over costs.
fn room_row(ui: &Rc<Ui>, r: &Value, dated: bool) -> gtk::Box {
    let fmt = formatter();
    let registration = text(r, "registration");
    let name = registration_label(registration);
    let hue = registration_hue(registration);
    let year = n(r, "year");
    let contributed = n(r, "contributed");
    let room = r["room"].as_i64();
    let (over, taxed) = (n(r, "over"), n(r, "over_taxed"));
    let block = gtk::Box::new(gtk::Orientation::Vertical, 6);
    block.add_css_class("tally-row");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    top.append(&badge_sized("coins", hue, 28));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    words.append(&label(&if dated { format!("{name} · {year}") } else { name.to_string() }, "money-row-title"));
    let reading = match room {
        Some(room) if over > 0 => format!("{} put in · room {}", fmt.format_whole(contributed), fmt.format_whole(room)),
        Some(room) => format!("{} of {} · {} left", fmt.format_whole(contributed), fmt.format_whole(room), fmt.format_whole(n(r, "left"))),
        None => format!("{} put in for {year}", fmt.format_whole(contributed)),
    };
    let said = label(&reading, "money-row-detail");
    said.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&said);
    top.append(&words);
    let symbol = label(&fmt.symbol, "money-suffix");
    symbol.set_valign(gtk::Align::Center);
    top.append(&symbol);
    top.append(&room_field(ui, registration, year, room, name));
    block.append(&top);
    if let Some(room) = room {
        let meter = meter_in(contributed as f64 / room.max(1) as f64, None, if over > 0 { Tone::Over } else { Tone::Hue(hue) }, 6);
        meter.set_margin_start(40);
        meter.update_property(&[gtk::accessible::Property::Label(&format!("{name} room: {reading}"))]);
        block.append(&meter);
    }
    // Over the room, CRA charges 1% a month on the excess; an RRSP's first $2,000 over is free.
    let (rrsp, buffer) = (registration == "RRSP", fmt.format_whole(200_000));
    let note = match room {
        Some(_) if over > 0 && rrsp && taxed == 0 => Some((format!("Over by {}, inside the {buffer} buffer CRA allows.", fmt.format_whole(over)), "money-ahead")),
        Some(_) if over > 0 && rrsp => Some((
            format!("Over by {}, {} past the {buffer} buffer. CRA charges 1% a month on that until it comes out.", fmt.format_whole(over), fmt.format_whole(taxed)),
            "money-over",
        )),
        Some(_) if over > 0 => Some((format!("Over by {}. CRA charges 1% a month on the excess until it comes out.", fmt.format_whole(over)), "money-over")),
        Some(_) => None,
        None => Some((
            match r["limit"].as_i64() {
                Some(limit) => format!("Type your {year} room from CRA My Account. The {year} limit is {}; your own room depends on past years.", fmt.format_whole(limit)),
                None => format!("Type your {year} room from CRA My Account."),
            },
            "",
        )),
    };
    if let Some((words, class)) = note {
        let line = label(&words, "money-row-detail");
        line.set_wrap(true);
        line.set_margin_start(40);
        if !class.is_empty() {
            line.add_css_class(class);
        }
        block.append(&line);
    }
    block
}

/// The room field: the CRA figure for `registration` in `year`, saved on Enter or on leaving it
/// (`money.invest.room`; empty removes it).
fn room_field(ui: &Rc<Ui>, registration: &str, year: i64, room: Option<i64>, name: &str) -> gtk::Entry {
    let field = gtk::Entry::new();
    field.add_css_class("money-field");
    field.set_widget_name(&format!("money-room-{}", registration.to_lowercase()));
    field.set_width_chars(9);
    gtk::prelude::EntryExt::set_alignment(&field, 1.0);
    field.set_valign(gtk::Align::Center);
    field.set_placeholder_text(Some("From CRA"));
    if let Some(room) = room {
        field.set_text(&formatter().format_input(room));
    }
    field.set_tooltip_text(Some(&format!("Your {year} {name} room, from CRA My Account. Leave it empty if you do not know it.")));
    field.update_property(&[gtk::accessible::Property::Label(&format!("{name} room for {year}"))]);
    let saved = Rc::new(Cell::new(room.unwrap_or(0)));
    let commit = {
        let weak = Rc::downgrade(ui);
        let registration = registration.to_string();
        move |field: &gtk::Entry| {
            let Some(ui) = weak.upgrade() else { return };
            if rebuilding() {
                return;
            }
            let typed = field.text();
            let value = if typed.trim().is_empty() { Some(0) } else { formatter().parse(&typed) };
            let Some(value) = value else {
                field.add_css_class("error");
                super::toast(&ui, "Type the room as an amount, like 7000. Leave it empty if you do not know it.", None);
                return;
            };
            field.remove_css_class("error");
            if value == saved.get() {
                return;
            }
            saved.set(value);
            let payload = json!({"registration":registration,"year":year,"amount":value});
            glib::spawn_future_local(async move {
                if let Err(e) = ui.call("money.invest.room", payload).await {
                    ui.show_error(&refusal(&e));
                }
                ui.refresh_page();
            });
        }
    };
    let on_enter = commit.clone();
    field.connect_activate(move |field| on_enter(field));
    let focus = gtk::EventControllerFocus::new();
    let weak = field.downgrade();
    focus.connect_leave(move |_| {
        if let Some(field) = weak.upgrade() {
            commit(&field);
        }
    });
    field.add_controller(focus);
    field
}

/// Holdings' figure columns: (caption, width in pixels, shown only on a wide page).
const HOLDING_COLUMNS: [(&str, i32, bool); 4] = [("Price", 96, true), ("Value", 108, false), ("Gain", 124, false), ("Weight", 56, true)];

fn holdings_head() -> gtk::Box {
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    head.add_css_class("tally-th");
    let caption = label("Holding", "tally-th-text");
    caption.set_hexpand(true);
    head.append(&caption);
    for (name, width, wide) in HOLDING_COLUMNS {
        let column = label(name, "tally-th-text");
        column.set_xalign(1.0);
        column.set_size_request(width, -1);
        if wide {
            flex(&column, Flex::Wide);
        }
        head.append(&column);
    }
    head
}

/// A holding: its symbol, its name over the account and units (and, quietly, a missing price or
/// an estimated rate), then price, value, gain and weight under [`holdings_head`]'s columns.
fn holding_row(h: &Value) -> gtk::Box {
    let fmt = formatter();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("tally-row");
    let symbol = text(h, "symbol");
    let mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    mark.add_css_class("invest-symbol");
    mark.set_valign(gtk::Align::Center);
    mark.set_hexpand(false);
    mark.set_size_request(48, 28);
    let short = label(symbol, "");
    short.set_halign(gtk::Align::Center);
    short.set_hexpand(true);
    short.set_max_width_chars(6);
    short.set_ellipsize(gtk::pango::EllipsizeMode::End);
    mark.append(&short);
    row.append(&mark);
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    let name = label(if text(h, "name").is_empty() { symbol } else { text(h, "name") }, "money-row-title");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&name);
    let quantity = n(h, "quantity");
    let mut detail = vec![text(h, "account").to_string(), format!("{} {}", units(quantity), if quantity == SCALE as i64 { "unit" } else { "units" })];
    let no_price = h["no_price"] == true;
    if no_price {
        detail.push(String::from("no price yet, shown at cost"));
    }
    if h["fx_estimated"] == true {
        detail.push(format!("{} at an estimated rate", text(h, "currency")));
    }
    let said = label(&detail.join(" · "), "money-row-detail");
    said.set_ellipsize(gtk::pango::EllipsizeMode::End);
    words.append(&said);
    row.append(&words);
    let [(_, price_width, _), (_, value_width, _), (_, gain_width, _), (_, weight_width, _)] = HOLDING_COLUMNS;
    let price = label(&h["price"].as_i64().map_or_else(|| String::from("—"), |p| price_text(p, text(h, "currency"))), "tally-figures");
    price.set_xalign(1.0);
    price.set_size_request(price_width, -1);
    if let Some(day) = h["price_date"].as_str() {
        price.set_tooltip_text(Some(&format!("The price on {}", human_date(day))));
    }
    flex(&price, Flex::Wide);
    row.append(&price);
    let value = label(&fmt.format(n(h, "value")), "money-amount");
    value.set_xalign(1.0);
    value.set_size_request(value_width, -1);
    row.append(&value);
    let gains = gtk::Box::new(gtk::Orientation::Vertical, 1);
    gains.set_valign(gtk::Align::Center);
    gains.set_size_request(gain_width, -1);
    if no_price {
        let none = label("—", "tally-left");
        none.set_xalign(1.0);
        none.set_tooltip_text(Some("No price yet: it is shown at what it cost"));
        gains.append(&none);
    } else {
        let gain = n(h, "gain");
        let amount = label(&fmt.format_signed(gain), "tally-figures");
        amount.set_xalign(1.0);
        if let Some(class) = gain_class(gain) {
            amount.add_css_class(class);
        }
        gains.append(&amount);
        if let Some(bps) = h["gain_bps"].as_i64() {
            let rate = label(&percent(bps, true), "tally-left");
            rate.set_xalign(1.0);
            gains.append(&rate);
        }
    }
    row.append(&gains);
    let weight = label(&percent(n(h, "weight_bps"), false), "tally-left");
    weight.set_xalign(1.0);
    weight.set_size_request(weight_width, -1);
    flex(&weight, Flex::Wide);
    row.append(&weight);
    row
}

/// Income by month as twelve bars, oldest first: the month now in ink, the ones before a step
/// back (one series, one colour, as Tally draws periods), an empty month a sliver of track.
fn income_bars(months: &[Value]) -> gtk::Box {
    let fmt = formatter();
    let block = gtk::Box::new(gtk::Orientation::Vertical, 4);
    block.add_css_class("tally-bars");
    let area = gtk::DrawingArea::new();
    area.set_content_height(56);
    area.set_hexpand(true);
    area.set_accessible_role(gtk::AccessibleRole::Img);
    let name = |m: &Value| month_of(text(m, "month")).map_or_else(|| text(m, "month").to_string(), |i| MONTHS[i].to_string());
    let spoken: Vec<String> = months.iter().map(|m| format!("{} {}", name(m), fmt.format_whole(n(m, "amount")))).collect();
    area.update_property(&[gtk::accessible::Property::Label(&format!("Income by month: {}", spoken.join(", ")))]);
    let amounts: Vec<i64> = months.iter().map(|m| n(m, "amount").max(0)).collect();
    area.set_draw_func(move |_, cr, width, height| {
        let (w, h) = (width as f64, height as f64);
        let top = amounts.iter().copied().max().unwrap_or(0).max(1) as f64;
        let slot = w / amounts.len().max(1) as f64;
        let bar = (slot * 0.56).max(2.0);
        let last = amounts.len().saturating_sub(1);
        for (i, amount) in amounts.iter().enumerate() {
            let tall = if *amount > 0 { (*amount as f64 / top * (h - 2.0)).max(2.0) } else { 2.0 };
            let (r, g, b) = rgb(if *amount == 0 {
                "#2A2826"
            } else if i == last {
                "#EDE9E2"
            } else {
                "#8C877F"
            });
            cr.set_source_rgb(r, g, b);
            cr.rectangle(slot * i as f64 + (slot - bar) / 2.0, h - tall, bar, tall);
            let _ = cr.fill();
        }
    });
    block.append(&area);
    let initials = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    initials.set_homogeneous(true);
    for m in months {
        let month: &'static str = month_of(text(m, "month")).map_or("", |i| MONTHS[i]);
        let initial = label(month.get(..1).unwrap_or(""), "tally-left");
        initial.set_xalign(0.5);
        initials.append(&initial);
    }
    block.append(&initials);
    block
}

/// An activity: what happened (to which security), in which account and when, and its amount
/// in its own currency, signed by the way the cash went. Its trash key, shown on hover, deletes
/// it ([`delete_activity`]): a line imported into the wrong account comes out here.
fn activity_row(ui: &Rc<Ui>, a: &Value) -> gtk::Box {
    let kind = text(a, "type");
    let symbol = a["symbol"].as_str().unwrap_or("");
    let held = units(n(a, "quantity"));
    let from = |what: &str| if symbol.is_empty() { what.to_string() } else { format!("{what} from {symbol}") };
    let title = match kind {
        "BUY" => format!("Bought {held} {symbol}"),
        "SELL" => format!("Sold {held} {symbol}"),
        "REINVEST" => format!("Reinvested in {held} {symbol}"),
        "SPLIT" => format!("Split: {held} more {symbol}"),
        "TRANSFER_IN" if !symbol.is_empty() => format!("{held} {symbol} transferred in"),
        "TRANSFER_OUT" if !symbol.is_empty() => format!("{held} {symbol} transferred out"),
        "TRANSFER_IN" => String::from("Transfer in"),
        "TRANSFER_OUT" => String::from("Transfer out"),
        "DEPOSIT" => String::from("Deposit"),
        "WITHDRAWAL" => String::from("Withdrawal"),
        "DIVIDEND" => from("Dividend"),
        "INTEREST" => String::from("Interest"),
        "RETURN_OF_CAPITAL" => from("Return of capital"),
        "NOTIONAL_DISTRIBUTION" => from("Notional distribution"),
        "FX" => format!("Converted {} to {}", text(a, "currency"), a["to_currency"].as_str().unwrap_or("")),
        "TAX" => String::from("Tax withheld"),
        other => word(other),
    };
    let glyph = match kind {
        "DEPOSIT" | "TRANSFER_IN" => "income",
        "WITHDRAWAL" | "TRANSFER_OUT" => "spend",
        "DIVIDEND" | "INTEREST" | "CREDIT" | "RETURN_OF_CAPITAL" | "NOTIONAL_DISTRIBUTION" => "coins",
        "FEE" | "TAX" => "card",
        "FX" => "repeat",
        _ => "chart",
    };
    let mut detail = vec![text(a, "account").to_string(), human_date(text(a, "date"))];
    if !text(a, "note").trim().is_empty() {
        detail.push(text(a, "note").trim().to_string());
    }
    let fmt = MoneyFormatter::new(text(a, "currency"), Locale::from_env());
    let amount = n(a, "amount");
    let minus = format!("\u{2212}{}", fmt.format(amount));
    let (figure, class) = match kind {
        "DIVIDEND" | "INTEREST" | "CREDIT" => (fmt.format_signed(amount), "money-in"),
        "DEPOSIT" | "SELL" | "RETURN_OF_CAPITAL" => (fmt.format_signed(amount), ""),
        "TRANSFER_IN" if symbol.is_empty() => (fmt.format_signed(amount), ""),
        "WITHDRAWAL" | "FEE" | "TAX" | "BUY" => (minus, ""),
        "TRANSFER_OUT" if symbol.is_empty() => (minus, ""),
        "FX" => {
            let to = MoneyFormatter::new(a["to_currency"].as_str().unwrap_or(""), Locale::from_env());
            (format!("{} \u{2192} {}", fmt.format(amount), to.format(n(a, "to_amount"))), "money-quiet")
        }
        "SPLIT" => (String::new(), ""),
        _ => (fmt.format(amount), "money-quiet"),
    };
    let row = icon_row(&tile(glyph), title.trim(), &detail.join(" · "), &figure, class);
    row.add_css_class("invest-activity");
    let trash = crate::app::icon_button("trash", "Delete activity");
    trash.add_css_class("money-row-delete");
    trash.set_valign(gtk::Align::Center);
    let weak = Rc::downgrade(ui);
    // Said as an entry's delete is: "Deleted Bought 10 XEQT · Thu 8 Oct".
    let (id, what) = (a["id"].clone(), format!("Deleted {} · {}", title.trim(), human_date(text(a, "date"))));
    trash.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            delete_activity(&ui, id.clone(), what.clone());
        }
    });
    row.append(&trash);
    row
}

/// Deletes activity `id` (`money.invest.delete`) with Undo on a toast rather than a confirmation
/// first, as Tally deletes an entry. Undo is `money.invest.restore`, the same row back.
fn delete_activity(ui: &Rc<Ui>, id: Value, what: String) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match ui.call("money.invest.delete", json!({"id":id})).await {
            Ok(_) => {
                let weak = Rc::downgrade(&ui);
                let undo: super::Action = Rc::new(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let id = id.clone();
                    glib::spawn_future_local(async move {
                        if let Err(e) = ui.call("money.invest.restore", json!({"id":id})).await {
                            ui.show_error(&refusal(&e));
                        }
                        ui.refresh_page();
                    });
                });
                super::toast(&ui, &what, Some(undo));
                ui.refresh_page();
            }
            Err(e) => ui.show_error(&refusal(&e)),
        }
    });
}

/// A refusal in the engine's own sentence, never its kind and code ("Invalid money.invalid: …").
fn refusal(error: &Error) -> String {
    match error {
        Error::Bus(e) if !matches!(e.code.as_str(), "bus.unknown_op" | "bus.not_implemented") => e.message.clone(),
        e => unavailable(e),
    }
}

/// The page's Import key (and Data's): the import flow, then a toast of what came in. It takes
/// any of the three files, so none is "asked" for and none is noted as the wrong one.
pub(super) fn import_from_page(ui: &Rc<Ui>) {
    let weak = Rc::downgrade(ui);
    import_flow(ui, None, "", move |result| {
        if let Some(ui) = weak.upgrade() {
            super::toast(&ui, &import_summary(&result), None);
            ui.refresh_page();
        }
    });
}

/// What an import brought in, in a sentence: "Imported 14 holdings, 212 activities and 2 new
/// accounts; 32 already here."
pub(crate) fn import_summary(result: &Value) -> String {
    let count = |key: &str| result[key].as_i64().or_else(|| result[key].as_array().map(|list| list.len() as i64)).unwrap_or(0);
    let mut parts = Vec::new();
    if count("holdings") > 0 {
        parts.push(copy::plural(count("holdings"), "holding"));
    }
    if count("activities") > 0 {
        parts.push(copy::plural_as(count("activities"), "activity", "activities"));
    }
    if count("accounts_created") > 0 {
        parts.push(copy::plural_as(count("accounts_created"), "new account", "new accounts"));
    }
    let mut line = if parts.is_empty() { String::from("Nothing new came in") } else { format!("Imported {}", join_and(&parts)) };
    if count("duplicates") > 0 {
        line += &format!("; {} already here", count("duplicates"));
    }
    if count("skipped") > 0 {
        line += &format!("; {} skipped", copy::plural(count("skipped"), "line"));
    }
    line + "."
}

/// A preview's counts: "14 holdings · 212 activities · 180 new · 32 already here · 3 lines skipped".
fn counts_line(v: &Value) -> String {
    let count = |key: &str| n(v, key).max(0);
    let skipped = rows(v, "skipped").len() as i64;
    let mut parts = Vec::new();
    if count("holdings") > 0 {
        parts.push(copy::plural(count("holdings"), "holding"));
    }
    if count("activities") > 0 {
        parts.push(copy::plural_as(count("activities"), "activity", "activities"));
    }
    parts.push(format!("{} new", count("new")));
    if count("duplicates") > 0 {
        parts.push(format!("{} already here", count("duplicates")));
    }
    if skipped > 0 {
        parts.push(format!("{} skipped", copy::plural(skipped, "line")));
    }
    parts.join(" · ")
}

/// A Wealthsimple file's kind, as a title and with its article, by `ImportPreview.kind` or an
/// import card's `expects`.
fn kind_words(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "holdings" => ("Holdings report", "a holdings report"),
        "activities" => ("Activities export", "an activities export"),
        "statement" => ("Monthly statement", "a monthly statement"),
        _ => ("Wealthsimple file", "a Wealthsimple file"),
    }
}

/// The Wealthsimple import, shared by this page's Import key and a thread's import card. With no
/// `path`, a file chooser first; then a sheet that previews the file (`money.invest.preview`),
/// maps each account in it to one of Tally's investment accounts or a new one, and imports it
/// (`money.invest.import`). `expects` (`holdings`, `activities` or `statement`) is the kind of
/// file asked for, empty when any will do. `on_done` gets the `ImportResult` once the import is
/// in; whatever the engine refuses stays in the sheet, inline.
pub(crate) fn import_flow(ui: &Rc<Ui>, path: Option<PathBuf>, expects: &str, on_done: impl Fn(Value) + 'static) {
    let on_done: Rc<dyn Fn(Value)> = Rc::new(on_done);
    let (ui, expects) = (ui.clone(), expects.to_string());
    glib::spawn_future_local(async move {
        let path = match path {
            Some(path) => path,
            None => match choose(&ui, &expects).await {
                Some(path) => path,
                None => return,
            },
        };
        let Some(panel) = Panel::toggle(&ui, "Import from Wealthsimple", 520) else { return };
        panel.add_css_class("money-sheet");
        panel.present();
        preview(&ui, &panel, path, expects, on_done);
    });
}

/// The file chooser, for a Wealthsimple .csv.
async fn choose(ui: &Rc<Ui>, expects: &str) -> Option<PathBuf> {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Wealthsimple files (.csv)"));
    filter.add_pattern("*.csv");
    filter.add_pattern("*.CSV");
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let title = format!("Choose {}", kind_words(expects).1);
    let dialog = gtk::FileDialog::builder().title(title.as_str()).filters(&filters).default_filter(&filter).build();
    let file = dialog.open_future(Some(&ui.window)).await.ok()?;
    let path = file.path();
    if path.is_none() {
        ui.show_error("That file is not on this PC's disk. Copy it here first.");
    }
    path
}

/// Chooses another file into the same sheet.
fn choose_again(ui: &Rc<Ui>, sheet: &Weak<Panel>, expects: &str, on_done: &Rc<dyn Fn(Value)>) {
    let (ui, sheet, expects, on_done) = (ui.clone(), sheet.clone(), expects.to_string(), on_done.clone());
    glib::spawn_future_local(async move {
        let Some(path) = choose(&ui, &expects).await else { return };
        if let Some(panel) = sheet.upgrade() {
            preview(&ui, &panel, path, expects, on_done);
        }
    });
}

/// Reads `path` into `panel`: the preview, or why the file cannot be read.
fn preview(ui: &Rc<Ui>, panel: &Rc<Panel>, path: PathBuf, expects: String, on_done: Rc<dyn Fn(Value)>) {
    clear(&panel.body);
    let file = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
    panel.body.append(&label(&format!("Reading {file}…"), "money-muted"));
    let (ui, sheet) = (ui.clone(), Rc::downgrade(panel));
    glib::spawn_future_local(async move {
        let shown = path.to_string_lossy().to_string();
        let (read, lists) = tokio::join!(ui.call("money.invest.preview", json!({"path":shown})), ui.call("money.lists", json!({})));
        let Some(panel) = sheet.upgrade() else { return };
        clear(&panel.body);
        match read {
            Ok(v) => sheet_body(&ui, &panel, &path, &expects, &v, lists.as_ref().ok(), on_done),
            Err(e) => refused(&ui, &panel, &file, &refusal(&e), &expects, on_done),
        }
    });
}

/// A file the preview refused: why, in the engine's words, and a key to choose another.
fn refused(ui: &Rc<Ui>, panel: &Rc<Panel>, file: &str, why: &str, expects: &str, on_done: Rc<dyn Fn(Value)>) {
    let body = &panel.body;
    body.append(&label(&format!("Tally could not read {file}"), "money-row-title"));
    let reason = label(why, "money-problem");
    reason.set_wrap(true);
    body.append(&reason);
    let hint = label(
        "Tally reads Wealthsimple's holdings report, activities export and monthly statements: the .csv files under Documents on my.wealthsimple.com.",
        "money-muted",
    );
    hint.set_wrap(true);
    body.append(&hint);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let another = button("Choose another file…", "primary");
    another.add_css_class("money-hero-action");
    another.set_widget_name("money-invest-another");
    another.set_hexpand(true);
    actions.append(&another);
    body.append(&actions);
    let (ui, sheet, expects) = (ui.clone(), Rc::downgrade(panel), expects.to_string());
    another.connect_clicked(move |_| choose_again(&ui, &sheet, &expects, &on_done));
    another.grab_focus();
}

/// The preview: the file and what it holds, its accounts each mapped to an investment account or
/// a new one (a statement names none, so the person chooses the one it goes into), the lines it
/// skips and why, and the Import key.
fn sheet_body(ui: &Rc<Ui>, panel: &Rc<Panel>, path: &Path, expects: &str, v: &Value, lists: Option<&Value>, on_done: Rc<dyn Fn(Value)>) {
    let body = &panel.body;
    let kind = text(v, "kind");
    let (kind_title, kind_said) = kind_words(kind);
    let file = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();

    // What the file is.
    let what = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    what.append(&tile("download"));
    let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Center);
    words.append(&label(kind_title, "money-row-title"));
    let mut detail = vec![file];
    if let Some(day) = v["as_of"].as_str() {
        detail.push(format!("as of {}", as_of(day)));
    }
    let said = label(&detail.join(" · "), "money-row-detail");
    said.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    words.append(&said);
    what.append(&words);
    body.append(&what);
    if !expects.is_empty() && kind != expects {
        let note = label(&format!("This is {kind_said}, not {} as asked. It imports all the same.", kind_words(expects).1), "money-muted");
        note.set_wrap(true);
        body.append(&note);
    }
    let skipped = rows(v, "skipped");
    let counts = label(&counts_line(v), "tally-figures");
    counts.set_wrap(true);
    body.append(&counts);

    // Its accounts, mapped to Tally's investment accounts.
    let held: Vec<Value> =
        lists.map(|l| rows(l, "accounts")).unwrap_or_default().into_iter().filter(|a| text(a, "type") == "INVESTMENT" && a["archived"] != true).collect();
    let investment: Vec<(i64, String)> = held.iter().map(|a| (n(a, "id"), text(a, "name").to_string())).collect();
    let found = rows(v, "accounts");
    let statement = kind == "statement";
    let mut mapping: Vec<(String, gtk::DropDown, Vec<Option<i64>>)> = Vec::new();
    let mut into: Option<(gtk::DropDown, Vec<Option<i64>>)> = None;
    if statement && investment.is_empty() {
        let none = label(
            "A statement does not name its account. Add the investment account it belongs to first, then choose the file again.",
            "money-muted",
        );
        none.set_wrap(true);
        body.append(&none);
        let add = button("Add an account…", "");
        add.set_halign(gtk::Align::Start);
        let (ui, sheet) = (ui.clone(), Rc::downgrade(panel));
        add.connect_clicked(move |_| {
            if let Some(sheet) = sheet.upgrade() {
                sheet.close();
            }
            super::add_investment_account(&ui);
        });
        body.append(&add);
    } else if statement {
        body.append(&label("INTO ACCOUNT", "money-label"));
        // Nothing is chosen for the person: a drop-down shows its first row, and an RRSP's
        // statement taken into the TFSA would count against the TFSA's room.
        let mut ids = vec![None];
        ids.extend(investment.iter().map(|(id, _)| Some(*id)));
        let mut names = vec!["Choose an account…"];
        names.extend(investment.iter().map(|(_, name)| name.as_str()));
        let picker = gtk::DropDown::from_strings(&names);
        picker.add_css_class("money-picker");
        picker.set_widget_name("money-invest-into");
        picker.update_property(&[gtk::accessible::Property::Label("The account this statement goes into")]);
        // Each choice previews the file again in that account, so "already here" counts its lines.
        let (ui, counts, shown, chosen, first) = (ui.clone(), counts.downgrade(), path.to_string_lossy().to_string(), ids.clone(), counts_line(v));
        let serial = Rc::new(Cell::new(0_u32));
        picker.connect_selected_notify(move |picker| {
            serial.set(serial.get().wrapping_add(1));
            let Some(Some(id)) = chosen.get(picker.selected() as usize).copied() else {
                if let Some(counts) = counts.upgrade() {
                    counts.set_text(&first);
                }
                return;
            };
            let (ui, counts, serial, asked, payload) = (ui.clone(), counts.clone(), serial.clone(), serial.get(), json!({"path":shown,"account_id":id}));
            glib::spawn_future_local(async move {
                let read = ui.call("money.invest.preview", payload).await;
                if let (Ok(v), Some(counts)) = (read, counts.upgrade().filter(|_| serial.get() == asked)) {
                    counts.set_text(&counts_line(&v));
                }
            });
        });
        body.append(&picker);
        into = Some((picker, ids));
    } else {
        body.append(&label("ACCOUNTS IN THE FILE", "money-label"));
        if found.is_empty() {
            body.append(&label("This file names no account.", "money-muted"));
        }
        for a in &found {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.add_css_class("money-import-account");
            let registration = text(a, "registration");
            row.append(&badge_sized("chart", registration_hue(registration), 28));
            let words = gtk::Box::new(gtk::Orientation::Vertical, 1);
            words.set_hexpand(true);
            words.set_valign(gtk::Align::Center);
            let name = label(text(a, "name"), "money-row-title");
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            words.append(&name);
            let lines = copy::plural(n(a, "rows").max(0), "line");
            let said = label(&format!("{} · {} · {lines}", registration_label(registration), text(a, "number")), "money-row-detail");
            said.set_ellipsize(gtk::pango::EllipsizeMode::End);
            words.append(&said);
            row.append(&words);
            let mut ids = vec![None];
            ids.extend(investment.iter().map(|(id, _)| Some(*id)));
            let mut names = vec!["A new account"];
            names.extend(investment.iter().map(|(_, name)| name.as_str()));
            let picker = gtk::DropDown::from_strings(&names);
            picker.add_css_class("money-picker");
            picker.set_valign(gtk::Align::Center);
            picker.set_tooltip_text(Some("Which of Tally's accounts this one goes into"));
            picker.update_property(&[gtk::accessible::Property::Label(&format!("Tally account for {}", text(a, "name")))]);
            let known = a["account_id"].as_i64().and_then(|id| ids.iter().position(|i| *i == Some(id)));
            picker.set_selected(known.unwrap_or(0) as u32);
            row.append(&picker);
            body.append(&row);
            // An account Tally already keeps by hand (or from another institution's files) may be
            // this one: as a new account it would count twice. Said until another is chosen.
            let same: Vec<&str> = held
                .iter()
                .filter(|t| text(t, "registration") == registration && text(t, "institution") != "Wealthsimple")
                .map(|t| text(t, "name"))
                .collect();
            if known.is_none() && !same.is_empty() {
                let (is, one) = if same.len() == 1 { ("is", "it") } else { ("are", "one of them") };
                let hint = label(
                    &format!("{} {is} already in Tally. If {one} is this account, choose it here; a new account would count it twice.", join_and(&same)),
                    "money-row-detail",
                );
                hint.add_css_class("money-ahead");
                hint.set_wrap(true);
                hint.set_margin_start(40);
                body.append(&hint);
                picker.connect_selected_notify(move |picker| hint.set_visible(picker.selected() == 0));
            }
            mapping.push((text(a, "number").to_string(), picker, ids));
        }
    }

    // The lines it skips, by reason.
    if !skipped.is_empty() {
        let mut reasons: Vec<(String, Vec<i64>)> = Vec::new();
        for line in &skipped {
            let (reason, at) = (text(line, "reason").to_string(), n(line, "line"));
            match reasons.iter().position(|(r, _)| *r == reason) {
                Some(i) => reasons[i].1.push(at),
                None => reasons.push((reason, vec![at])),
            }
        }
        body.append(&label("SKIPPED", "money-label"));
        for (reason, at) in reasons.iter().take(5) {
            let shown: Vec<String> = at.iter().take(6).map(|l| l.to_string()).collect();
            let which = match at.len() {
                1 => format!("line {}", shown.join("")),
                more => format!("lines {}{}", shown.join(", "), if more > 6 { ", …" } else { "" }),
            };
            let said = label(&format!("{reason} ({which})"), "money-row-detail");
            said.set_wrap(true);
            body.append(&said);
        }
        if reasons.len() > 5 {
            body.append(&label(&format!("and {} more", copy::plural(reasons.len() as i64 - 5, "reason")), "money-row-detail"));
        }
    }

    let problem = label("", "money-problem");
    problem.set_wrap(true);
    problem.set_visible(false);
    body.append(&problem);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.add_css_class("money-sheet-actions");
    let another = button("Choose another file…", "money-secondary");
    another.set_widget_name("money-invest-another");
    let import = button("Import", "primary");
    import.add_css_class("money-hero-action");
    import.set_widget_name("money-invest-import-save");
    import.set_hexpand(true);
    // A statement's Import waits for its account to be chosen.
    import.set_sensitive(!statement || into.as_ref().is_some_and(|(picker, _)| picker.selected() > 0));
    actions.append(&another);
    actions.append(&import);
    body.append(&actions);
    match &into {
        Some((picker, _)) => {
            let key = import.downgrade();
            picker.connect_selected_notify(move |picker| {
                // Not while an import runs: its key says so until it ends.
                if let Some(key) = key.upgrade().filter(|key| key.label().is_some_and(|l| l == "Import")) {
                    key.set_sensitive(picker.selected() > 0);
                }
            });
            if import.is_sensitive() {
                import.grab_focus();
            } else {
                picker.grab_focus();
            }
        }
        None => {
            import.grab_focus();
        }
    }
    {
        let (ui, sheet, expects, on_done) = (ui.clone(), Rc::downgrade(panel), expects.to_string(), on_done.clone());
        another.connect_clicked(move |_| choose_again(&ui, &sheet, &expects, &on_done));
    }
    let (ui, sheet, shown) = (ui.clone(), Rc::downgrade(panel), path.to_string_lossy().to_string());
    let problem_line = problem.clone();
    import.connect_clicked(move |key| {
        let accounts: Vec<Value> = mapping
            .iter()
            .map(|(number, picker, ids)| {
                let mut entry = json!({"number":number});
                if let Some(Some(id)) = ids.get(picker.selected() as usize) {
                    entry["account_id"] = json!(id);
                }
                entry
            })
            .collect();
        let mut payload = json!({"path":shown,"accounts":accounts});
        if let Some((picker, ids)) = &into {
            match ids.get(picker.selected() as usize).copied().flatten() {
                Some(id) => payload["account_id"] = json!(id),
                None => {
                    problem_line.set_text("Choose the account this statement goes into.");
                    problem_line.set_visible(true);
                    return;
                }
            }
        }
        problem_line.set_visible(false);
        key.set_sensitive(false);
        key.set_label("Importing…");
        let (ui, sheet, problem, key, on_done) = (ui.clone(), sheet.clone(), problem_line.clone(), key.clone(), on_done.clone());
        glib::spawn_future_local(async move {
            match ui.call("money.invest.import", payload).await {
                Ok(result) => {
                    if let Some(sheet) = sheet.upgrade() {
                        sheet.close();
                    }
                    on_done(result);
                }
                Err(e) => {
                    problem.set_text(&refusal(&e));
                    problem.set_visible(true);
                    key.set_label("Import");
                    key.set_sensitive(true);
                }
            }
        });
    });
}
