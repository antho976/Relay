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
//! In the premium look: the bank picture with the one way in, then the banks on one card, each
//! with a state dot and Sign in when it needs one.

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

/// What a request in flight is for.
#[derive(Clone, Debug, PartialEq)]
enum Op {
    List,
    HostedLink,
    Status,
    Follow(String),
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
}

#[derive(Default)]
pub struct State {
    answer: Option<Answer>,
    pending: Vec<(Op, u64)>,
    link: Option<Link>,
    /// A bank just connected (or opened to edit), its accounts being picked.
    choosing: Option<Bank>,
    since: Since,
    working: Option<String>,
    problem: Option<String>,
    notice: Option<String>,
    mine: Vec<Mine>,
    removing: Option<Bank>,
    /// The bank account whose "Into" sheet is open.
    picking: Option<String>,
    sheet: Scroll,
}

impl State {
    fn busy(&self) -> bool {
        self.pending.iter().any(|(op, _)| !matches!(op, Op::List | Op::Status))
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
            let st = &mut app.st.banks;
            st.link = None;
            st.working = None;
            st.problem = Some("That Plaid page expired before a bank was connected. Connect again.".into());
        } else if now >= link.asked_ns + POLL_NS && !app.st.banks.pending.iter().any(|(o, _)| *o == Op::Status) {
            link.asked_ns = now;
            let token = link.token.clone();
            ask(app, Op::Status, "money.bank.link_status", json!({ "link_token": token }));
        }
    }
}

fn answered(app: &mut App, op: Op, r: Result<Value, crate::sync::Failure>) {
    match op {
        Op::List => app.st.banks.answer = Some(logic::answer(r)),
        Op::HostedLink => {
            let st = &mut app.st.banks;
            match r.map(|v| logic::parse_hosted_link(&v)) {
                Ok(Some(link)) => {
                    let now = app.now_ns;
                    let until = match link.expires_at {
                        Some(at) => now + ((at - now_ms()).max(0) as u64).saturating_mul(1_000_000),
                        None => now + LINK_LIFE_NS,
                    };
                    st.working = Some("Waiting for Plaid: sign in to your bank on the page that opened, then come back".into());
                    st.link = Some(Link { token: link.link_token, url: link.url.clone(), until_ns: until, asked_ns: now });
                    app.host.open_url(&link.url);
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
        Op::Status => {
            // A failed check is asked again at the next turn; the page says why meanwhile.
            let state = match r {
                Ok(v) => logic::parse_link_state(&v),
                Err(f) => {
                    app.st.banks.problem = Some(logic::failure_words(&f));
                    return;
                }
            };
            let mine = app.st.banks.mine.clone();
            let st = &mut app.st.banks;
            match state {
                Some(LinkState::Waiting) | None => {}
                Some(LinkState::Connected(bank)) => {
                    st.link = None;
                    st.working = None;
                    st.problem = None;
                    st.choosing = Some(logic::suggested(&bank, &mine));
                }
                Some(LinkState::Exited(why)) => {
                    st.link = None;
                    st.working = None;
                    st.problem = Some(why.unwrap_or_else(|| "Plaid's page closed before a bank was connected".into()));
                }
                Some(LinkState::Expired) => {
                    st.link = None;
                    st.working = None;
                    st.problem = Some("That Plaid page expired before a bank was connected. Connect again.".into());
                }
            }
        }
        Op::Follow(institution) => {
            app.st.banks.working = None;
            match r {
                Ok(_) => {
                    app.st.sync.sync_now();
                    let st = &mut app.st.banks;
                    st.choosing = None;
                    st.notice = Some(format!("{institution} is connected. New purchases arrive with the next sync."));
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

pub fn draw(app: &mut App, dt: f32) -> bool {
    if app.fresh {
        app.st.banks.mine = mine(app);
        if !app.st.banks.pending.iter().any(|(o, _)| *o == Op::List) {
            ask(app, Op::List, "money.bank.list", json!({}));
        }
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
    let st = &app.st.banks;
    pg_raw.moving || !st.pending.is_empty() || st.link.is_some() || st.picking.is_some()
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
            ask(app, Op::HostedLink, "money.bank.hosted_link", logic::hosted_link_payload());
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
            app.st.banks.choosing = Some(b);
            app.scroll().off = 0.0;
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
            ask(app, Op::Follow(b.institution.clone()), "money.bank.accounts", logic::follow_payload(&b, &since));
        }
        Act::Cancel => {
            app.st.banks.choosing = None;
            app.scroll().off = 0.0;
        }
        Act::Refresh => {
            let st = &mut app.st.banks;
            st.problem = None;
            st.working = Some("Checking your banks".into());
            ask(app, Op::Refresh, "money.bank.refresh", json!({}));
        }
        Act::AskRemove(b) => app.st.banks.removing = Some(b),
        Act::Remove(Some(true)) => {
            let Some(b) = app.st.banks.removing.take() else { return };
            // Disconnecting is asked from the bank's own page: back to the list.
            app.st.banks.choosing = None;
            app.scroll().off = 0.0;
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
                y += sk::rows_card(cx, y, &rows).0 + 14.0;
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
/// in `waiting`), and Sign in when it needs one.
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
        if b.problem.is_some() {
            let pw = sk::pill_w(cx, "Sign in");
            if sk::pill(cx, right, ry + rh / 2.0, "Sign in") && free {
                signed = true;
                *act = Some(Act::Connect);
            }
            right -= pw + 12.0;
        } else {
            cx.c.icon("chevron-right", right - 7.0, ry + rh / 2.0, 14.0, p.ink3);
            right -= 22.0;
        }
        let room = right - tx;
        let tt = Text::new(16.0, p.ink).medium();
        let lt = Text::new(13.0, if b.problem.is_some() { p.waiting_text } else { p.ink2 });
        let top = ry + (rh - tt.lh() - 3.0 - lt.lh()) / 2.0;
        cx.c.text_fit(&b.institution, tx, top, room, tt);
        let followed = b.accounts.iter().filter(|a| a.follow).count() as i64;
        let words = match &b.problem {
            Some(why) => why.clone(),
            None => {
                let read = b.last_sync.map(|t| format!("Synced {}", logic::ago(t, now))).unwrap_or_else(|| "Not read yet".into());
                format!("{read} · {}", copy::plural(followed, "account"))
            }
        };
        let ly = top + tt.lh() + 3.0;
        cx.c.circle(tx + 3.5, ly + lt.lh() / 2.0, 3.5, if b.problem.is_some() { p.waiting } else { p.live });
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
    let (back, _, h) = widgets::page_header(cx, y, &bank.institution, None);
    if back {
        *act = Some(Act::Cancel);
    }
    let mut y = y + h + 4.0;
    y += status(cx, y, st, act);
    // The connected mark, the title and a line.
    let mid = WIDTH / 2.0;
    y += 8.0;
    cx.c.circle(mid, y + 32.0, 32.0, theme::soft_of(p.live));
    cx.c.icon("check", mid, y + 32.0, 28.0, p.live_text);
    y += 64.0 + 16.0;
    let tt = Text::new(24.0, p.ink).semi().center().line(1.2);
    y += cx.c.para(&format!("{} is connected", bank.institution), PAD, y, W, tt, 2) + 8.0;
    let n = bank.accounts.len();
    let line = format!("Tally found {}. Pick the ones you pay with; the rest stay out.", copy::plural(n as i64, "account"));
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
            if cx.tap(x + 8.0, ry + 30.0, sw + 34.0, 32.0) {
                *act = Some(Act::PickInto(a.id.clone()));
            }
        }
        widgets::switch(cx, PAD + W - 16.0 - 52.0, ry + (rh - 30.0) / 2.0, a.follow);
        if cx.tap(PAD + W - 16.0 - 64.0, ry, 80.0, rh) {
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
        app.st.banks.choosing = Some(logic::suggested(&bank, &mine));
    }
}
