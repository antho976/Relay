//! Settings, Appearance, Currency and dates, Backup and About (Kotlin: ui/settings/SettingsScreens.kt,
//! SettingsViewModels.kt, BackupViewModel.kt), in the premium look: the coin and the ledger's line
//! at the top, the appearance as one choice, then cards of rows with hairlines.
//!
//! Differences from the Kotlin app, each because Tally RS has no such machinery yet: no weekly
//! backup or backup folder (no background worker), and Export lives on the Backup page (one place
//! for every file out). A switch to a currency with other decimals is refused by the ledger unless
//! it is empty, and the page says so.

use relay_money::backup::BackupFile;
use relay_money::views::TxQuery;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, Ctx, PAD};
use crate::nav::Route;
use crate::settings_files::{self, Job, Poll};
use crate::settings_kit::{self as sk, GAP, GROUP_GAP, Row, W};
use crate::settings_logic as logic;
use crate::settings_store::{self as store, Counts};
use crate::sync::{self, Tone};
use crate::theme::{self, Palette};
use crate::widgets;

#[derive(Default)]
pub struct State {
    /// The ledger generation the counts were read at.
    read_at: Option<u64>,
    counts: Counts,
    settings: Option<relay_money::views::Settings>,
    period: String,
    /// Nothing of the owner's here: the sample may load.
    can_sample: bool,
    erasing: bool,
    /// The sample's question is open.
    sampling: bool,
    /// A restore's pick is out with the platform.
    asked: bool,
    reading: Option<Job<Result<BackupFile, String>>>,
    /// A file being made to share: its name, type and bytes. The worker only makes it; the share
    /// sheet is opened from the frame, on the loop's thread, as every call into the activity is
    /// (from a worker it could reach an activity already destroyed).
    sharing: Option<Job<(String, &'static str, Vec<u8>)>>,
    /// A backup read and waiting on the person's yes.
    restore: Option<(BackupFile, logic::RestorePreview)>,
    /// What the last act on Backup did (words, whether it failed).
    result: Option<(String, bool)>,
}

impl State {
    fn busy(&self) -> Option<&'static str> {
        if self.reading.is_some() {
            Some("Reading the file")
        } else if self.sharing.is_some() {
            Some("Getting the file ready")
        } else {
            None
        }
    }
}

enum Act {
    Back,
    Go(Route),
    Mode(&'static str),
    Currency(String),
    MonthStart(i64),
    Week(bool),
    Sample(Option<bool>),
    Erase(Option<bool>),
    ShareBackup,
    ShareCsv,
    Restore,
    RestoreAnswer(bool),
}

fn reload(app: &mut App) {
    let st = &mut app.st.settings;
    st.counts = store::counts(&app.store);
    st.settings = app.store.ledger.settings().ok();
    st.can_sample = app.store.can_load_sample();
    st.period = app
        .store
        .ledger
        .tx_list(&TxQuery { limit: Some(1), ..Default::default() }, crate::model::today())
        .map(|p| crate::model::period_name(&p.period.start, &p.period.end_exclusive))
        .unwrap_or_default();
    st.read_at = Some(app.ledger_gen);
}

/// Back closes the open question (erase, sample, restore) before the page.
pub fn back(app: &mut App) -> bool {
    let st = &mut app.st.settings;
    std::mem::take(&mut st.erasing) | std::mem::take(&mut st.sampling) | st.restore.take().is_some()
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    // Opened anew (not uncovered by Back): a question left open when the person went elsewhere
    // (the drawer, a notification) is not asked again with its destructive key ready. A backup
    // read for a restore is answered later, by `backup_poll`, so open_backup's file still stands.
    if app.fresh && !app.returned {
        let st = &mut app.st.settings;
        st.erasing = false;
        st.sampling = false;
        st.restore = None;
    }
    if app.fresh || app.st.settings.read_at != Some(app.ledger_gen) {
        reload(app);
    }
    let route = app.route().clone();
    let waiting = if route == Route::Backup { backup_poll(app) } else { false };
    let pg = app.page_begin(dt);
    let mut act = None;
    let bottom;
    {
        let paired = app.st.sync.paired(&app.store);
        let sync_off = app.store.pref(sync::pref::PHONE_ONLY).is_some();
        let line = app.st.sync.line(&app.store, sync::now_ms());
        let mode = app.appearance();
        let locale = app.store.locale.clone();
        let mut cx = cx!(app);
        let st = &mut app.st.settings;
        let dialog = st.erasing || st.sampling || st.restore.is_some();
        let held = if dialog { widgets::hold(&mut cx) } else { None };
        let mut y = pg.top;
        let title = match route {
            Route::Appearance => "Appearance",
            Route::Format => "Currency and dates",
            Route::Backup => "Backup and export",
            Route::About => "About",
            _ => "Settings",
        };
        let (back, _, h) = widgets::page_header(&mut cx, y, title, None);
        if back {
            act = Some(Act::Back);
        }
        y += h;
        y = match route {
            Route::Appearance => appearance(&mut cx, y, &mode, &mut act),
            Route::Format => format_page(&mut cx, y, st, &locale, &mut act),
            Route::Backup => backup(&mut cx, y, st, paired, &mut act),
            Route::About => about(&mut cx, y),
            _ => settings(&mut cx, y, st, &line, &mode, &mut act),
        };
        bottom = y;
        if dialog {
            widgets::release(&mut cx, held);
            if st.erasing {
                let body = if paired && sync_off {
                    // Sync off, no sync comes by itself: the first page then asks whether to turn
                    // it on and bring the ledger back.
                    "Every entry, account, budget, bill and goal goes, on this phone only: your server keeps its ledger. Sync is off, so it comes back only if you turn sync on. To start this phone from nothing, disconnect first."
                } else if paired {
                    "Every entry, account, budget, bill and goal goes, on this phone only: your server keeps its ledger, and the next sync brings it back. To start this phone from nothing, disconnect first."
                } else {
                    "Every entry, account, budget, bill and goal on this phone goes. This cannot be undone."
                };
                if let Some(yes) = widgets::confirm(&mut cx, "Erase everything?", body, "Erase", true) {
                    act = Some(Act::Erase(Some(yes)));
                }
            } else if st.sampling {
                let body = "A labelled sample household (accounts, entries, budgets, bills and goals) takes the place of the categories here. It stays on this phone, with sync off. Erase everything removes it.";
                if let Some(yes) = widgets::confirm(&mut cx, "Load the sample household?", body, "Load it", false) {
                    act = Some(Act::Sample(Some(yes)));
                }
            } else if let Some((_, preview)) = &st.restore {
                let saved = preview.saved_on.map(|d| crate::model::short_date(&d.to_string()));
                let body = logic::restore_prompt(preview, saved.as_deref());
                if let Some(yes) = widgets::confirm(&mut cx, "Restore this backup?", &body, "Restore", true) {
                    act = Some(Act::RestoreAnswer(yes));
                }
            }
        }
    }
    app.page_end(&pg, bottom);
    if let Some(a) = act {
        perform(app, a);
    }
    pg.moving || waiting
}

fn perform(app: &mut App, act: Act) {
    match act {
        Act::Back => {
            app.pop();
        }
        Act::Go(r) => {
            // Backup opened afresh starts clean: no line left from its last visit, and no pick
            // still counted as asked for (a cancelled one would take the next file picked
            // anywhere). Cleared here, not on the page's first frame: open_backup asks for its
            // file just after Backup opens, and that must stand.
            if r == Route::Backup {
                let st = &mut app.st.settings;
                st.result = None;
                st.asked = false;
            }
            app.push(r)
        }
        Act::Mode(m) => app.set_appearance(m),
        Act::Currency(code) => change_currency(app, &code),
        Act::MonthStart(d) => match app.store.ledger.set_month_start_day(d) {
            Ok(()) => app.invalidate(),
            Err(e) => app.say(e.to_string(), None),
        },
        Act::Week(monday) => match store::set_week_starts_monday(&app.store, monday) {
            Ok(()) => app.invalidate(),
            Err(e) => app.say(e, None),
        },
        Act::Sample(None) => app.st.settings.sampling = true,
        Act::Sample(Some(false)) => app.st.settings.sampling = false,
        Act::Sample(Some(true)) => {
            app.st.settings.sampling = false;
            match app.store.load_sample(crate::model::today()) {
                Ok(_) => {
                    // The sample is for trying the app here: it never goes to the server.
                    app.st.sync.keep_on_phone(&mut app.store);
                    let _ = app.store.set_pref("onboarded", "1");
                    app.invalidate();
                    // Its bills post what is due today now, not tomorrow.
                    app.bills_changed();
                    app.say("Sample household loaded, on this phone only; Erase everything removes it", None);
                }
                Err(e) => app.say(e, None),
            }
        }
        Act::Erase(None) => app.st.settings.erasing = true,
        Act::Erase(Some(false)) => app.st.settings.erasing = false,
        Act::Erase(Some(true)) => {
            app.st.settings.erasing = false;
            match app.store.ledger.reset() {
                Ok(()) => {
                    // Its tombstones must never reach the server: the next sync pulls instead.
                    app.st.sync.ledger_replaced(&mut app.store);
                    let _ = app.store.ledger.ensure_categories();
                    let _ = app.store.clear_pref("onboarded");
                    app.invalidate();
                    // Start again from the first page, with nothing under it to go back to.
                    app.push(Route::Overview);
                    if let Some(first) = app.nav.first_mut() {
                        first.0 = Route::Onboarding;
                    }
                    app.fresh = true;
                }
                Err(e) => app.say(format!("Nothing was erased: {e}"), None),
            }
        }
        Act::ShareBackup => share_backup(app),
        Act::ShareCsv => share_csv(app),
        Act::Restore if app.st.sync.paired(&app.store) => {
            app.st.settings.result = Some((RESTORE_PAIRED.into(), true));
        }
        Act::Restore => {
            let st = &mut app.st.settings;
            st.result = None;
            st.asked = true;
            app.host.pick_file("application/json");
            // The desktop answers at once; Android answers through the activity, later.
            backup_poll(app);
        }
        Act::RestoreAnswer(false) => app.st.settings.restore = None,
        Act::RestoreAnswer(true) => {
            let Some((file, _)) = app.st.settings.restore.take() else { return };
            if app.st.sync.paired(&app.store) {
                app.st.settings.result = Some((RESTORE_PAIRED.into(), true));
                return;
            }
            match app.store.ledger.import_backup(&file) {
                Ok(n) => {
                    // It stays on this phone: the next sync, if it connects again, pulls.
                    app.st.sync.ledger_replaced(&mut app.store);
                    let words = format!("Restored {}", logic::entries_text(n as i64));
                    app.st.settings.result = Some((words.clone(), false));
                    let _ = app.store.set_pref("onboarded", "1");
                    app.invalidate();
                    // The restored bills post what is due today now, not tomorrow.
                    app.bills_changed();
                    app.say(words, None);
                }
                Err(e) => app.st.settings.result = Some((format!("Nothing was restored: {e}"), true)),
            }
        }
    }
}

/// Takes a backup file already in hand (one picked as the system stopped Tally, or a test) as
/// if Backup's own Restore had asked for it: it is read and waits on the same "Restore this
/// backup?" question. Nothing is restored without the person's yes. Open Backup first.
pub fn open_backup(app: &mut App, name: String, bytes: Vec<u8>) {
    // As Restore itself: while paired, the restore would be undone by the next sync.
    if app.st.sync.paired(&app.store) {
        app.st.settings.result = Some((RESTORE_PAIRED.into(), true));
        return;
    }
    let st = &mut app.st.settings;
    st.result = None;
    st.asked = true;
    app.file_picked(name, bytes);
}

/// A backup read and waiting on the person's yes (for tests).
pub fn restore_waiting(app: &App) -> bool {
    app.st.settings.restore.is_some()
}

/// Why Restore waits while this phone is paired.
const RESTORE_PAIRED: &str = "Disconnect from your server first, in Settings, Your server. While this phone is connected, its next sync would put your server's ledger back in place of the restore.";

/// Moves Backup's file work along. True while something is still on its way.
fn backup_poll(app: &mut App) -> bool {
    // The person cancelled the picker: Restore no longer waits, and the next file picked (by
    // Import, say) is not taken for a backup.
    if app.st.settings.asked && settings_files::take_cancelled() {
        app.st.settings.asked = false;
    }
    if app.st.settings.asked
        && let Some((_, bytes)) = settings_files::take_picked(app)
    {
        let st = &mut app.st.settings;
        st.asked = false;
        st.reading = Some(Job::spawn(move || logic::read_backup(&bytes)));
    }
    let st = &mut app.st.settings;
    if let Some(job) = &st.reading {
        match job.poll() {
            Poll::Running => {}
            Poll::Done(Ok(file)) => {
                let preview = logic::restore_preview(&file);
                st.restore = Some((file, preview));
                st.reading = None;
            }
            Poll::Done(Err(why)) => {
                st.result = Some((why, true));
                st.reading = None;
            }
            Poll::Lost => {
                st.result = Some(("That file could not be read.".into(), true));
                st.reading = None;
            }
        }
    }
    if let Some(job) = &st.sharing {
        match job.poll() {
            Poll::Running => {}
            Poll::Done((name, mime, bytes)) => {
                app.host.share_file(&name, mime, &bytes);
                st.result = Some((format!("{name} is ready: pick where it goes"), false));
                st.sharing = None;
            }
            Poll::Lost => {
                st.result = Some(("The file could not be made.".into(), true));
                st.sharing = None;
            }
        }
    }
    st.reading.is_some() || st.sharing.is_some()
}

fn share_backup(app: &mut App) {
    let today = settings_files::stamp_today();
    let file = match app.store.ledger.export_backup(&today) {
        Ok(f) => f,
        Err(e) => {
            app.st.settings.result = Some((format!("The backup could not be made: {e}"), true));
            return;
        }
    };
    let name = logic::backup_file_name(&today);
    app.st.settings.result = None;
    app.st.settings.sharing = Some(Job::spawn(move || (name, "application/json", relay_money::backup::encode(&file).into_bytes())));
}

fn share_csv(app: &mut App) {
    let rows = store::csv_rows(&app.store);
    if rows.is_empty() {
        app.st.settings.result = Some(("There are no entries to export yet.".into(), true));
        return;
    }
    let fmt = app.store.formatter();
    let name = logic::csv_file_name(&settings_files::stamp_today());
    app.st.settings.result = None;
    app.st.settings.sharing = Some(Job::spawn(move || (name, "text/csv", relay_money::csv::encode(&rows, &fmt.currency, fmt.fraction_digits).into_bytes())));
}

// The pages

fn ledger_settings(st: &State) -> relay_money::views::Settings {
    st.settings.clone().unwrap_or(relay_money::views::Settings { currency: "CAD".into(), month_start_day: 1, week_starts_monday: true })
}

/// The colour a sync line's words take as a row's value: the good state in `live_text`.
fn tone_text(p: &Palette, tone: Tone) -> Rgba {
    match tone {
        Tone::Live => p.live_text,
        Tone::Waiting => p.waiting_text,
        Tone::Held => p.held_text,
        Tone::Off => p.ink2,
    }
}

fn mode_index(mode: &str) -> usize {
    let name = logic::palette_name(mode);
    theme::MODES.iter().position(|(_, n)| *n == name).unwrap_or(0)
}

fn settings(cx: &mut Ctx, mut y: f32, st: &State, line: &sync::Line, mode: &str, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let s = ledger_settings(st);
    let c = &st.counts;

    // The coin, Tally, and what the ledger holds.
    y += 6.0;
    let d = 64.0;
    sk::coin(cx, PAD, y, d);
    let tx = PAD + d + 16.0;
    let bt = Text::new(26.0, p.ink).brand().line(1.15);
    let lt = Text::new(14.0, p.ink2);
    let block = bt.lh() + 3.0 + lt.lh();
    let ty0 = y + (d - block) / 2.0;
    cx.c.text("Tally", tx, ty0, bt);
    cx.c.text_fit(&logic::ledger_line(c.entries, c.first.as_deref()), tx, ty0 + bt.lh() + 3.0, PAD + W - tx, lt);
    y += d + 30.0;

    // Appearance: one choice, its previews a tap away.
    let et = Text::new(12.0, p.ink3).semi();
    let more = Text::new(13.0, p.ink2).medium().right();
    let mw = cx.c.measure("Previews", more);
    cx.c.text("Previews", PAD + W - 2.0, y + (et.lh() - more.lh()) / 2.0, more);
    if cx.tap(PAD + W - mw - 16.0, y - 10.0, mw + 24.0, et.lh() + 20.0) {
        *act = Some(Act::Go(Route::Appearance));
    }
    y += sk::eyebrow(cx, y, "Appearance");
    let labels: Vec<&str> = theme::MODES.iter().map(|(_, n)| *n).collect();
    if let Some(i) = widgets::segmented(cx, PAD, y, W, &labels, mode_index(mode)) {
        *act = Some(Act::Mode(theme::MODES[i].0));
    }
    y += 44.0 + GROUP_GAP;

    // Your money
    let accounts = c.accounts.to_string();
    let categories = (c.spending + c.income).to_string();
    let rows = [
        Row::new("Currency and dates").icon("coins").value(&s.currency).opens(),
        Row::new("Accounts").icon("wallet").value(&accounts).opens(),
        Row::new("Categories").icon("tag").value(&categories).opens(),
    ];
    let (h, tapped) = sk::group(cx, y, Some("Your money"), &rows);
    if let Some(i) = tapped {
        *act = Some(Act::Go([Route::Format, Route::Accounts, Route::Categories][i].clone()));
    }
    y += h + GROUP_GAP;

    // Sync and banks
    let rows = [
        Row::new("Your server").icon("cloud").value(&line.text).value_color(tone_text(&p, line.tone)).opens(),
        Row::new("Connected banks").icon("bank").opens(),
        Row::new("Import a statement").icon("upload").opens(),
    ];
    let (h, tapped) = sk::group(cx, y, Some("Sync and banks"), &rows);
    if let Some(i) = tapped {
        *act = Some(Act::Go([Route::Sync, Route::Banks, Route::Import][i].clone()));
    }
    y += h + GROUP_GAP;

    // Your data
    let empty = st.can_sample;
    let version = env!("CARGO_PKG_VERSION");
    let mut rows = vec![Row::new("Backup and export").icon("save").opens()];
    if empty {
        rows.push(Row::new("Try it with sample data").icon("sparkle").line("A labelled sample household; erasing removes it").opens());
    }
    rows.push(Row::new("About Tally").icon("notes").value(version).opens());
    let (h, tapped) = sk::group(cx, y, Some("Your data"), &rows);
    match tapped {
        Some(0) => *act = Some(Act::Go(Route::Backup)),
        Some(1) if empty => *act = Some(Act::Sample(None)),
        Some(_) => *act = Some(Act::Go(Route::About)),
        None => {}
    }
    y += h + GROUP_GAP;

    let erase_sub = format!("{} on this phone", logic::entries_text(c.entries));
    let (h, tapped) = sk::rows_card(cx, y, &[Row::new("Erase everything").glyph("trash", p.held).color(p.held_text).line(&erase_sub)]);
    if tapped.is_some() {
        *act = Some(Act::Erase(None));
    }
    y + h
}

fn appearance(cx: &mut Ctx, mut y: f32, mode: &str, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    y += 6.0;
    let chosen = mode_index(mode);
    let gap = 10.0;
    let tw = (W - gap * 2.0) / 3.0;
    let ph = 196.0;
    let name_t = Text::new(15.0, p.ink).semi().center();
    let hint_t = Text::new(12.5, p.ink2).center().line(1.35);
    let mut tallest: f32 = 0.0;
    for (i, (key, name)) in theme::MODES.iter().enumerate() {
        let x = PAD + i as f32 * (tw + gap);
        let on = i == chosen;
        match *key {
            "light" => preview(cx, x, y, tw, ph, &theme::LIGHT),
            "dark" => preview(cx, x, y, tw, ph, &theme::DARK),
            _ => {
                let dy = cx.c.dy;
                cx.c.push_clip(x, y + dy, tw / 2.0, ph);
                preview(cx, x, y, tw, ph, &theme::LIGHT);
                cx.c.pop_clip();
                cx.c.push_clip(x + tw / 2.0, y + dy, tw / 2.0, ph);
                preview(cx, x, y, tw, ph, &theme::DARK);
                cx.c.pop_clip();
            }
        }
        let edge = if on { p.signal } else { p.line_subtle };
        cx.c.outline(x - 3.0, y - 3.0, tw + 6.0, ph + 6.0, 21.0, if on { 2.0 } else { 1.0 }, edge);
        let mut ty0 = y + ph + 14.0;
        cx.c.text(name, x + tw / 2.0, ty0, name_t);
        ty0 += name_t.lh() + 2.0;
        let hh = cx.c.para(logic::palette_hint(key), x + 2.0, ty0, tw - 4.0, hint_t, 2);
        let total = ph + 14.0 + name_t.lh() + 2.0 + hh;
        tallest = tallest.max(total);
        if cx.tap(x - 4.0, y - 4.0, tw + 8.0, total + 8.0) && !on {
            *act = Some(Act::Mode(*key));
        }
    }
    y += tallest + 22.0;
    y += sk::note(cx, y, None, "Light and Dark keep the same cards, colours and readings. System changes with the phone, at night too.");
    y
}

/// A small Overview in palette `q`: its ground, the month's card, two cards side by side and a
/// couple of rows, `w` by `h`.
fn preview(cx: &mut Ctx, x: f32, y: f32, w: f32, h: f32, q: &Palette) {
    cx.c.rect(x, y, w, h, 18.0, q.ground[0]);
    let pad = 9.0;
    let (ix, iw) = (x + pad, w - pad * 2.0);
    let mut at = y + 14.0;
    cx.c.rect(ix, at, iw * 0.42, 6.0, 3.0, q.ink);
    at += 16.0;
    // The month: a dark card on both, as Overview's is.
    let hero = 54.0;
    cx.c.rect(ix, at, iw, hero, 10.0, theme::DARK.slab);
    if q.light {
        cx.c.outline(ix, at, iw, hero, 10.0, 1.0, q.line_subtle);
    } else {
        cx.c.outline(ix, at, iw, hero, 10.0, 1.0, theme::DARK.line_subtle.a(0.2));
    }
    cx.c.rect(ix + 8.0, at + 10.0, iw * 0.34, 4.0, 2.0, theme::DARK.ink2);
    cx.c.rect(ix + 8.0, at + 20.0, iw * 0.52, 9.0, 3.0, theme::DARK.ink);
    cx.c.rect(ix + 8.0, at + hero - 12.0, iw - 16.0, 3.0, 1.5, theme::DARK.track);
    cx.c.rect(ix + 8.0, at + hero - 12.0, (iw - 16.0) * 0.6, 3.0, 1.5, theme::DARK.signal);
    at += hero + 7.0;
    // This week and the next bill.
    let cw = (iw - 6.0) / 2.0;
    for k in 0..2 {
        let cx0 = ix + k as f32 * (cw + 6.0);
        cx.c.rect(cx0, at, cw, 40.0, 9.0, q.slab);
        cx.c.outline(cx0, at, cw, 40.0, 9.0, 1.0, q.line_subtle);
        cx.c.rect(cx0 + 6.0, at + 8.0, cw * 0.5, 3.5, 1.75, q.ink2);
        cx.c.rect(cx0 + 6.0, at + 17.0, cw * 0.62, 6.0, 3.0, q.ink);
        if k == 1 {
            cx.c.rect(cx0 + 6.0, at + 29.0, cw * 0.4, 3.5, 1.75, q.signal);
        }
    }
    at += 40.0 + 10.0;
    // Two entries.
    let hues = [crate::theme::CATEGORY_PAIRS[6], crate::theme::CATEGORY_PAIRS[0]];
    for (dk, lt) in hues {
        let hue = if q.light { lt } else { dk };
        if at + 20.0 > y + h - 8.0 {
            break;
        }
        cx.c.rect(ix, at, 18.0, 18.0, 6.0, hue.a(if q.light { 0.14 } else { 0.19 }));
        cx.c.circle(ix + 9.0, at + 9.0, 3.0, hue);
        cx.c.rect(ix + 24.0, at + 3.0, iw * 0.4, 4.0, 2.0, q.ink);
        cx.c.rect(ix + 24.0, at + 11.0, iw * 0.28, 3.0, 1.5, q.ink3);
        cx.c.rect(ix + iw - iw * 0.2, at + 3.0, iw * 0.2, 4.0, 2.0, q.ink);
        at += 26.0;
    }
}

fn format_page(cx: &mut Ctx, mut y: f32, st: &State, locale: &relay_money::money::Locale, act: &mut Option<Act>) -> f32 {
    let s = ledger_settings(st);
    if !st.period.is_empty() {
        y += sk::note(cx, y - 4.0, None, &format!("This budget month: {}", st.period)) + 16.0;
    }
    y += sk::eyebrow(cx, y, "Currency");
    let codes = logic::currency_codes(&s.currency);
    let options: Vec<logic::CurrencyOption> = codes.iter().map(|c| logic::currency_option(c, locale)).collect();
    let rh = widgets::row_h(true);
    let list_h = 8.0 + options.len() as f32 * rh;
    kit::card(cx, PAD, y, W, list_h);
    let mut ry = y + 4.0;
    for (i, o) in options.iter().enumerate() {
        if i > 0 {
            sk::divider(cx, ry, false);
        }
        let sub = format!("{} · {}", o.code, o.subtitle());
        let (h, tapped) = sk::pick_row(cx, PAD + 4.0, ry, W - 8.0, &o.name, Some(&sub), None, o.code == s.currency);
        if tapped && o.code != s.currency {
            *act = Some(Act::Currency(o.code.clone()));
        }
        ry += h;
    }
    y += list_h + 10.0;
    y += sk::note(cx, y, None, "Your server and PC take it on the next sync. A currency with other decimals, like the yen, can be chosen only while the ledger is empty.") + GROUP_GAP;

    y += sk::eyebrow(cx, y, "The month starts on");
    let days: Vec<String> = (1..=28).map(logic::ordinal).collect();
    let labels: Vec<&str> = days.iter().map(String::as_str).collect();
    let (picked, h) = widgets::pills(cx, PAD, y, W, &labels, Some((s.month_start_day - 1).max(0) as usize));
    if let Some(i) = picked
        && i as i64 + 1 != s.month_start_day
    {
        *act = Some(Act::MonthStart(i as i64 + 1));
    }
    y += h + 10.0;
    y += sk::note(cx, y, None, "For a budget month that follows your pay day.") + GROUP_GAP;

    y += sk::eyebrow(cx, y, "The week starts on");
    if let Some(i) = widgets::segmented(cx, PAD, y, W, &["Monday", "Sunday"], if s.week_starts_monday { 0 } else { 1 }) {
        *act = Some(Act::Week(i == 0));
    }
    y += 44.0 + 10.0;
    y += sk::note(cx, y, None, "Weeks in Tally count from it. Amounts are kept to the cent.");
    y
}

fn backup(cx: &mut Ctx, mut y: f32, st: &State, paired: bool, act: &mut Option<Act>) -> f32 {
    y += 4.0;
    let entries = logic::entries_text(st.counts.entries);
    let ch = 370.0;
    kit::pic_card(
        cx,
        PAD,
        y,
        W,
        ch,
        "receipts",
        &kit::PicCard { eyebrow: Some("On this phone"), title: &entries, line: Some("Keep it all as one file, or take the entries to a spreadsheet."), ..Default::default() },
    );
    y += ch + GROUP_GAP;
    let busy = st.busy();
    let rows = [
        Row::new("Save a backup file").icon("save").line("Everything, as Tally restores it. Share it to Drive, mail or Files.").opens(),
        Row::new("Entries as CSV").icon("upload").line("For a spreadsheet; Tally reads it back too").opens(),
    ];
    let (h, tapped) = sk::group(cx, y, Some("Save and export"), &rows);
    if busy.is_none() {
        match tapped {
            Some(0) => *act = Some(Act::ShareBackup),
            Some(_) => *act = Some(Act::ShareCsv),
            None => {}
        }
    }
    y += h + GAP;
    if let Some(words) = busy {
        y += sk::status(cx, y, sk::Tone::Working, words).0 + GAP;
    } else if let Some((words, failed)) = &st.result {
        y += sk::status(cx, y, if *failed { sk::Tone::Problem } else { sk::Tone::Good }, words).0 + GAP;
    }
    y += GROUP_GAP - GAP;
    y += sk::eyebrow(cx, y, "Restore");
    if sk::wide_key(cx, PAD, y, W, "Restore from a file", Some("download"), false, busy.is_none() && !paired) {
        *act = Some(Act::Restore);
    }
    y += 52.0 + 12.0;
    let note = if paired {
        "Disconnect from your server first: while this phone is connected, its next sync would put your server's ledger back. A restore replaces this phone's ledger, after Tally keeps a copy of it."
    } else {
        "A restore replaces this phone's ledger, after Tally keeps a copy of it. It stays on this phone: Tally RS never sends a ledger to replace your server's."
    };
    y += sk::note(cx, y, None, note);
    y
}

fn about(cx: &mut Ctx, mut y: f32) -> f32 {
    let p = cx.p;
    y += 18.0;
    let d = 96.0;
    sk::coin(cx, WIDTH / 2.0 - d / 2.0, y, d);
    y += d + 16.0;
    let brand = Text::new(32.0, p.ink).brand().center().line(1.15);
    cx.c.text("Tally", WIDTH / 2.0, y, brand);
    y += brand.lh() + 4.0;
    let vt = Text::new(13.0, p.ink3).center();
    cx.c.text(&format!("Version {}", env!("CARGO_PKG_VERSION")), WIDTH / 2.0, y, vt);
    y += vt.lh() + 14.0;
    let body = Text::new(15.0, p.ink2).center().line(1.45);
    let words = "Spending, bills and goals read as a pace: how much room is left and whether the month is on track. Kept in step with your own server.";
    y += cx.c.para(words, PAD + 20.0, y, W - 40.0, body, 6) + 30.0;
    let rows = [
        Row::new("Server").icon("cloud").line("The one this phone pairs with in Your server"),
        Row::new("Bank connections").icon("bank").line("Plaid, read only. The keys stay on your server."),
        Row::new("Fonts").icon("notes").line("Geist (Vercel) and Sora, under the SIL Open Font License"),
        Row::new("Data").icon("shield").line("On this phone, on your server and on your PC. Nothing else: no account, no analytics."),
    ];
    let (h, _) = sk::group(cx, y, Some("How it is made"), &rows);
    y + h + 8.0
}

/// Change the ledger's currency, refusing one with other decimals once anything is kept (every
/// amount is in minor units and nothing converts them).
pub fn change_currency(app: &mut App, code: &str) {
    let code = code.to_string();
    match {
        // Budgets, goals, bills and pot money are kept in minor units too, and relay-money's
        // own check counts only accounts and entries: with other decimals they would all read
        // 100 times off. Refuse here whenever the ledger holds anything at all.
        let fd = relay_money::money::fraction_digits;
        let current = app.store.ledger.settings().ok().and_then(|s| fd(&s.currency));
        if fd(&code).is_some() && fd(&code) != current && app.store.holds_anything() {
            Err(relay_money::ledger::LedgerError::Invalid("different decimals".into()))
        } else {
            app.store.ledger.set_currency(&code)
        }
    } {
        Ok(()) => app.invalidate(),
        Err(e) => {
            // relay-money's refusal names the retired Kotlin app, which converted amounts;
            // Tally RS does not convert, so say what it can do instead.
            let fd = relay_money::money::fraction_digits;
            let current = app.store.ledger.settings().ok().and_then(|s| fd(&s.currency));
            if fd(&code).is_some() && fd(&code) != current {
                app.say(
                    "That currency has a different number of decimals, so it can only be chosen while nothing is kept yet (budgets and goals count too). Pick one with the same decimals, or Erase everything first.",
                    None,
                )
            } else {
                app.say(e.to_string(), None)
            }
        }
    }
}
