//! Overview (DESIGN.md, "Premium, dark and light"): the month on a dark card with the wallet
//! picture, this week and the next bill side by side, recent entries, the budgets under the most
//! pressure, and the goals last, as picture cards that swipe sideways. Adding is the tab bar's +.

use relay_money::copy;
use relay_money::model::TxType;
use relay_money::pace::PaceStatus;

use crate::app::App;
use crate::cx;
use crate::draw::{Rgba, Text, WIDTH};
use crate::kit::{self, CW, Ctx, KeyKind, PAD, PicCard, ty};
use crate::model::{self, Home};
use crate::nav::Route;
use crate::theme;
use crate::ui::HScroll;

#[derive(Default)]
pub struct State {
    goals: HScroll,
}

/// What a tap on Overview asks for, done once drawing is over.
enum Act {
    Go(Route),
    Sample,
}

/// Draw Overview. Returns true while it is still moving (scrolling).
pub fn draw(app: &mut App, dt: f32) -> bool {
    let pg = app.page_begin(dt);
    let p = app.palette;
    let mut act = None;
    let bottom;
    let mut moving = pg.moving;
    {
        let mut cx = cx!(app);
        let st = &mut app.st.overview;
        bottom = match &app.home {
            None => {
                let words = app.problem.as_deref().unwrap_or("Opening the ledger");
                cx.c.text(words, PAD, pg.top, Text::new(ty::UI, p.ink2));
                pg.top + 40.0
            }
            Some(h) => content(&mut cx, h, st, pg.top + 8.0, dt, &mut act, &mut moving),
        };
    }
    app.page_end(&pg, bottom);
    match act {
        // "See all" under Recent opens Entries over Overview, with Back.
        Some(Act::Go(r)) => app.push_over(r),
        Some(Act::Sample) => {
            match app.store.load_sample(model::today()) {
                Ok(n) => app.say(format!("Loaded the sample ledger: {n} rows"), None),
                Err(e) => app.say(e, None),
            }
            app.invalidate();
        }
        None => {}
    }
    moving
}

fn content(cx: &mut Ctx, h: &Home, st: &mut State, top: f32, dt: f32, act: &mut Option<Act>, moving: &mut bool) -> f32 {
    let s = &h.s;
    let mut y = top;

    if !h.has_entries() && s.accounts.is_empty() {
        let ch = 420.0;
        let (_, _) = kit::pic_card(
            cx,
            PAD,
            y,
            CW,
            ch,
            "wallet",
            &PicCard {
                eyebrow: Some("Nothing logged yet"),
                title: "Every dollar, where you put it",
                line: Some("Log what you spend with +, or connect your server in Settings. Or look around with the sample ledger."),
                ..Default::default()
            },
        );
        y += ch + 16.0;
        let kw = (CW - 10.0) / 2.0;
        if kit::key(cx, PAD, y, kw, 52.0, "Sample ledger", None, KeyKind::Plain) {
            *act = Some(Act::Sample);
        }
        if kit::key(cx, PAD + kw + 10.0, y, kw, 52.0, "Log an entry", Some("plus"), KeyKind::Primary) {
            *act = Some(Act::Go(Route::Entry { id: None, kind: None, account: None, category: None }));
        }
        return y + 52.0;
    }

    y = hero(cx, h, y, act) + 28.0;
    y = week_and_bill(cx, h, y, act) + 30.0;
    y = recent(cx, h, y, act) + 26.0;
    if !s.budgets.is_empty() {
        y = budgets(cx, h, y, act) + 30.0;
    }
    if !s.goals.is_empty() {
        y = goals(cx, h, st, y, dt, act, moving);
    }
    y
}

/// The month on a dark card that stays dark on Light, like a metal card in a light wallet: what
/// is left, of what, how many days, and the pace on a thin brass line.
fn hero(cx: &mut Ctx, h: &Home, y: f32, act: &mut Option<Act>) -> f32 {
    let fmt = &h.fmt;
    let s = &h.s;
    let ch = 236.0;
    let (x, w) = (PAD, CW);
    cx.c.rect_grad(x, y, w, ch, kit::PIC_R, Rgba::hex(0x090808), Rgba::hex(0x2C2B2C));
    // The still life sits on the right; the words on the left.
    if crate::pictures::ensure(cx.c, "hero") {
        // At its own shape the picture is wider than the card: keep its left part, from 56 in,
        // and carry its own left edge across the first 56 so there is no seam.
        cx.c.image_part("hero", x, y, 56.0 + kit::PIC_R + 2.0, ch, kit::PIC_R, Rgba::WHITE, [0.0, 0.0, 0.004, 1.0]);
        // Its rounded left corners sit over the carried edge, the same colour: no seam shows.
        cx.c.image_cover_at("hero", x + 56.0, y, w - 56.0, ch, kit::PIC_R, Rgba::WHITE, (0.0, 0.5));
    }
    if !cx.p.light {
        cx.c.outline(x, y, w, ch, kit::PIC_R, 1.0, Rgba::WHITE.a(0.1));
    }
    let (ink, ink2) = (Rgba::hex(0xF7F4EE), Rgba::hex(0xBDB7AE));
    let lx = x + 22.0;
    let period = model::period_name(&s.period.start, &s.period.end_exclusive);
    match h.reading() {
        Some(r) => {
            let reached = model::limit_reached(r);
            let eyebrow = if reached { "LIMIT REACHED".to_string() } else { format!("LEFT TO SPEND IN {}", period.to_uppercase()) };
            cx.c.text(&eyebrow, lx, y + 22.0, Text::new(12.0, ink2).semi());
            let amount = fmt.format(if reached { r.spent } else { r.remaining });
            kit::money(cx, &amount, lx, y + 42.0, 44.0, if reached { Rgba::hex(0xF2A090) } else { ink }, Rgba::hex(0x9D978E));
            let sub = format!("of {}  ·  {}", fmt.format_whole(r.budget), copy::plural(r.days_left, "day") + " left");
            cx.c.text(&sub, lx, y + 98.0, Text::new(14.0, ink2));
            let my = y + ch - 22.0 - 17.0 - 12.0 - 5.0;
            let mw = w - 44.0;
            cx.c.rect(lx, my, mw, 5.0, 2.5, Rgba::WHITE.a(0.14));
            let f = r.spent_fraction.clamp(0.0, 1.0);
            if f > 0.0 {
                cx.c.rect(lx, my, (mw * f).max(5.0), 5.0, 2.5, if reached { Rgba::hex(0xF2A090) } else { Rgba::hex(0xD8B67C) });
            }
            cx.c.rect(lx + mw * r.pace_fraction.clamp(0.0, 1.0) - 1.0, my - 3.0, 2.0, 11.0, 1.0, ink2);
            let (words, _) = model::hero_line(r, fmt);
            cx.c.text_fit(&words, lx, my + 5.0 + 12.0, mw, Text::new(13.0, Rgba::hex(0xE8E3DA)).medium());
        }
        None => {
            cx.c.text(&format!("SPENT IN {}", period.to_uppercase()), lx, y + 22.0, Text::new(12.0, ink2).semi());
            kit::money(cx, &fmt.format(s.spent), lx, y + 42.0, 44.0, ink, Rgba::hex(0x9D978E));
            cx.c.para(
                "Set a monthly limit and Tally reads the month as a pace: what is left and what each day can take.",
                lx,
                y + ch - 22.0 - 40.0,
                w - 150.0,
                Text::new(13.0, ink2).line(1.4),
                2,
            );
        }
    }
    if cx.tap(x, y, w, ch) {
        *act = Some(Act::Go(Route::Budgets));
    }
    y + ch
}

/// This week as seven bars beside the next bill (or what each day can take, with no bills).
fn week_and_bill(cx: &mut Ctx, h: &Home, y: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &h.fmt;
    let ch = 164.0;
    let cw = (CW - 12.0) / 2.0;
    // This week.
    let x = PAD;
    if kit::card_button(cx, x, y, cw, ch) {
        *act = Some(Act::Go(Route::Entries));
    }
    cx.c.text("This week", x + 18.0, y + 18.0, Text::new(14.0, p.ink2).medium());
    kit::money(cx, &fmt.format(h.week_spent()), x + 18.0, y + 40.0, 26.0, p.ink, p.ink2);
    // Today's spending sits beside the title, so the chart gets the card's lower half.
    let today_words = format!("{} today", fmt.format_whole(h.spent_today));
    cx.c.text(&today_words, x + cw - 18.0, y + 19.0, Text::new(13.0, p.ink3).right());
    let (bx, bw, base, chart) = (x + 18.0, cw - 36.0, y + ch - 32.0, 52.0);
    let max = h.week.iter().copied().max().unwrap_or(0).max(1) as f32;
    let slot = bw / 7.0;
    let bar_w = (slot * 0.62).min(16.0);
    let (bar, today) = (theme::category_color(Some(2)), theme::category_color(Some(2)));
    for (i, v) in h.week.iter().enumerate() {
        let cxm = bx + slot * i as f32 + slot / 2.0;
        let is_today = i == h.today_in_week;
        if *v > 0 {
            let bh = (chart * *v as f32 / max).max(4.0);
            cx.c.rect(cxm - bar_w / 2.0, base - bh, bar_w, bh, 4.0, if is_today { today } else { bar.a(0.42) });
        } else {
            cx.c.rect(cxm - bar_w / 2.0, base - 3.0, bar_w, 3.0, 1.5, p.track);
        }
        let d = h.week_start.checked_add(jiff::Span::new().days(i as i64)).unwrap_or(h.week_start);
        let t = Text::new(10.5, if is_today { p.ink } else { p.ink3 }).center();
        cx.c.text(model::weekday_initial(d), cxm, base + 6.0, if is_today { t.semi() } else { t });
    }
    // The next bill.
    let x = PAD + cw + 12.0;
    let next = h.s.bills.iter().filter(|b| b.days_until >= 0).min_by_key(|b| b.days_until);
    match next {
        Some(b) => {
            if kit::card_button(cx, x, y, cw, ch) {
                *act = Some(Act::Go(Route::Bills));
            }
            cx.c.text("Next bill", x + 18.0, y + 18.0, Text::new(14.0, p.ink2).medium());
            cx.c.text_fit(&b.name, x + 18.0, y + 40.0, cw - 36.0, Text::new(17.0, p.ink).medium());
            let amount = match b.r#type {
                TxType::Income => format!("+{}", fmt.format(b.amount)),
                _ => fmt.format(b.amount),
            };
            kit::money(cx, &amount, x + 18.0, y + 64.0, 26.0, if b.r#type == TxType::Income { p.live_text } else { p.ink }, p.ink2);
            let when = match b.days_until {
                0 => "today".to_string(),
                1 => "tomorrow".to_string(),
                n => format!("in {n} days"),
            };
            cx.c.text(&when, x + 18.0, y + ch - 18.0 - 23.0, Text::new(17.0, p.signal).medium());
        }
        None => {
            if kit::card_button(cx, x, y, cw, ch) {
                *act = Some(Act::Go(Route::Budgets));
            }
            cx.c.text("Each day can take", x + 18.0, y + 18.0, Text::new(14.0, p.ink2).medium());
            let (each, sub) = match h.reading() {
                Some(r) if !model::limit_reached(r) => (fmt.format_whole(r.daily_allowance), format!("for {}", copy::plural(r.days_left, "day"))),
                Some(_) => ("Nothing".to_string(), "until the month resets".to_string()),
                None => ("No limit".to_string(), "set one in Budgets".to_string()),
            };
            cx.c.text_fit(&each, x + 18.0, y + 42.0, cw - 36.0, Text::new(26.0, p.ink).medium());
            cx.c.text(&sub, x + 18.0, y + ch - 18.0 - 23.0, Text::new(17.0, p.signal).medium());
        }
    }
    y + ch
}

/// The latest few entries.
fn recent(cx: &mut Ctx, h: &Home, y: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &h.fmt;
    let (hh, all) = kit::section(cx, y, "Recent", Some("See all"));
    if all {
        *act = Some(Act::Go(Route::Entries));
    }
    let mut y = y + hh + 12.0;
    if h.s.recent.is_empty() {
        cx.c.text("Nothing logged this month yet. The + below adds one.", PAD, y + 8.0, Text::new(ty::UI, p.ink3));
        return y + 34.0;
    }
    // The rows sit in a card, as every list in the app does.
    let n = h.s.recent.len().min(4);
    kit::card(cx, PAD, y, CW, n as f32 * kit::ENTRY_H + 8.0);
    y += 4.0;
    let (rx, rw) = (PAD + 14.0, CW - 28.0);
    for (i, t) in h.s.recent.iter().take(4).enumerate() {
        if i > 0 {
            cx.c.rect(rx + 58.0, y, rw - 58.0, 1.0, 0.0, p.line_subtle);
        }
        let (icon, colour) = if t.r#type == TxType::Transfer {
            ("transfer".to_string(), theme::category_color(Some(2)))
        } else {
            (t.icon.clone().unwrap_or_else(|| "dots".into()), theme::category_color(t.color))
        };
        let (amount, ac) = match t.r#type {
            TxType::Income => (format!("+{}", fmt.format(t.amount)), p.live_text),
            TxType::Transfer => (fmt.format(t.amount), p.ink2),
            TxType::Expense => (format!("−{}", fmt.format(t.amount)), p.ink),
        };
        let title = if !t.note.is_empty() {
            t.note.clone()
        } else if t.r#type == TxType::Transfer {
            format!("To {}", t.to_account.as_deref().unwrap_or("another account"))
        } else {
            t.category.clone().unwrap_or_else(|| "Uncategorised".into())
        };
        let sub = if t.r#type == TxType::Transfer {
            t.account.clone()
        } else {
            format!("{} · {}", t.category.clone().unwrap_or_else(|| "Uncategorised".into()), t.account)
        };
        let when = model::day_word(&t.date, h.today);
        if kit::entry_row(cx, rx, y, rw, &icon, colour, &title, &sub, &amount, ac, &when) {
            *act = Some(Act::Go(Route::EntryView { id: t.id }));
        }
        y += kit::ENTRY_H;
    }
    y + 4.0
}

/// The budgets under the most pressure: over first, then the closest to their limit.
fn budgets(cx: &mut Ctx, h: &Home, y: f32, act: &mut Option<Act>) -> f32 {
    let p = cx.p;
    let fmt = &h.fmt;
    let s = &h.s;
    let all = format!("All {}", s.budgets.len());
    let (hh, tapped) = kit::section(cx, y, "Budgets", Some(&all));
    if tapped {
        *act = Some(Act::Go(Route::Budgets));
    }
    let mut y = y + hh + 12.0;
    let mut order: Vec<_> = s.budgets.iter().filter(|e| e.budget > 0).collect();
    order.sort_by(|a, b| {
        let fa = a.spent as f64 / a.budget as f64;
        let fb = b.spent as f64 / b.budget as f64;
        fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
    });
    let elapsed = s.pace.elapsed_days;
    let k = order.len().min(3);
    let rh = 46.0;
    // In a card, like Recent above it.
    kit::card(cx, PAD, y, CW, k as f32 * (rh + 18.0) - 18.0 + 32.0);
    y += 16.0;
    let (bx, bw) = (PAD + 16.0, CW - 32.0);
    for e in order.into_iter().take(3) {
        let colour = theme::category_color(Some(e.color));
        if cx.pressed(bx - 8.0, y - 6.0, bw + 16.0, rh + 12.0) {
            cx.c.rect(bx - 8.0, y - 6.0, bw + 16.0, rh + 12.0, 14.0, p.wash);
        }
        kit::badge(cx, bx, y + 3.0, 40.0, &e.icon, colour);
        let tx = bx + 54.0;
        let room = bw - 54.0;
        let over = e.spent > e.budget;
        let note = if over { format!("{} over", fmt.format_whole(e.spent - e.budget)) } else { format!("{} left", fmt.format_whole(e.budget - e.spent)) };
        let nt = Text::new(14.0, if over { p.held_text } else { p.ink2 }).medium().right();
        let nw = cx.c.measure(&note, nt);
        cx.c.text(&note, bx + bw, y + 6.0, nt);
        cx.c.text_fit(&e.name, tx, y + 5.0, room - nw - 10.0, Text::new(15.0, p.ink).medium());
        let r = relay_money::pace::PaceReading::new(e.budget, e.spent, s.period.days.max(1), elapsed);
        let fill = if over {
            p.held
        } else if r.status == PaceStatus::OverPace {
            p.waiting
        } else {
            colour
        };
        kit::meter(cx, tx, y + 32.0, room, r.spent_fraction, Some(r.pace_fraction), fill);
        if cx.tap(bx - 8.0, y - 6.0, bw + 16.0, rh + 12.0) {
            *act = Some(Act::Go(Route::BudgetEdit { category: Some(e.category_id) }));
        }
        y += rh + 18.0;
    }
    y - 18.0 + 16.0
}

/// The goals, last: picture cards that swipe sideways, the next one peeking in.
#[allow(clippy::too_many_arguments)]
fn goals(cx: &mut Ctx, h: &Home, st: &mut State, y: f32, dt: f32, act: &mut Option<Act>, moving: &mut bool) -> f32 {
    let fmt = &h.fmt;
    let (hh, all) = kit::section(cx, y, "Goals", Some("See all"));
    if all {
        *act = Some(Act::Go(Route::Goals));
    }
    let y = y + hh + 14.0;
    let (cw, ch, gap) = (300.0, 380.0, 12.0);
    let n = h.s.goals.len();
    let content = n as f32 * cw + (n.saturating_sub(1)) as f32 * gap;
    let (x0, m) = kit::carousel(cx, &mut st.goals, y, ch, content, dt);
    *moving |= m;
    for (i, g) in h.s.goals.iter().enumerate() {
        let x = x0 + i as f32 * (cw + gap);
        if x > WIDTH || x + cw < 0.0 {
            continue;
        }
        let pic = crate::pictures::for_goal(&g.name, g.kind == relay_money::model::GoalKind::Invest);
        let line = format!("{} of {}", fmt.format_whole(g.saved), fmt.format_whole(g.target));
        let f = if g.target > 0 { g.saved as f32 / g.target as f32 } else { 0.0 };
        let (card, button) = kit::pic_card(
            cx,
            x,
            y,
            cw,
            ch,
            pic,
            &PicCard { title: &g.name, line: Some(&line), meter: Some(f), button: Some("Add money"), ..Default::default() },
        );
        if card || button {
            *act = Some(Act::Go(Route::Goal { id: g.id }));
        }
    }
    kit::carousel_end(cx);
    y + ch
}
