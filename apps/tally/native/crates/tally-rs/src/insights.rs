//! Insights, as the Kotlin app's InsightsScreen.kt, in the premium look (DESIGN.md): where the
//! month's money went and how it moved, in three lenses. Month: the pace against last month first,
//! then one card of where it went (the donut and its categories together), the readings, where it
//! came from, the top payees and the largest entries. Trend: six months of bars and their averages.
//! Calendar: the days as a heat grid, a picked day's entries, and the weekdays.

use relay_money::Date;
use relay_money::copy;
use relay_money::money::MoneyFormatter;

use crate::activity_kit::{self as ak, W};
use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::insights_charts as charts;
use crate::insights_data::{self as data, Insights};
use crate::kit::{self, Ctx, PAD, ty};
use crate::model;
use crate::nav::Route;
use crate::theme;
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Lens {
    #[default]
    Month,
    Trend,
    Calendar,
}

const LENSES: [Lens; 3] = [Lens::Month, Lens::Trend, Lens::Calendar];
const LABELS: [&str; 3] = ["Month", "Trend", "Calendar"];
/// A card's inner margin.
const CP: f32 = 20.0;
const IW: f32 = W - CP * 2.0;

#[derive(Default)]
pub struct State {
    /// 0 is the current period, -1 the one before; never ahead of today.
    pub offset: i64,
    pub lens: Lens,
    /// The day read on the pace chart, and the day picked on the calendar.
    line_day: Option<usize>,
    cal_day: Option<usize>,
    cache: Option<((u64, Date, i64), Insights)>,
    problem: Option<String>,
    /// The reading that failed, so it is not tried again every frame.
    failed: Option<(u64, Date, i64)>,
}

enum Act {
    Go(Route),
    Offset(i64),
}

pub fn draw(app: &mut App, dt: f32) -> bool {
    let today = model::today();
    let st = &mut app.st.insights;
    let key = (app.ledger_gen, today, st.offset);
    if st.cache.as_ref().is_none_or(|(k, _)| *k != key) && st.failed != Some(key) {
        match data::read(&app.store, today, st.offset) {
            Ok(i) => {
                if st.cache.as_ref().is_some_and(|(k, _)| k.2 != key.2) {
                    st.line_day = None;
                    st.cal_day = None;
                }
                st.cache = Some((key, i));
                st.failed = None;
                st.problem = None;
            }
            Err(e) => {
                // Another month's reading is not this one's: the problem shows in its place.
                st.cache = None;
                st.failed = Some(key);
                st.problem = Some(e);
            }
        }
    }
    let fmt = app.store.formatter();
    let pg = app.page_begin(dt);
    let mut todo = None;
    let bottom;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.insights;
        let p = cx.p;
        let mut y = pg.top + 2.0;
        let at = LENSES.iter().position(|l| *l == st.lens).unwrap_or(0);
        if let Some(i) = widgets::segmented(&mut cx, PAD, y, W, &LABELS, at) {
            st.lens = LENSES[i];
        }
        y += 44.0 + 10.0;
        match &st.cache {
            None => {
                let words = st.problem.as_deref().unwrap_or("Reading the ledger");
                y += 10.0 + cx.c.para(words, PAD, y + 10.0, W, Text::new(ty::UI, p.ink2).line(1.4), 4) + 16.0;
                // A month that cannot be read is not where the page stays.
                if st.failed.is_some() && st.offset != 0 {
                    let t = Text::new(ty::BODY, p.ink).semi();
                    let kw = cx.c.measure("Back to this month", t) + 32.0;
                    if kit::key(&mut cx, PAD, y, kw, 44.0, "Back to this month", None, kit::KeyKind::Plain) {
                        todo = Some(Act::Offset(0));
                    }
                    y += 44.0 + 10.0;
                }
            }
            Some((_, s)) => {
                // The month switcher: back a month, the month, forward (never past this one).
                if ak::glyph_key(&mut cx, PAD - 12.0, y, "chevron-left", true) {
                    todo = Some(Act::Offset(st.offset - 1));
                }
                if ak::glyph_key(&mut cx, PAD + W - 32.0, y, "chevron-right", !s.current()) {
                    todo = Some(Act::Offset(st.offset + 1));
                }
                let mt = Text::new(15.0, p.ink).semi().center();
                cx.c.text_fit(&month_name(s), WIDTH / 2.0, y + (44.0 - mt.lh()) / 2.0, W - 100.0, mt);
                y += 44.0 + 10.0;
                y = match st.lens {
                    Lens::Month => month(&mut cx, y, s, &fmt, &mut st.line_day, &mut todo),
                    Lens::Trend => trend(&mut cx, y, s, &fmt, &mut todo),
                    Lens::Calendar => calendar(&mut cx, y, s, &fmt, &mut st.cal_day, &mut todo),
                };
            }
        }
        bottom = y;
    }
    app.page_end(&pg, bottom);
    match todo {
        Some(Act::Go(r)) => app.push(r),
        Some(Act::Offset(o)) => {
            if app.st.insights.offset != o.min(0) {
                app.st.insights.offset = o.min(0);
                *app.scroll() = Default::default();
            }
        }
        None => {}
    }
    pg.moving
}

/// "November 2026", or "Oct 15 – Nov 14" for a period that does not start on the 1st.
fn month_name(s: &Insights) -> String {
    if s.period.start.day() == 1 { s.period.start.strftime("%B %Y").to_string() } else { crate::activity_data::period_title(&s.period) }
}

fn percent(part: i64, whole: i64) -> i64 {
    if whole <= 0 { 0 } else { (part as f64 * 100.0 / whole as f64).round() as i64 }
}

/// A legend's mark and its words. Returns the width used.
fn legend(cx: &mut Ctx, x: f32, y: f32, colour: Rgba, words: &str) -> f32 {
    let t = Text::new(12.0, cx.p.ink3).medium();
    cx.c.rect(x, y + (t.lh() - 8.0) / 2.0, 8.0, 8.0, 2.0, colour);
    x + 14.0 + cx.c.text(words, x + 14.0, y, t)
}

/// The day a period's index falls on.
fn day_of(s: &Insights, i: usize) -> Date {
    s.period.start.checked_add(jiff::Span::new().days(i as i64)).unwrap_or(s.period.start)
}

// Month

fn month(cx: &mut Ctx, mut y: f32, s: &Insights, fmt: &MoneyFormatter, picked: &mut Option<usize>, todo: &mut Option<Act>) -> f32 {
    y = pace(cx, y, s, fmt, picked) + 14.0;

    if s.categories.is_empty() {
        y += widgets::empty(cx, PAD, y, W, "pie", "Nothing spent in this month", "What you log as spending shows here by category.");
    } else {
        y = where_it_went(cx, y, s, fmt, todo) + 14.0;
    }

    let projected = s.projected();
    let tiles: Vec<ak::Stat> = vec![
        ("A day", fmt.format_whole(s.per_day()), Some(format!("over {}", copy::plural(s.elapsed, "day"))), None),
        (
            "Projected",
            projected.map(|v| fmt.format_whole(v)).unwrap_or_else(|| "–".into()),
            Some(if projected.is_some() { "at this pace" } else { "once the month is under way" }.to_string()),
            None,
        ),
        ("Vs last month", fmt.format_whole(s.last_same_day), Some(copy::versus_last_line(s.spent, s.last_same_day, fmt)), None),
        ("Kept", s.kept().map(|k| format!("{k}%")).unwrap_or_else(|| "–".into()), Some(if s.income > 0 { "of what came in" } else { "nothing came in yet" }.to_string()), None),
    ];
    y = ak::stat_grid(cx, y, &tiles) + 30.0;

    // Where it came from.
    if !s.income_sources.is_empty() {
        y += kit::section(cx, y, "Where it came from", None).0 + 12.0;
        let rh = 56.0;
        let ch = 8.0 + s.income_sources.len() as f32 * rh + 8.0;
        kit::card(cx, PAD, y, W, ch);
        let mut ry = y + 8.0;
        for sl in &s.income_sources {
            let x = PAD + CP;
            if cx.pressed(PAD, ry, W, rh) && sl.folded == 0 {
                let wash = cx.p.wash;
                cx.c.rect(PAD + 6.0, ry, W - 12.0, rh, 14.0, wash);
            }
            kit::badge(cx, x, ry + 10.0, 36.0, &sl.icon, theme::category_color(sl.color));
            let amount = format!("+{}", fmt.format(sl.total));
            let at = Text::new(15.0, cx.p.live_text).medium().right();
            let aw = cx.c.measure(&amount, at);
            let t = Text::new(15.0, cx.p.ink).medium();
            cx.c.text(&amount, PAD + W - CP, ry + (rh - at.lh()) / 2.0, at);
            cx.c.text_fit(&sl.name, x + 48.0, ry + (rh - t.lh()) / 2.0, IW - 48.0 - aw - 10.0, t);
            if cx.tap(PAD, ry, W, rh)
                && sl.folded == 0
                && let Some(id) = sl.category_id
            {
                *todo = Some(Act::Go(Route::Transactions { category: Some(id), account: None }));
            }
            ry += rh;
        }
        y += ch + 30.0;
    }

    // Top payees.
    if !s.payees.is_empty() {
        y += kit::section(cx, y, "Top payees", None).0 + 12.0;
        let rh = 54.0;
        let ch = 8.0 + s.payees.len() as f32 * rh + 8.0;
        kit::card(cx, PAD, y, W, ch);
        let mut ry = y + 8.0;
        let p = cx.p;
        for (i, pay) in s.payees.iter().enumerate() {
            let x = PAD + CP;
            let amount = fmt.format(pay.total);
            let at = Text::new(15.0, p.ink).medium().right();
            let aw = cx.c.measure(&amount, at);
            cx.c.text(&amount, PAD + W - CP, ry + (rh - at.lh()) / 2.0, at);
            let nt = Text::new(13.0, p.ink3).semi().center();
            cx.c.rect(x, ry + 11.0, 32.0, 32.0, 10.0, p.card2);
            cx.c.text(&(i + 1).to_string(), x + 16.0, ry + 11.0 + (32.0 - nt.lh()) / 2.0, nt);
            cx.c.text_fit(&pay.note, x + 46.0, ry + 8.0, IW - 46.0 - aw - 10.0, Text::new(15.0, p.ink).medium());
            cx.c.text(&copy::plural(pay.count as i64, "time"), x + 46.0, ry + 29.0, Text::new(13.0, p.ink2));
            ry += rh;
        }
        y += ch + 30.0;
    }

    // Largest.
    if !s.largest.is_empty() {
        let (hh, _) = kit::section(cx, y, "Largest", None);
        let together = format!("{} together", fmt.format_whole(s.largest_total()));
        let tt = Text::new(13.0, cx.p.ink3).right();
        cx.c.text(&together, PAD + W, y + (hh - tt.lh()) / 2.0, tt);
        y += hh + 12.0;
        let (b, tapped) = ak::row_card(cx, y, &s.largest, fmt, |t| ak::short_day(&t.date, s.today));
        y = b;
        if let Some(id) = tapped {
            *todo = Some(Act::Go(Route::EntryView { id }));
        }
    }
    y
}

/// The pace card: this month's running spend against last month's, the headline reading the day
/// shown (today, or the day tapped). Returns its bottom.
fn pace(cx: &mut Ctx, y: f32, s: &Insights, fmt: &MoneyFormatter, picked: &mut Option<usize>) -> f32 {
    let p = cx.p;
    let last_name = crate::activity_data::period_title(&s.period.shift(-1));
    let this_name = crate::activity_data::period_title(&s.period);
    let days = if s.current() { (s.elapsed as usize).max(1) } else { s.period.days() as usize };
    let this = &s.cumulative[..s.cumulative.len().min(days)];
    let last = &s.last_cumulative[..s.last_cumulative.len().min(days)];
    let shown = picked.filter(|d| *d < this.len()).or(this.len().checked_sub(1));
    let last_spent = last.last().copied().unwrap_or(0) > 0;

    let chart_h = 150.0;
    let ht = Text::new(20.0, p.ink).semi();
    let lt = Text::new(12.0, p.ink3).medium();
    let h = CP + 18.0 + 4.0 + ht.lh() + 12.0 + lt.lh() + 14.0 + chart_h + CP;
    kit::card(cx, PAD, y, W, h);
    let x = PAD + CP;
    let mut ty_ = y + CP;
    ty_ += ak::card_title(cx, x, ty_, IW, &format!("Pace against {last_name}")) + 4.0;
    let headline = match shown {
        None => "Nothing spent yet".to_string(),
        Some(_) if !last_spent => format!("Nothing spent in {last_name} to pace against"),
        Some(d) => match data::ahead_by(this, last, d + 1) {
            Some(v) if v > 0 => format!("Ahead by {} on day {}", fmt.format_whole(v), d + 1),
            Some(v) if v < 0 => format!("Behind by {} on day {}", fmt.format_whole(-v), d + 1),
            Some(_) => format!("Level with {last_name} on day {}", d + 1),
            None => format!("{last_name} ended before day {}", d + 1),
        },
    };
    cx.c.text_fit(&headline, x, ty_, IW, ht);
    ty_ += ht.lh() + 12.0;

    // The legend, with what each line reads on the day shown, and the limit's pace when there is one.
    let at = |vals: &[i64]| shown.and_then(|d| vals.get(d.min(vals.len().saturating_sub(1)))).copied();
    let quiet = p.ink3.a(if p.light { 0.7 } else { 0.85 });
    let mut lx = x;
    let word = |name: &str, v: Option<i64>| match v {
        Some(v) => format!("{name} {}", fmt.format_whole(v)),
        None => name.to_string(),
    };
    lx = legend(cx, lx, ty_, quiet, &word(&last_name, at(last).filter(|_| last_spent))) + 16.0;
    lx = legend(cx, lx, ty_, p.signal, &word(&this_name, at(this))) + 16.0;
    if let (Some(b), Some(d)) = (s.budget, shown) {
        let pace = format!("Pace {}", fmt.format_whole(data::pace_at(b, d, s.period.days())));
        cx.c.text_fit(&pace, PAD + W - CP, ty_, (PAD + W - CP - lx).max(0.0), lt.right());
    }
    ty_ += lt.lh() + 14.0;

    let top = this.iter().chain(last.iter()).copied().max().unwrap_or(0);
    let ticks = data::ticks(top, relay_money::money::pow10(fmt.fraction_digits));
    if let Some(d) = charts::pace_lines(cx, x, ty_, IW, chart_h, this, last, days, &ticks, fmt, *picked) {
        *picked = if *picked == Some(d) { None } else { Some(d) };
    }
    y + h
}

/// One card: the donut with what was spent in its middle, then every category under it with its
/// share, amount and a thin meter. Returns its bottom.
fn where_it_went(cx: &mut Ctx, y: f32, s: &Insights, fmt: &MoneyFormatter, todo: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let size = 220.0;
    let thick = 26.0;
    let rh = 54.0;
    let title_h = Text::new(13.0, p.ink2).medium().lh();
    let h = CP + title_h + 18.0 + size + 22.0 + s.categories.len() as f32 * rh + CP - 6.0;
    if !ak::visible(cx, y, h) {
        return y + h;
    }
    kit::card(cx, PAD, y, W, h);
    let x = PAD + CP;
    ak::card_title(cx, x, y + CP, IW, "Where it went");
    let (mid, cy) = (WIDTH / 2.0, y + CP + title_h + 18.0 + size / 2.0);
    let parts: Vec<(Rgba, i64)> = s.categories.iter().map(|c| (theme::category_color(c.color), c.total)).collect();
    kit::donut(cx, mid, cy, size, thick, &parts);
    // The middle: what was spent, and how it stands against last month by the same day.
    let st = Text::new(13.0, p.ink2).medium().center();
    let fig = 30.0;
    let spent = fmt.format(s.spent);
    let mw = kit::money_w(cx.c, &spent, fig);
    let diff = s.last_same_day - s.spent;
    let last_name = crate::activity_data::period_title(&s.period.shift(-1));
    let versus = if s.last_same_day <= 0 || diff == 0 {
        None
    } else if diff > 0 {
        Some((format!("{} less than {last_name}", fmt.format_whole(diff)), p.live_text))
    } else {
        Some((format!("{} more than {last_name}", fmt.format_whole(-diff)), p.ink2))
    };
    let block = st.lh() + 2.0 + fig * 1.15 + if versus.is_some() { 4.0 + 16.0 } else { 0.0 };
    let mut ty_ = cy - block / 2.0;
    cx.c.text("Spent", mid, ty_, st);
    ty_ += st.lh() + 2.0;
    kit::money(cx, &spent, mid - mw / 2.0, ty_, fig, p.ink, p.ink2);
    ty_ += fig * 1.15 + 4.0;
    if let Some((words, colour)) = versus {
        cx.c.text_fit(&words, mid, ty_, size - thick * 2.0 - 12.0, Text::new(12.0, colour).medium().center());
    }

    // The categories, under it in the same card.
    let top_total = s.categories.iter().map(|c| c.total).max().unwrap_or(1).max(1);
    let mut ry = y + CP + title_h + 18.0 + size + 22.0;
    for sl in &s.categories {
        let colour = theme::category_color(sl.color);
        if cx.pressed(PAD, ry, W, rh) && sl.folded == 0 {
            cx.c.rect(PAD + 6.0, ry, W - 12.0, rh, 14.0, p.wash);
        }
        kit::badge(cx, x, ry + 9.0, 36.0, &sl.icon, colour);
        let tx = x + 48.0;
        let room = IW - 48.0;
        let amount = fmt.format(sl.total);
        let at = Text::new(15.0, p.ink).medium().right();
        let aw = cx.c.measure(&amount, at);
        cx.c.text(&amount, PAD + W - CP, ry + 9.0, at);
        let share = format!("{}%", percent(sl.total, s.spent));
        let sht = Text::new(13.0, p.ink3).medium().right();
        let sw = cx.c.measure(&share, sht);
        cx.c.text(&share, PAD + W - CP - aw - 10.0, ry + 9.0 + (at.lh() - sht.lh()) / 2.0, sht);
        cx.c.text_fit(&sl.name, tx, ry + 9.0, room - aw - sw - 20.0, Text::new(15.0, p.ink).medium());
        let f = sl.total as f32 / top_total as f32;
        let my = ry + 36.0;
        cx.c.rect(tx, my, room, 4.0, 2.0, p.track);
        cx.c.rect(tx, my, (room * f).max(4.0), 4.0, 2.0, colour);
        if cx.tap(PAD, ry, W, rh)
            && sl.folded == 0
            && let Some(id) = sl.category_id
        {
            *todo = Some(Act::Go(Route::Transactions { category: Some(id), account: None }));
        }
        ry += rh;
    }
    y + h
}

// Trend

fn trend(cx: &mut Ctx, mut y: f32, s: &Insights, fmt: &MoneyFormatter, todo: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let t = &s.trend_summary;
    let chart_h = 140.0;
    let lt = Text::new(12.0, p.ink3).medium();
    let ht = Text::new(20.0, p.ink).semi();
    let h = CP + 18.0 + 4.0 + ht.lh() + 12.0 + lt.lh() + 18.0 + chart_h + 30.0 + CP - 6.0;
    kit::card(cx, PAD, y, W, h);
    let x = PAD + CP;
    let mut ty_ = y + CP;
    ty_ += ak::card_title(cx, x, ty_, IW, "Six months") + 4.0;
    cx.c.text_fit(&format!("{} out a month", fmt.format_whole(t.average_spent)), x, ty_, IW, ht);
    ty_ += ht.lh() + 12.0;
    let lx = legend(cx, x, ty_, p.ink, "Out") + 16.0;
    let lx = legend(cx, lx, ty_, p.live, "In") + 16.0;
    cx.c.text_fit("Tap a month to read it", PAD + W - CP, ty_, PAD + W - CP - lx, lt.right());
    ty_ += lt.lh() + 18.0;
    let spent: Vec<i64> = s.trend.iter().map(|t| t.spent).collect();
    let income: Vec<i64> = s.trend.iter().map(|t| t.income).collect();
    let labels: Vec<String> = s.trend.iter().map(|t| t.period.start.strftime("%b").to_string()).collect();
    let highlight = s.trend.iter().position(|t| t.offset == s.offset).unwrap_or(0);
    if let Some(i) = charts::trend_bars(cx, x, ty_, IW, chart_h, &spent, &income, &labels, highlight)
        && let Some(pt) = s.trend.get(i)
    {
        *todo = Some(Act::Offset(pt.offset));
    }
    y += h + 14.0;

    let months = format!("average of {}", copy::plural(t.months as i64, "month"));
    let net = t.average_net();
    let tiles: Vec<ak::Stat> = vec![
        ("Spent a month", fmt.format_whole(t.average_spent), Some(months.clone()), None),
        ("Earned a month", fmt.format_whole(t.average_income), Some(months), Some(p.live_text)),
        ("Net a month", format!("{}{}", if net < 0 { "−" } else { "" }, fmt.format_whole(net.abs())), None, (net < 0).then_some(p.held_text)),
        ("Over budget", copy::plural(t.over_budget as i64, "month"), Some(format!("of the last {}", s.trend.len())), None),
    ];
    y = ak::stat_grid(cx, y, &tiles) + 30.0;

    y += kit::section(cx, y, "By month", None).0 + 12.0;
    let rh = 52.0;
    let ch = 8.0 + s.trend.len() as f32 * rh + 8.0;
    kit::card(cx, PAD, y, W, ch);
    let mut ry = y + 8.0;
    for pt in s.trend.iter().rev() {
        let on = pt.offset == s.offset;
        if on || cx.pressed(PAD, ry, W, rh) {
            cx.c.rect(PAD + 6.0, ry, W - 12.0, rh, 14.0, p.wash);
        }
        let name = format!("{}{}", crate::activity_data::period_title(&pt.period), if pt.running() { " (so far)" } else { "" });
        let n = pt.net();
        let net = format!("{}{}", if n >= 0 { "+" } else { "−" }, fmt.format_whole(n.abs()));
        let nt = Text::new(15.0, if n >= 0 { p.live_text } else { p.held_text }).medium().right();
        let nw = cx.c.measure(&net, nt);
        let mid = ry + (rh - nt.lh()) / 2.0;
        cx.c.text(&net, PAD + W - CP, mid, nt);
        let st = Text::new(13.0, p.ink2).right();
        let sw = cx.c.text(&format!("{} out", fmt.format_whole(pt.spent)), PAD + W - CP - nw - 14.0, mid + 1.0, st);
        let tt = Text::new(15.0, p.ink);
        cx.c.text_fit(&name, PAD + CP, mid, IW - nw - sw - 28.0, if on { tt.semi() } else { tt.medium() });
        if cx.tap(PAD, ry, W, rh) {
            *todo = Some(Act::Offset(pt.offset));
        }
        ry += rh;
    }
    y + ch
}

// Calendar

fn weekday_name(d: jiff::civil::Weekday) -> &'static str {
    use jiff::civil::Weekday::*;
    match d {
        Monday => "Monday",
        Tuesday => "Tuesday",
        Wednesday => "Wednesday",
        Thursday => "Thursday",
        Friday => "Friday",
        Saturday => "Saturday",
        Sunday => "Sunday",
    }
}

fn calendar(cx: &mut Ctx, mut y: f32, s: &Insights, fmt: &MoneyFormatter, picked: &mut Option<usize>, todo: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let cal = &s.calendar;
    let first = if s.week_starts_monday { jiff::civil::Weekday::Monday } else { jiff::civil::Weekday::Sunday };
    let lead = if s.week_starts_monday { s.period.start.weekday().to_monday_zero_offset() } else { s.period.start.weekday().to_sunday_zero_offset() } as usize;
    let mut weekdays = [""; 7];
    for (i, w) in weekdays.iter_mut().enumerate() {
        *w = &weekday_name(first.wrapping_add(i as i64))[..1];
    }
    let gh = charts::heat_grid_h(IW, s.heat.len(), lead);
    let ht = Text::new(20.0, p.ink).semi();
    let h = CP + 18.0 + 4.0 + ht.lh() + 16.0 + gh + CP;
    kit::card(cx, PAD, y, W, h);
    let x = PAD + CP;
    let mut ty_ = y + CP;
    ty_ += ak::card_title(cx, x, ty_, IW, "Days with spending") + 4.0;
    cx.c.text_fit(&format!("{} of {}", copy::plural(cal.active_days, "day"), cal.days_so_far), x, ty_, IW, ht);
    ty_ += ht.lh() + 16.0;
    let numbers: Vec<i8> = (0..s.heat.len()).map(|i| day_of(s, i).day()).collect();
    if let Some(d) = charts::heat_grid(cx, x, ty_, IW, &s.heat, lead, s.elapsed as usize, &weekdays, &numbers, *picked) {
        *picked = if *picked == Some(d) { None } else { Some(d) };
    }
    y += h + 30.0;

    if let Some(i) = *picked {
        let date = day_of(s, i);
        let rows = s.by_day.get(&date).map(Vec::as_slice).unwrap_or(&[]);
        let (hh, _) = kit::section(cx, y, &ak::day_name(date, s.today), None);
        let spent = format!("{} spent", fmt.format_whole(s.daily.get(i).copied().unwrap_or(0)));
        let tt = Text::new(13.0, p.ink3).right();
        cx.c.text(&spent, PAD + W, y + (hh - tt.lh()) / 2.0, tt);
        y += hh + 6.0;
        if rows.is_empty() {
            let words = match (s.today - date).get_days() {
                0 => "Nothing logged today".to_string(),
                1 => "Nothing logged yesterday".to_string(),
                _ => format!("Nothing logged on {}", model::day_word(&date.to_string(), s.today)),
            };
            cx.c.text(&words, PAD, y + 8.0, Text::new(ty::UI, p.ink3));
            y += 34.0;
        }
        if !rows.is_empty() {
            y += 6.0;
            let (b, tapped) = ak::row_card(cx, y, rows, fmt, |t| ak::when_in_day(t, &s.repeats).to_string());
            y = b;
            if let Some(id) = tapped {
                *todo = Some(Act::Go(Route::EntryView { id }));
            }
        }
        y += 30.0;
    }

    let busiest = cal.busiest.map(|(d, _)| d.strftime("%-d %b").to_string()).unwrap_or_else(|| "–".into());
    let tiles: Vec<ak::Stat> = vec![
        ("Days with spending", cal.active_days.to_string(), Some(format!("of {} so far", cal.days_so_far)), None),
        ("Longest quiet run", copy::plural(cal.longest_quiet_run, "day"), None, None),
        ("Busiest day", busiest, cal.busiest.map(|(_, v)| fmt.format_whole(v)), None),
        ("Weekends", fmt.format_whole(cal.weekend_total), cal.top_weekday.map(|d| format!("Most on {}s", weekday_name(d))), None),
    ];
    y = ak::stat_grid(cx, y, &tiles) + 30.0;

    if !cal.weekdays.is_empty() {
        let (hh, _) = kit::section(cx, y, "By weekday", None);
        let tt = Text::new(13.0, p.ink3).right();
        cx.c.text("What a day of each averages", PAD + W, y + (hh - tt.lh()) / 2.0, tt);
        y += hh + 12.0;
        let rh = 34.0;
        let ch = 14.0 + cal.weekdays.len() as f32 * rh + 14.0;
        kit::card(cx, PAD, y, W, ch);
        let mut ry = y + 14.0;
        let top_avg = cal.weekdays.iter().map(|w| w.average()).max().unwrap_or(0).max(1) as f32;
        for w in &cal.weekdays {
            let t = Text::new(13.0, p.ink2).medium();
            cx.c.text(&weekday_name(w.day)[..3], PAD + CP, ry + (rh - t.lh()) / 2.0, t);
            let amount = fmt.format_whole(w.average());
            let at = Text::new(13.0, p.ink).medium().right();
            cx.c.text(&amount, PAD + W - CP, ry + (rh - at.lh()) / 2.0, at);
            let (mx, mw) = (PAD + CP + 44.0, IW - 44.0 - 70.0);
            let top = Some(w.day) == cal.top_weekday;
            cx.c.rect(mx, ry + rh / 2.0 - 4.0, mw, 8.0, 4.0, p.track);
            if w.average() > 0 {
                cx.c.rect(mx, ry + rh / 2.0 - 4.0, (mw * w.average() as f32 / top_avg).max(8.0), 8.0, 4.0, if top { p.ink } else { p.ink.a(0.35) });
            }
            ry += rh;
        }
        y += ch;
    }
    y
}
