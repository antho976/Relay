//! Bank connections through Plaid (Kotlin: ui/banks/BanksScreen.kt, BanksViewModel.kt,
//! BanksCard.kt; data/banks/Banks.kt; the spec is docs/PLAID.md).
//!
//! Tally RS has no Java Plaid SDK, so it connects through Plaid's Hosted Link: the server makes a
//! link token with a hosted-link page (`money.bank.hosted_link`), the phone opens that page in the
//! browser, and then asks the server every few seconds whether a bank came of it
//! (`money.bank.link_status`); the server trades Plaid's public token itself. From there it is the
//! Kotlin flow: pick the accounts to follow (`money.bank.accounts`), and the server's entries
//! arrive by the ordinary sync. Every request goes through `app.st.sync`'s request and reply; the
//! page never blocks on one, and says honestly when this phone is not paired or the server has no
//! bank connections.
//!
//! A bank that asks for a sign-in again (signed out, or its consent running out within the week)
//! has Fix on its row, which opens the same hosted page in Plaid's update mode
//! (`money.bank.hosted_link` with `bank_id`): one sign-in, and the bank stays as it was. While
//! Tally is open it lists the banks in the background every few hours ([`watch`]), so Overview
//! can say a week ahead that a bank wants a sign-in.
//!
//! In the premium look: the bank picture with the one way in, then the banks on one card, each
//! with a state dot and Fix when it needs signing in again.

use relay_money::copy;
use relay_money::model::AccountType;
use serde_json::{Value, json};

use crate::app::App;
use crate::banks_logic::{self as logic, Answer, Bank, LinkState, Mine, Since};
use crate::cx;
use crate::draw::{Text, WIDTH};
use crate::kit::{self, Ctx, PAD, PicCard};
use crate::nav::Route;
use crate::settings_kit::{self as sk, GAP, GROUP_GAP, Row, W};
use crate::sync::Reply;
use crate::theme;
use crate::ui::Scroll;
use crate::widgets::{self, SheetOut};

/// How often the phone asks whether Plaid's page connected a bank.
const POLL_NS: u64 = 3_000_000_000;
/// How long it keeps asking when the server gives no expiry: Plaid's hosted pages last 30 minutes.
const LINK_LIFE_NS: u64 = 30 * 60 * 1_000_000_000;
/// A Plaid page that comes back within this long of Connect opens in the browser on its own; a
/// later one (the person left Banks meanwhile) waits for "Open Plaid again".
const OPEN_FRESH_NS: u64 = 30 * 1_000_000_000;

/// What a request in flight is for.
#[derive(Clone, Debug, PartialEq)]
enum Op {
    List,
    /// A Plaid page for a new bank, or (with the bank) to sign in to that one again.
    HostedLink(Option<Bank>),
    /// The state of the Plaid page with this link token: an answer for a page given up on (Stop
    /// waiting, then Connect again) must not end the newer one.
    Status(String),
    /// Following a bank's accounts: the bank's id (its page may have been left for another's
    /// meanwhile), its name, and whether it still needs signing in again (opened from Fix).
    Follow { bank: String, institution: String, signed_out: bool },
    Refresh,
    Remove(String),
}

/// A Hosted Link page open in the browser, waiting on the person.
#[derive(Clone, Debug)]
struct Link {
    token: String,
    url: String,
    until_ns: u64,
    asked_ns: u64,
    /// The bank being signed in to again, by name; none for a new bank.
    fix: Option<String>,
}

#[derive(Default)]
pub struct State {
    answer: Option<Answer>,
    pending: Vec<(Op, u64)>,
    link: Option<Link>,
    /// A bank just connected (or opened to edit), its accounts being picked.
    choosing: Option<Bank>,
    /// A bank that finished connecting while another bank's page was open: it opens once that
    /// page closes, so the switches set there are not thrown away.
    next: Option<Bank>,
    since: Since,
    working: Option<String>,
    problem: Option<String>,
    notice: Option<String>,
    mine: Vec<Mine>,
    /// `app.ledger_gen` when `mine` was read: a sync bringing in the accounts a Follow made is
    /// read again, so "Into" names them.
    mine_gen: u64,
    /// When Connect asked for a Plaid page (`app.now_ns`).
    connect_ns: u64,
    removing: Option<Bank>,
    /// The bank account whose "Into" sheet is open.
    picking: Option<String>,
    sheet: Scroll,
    /// The background listing in flight ([`watch`]).
    watching: Option<u64>,
    /// What wants a sign-in, as last listed ([`logic::renew_pref`] until then).
    renew: Option<Vec<logic::Renew>>,
}

impl State {
    fn busy(&self) -> bool {
        self.pending.iter().any(|(op, _)| !matches!(op, Op::List | Op::Status(_)))
    }
}

/// The Tally accounts the server's connections already follow into, so a bank being placed is
/// not suggested into one of them: from the list as last read, else as the Banks page last kept
/// it. The bank being placed may be among them; its own places are taken anyway.
fn followed_elsewhere(app: &App) -> Vec<String> {
    match &app.st.banks.answer {
        Some(Answer::Ready(list)) => logic::followed_uids(list),
        _ => logic::followed_pref(&app.store).unwrap_or_default(),
    }
}

/// Tally's own accounts with their uids, which "Into" names.
fn mine(app: &App) -> Vec<Mine> {
    let types = |t: &str| match t {
        "CASH" => AccountType::Cash,
        "SAVINGS" => AccountType::Savings,
        "CREDIT" => AccountType::Credit,
        "INVESTMENT" => AccountType::Investment,
        _ => AccountType::Chequing,
    };
    let Ok(db) = app.store.sql() else { return Vec::new() };
    let Ok(mut st) = db.prepare("SELECT id, uid, name, type, archived, institution, external_ref FROM accounts WHERE deleted = 0 ORDER BY sort_order, id") else {
        return Vec::new();
    };
    st.query_map([], |r| {
        Ok(Mine {
            id: r.get(0)?,
            uid: r.get(1)?,
            name: r.get(2)?,
            r#type: types(&r.get::<_, String>(3)?),
            archived: r.get::<_, i64>(4)? != 0,
            institution: r.get(5)?,
            external_ref: r.get(6)?,
        })
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

fn ask(app: &mut App, op: Op, name: &str, payload: Value) {
    let ticket = app.st.sync.request(&app.store, name, payload);
    app.st.banks.pending.push((op, ticket));
}

fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// Collects the answers that came, and asks Plaid's page's state again when it is time.
fn pump(app: &mut App) {
    let pending = std::mem::take(&mut app.st.banks.pending);
    let mut still = Vec::new();
    for (op, ticket) in pending {
        match app.st.sync.reply(ticket) {
            Reply::Pending => still.push((op, ticket)),
            Reply::Unknown => {}
            Reply::Done(r) => answered(app, op, r),
        }
    }
    let st = &mut app.st.banks;
    still.append(&mut st.pending);
    st.pending = still;
    let now = app.now_ns;
    if let Some(link) = &mut app.st.banks.link {
        if now > link.until_ns {
            let words = match &link.fix {
                Some(b) => format!("That Plaid page expired before you signed in to {b}. Tap Fix again."),
                None => "That Plaid page expired before a bank was connected. Connect again.".into(),
            };
            let st = &mut app.st.banks;
            st.link = None;
            st.working = None;
            st.problem = Some(words);
        } else if now >= link.asked_ns + POLL_NS && !app.st.banks.pending.iter().any(|(o, _)| matches!(o, Op::Status(t) if *t == link.token)) {
            link.asked_ns = now;
            let token = link.token.clone();
            ask(app, Op::Status(token.clone()), "money.bank.link_status", json!({ "link_token": token }));
        }
    }
}

fn answered(app: &mut App, op: Op, r: Result<Value, crate::sync::Failure>) {
    match op {
        Op::List => {
            let answer = logic::answer(r);
            // The Accounts page says which accounts are connected by this, the server's own word.
            if let logic::Answer::Ready(list) = &answer {
                listed(app, list);
            }
            app.st.banks.answer = Some(answer);
        }
        Op::HostedLink(fix) => {
            let on_banks = app.route() == &Route::Banks;
            // A server from before the sign-in again: the old way out, on the bank's own page.
            if let (Some(b), Err(f)) = (&fix, &r)
                && logic::fix_not_on_server(f)
            {
                app.st.banks.working = None;
                open_signed_out(app, b.clone());
                return;
            }
            let st = &mut app.st.banks;
            match r.map(|v| logic::parse_hosted_link(&v)) {
                Ok(Some(link)) => {
                    let now = app.now_ns;
                    let until = match link.expires_at {
                        Some(at) => now + ((at - now_ms()).max(0) as u64).saturating_mul(1_000_000),
                        None => now + LINK_LIFE_NS,
                    };
                    let name = fix.as_ref().map(|b| b.institution.clone());
                    let bank = name.clone().unwrap_or_else(|| "your bank".into());
                    st.link = Some(Link { token: link.link_token, url: link.url.clone(), until_ns: until, asked_ns: now, fix: name });
                    // The browser opens on its own only right after Connect, with the person still
                    // here; an answer read long after (they left Banks meanwhile) waits for them.
                    let fresh = now.saturating_sub(st.connect_ns) < OPEN_FRESH_NS && on_banks;
                    if fresh {
                        st.working = Some(format!("Waiting for Plaid: sign in to {bank} on the page that opened, then come back"));
                        app.host.open_url(&link.url);
                    } else {
                        st.working = Some(format!("Plaid's page is ready: open it to sign in to {bank}, then come back"));
                    }
                }
                Ok(None) => {
                    st.working = None;
                    st.problem = Some("Your server did not hand back a Plaid page".into());
                }
                Err(f) => {
                    st.working = None;
                    st.problem = Some(logic::failure_words(&f));
                }
            }
        }
        Op::Status(token) => {
            // About a page no longer waited on: the one waited on now (if any) has its own asks.
            if app.st.banks.link.as_ref().is_none_or(|l| l.token != token) {
                return;
            }
            // A failed check is asked again at the next turn; the page says why meanwhile.
            let state = match r {
                Ok(v) => logic::parse_link_state(&v),
                Err(f) => {
                    app.st.banks.problem = Some(logic::failure_words(&f));
                    return;
                }
            };
            let mine = app.st.banks.mine.clone();
            let elsewhere = followed_elsewhere(app);
            let fix = app.st.banks.link.as_ref().and_then(|l| l.fix.clone());
            let st = &mut app.st.banks;
            match state {
                Some(LinkState::Waiting) | None => {}
                // Signed in to again: the bank is as it was, so there is nothing to pick.
                Some(LinkState::Connected(bank)) if fix.is_some() => {
                    st.link = None;
                    st.working = None;
                    st.problem = None;
                    st.notice = Some(format!("{} is signed in again. New purchases arrive with the next sync.", bank.institution));
                    app.st.sync.sync_now();
                    ask(app, Op::List, "money.bank.list", json!({}));
                }
                Some(LinkState::Connected(bank)) => {
                    st.link = None;
                    st.working = None;
                    st.problem = None;
                    let suggested = logic::suggested_beside(&bank, &mine, &elsewhere);
                    if st.choosing.as_ref().is_some_and(|b| b.id != suggested.id) {
                        st.notice = Some(waiting_line(&suggested.institution));
                        st.next = Some(suggested);
                    } else {
                        st.choosing = Some(suggested);
                    }
                    // The server holds the new bank now: ask the list again, so leaving its
                    // accounts (Cancel, Back) shows it there rather than inviting a second connect.
                    if !st.pending.iter().any(|(o, _)| *o == Op::List) {
                        ask(app, Op::List, "money.bank.list", json!({}));
                    }
                }
                Some(LinkState::Exited(why)) => {
                    st.link = None;
                    st.working = None;
                    st.problem = Some(why.unwrap_or_else(|| match &fix {
                        Some(b) => format!("Plaid's page closed before you signed in to {b}"),
                        None => "Plaid's page closed before a bank was connected".into(),
                    }));
                }
                Some(LinkState::Expired) => {
                    st.link = None;
                    st.working = None;
                    st.problem = Some(match &fix {
                        Some(b) => format!("That Plaid page expired before you signed in to {b}. Tap Fix again."),
                        None => "That Plaid page expired before a bank was connected. Connect again.".into(),
                    });
                }
            }
        }
        Op::Follow { bank, institution, signed_out } => {
            app.st.banks.working = None;
            match r {
                Ok(_) => {
                    app.st.sync.sync_now();
                    let st = &mut app.st.banks;
                    // Only this bank's page closes: another opened while the answer came stays.
                    if st.choosing.as_ref().is_some_and(|b| b.id == bank) {
                        close_bank(st);
                    }
                    // A bank that needs signing in again is not connected yet: its accounts are
                    // kept, but nothing arrives until the person signs in.
                    st.notice = Some(if signed_out {
                        format!("{institution}'s accounts are saved. Sign in to it again before new purchases can arrive.")
                    } else {
                        format!("{institution} is connected. New purchases arrive with the next sync.")
                    });
                    ask(app, Op::List, "money.bank.list", json!({}));
                }
                Err(f) => app.st.banks.problem = Some(logic::failure_words(&f)),
            }
        }
        Op::Refresh => {
            app.st.banks.working = None;
            match r {
                Ok(_) => {
                    app.st.sync.sync_now();
                    app.st.banks.notice = Some("Your server is reading your banks. New purchases arrive with the next sync.".into());
                }
                Err(f) => app.st.banks.problem = Some(logic::failure_words(&f)),
            }
        }
        Op::Remove(institution) => {
            app.st.banks.working = None;
            match r {
                Ok(_) => {
                    app.st.banks.notice = Some(format!("{institution} is disconnected. Its entries stay in your ledger."));
                    ask(app, Op::List, "money.bank.list", json!({}));
                }
                Err(f) => app.st.banks.problem = Some(logic::failure_words(&f)),
            }
        }
    }
}

enum Act {
    Back,
    Sync,
    Reload,
    Connect,
    OpenAgain,
    StopWaiting,
    Dismiss,
    Edit(Bank),
    SignInAgain(Bank),
    Toggle(String),
    PickInto(String),
    Into(String, Option<String>),
    CloseSheet,
    Since(Since),
    Follow,
    Cancel,
    Refresh,
    AskRemove(Bank),
    Remove(Option<bool>),
}

/// Back closes the sheet or the dialog open over the page, then leaves a bank's accounts for the
/// list (as Cancel does).
pub fn back(app: &mut App) -> bool {
    let st = &mut app.st.banks;
    if st.picking.take().is_some() || st.removing.take().is_some() {
        return true;
    }
    if st.choosing.is_some() {
        leave_bank(app);
        return true;
    }
    false
}

/// A server from before the sign-in again can only connect a bank anew, which would add this one a
/// second time: its page opens with the way out instead (see `logic::sign_in_again_line`).
fn open_signed_out(app: &mut App, b: Bank) {
    let st = &mut app.st.banks;
    st.notice = None;
    st.problem = Some(logic::sign_in_again_line(&b.institution));
    // As Edit: an Into a Tally account deleted since is placed afresh, or Follow would send the
    // deleted uid while the page reads "Into a new account".
    reload_mine(app, true);
    let elsewhere = followed_elsewhere(app);
    app.st.banks.choosing = Some(logic::with_places_beside(&b, &app.st.banks.mine, &elsewhere));
    app.scroll().off = 0.0;
}

/// The server just listed `banks`: what they follow (the Accounts page reads it) and what wants a
/// sign-in (Overview reads it) are kept.
fn listed(app: &mut App, banks: &[Bank]) {
    if logic::remember_followed(&mut app.store, banks) {
        app.st.accounts.forget_list();
    }
    app.st.banks.renew = Some(logic::remember_renewals(&mut app.store, banks, now_ms()));
}

/// The banks that want a sign-in, as last listed: Overview's line.
pub fn renewals(app: &mut App) -> &[logic::Renew] {
    if app.st.banks.renew.is_none() {
        app.st.banks.renew = Some(logic::renew_pref(&app.store));
    }
    app.st.banks.renew.as_deref().unwrap_or_default()
}

/// Once a frame: while Tally is open and paired, lists the banks every few hours, so a bank that
/// asks for a sign-in within the week is said on Overview without opening Banks. The listing asks
/// the server only (`money.bank.list` reaches no bank and no Plaid). Banks lists them itself.
pub fn watch(app: &mut App) {
    if let Some(ticket) = app.st.banks.watching {
        match app.st.sync.reply(ticket) {
            Reply::Pending => return,
            Reply::Unknown => app.st.banks.watching = None,
            Reply::Done(r) => {
                app.st.banks.watching = None;
                match logic::answer(r) {
                    Answer::Ready(list) => listed(app, &list),
                    // Not on this server, or not answering: asked again at the next turn, not
                    // every frame. What was last known stays.
                    _ => {
                        let _ = app.store.set_pref(logic::CHECKED_PREF, &now_ms().to_string());
                    }
                }
            }
        }
        return;
    }
    if app.route() == &Route::Banks || !app.st.sync.paired(&app.store) || !logic::check_due(&app.store, now_ms()) {
        return;
    }
    app.st.banks.watching = Some(app.st.sync.request(&app.store, "money.bank.list", json!({})));
}

/// From a bank's page back to the list (Cancel, Back).
fn leave_bank(app: &mut App) {
    let st = &mut app.st.banks;
    // The sign-in-again words are about this bank's page; they don't follow back to the list.
    if let Some(b) = st.choosing.as_ref()
        && st.problem.as_deref() == Some(logic::sign_in_again_line(&b.institution).as_str())
    {
        st.problem = None;
    }
    close_bank(st);
    app.scroll().off = 0.0;
}

/// What the page says while a bank that just connected waits for the open one to close.
fn waiting_line(institution: &str) -> String {
    format!("{institution} is connected. Its accounts open once you leave this bank's page.")
}

/// Closes the bank's page open, opening the one that connected meanwhile, if any.
fn close_bank(st: &mut State) {
    st.choosing = st.next.take();
    if let Some(b) = &st.choosing
        && st.notice.as_deref() == Some(waiting_line(&b.institution).as_str())
    {
        st.notice = None;
    }
}

/// Reads Tally's accounts again when the ledger changed since (a sync bringing in what a Follow
/// made), or `always`.
fn reload_mine(app: &mut App, always: bool) {
    if always || app.st.banks.mine_gen != app.ledger_gen {
        app.st.banks.mine = mine(app);
        app.st.banks.mine_gen = app.ledger_gen;
    }
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    // Banks opened anew starts on the list: a bank's page left by the drawer is not where it
    // reopens (one that connected meanwhile still opens, as leaving a bank's page does).
    if app.fresh && !app.returned && app.st.banks.choosing.is_some() {
        app.st.banks.picking = None;
        leave_bank(app);
    }
    reload_mine(app, app.fresh);
    if app.fresh && !app.st.banks.pending.iter().any(|(o, _)| *o == Op::List) {
        ask(app, Op::List, "money.bank.list", json!({}));
    }
    pump(app);
    let pg_raw = app.page_begin(dt);
    let mut pg = pg_raw;
    let choosing = app.st.banks.choosing.is_some();
    let ready = matches!(app.st.banks.answer, Some(Answer::Ready(_)));
    let has_footer = choosing || (ready && app.st.banks.link.is_some());
    let mut act = None;
    let bottom;
    let footer_at;
    let footer_tap;
    let held;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.banks;
        let dialog = st.picking.is_some() || st.removing.is_some();
        footer_at = sk::footer_top(&cx, sk::FOOTER_KEYS);
        held = if dialog { widgets::hold(&mut cx) } else { None };
        footer_tap = if has_footer { sk::footer_hold(&mut cx, footer_at) } else { None };
        let y = pg.top;
        let y = if choosing { choose_page(&mut cx, y, st, &mut act) } else { list_page(&mut cx, y, st, &mut act) };
        bottom = y;
    }
    if has_footer {
        pg.bottom = footer_at;
    }
    app.page_end(&pg, bottom);
    {
        let mut cx = cx!(app);
        let st = &mut app.st.banks;
        if has_footer {
            cx.ui.tap = footer_tap;
            sk::footer_ground(&mut cx, footer_at);
            let ky = footer_at + 14.0;
            let free = !st.busy();
            if let Some(b) = &st.choosing {
                let n = b.accounts.iter().filter(|a| a.follow).count();
                if sk::wide_key(&mut cx, PAD, ky, W, &logic::follow_label(n), None, true, free) {
                    act = Some(Act::Follow);
                }
            } else if st.link.is_some() {
                let w1 = (W - 8.0) / 2.0;
                if kit::key(&mut cx, PAD, ky, w1, sk::FOOTER_KEYS, "Stop waiting", None, kit::KeyKind::Plain) {
                    act = Some(Act::StopWaiting);
                }
                if kit::key(&mut cx, PAD + w1 + 8.0, ky, w1, sk::FOOTER_KEYS, "Open Plaid again", Some("external"), kit::KeyKind::Primary) {
                    act = Some(Act::OpenAgain);
                }
            }
        }
        widgets::release(&mut cx, held);
        if let Some(bank) = st.removing.clone() {
            let body = "Your server stops reading it and forgets its access. Entries it brought in stay in your ledger.";
            if let Some(yes) = widgets::confirm(&mut cx, &format!("Disconnect {}?", bank.institution), body, "Disconnect", true) {
                act = Some(Act::Remove(Some(yes)));
            }
        } else if let (Some(id), Some(bank)) = (st.picking.clone(), st.choosing.clone())
            && let Some(acc) = bank.accounts.iter().find(|a| a.id == id)
        {
            let live: Vec<&Mine> = st.mine.iter().filter(|m| !m.archived).collect();
            let mut items = vec![("A new account".to_string(), Some(format!("Named {}", acc.name)))];
            items.extend(live.iter().map(|m| (m.name.clone(), Some(type_word(m.r#type).to_string()))));
            let sel = match &acc.into {
                None => Some(0),
                Some(uid) => live.iter().position(|m| &m.uid == uid).map(|i| i + 1),
            };
            match widgets::sheet_list(&mut cx, &format!("Where {} lands", acc.name), &items, sel, &mut st.sheet, dt) {
                SheetOut::Pick(i) => act = Some(Act::Into(id.clone(), if i == 0 { None } else { live.get(i - 1).map(|m| m.uid.clone()) })),
                SheetOut::Close => act = Some(Act::CloseSheet),
                SheetOut::Nothing => {}
            }
        }
    }
    if let Some(a) = act {
        perform(app, a);
    }
    // Waiting on a Plaid page is not drawing every frame: wake for the next ask (or its expiry).
    if let Some(l) = &app.st.banks.link {
        let at = (l.asked_ns + POLL_NS).min(l.until_ns + 1);
        app.wake_at(at);
    }
    let st = &app.st.banks;
    pg_raw.moving || !st.pending.is_empty() || st.picking.is_some()
}

fn type_word(t: AccountType) -> &'static str {
    match t {
        AccountType::Cash => "Cash",
        AccountType::Chequing => "Chequing",
        AccountType::Savings => "Savings",
        AccountType::Credit => "Credit",
        AccountType::Investment => "Investment",
    }
}

fn perform(app: &mut App, act: Act) {
    match act {
        Act::Back => {
            app.pop();
        }
        Act::Sync => app.push(Route::Sync),
        Act::Reload => {
            app.st.banks.answer = None;
            ask(app, Op::List, "money.bank.list", json!({}));
        }
        Act::Connect => {
            let st = &mut app.st.banks;
            st.problem = None;
            st.notice = None;
            st.working = Some("Asking your server to open Plaid".into());
            st.connect_ns = app.now_ns;
            ask(app, Op::HostedLink(None), "money.bank.hosted_link", logic::hosted_link_payload(None));
        }
        Act::OpenAgain => {
            if let Some(l) = &app.st.banks.link {
                app.host.open_url(&l.url);
            }
        }
        Act::StopWaiting => {
            let st = &mut app.st.banks;
            st.link = None;
            st.working = None;
        }
        Act::Dismiss => {
            let st = &mut app.st.banks;
            st.notice = None;
            st.problem = None;
        }
        Act::Edit(b) => {
            // Its accounts may be new since the page opened: "Into" must name them.
            reload_mine(app, true);
            // An account not yet placed is suggested a place, as on a fresh connect; what it
            // follows stays as the server has it.
            let elsewhere = followed_elsewhere(app);
            app.st.banks.choosing = Some(logic::with_places_beside(&b, &app.st.banks.mine, &elsewhere));
            app.scroll().off = 0.0;
        }
        Act::SignInAgain(b) => {
            // Plaid's update mode for this bank: the same connection, signed in to again. From the
            // bank's own page, back to the list, where the wait and its outcome show.
            if app.st.banks.choosing.as_ref().is_some_and(|c| c.id == b.id) {
                leave_bank(app);
            }
            let st = &mut app.st.banks;
            st.problem = None;
            st.notice = None;
            st.working = Some(format!("Asking your server to open Plaid for {}", b.institution));
            st.connect_ns = app.now_ns;
            let payload = logic::hosted_link_payload(Some(&b.id));
            ask(app, Op::HostedLink(Some(b)), "money.bank.hosted_link", payload);
        }
        Act::Toggle(id) => {
            if let Some(b) = &mut app.st.banks.choosing
                && let Some(a) = b.accounts.iter_mut().find(|a| a.id == id)
            {
                a.follow = !a.follow;
            }
        }
        Act::PickInto(id) => {
            let st = &mut app.st.banks;
            st.picking = Some(id);
            st.sheet = Scroll::default();
        }
        Act::Into(id, uid) => {
            let st = &mut app.st.banks;
            st.picking = None;
            if let Some(b) = &mut st.choosing
                && let Some(a) = b.accounts.iter_mut().find(|a| a.id == id)
            {
                a.into = uid;
            }
        }
        Act::CloseSheet => app.st.banks.picking = None,
        Act::Since(s) => app.st.banks.since = s,
        Act::Follow => {
            let Some(b) = app.st.banks.choosing.clone() else { return };
            let st = &mut app.st.banks;
            st.problem = None;
            st.working = Some("Bringing in your purchases".into());
            let since = st.since.date(crate::model::today());
            let op = Op::Follow { bank: b.id.clone(), institution: b.institution.clone(), signed_out: b.problem.is_some() };
            ask(app, op, "money.bank.accounts", logic::follow_payload(&b, &since));
        }
        Act::Cancel => leave_bank(app),
        Act::Refresh => {
            let st = &mut app.st.banks;
            st.problem = None;
            st.working = Some("Checking your banks".into());
            ask(app, Op::Refresh, "money.bank.refresh", json!({}));
        }
        Act::AskRemove(b) => app.st.banks.removing = Some(b),
        Act::Remove(Some(true)) => {
            let Some(b) = app.st.banks.removing.take() else { return };
            // Disconnecting is asked from the bank's own page: back to the list (or to a bank
            // that connected meanwhile).
            close_bank(&mut app.st.banks);
            app.scroll().off = 0.0;
            app.st.banks.problem = None;
            app.st.banks.working = Some(format!("Disconnecting {}", b.institution));
            ask(app, Op::Remove(b.institution.clone()), "money.bank.remove", json!({ "bank_id": b.id }));
        }
        Act::Remove(_) => app.st.banks.removing = None,
    }
}

/// What the page is doing, what went wrong, or what it did; a tap clears the last two.
fn status(cx: &mut Ctx, y: f32, st: &State, act: &mut Option<Act>) -> f32 {
    let (line, tone) = match (&st.working, &st.problem, &st.notice) {
        (Some(w), _, _) => (w, sk::Tone::Working),
        (None, Some(p), _) => (p, sk::Tone::Problem),
        (None, None, Some(n)) => (n, sk::Tone::Good),
        _ => return 0.0,
    };
    let (h, tapped) = sk::status(cx, y, tone, line);
    if tapped && st.working.is_none() {
        *act = Some(Act::Dismiss);
    }
    h + GAP
}

/// The bank picture with a title, a line and one button. Returns (its height, button tapped).
fn banks_card(cx: &mut Ctx, y: f32, eyebrow: Option<&str>, title: &str, line: &str, button: Option<&str>) -> (f32, bool) {
    let h = 420.0;
    let (_, tapped) = kit::pic_card(cx, PAD, y, W, h, "bank", &PicCard { eyebrow, title, line: Some(line), button, ..Default::default() });
    (h, tapped)
}

/// The quiet line under the page: where bank links go.
const TRUST: &str = "Bank links go through your own server. Tally keeps the transactions, never your password.";

fn list_page(cx: &mut Ctx, y: f32, st: &State, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let (back, _, h) = widgets::page_header(cx, y, "Connected banks", None);
    if back {
        *act = Some(Act::Back);
    }
    let mut y = y + h + 4.0;
    y += status(cx, y, st, act);
    let title = "Bring your bank in";
    let line = "New transactions arrive on their own. You sign in on your bank's page, never in Tally.";
    match &st.answer {
        None => {
            y += banks_card(cx, y, Some("Asking your server"), title, line, None).0 + 14.0;
        }
        Some(Answer::NotPaired) => {
            let (h, t) = banks_card(cx, y, None, "Your server comes first", "Bank links live on your server. Connect this phone to it, then come back here.", Some("Connect your server"));
            if t {
                *act = Some(Act::Sync);
            }
            y += h + 14.0;
        }
        Some(Answer::NotOnServer) => {
            let (h, t) = banks_card(cx, y, None, "Not on your server yet", "Your server's Relay needs the bank update and your Plaid keys.", Some("Ask again"));
            if t {
                *act = Some(Act::Reload);
            }
            y += h + 14.0;
            y += sk::note(cx, y, None, "docs/PLAID.md in Tally's repository says what the update takes. Tally is ready for it.") + 14.0;
        }
        Some(Answer::Failed(reason)) => {
            y += banks_card(cx, y, None, title, line, None).0 + 14.0;
            let tt = Text::new(17.0, p.ink).semi();
            let ct = Text::new(13.0, p.ink2).line(1.45);
            let rh = cx.c.para_h(reason, W - 40.0, ct, 6);
            let h = 20.0 + tt.lh() + 6.0 + rh + 16.0 + 48.0 + 20.0;
            kit::card(cx, PAD, y, W, h);
            cx.c.text("Your server did not answer", PAD + 20.0, y + 20.0, tt);
            cx.c.para(reason, PAD + 20.0, y + 20.0 + tt.lh() + 6.0, W - 40.0, ct, 6);
            if kit::key(cx, PAD + 20.0, y + h - 20.0 - 48.0, W - 40.0, 48.0, "Try again", None, kit::KeyKind::Plain) {
                *act = Some(Act::Reload);
            }
            y += h + 14.0;
        }
        Some(Answer::Ready(banks)) => {
            let free = !st.busy() && st.link.is_none();
            let (h, t) = banks_card(cx, y, None, title, line, Some("Connect a bank"));
            if t && free {
                *act = Some(Act::Connect);
            }
            y += h + 14.0;
            if banks.is_empty() {
                let rows = [Row::new("No bank connected yet").icon("bank").line("Desjardins and Wealthsimple work through Plaid, read only")];
                // The row presses like a button, so a tap on it does what "Connect a bank" does.
                let (h, tapped) = sk::rows_card(cx, y, &rows);
                if tapped.is_some() && free {
                    *act = Some(Act::Connect);
                }
                y += h + 14.0;
            } else {
                y += bank_list(cx, y, banks, free, act) + 14.0;
                if sk::wide_key(cx, PAD, y, W, "Check for new purchases now", Some("sync"), false, !st.busy()) {
                    *act = Some(Act::Refresh);
                }
                y += 52.0 + 14.0;
            }
        }
    }
    y += sk::note(cx, y, None, TRUST);
    y
}

/// The banks on one card: the bank's tile, its name, a line with a dot (good in `live`, a problem
/// in `waiting`), and Fix when it needs signing in again.
fn bank_list(cx: &mut Ctx, y: f32, banks: &[Bank], free: bool, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let now = now_ms();
    let rh = 70.0;
    let h = 8.0 + banks.len() as f32 * rh;
    kit::card(cx, PAD, y, W, h);
    let (x, w) = (PAD + 4.0, W - 8.0);
    let mut ry = y + 4.0;
    for (i, b) in banks.iter().enumerate() {
        if i > 0 {
            sk::divider(cx, ry, true);
        }
        if cx.pressed(x, ry, w, rh) {
            cx.c.rect(x, ry + 2.0, w, rh - 4.0, 14.0, p.wash);
        }
        sk::bank_mark(cx, x + 12.0, ry + (rh - 36.0) / 2.0, 36.0, &b.institution);
        let tx = x + 12.0 + 36.0 + 14.0;
        let mut right = x + w - 12.0;
        let mut signed = false;
        let renew = logic::renewals(std::slice::from_ref(b), now).pop();
        if renew.is_some() {
            let pw = sk::pill_w(cx, "Fix");
            if sk::pill(cx, right, ry + rh / 2.0, "Fix") && free {
                signed = true;
                *act = Some(Act::SignInAgain(b.clone()));
            }
            right -= pw + 12.0;
        } else {
            cx.c.icon("chevron-right", right - 7.0, ry + rh / 2.0, 14.0, p.ink3);
            right -= 22.0;
        }
        let room = right - tx;
        let tt = Text::new(16.0, p.ink).medium();
        let lt = Text::new(13.0, if renew.is_some() { p.waiting_text } else { p.ink2 });
        let top = ry + (rh - tt.lh() - 3.0 - lt.lh()) / 2.0;
        cx.c.text_fit(&b.institution, tx, top, room, tt);
        let followed = b.accounts.iter().filter(|a| a.follow).count() as i64;
        let words = match (&b.problem, &renew) {
            (Some(why), _) => why.clone(),
            (None, Some(r)) => logic::renew_line(r, now),
            (None, None) => {
                let read = b.last_sync.map(|t| format!("Synced {}", logic::ago(t, now))).unwrap_or_else(|| "Not read yet".into());
                format!("{read} · {}", copy::plural(followed, "account"))
            }
        };
        let ly = top + tt.lh() + 3.0;
        cx.c.circle(tx + 3.5, ly + lt.lh() / 2.0, 3.5, if renew.is_some() { p.waiting } else { p.live });
        cx.c.text_fit(&words, tx + 13.0, ly, room - 13.0, lt);
        if !signed && cx.tap(x, ry, w, rh) {
            *act = Some(Act::Edit(b.clone()));
        }
        ry += rh;
    }
    h
}

fn choose_page(cx: &mut Ctx, y: f32, st: &State, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let Some(bank) = &st.choosing else { return y };
    // A bank's page always has Back to the list, even with Banks opened from the drawer: the
    // drawer's key there would leave no way out but Follow.
    let top_level = std::mem::replace(&mut cx.top_level, false);
    let (back, _, h) = widgets::page_header(cx, y, &bank.institution, None);
    cx.top_level = top_level;
    if back {
        *act = Some(Act::Cancel);
    }
    let mut y = y + h + 4.0;
    y += status(cx, y, st, act);
    // The connected mark, the title and a line; a bank that needs signing in again (opened from
    // Fix) says so instead of reading as connected.
    let mid = WIDTH / 2.0;
    y += 8.0;
    let n = bank.accounts.len();
    let (icon, tone, tone_text, title, line) = match &bank.problem {
        Some(why) => ("lock", p.waiting, p.waiting_text, format!("{} needs you to sign in again", bank.institution), why.clone()),
        None => (
            "check",
            p.live,
            p.live_text,
            format!("{} is connected", bank.institution),
            format!("Tally found {}. Pick the ones you pay with; the rest stay out.", copy::plural(n as i64, "account")),
        ),
    };
    cx.c.circle(mid, y + 32.0, 32.0, theme::soft_of(tone));
    cx.c.icon(icon, mid, y + 32.0, 28.0, tone_text);
    y += 64.0 + 16.0;
    let tt = Text::new(24.0, p.ink).semi().center().line(1.2);
    y += cx.c.para(&title, PAD, y, W, tt, 2) + 8.0;
    y += cx.c.para(&line, PAD + 12.0, y, W - 24.0, Text::new(15.0, p.ink2).center().line(1.42), 4) + GROUP_GAP;

    // The accounts, each with its switch and where it lands.
    y += sk::eyebrow(cx, y, "Accounts");
    let rh = 66.0;
    let card_h = 8.0 + n as f32 * rh;
    kit::card(cx, PAD, y, W, card_h);
    let mut ry = y + 4.0;
    for (i, a) in bank.accounts.iter().enumerate() {
        if i > 0 {
            sk::divider(cx, ry, false);
        }
        let x = PAD + 4.0;
        let switch_x = PAD + W - 16.0 - 64.0;
        let room = W - 8.0 - 16.0 - 12.0 - 52.0 - 10.0;
        cx.c.text_fit(&format!("{}{}", a.name, logic::mask(a)), x + 16.0, ry + 12.0, room, Text::new(16.0, p.ink).medium());
        let target = a.into.as_ref().and_then(|u| st.mine.iter().find(|m| &m.uid == u)).map(|m| m.name.clone());
        let sub = if a.follow {
            format!("Into {}", target.unwrap_or_else(|| "a new account".into()))
        } else {
            let kind = if a.kind.trim().is_empty() { "Left out".to_string() } else { a.kind.clone() };
            // "tfsa" and "rrsp" are initials; "credit card" is words.
            let kind = if kind.len() <= 4 && kind.chars().all(|c| c.is_ascii_alphabetic()) {
                kind.to_uppercase()
            } else {
                let mut k = kind.chars();
                k.next().map(|c| c.to_uppercase().collect::<String>() + k.as_str()).unwrap_or_default()
            };
            format!("{kind} · left out")
        };
        let st_t = Text::new(13.0, if a.follow { p.signal } else { p.ink3 });
        let sw = cx.c.text_fit(&sub, x + 16.0, ry + 36.0, room, st_t);
        if a.follow {
            cx.c.icon("chevron-down", x + 16.0 + sw + 10.0, ry + 36.0 + st_t.lh() / 2.0, 12.0, p.signal);
            // Stops short of the switch's own tap box, so a long name can't take its taps.
            if cx.tap(x + 8.0, ry + 30.0, (sw + 34.0).min(switch_x - (x + 8.0)), 32.0) {
                *act = Some(Act::PickInto(a.id.clone()));
            }
        }
        widgets::switch(cx, PAD + W - 16.0 - 52.0, ry + (rh - 30.0) / 2.0, a.follow);
        if cx.tap(switch_x, ry, 80.0, rh) {
            *act = Some(Act::Toggle(a.id.clone()));
        }
        ry += rh;
    }
    y += card_h + GROUP_GAP;
    y += sk::eyebrow(cx, y, "Bring in");
    let labels: Vec<&str> = Since::ALL.iter().map(|s| s.label()).collect();
    let sel = Since::ALL.iter().position(|s| *s == st.since);
    let (picked, h) = widgets::pills(cx, PAD, y, W, &labels, sel);
    if let Some(i) = picked {
        *act = Some(Act::Since(Since::ALL[i]));
    }
    y += h + 12.0;
    y += sk::note(cx, y, None, "Entries you already have, from a CSV or by hand, are matched and skipped.");
    // A bank already connected can be let go from here.
    let known = matches!(&st.answer, Some(Answer::Ready(list)) if list.iter().any(|b| b.id == bank.id));
    // A bank that wants a sign-in is signed in to from its page too, unless this server can't
    // (the page then says how instead).
    let old_server = st.problem.as_deref() == Some(logic::sign_in_again_line(&bank.institution).as_str());
    if known && !old_server && logic::wants_sign_in(bank, now_ms()) {
        y += GROUP_GAP;
        let words = format!("Sign in to {} again", bank.institution);
        let free = !st.busy() && st.link.is_none();
        let (h, tapped) = sk::rows_card(cx, y, &[Row::new(&words).glyph("lock", p.waiting).line("On Plaid's page. Your accounts and choices stay").opens()]);
        if tapped.is_some() && free {
            *act = Some(Act::SignInAgain(bank.clone()));
        }
        y += h;
    }
    if known {
        y += GROUP_GAP;
        let words = format!("Disconnect {}", bank.institution);
        let (h, tapped) = sk::rows_card(cx, y, &[Row::new(&words).glyph("close", p.held).color(p.held_text).line("Its entries stay in your ledger")]);
        if tapped.is_some() {
            *act = Some(Act::AskRemove(bank.clone()));
        }
        y += h;
    }
    y
}

/// Shows the page as if the server had answered `list` to `money.bank.list` and, with `connected`,
/// as if Plaid's page had just connected that bank (for screenshots and tests; the page itself only
/// learns these from the server).
#[doc(hidden)]
pub fn show(app: &mut App, list: Value, connected: Option<Value>) {
    app.st.banks.mine = mine(app);
    app.st.banks.answer = Some(logic::answer(Ok(list)));
    app.st.banks.pending.retain(|(op, _)| *op != Op::List);
    if let Some(bank) = connected.as_ref().and_then(logic::parse_bank) {
        let mine = app.st.banks.mine.clone();
        let elsewhere = followed_elsewhere(app);
        app.st.banks.choosing = Some(logic::suggested_beside(&bank, &mine, &elsewhere));
    }
}
