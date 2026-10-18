//! `arbiter.*` — Arbiter (docs/ARBITER.md). The desk (`crate::arbiter`) holds the book, the
//! exchange and the order path; these handlers only read and route.
//!
//! Nothing here holds the store's lock across a network call: reads of the book and the exchange
//! are `register_unlocked`, and every op that talks to the exchange or the keyring is staged, its
//! slow work in `prepare`. The book is its own file, so a staged op's book writes in `prepare`
//! are committed whether or not the store transaction around `finish` commits — as Tally's
//! ledger writes are.

use crate::arbiter::{self as desk, with_book, Outcome};
use crate::engine::Engine;
use relay_arbiter::backtest::{self, Config};
use relay_arbiter::book::{self, Position, StrategyRec};
use relay_arbiter::gate::Intent;
use relay_arbiter::model::*;
use relay_arbiter::views::*;
use relay_arbiter::{rules, Decimal};
use relay_bus::ops::arbiter::*;
use relay_bus::{BusError, Empty};
use rust_decimal::prelude::ToPrimitive;
use serde_json::json;
use std::str::FromStr;

/// The products shown beside the strategies when none trade them yet.
const WATCH: [&str; 3] = ["BTC", "ETH", "SOL"];

fn amount(s: &str, what: &str) -> Result<Decimal, BusError> {
    Decimal::from_str(s.trim()).ok().filter(|d| *d > Decimal::ZERO).ok_or_else(|| BusError::invalid("arbiter.invalid", format!("{what} must be a positive number, like 50 or 0.25")))
}

fn row(engine: &Engine, s: &StrategyRec, taker: Decimal) -> Result<StrategyRow, BusError> {
    let (pos, open) = with_book(engine, |b| Ok((b.position(s.id, s.venue)?, b.open_orders(Some(s.id))?.len())))?;
    let price = desk::last_price(engine, &s.product);
    let open_pnl = pos.open_pnl(price, taker);
    Ok(StrategyRow {
        id: s.id,
        name: s.name.clone(),
        product: s.product.clone(),
        granularity: s.granularity,
        mode: s.mode,
        venue: s.venue,
        state: s.state,
        halt_reason: s.halt_reason.clone(),
        version: s.version,
        sentences: rules::describe(&s.rule, &s.product, s.granularity),
        doing: book::doing(s, &pos, open),
        held_base: pos.base.normalize(),
        held_cost: pos.cost.round_dp(2),
        pnl_realized: pos.realized.round_dp(2),
        pnl_open: open_pnl.round_dp(2),
        pnl_today: (pos.realized_today + open_pnl).round_dp(2),
        trades: pos.sells,
        last_price: price,
        currency: s.product.split_once('-').map(|(_, q)| q.to_string()).unwrap_or_default(),
    })
}

fn record(p: &Position, price: Option<f64>, taker: Decimal) -> Record {
    let pnl = p.realized + p.open_pnl(price, taker);
    Record {
        days: p.first_at.map_or(0.0, |f| (book::now() - f) as f64 / 86400.0),
        trades: p.sells,
        won_pct: (p.sells > 0).then(|| f64::from(p.won) / f64::from(p.sells) * 100.0),
        pnl: pnl.round_dp(2),
        fees: p.fees.round_dp(2),
        return_pct: (p.max_cost > Decimal::ZERO).then(|| (pnl / p.max_cost * Decimal::ONE_HUNDRED).to_f64().unwrap_or(0.0)),
    }
}

pub fn detail(engine: &Engine, id: i64) -> Result<StrategyDetail, BusError> {
    let s = with_book(engine, |b| b.strategy(id))?;
    let conn = with_book(engine, |b| b.connection())?;
    let global = with_book(engine, |b| b.global_limits())?;
    let price = desk::last_price(engine, &s.product);
    let (paper, live, decisions, orders, fills) = with_book(engine, |b| {
        Ok((b.position(id, Venue::Paper)?, b.position(id, Venue::Live)?, b.decisions(Some(id), 50)?, b.orders(Some(id), 50)?, b.fills(Some(id), 50)?))
    })?;
    Ok(StrategyDetail {
        row: row(engine, &s, conn.fees.taker)?,
        break_even_pct: backtest::break_even_pct(&conn.fees, s.rule.pricing, global.slippage_pct),
        rule: s.rule.clone(),
        limits: s.limits.clone(),
        rule_hash: s.rule_hash.clone(),
        variants_tried: s.variants_tried,
        backtest: s.backtest.clone(),
        paper: record(&paper, price, conn.fees.taker),
        live: record(&live, price, conn.fees.taker),
        decisions,
        orders,
        fills,
    })
}

fn summary(engine: &Engine) -> Result<Summary, BusError> {
    let now = book::now();
    let month = jiff::Timestamp::from_second(now).ok().map(|t| t.to_zoned(jiff::tz::TimeZone::UTC).date().first_of_month())
        .and_then(|d| d.to_zoned(jiff::tz::TimeZone::UTC).ok()).map_or(0, |z| z.timestamp().as_second());
    let (conn, home, strategies, halted, pending, open, fills, global, (runner_at, runner_error)) = with_book(engine, |b| {
        Ok((b.connection()?, b.home()?, b.strategies()?, b.halted()?, b.proposals(true, 50)?, b.open_orders(None)?, b.fills(None, 10)?, b.global_limits()?, b.runner_status()?))
    })?;
    let taker = conn.fees.taker;
    let rows = strategies.iter().map(|s| row(engine, s, taker)).collect::<Result<Vec<_>, _>>()?;
    let account = |venue: Venue| -> Result<AccountView, BusError> {
        let mine: Vec<&StrategyRow> = rows.iter().filter(|r| r.venue == venue).collect();
        let (pnl_today, pnl_total) = with_book(engine, |b| {
            let mut today = Decimal::ZERO;
            let mut total = Decimal::ZERO;
            for s in &strategies {
                let p = b.position(s.id, venue)?;
                let open = p.open_pnl(desk::last_price(engine, &s.product), taker);
                total += p.realized + open;
                if s.venue == venue {
                    today += p.realized_today + open;
                }
            }
            Ok((today, total))
        })?;
        let _ = mine;
        let fees_month = with_book(engine, |b| b.fees_since(venue, month))?;
        let equity = with_book(engine, |b| b.equity(venue, now - 30 * 86400))?;
        Ok(match venue {
            Venue::Paper => {
                let cash = with_book(engine, |b| b.paper_cash_now())?;
                AccountView { venue, value: desk::paper_value(engine)?.round_dp(2), cash: cash.round_dp(2), pnl_today: pnl_today.round_dp(2), pnl_total: pnl_total.round_dp(2), fees_month: fees_month.round_dp(2), balances: Vec::new(), equity }
            }
            Venue::Live => {
                let balances = with_book(engine, |b| b.balances())?;
                let value: Decimal = balances.iter().filter_map(|b| b.value).sum();
                let cash = balances.iter().filter(|b| b.currency == home).map(|b| b.available + b.hold).sum::<Decimal>();
                AccountView { venue, value: value.round_dp(2), cash: cash.round_dp(2), pnl_today: pnl_today.round_dp(2), pnl_total: pnl_total.round_dp(2), fees_month: fees_month.round_dp(2), balances, equity }
            }
        })
    };
    let paper = account(Venue::Paper)?;
    let live = if conn.state == "none" || conn.state.is_empty() { None } else { Some(account(Venue::Live)?) };

    // Today's use of the Arbiter-wide limits, for the meters.
    let held: f64 = rows.iter().map(|r| r.held_cost.to_f64().unwrap_or(0.0)).sum();
    let loss_today: f64 = rows.iter().map(|r| r.pnl_today.to_f64().unwrap_or(0.0)).filter(|p| *p < 0.0).map(|p| -p).sum();
    let orders_hour = with_book(engine, |b| Ok(b.orders_since(None, Venue::Paper, now - 3600)? + b.orders_since(None, Venue::Live, now - 3600)?))?;
    let limits = vec![
        LimitUse { label: "Daily loss".into(), used: loss_today, limit: global.daily_loss.and_then(|d| d.to_f64()), unit: "money".into() },
        LimitUse { label: "In the market".into(), used: held, limit: global.max_exposure.and_then(|d| d.to_f64()), unit: "money".into() },
        LimitUse { label: "Orders this hour".into(), used: f64::from(orders_hour), limit: global.orders_per_hour.map(f64::from), unit: "count".into() },
    ];

    let mut watched: Vec<String> = strategies.iter().map(|s| s.product.clone()).collect();
    for base in WATCH {
        watched.push(format!("{base}-{home}"));
    }
    watched.dedup();
    let mut seen = std::collections::HashSet::new();
    let prices = watched
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .filter_map(|p| {
            let cached = with_book(engine, |b| b.cached_product(&p)).ok().flatten();
            let price = desk::last_price(engine, &p).or(cached.as_ref().and_then(|c| c.price))?;
            Some(PriceRow { product: p, price, change_24h: cached.and_then(|c| c.change_24h) })
        })
        .take(8)
        .collect();
    Ok(Summary {
        connection: if conn.state.is_empty() { Connection { state: "none".into(), exchange: "Coinbase".into(), ..conn } } else { conn },
        home,
        live,
        paper,
        strategies: rows,
        limits,
        pending,
        open_orders: open,
        fills,
        prices,
        halted: halted.is_some(),
        halt_reason: halted,
        checked_at: runner_at.map(book::rfc3339),
        runner_error,
    })
}

fn settings_out(engine: &Engine) -> Result<SettingsOut, BusError> {
    with_book(engine, |b| Ok(SettingsOut { home: b.home()?, paper_cash: b.paper_cash()?.normalize().to_string(), global: b.global_limits()? }))
}

/// Checks a draft's product against the exchange's list: an invented symbol never becomes a
/// strategy.
fn check_product(engine: &Engine, draft: &Draft) -> Result<(), BusError> {
    match desk::product(engine, &draft.product)? {
        Some(_) => Ok(()),
        None => Err(BusError::invalid("arbiter.unknown_product", format!("{} is not a product on Coinbase. Call arbiter.products for the ones that exist.", draft.product))),
    }
}

fn run_backtest(engine: &Engine, p: &BacktestIn) -> Result<backtest::Report, BusError> {
    let (draft, strategy) = match (&p.draft, p.strategy_id) {
        (Some(d), sid) => (d.clone(), sid),
        (None, Some(id)) => (with_book(engine, |b| b.strategy(id))?.draft(), Some(id)),
        (None, None) => return Err(BusError::invalid("arbiter.invalid", "Give a strategy_id or a draft to backtest")),
    };
    rules::validate(&draft.rule).map_err(|m| BusError::invalid("arbiter.invalid", m))?;
    check_product(engine, &draft)?;
    let days = i64::from(p.days.unwrap_or(180).clamp(1, 730));
    let g = draft.granularity.seconds();
    let now = book::now();
    let end = now.div_euclid(g) * g; // closed bars only
    let start = end - days * 86400;
    let warm = i64::from(rules::warmup(&draft.rule));
    if (end - start) / g > 20_000 {
        return Err(BusError::invalid("arbiter.too_many_bars", format!("{days} days of {} bars is too many; use longer bars or fewer days", draft.granularity.short())));
    }
    let bars = desk::candles(engine, &draft.product, draft.granularity, start - warm * g, end)?;
    let first = bars.iter().position(|c| c.start >= start).unwrap_or(bars.len());
    let (conn, global) = with_book(engine, |b| Ok((b.connection()?, b.global_limits()?)))?;
    let cfg = Config { fees: conn.fees, slippage_pct: global.slippage_pct.to_f64().unwrap_or(0.1), tuned_fraction: p.tuned_fraction.unwrap_or(0.67), first };
    let mut report = backtest::run(&draft.rule, &bars, &cfg);
    if let Some(id) = strategy {
        let hash = rules::hash(&draft.rule);
        with_book(engine, |b| b.record_backtest(id, &hash, &report))?;
        let tried = with_book(engine, |b| b.strategy(id))?.variants_tried;
        if tried > 1 {
            report.warnings.push(format!("{tried} versions of this rule have been backtested. The more versions tried, the more the best one flatters itself: trust the judged part over the tuned part."));
        }
    }
    Ok(report)
}

fn place_out(o: Outcome) -> OrderPlaceOut {
    match o {
        Outcome::Placed(order) => OrderPlaceOut { outcome: "placed".into(), order: Some(*order), proposal: None, refusal: None },
        Outcome::Proposed(p) => OrderPlaceOut { outcome: "proposed".into(), order: None, proposal: Some(*p), refusal: None },
        Outcome::Refused(r) => OrderPlaceOut { outcome: "refused".into(), order: None, proposal: None, refusal: Some(r) },
    }
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<SummaryOp>(|ctx, _: Empty| summary(ctx.engine()));
    e.register_unlocked::<StrategyGet>(|ctx, p| detail(ctx.engine(), p.id));
    e.register_unlocked::<OrderList>(|ctx, p| Ok(OrdersOut { orders: with_book(ctx.engine(), |b| b.orders(p.strategy_id, p.limit.unwrap_or(100)))? }));
    e.register_unlocked::<DecisionList>(|ctx, p| Ok(DecisionsOut { decisions: with_book(ctx.engine(), |b| b.decisions(p.strategy_id, p.limit.unwrap_or(100)))? }));
    e.register_unlocked::<ProposalList>(|ctx, p| Ok(ProposalsOut { proposals: with_book(ctx.engine(), |b| b.proposals(p.pending.unwrap_or(false), p.limit.unwrap_or(50)))? }));
    e.register_unlocked::<SettingsGet>(|ctx, _: Empty| settings_out(ctx.engine()));
    e.register_unlocked::<Products>(|ctx, p| {
        let q = p.query.as_deref().map(str::to_ascii_uppercase);
        let mut products: Vec<Product> = desk::products(ctx.engine())?
            .into_iter()
            .filter(|x| p.quote.as_deref().is_none_or(|c| x.quote.eq_ignore_ascii_case(c)))
            .filter(|x| q.as_deref().is_none_or(|q| x.id.contains(q) || x.base.contains(q)))
            .collect();
        products.sort_by(|a, b| b.tradable.cmp(&a.tradable).then(a.id.cmp(&b.id)));
        Ok(ProductsOut { products })
    });
    e.register_unlocked::<SeriesOp>(|ctx, p| {
        let engine = ctx.engine();
        if desk::product(engine, &p.product)?.is_none() {
            return Err(BusError::invalid("arbiter.unknown_product", format!("{} is not a product on Coinbase", p.product)));
        }
        let g = p.granularity.unwrap_or_default();
        let bars = i64::from(p.bars.unwrap_or(300).clamp(10, 1500));
        let now = book::now();
        let candles = desk::candles(engine, &p.product, g, now - bars * g.seconds(), now + g.seconds())?;
        let mut markers = Vec::new();
        if let Some(sid) = p.strategy_id {
            let since = candles.first().map_or(0, |c| c.start);
            for f in with_book(engine, |b| b.fills(Some(sid), 500))? {
                let at = jiff::Timestamp::from_str(&f.at).map(|t| t.as_second()).unwrap_or(0);
                if at >= since && f.product == p.product {
                    markers.push(Marker { at, side: f.side, price: f.price.to_f64().unwrap_or(0.0), kind: f.venue.as_str().into() });
                }
            }
            markers.reverse();
        }
        Ok(Series { product: p.product, granularity: g, candles, markers })
    });
    e.register_unlocked::<Backtest>(|ctx, p| run_backtest(ctx.engine(), &p));

    e.register_staged::<KeySet, Connection>(
        |ctx, p| desk::set_key(ctx.engine(), &p.key_name, &p.private_key),
        |ctx, _, conn| {
            ctx.emit("arbiter.changed", json!({"key": true}));
            Ok(conn)
        },
    );
    e.register_staged::<KeyRemove, Connection>(
        |ctx, _| {
            let engine = ctx.engine();
            // Without the key a live order can be neither read back nor cancelled: it would stay
            // open for good, its fill never booked. The trade turn is held from the check until
            // the key is gone, so no order is placed in between.
            let _turn = engine.arbiter.trade.lock().unwrap_or_else(|p| p.into_inner());
            if with_book(engine, |b| b.open_orders(None))?.iter().any(|o| o.venue == Venue::Live) {
                return Err(BusError::refused("arbiter.order_open", "A live order is open: halt first, which cancels it"));
            }
            desk::forget_key(engine)?;
            with_book(engine, |b| {
                for s in b.strategies()?.into_iter().filter(|s| s.venue == Venue::Live && s.state == RunState::Running) {
                    b.set_state(s.id, RunState::Stopped, None)?;
                }
                b.set_connection(&Connection { state: "none".into(), exchange: "Coinbase".into(), ..Default::default() })?;
                b.set_balances(&[])?;
                b.log(None, "setting", "Forgot the Coinbase key; live strategies stopped.", None)?;
                b.connection()
            })
        },
        |ctx, _, conn| {
            ctx.emit("arbiter.changed", json!({"key": false}));
            Ok(conn)
        },
    );
    e.register_staged::<Refresh, Connection>(
        |ctx, _| desk::refresh_account(ctx.engine(), true),
        |ctx, _, conn| {
            ctx.emit("arbiter.changed", json!({}));
            Ok(conn)
        },
    );

    // Saving checks the product against the exchange's list, so it is staged.
    e.register_staged::<StrategySave, ()>(
        |ctx, p| check_product(ctx.engine(), &p.draft),
        |ctx, p, ()| {
            let engine = ctx.engine();
            let id = match p.id {
                Some(id) => {
                    with_book(engine, |b| b.save_strategy(id, &p.draft))?;
                    id
                }
                None => with_book(engine, |b| b.create_strategy(&p.draft))?,
            };
            let out = detail(engine, id)?;
            ctx.emit("arbiter.changed", json!({"strategy": id}));
            Ok(out)
        },
    );
    e.register::<StrategySet>(|ctx, p| {
        let engine = ctx.engine();
        if p.venue == Some(Venue::Live) {
            desk::check_live(engine)?;
        }
        with_book(engine, |b| {
            if let Some(m) = p.mode {
                b.set_mode(p.id, m)?;
            }
            if let Some(v) = p.venue {
                b.set_venue(p.id, v)?;
            }
            if let Some(run) = p.running {
                let s = b.strategy(p.id)?;
                match (run, s.state) {
                    (true, RunState::Halted) => return Err(book::BookError::Invalid("It is halted: restart it instead".into())),
                    (true, _) => b.set_state(p.id, RunState::Running, None)?,
                    (false, RunState::Running) => b.set_state(p.id, RunState::Stopped, None)?,
                    (false, _) => {}
                }
            }
            Ok(())
        })?;
        let out = detail(engine, p.id)?;
        ctx.emit("arbiter.changed", json!({"strategy": p.id}));
        Ok(out)
    });
    e.register::<StrategyDelete>(|ctx, p| {
        with_book(ctx.engine(), |b| b.delete_strategy(p.id))?;
        ctx.emit("arbiter.changed", json!({"strategy": p.id, "deleted": true}));
        Ok(Empty {})
    });
    e.register::<LimitsSet>(|ctx, p| {
        let engine = ctx.engine();
        match (p.strategy_id, p.limits, p.global) {
            (Some(id), Some(l), _) => with_book(engine, |b| b.set_strategy_limits(id, &l))?,
            (None, _, Some(g)) => with_book(engine, |b| {
                b.set_global_limits(&g)?;
                b.log(None, "setting", "Changed Arbiter-wide limits.", None).map(|_| ())
            })?,
            _ => return Err(BusError::invalid("arbiter.invalid", "Give a strategy_id with limits, or global limits")),
        }
        ctx.emit("arbiter.changed", json!({}));
        Ok(Empty {})
    });
    e.register::<SettingsSet>(|ctx, p| {
        let engine = ctx.engine();
        with_book(engine, |b| {
            if let Some(h) = p.home.as_deref() {
                b.set_home(h)?;
            }
            if let Some(c) = p.paper_cash.as_deref() {
                let d = Decimal::from_str(c.trim()).map_err(|_| book::BookError::Invalid(format!("{c} is not an amount")))?;
                b.set_paper_cash(d)?;
            }
            if p.reset_paper == Some(true) {
                if !b.open_orders(None)?.iter().all(|o| o.venue == Venue::Live) {
                    return Err(book::BookError::Invalid("A paper order is open: halt first".into()));
                }
                b.reset_paper()?;
            }
            Ok(())
        })?;
        let out = settings_out(engine)?;
        ctx.emit("arbiter.changed", json!({}));
        Ok(out)
    });
    e.register_staged::<Halt, HaltOut>(
        |ctx, p| {
            let reason = p.reason.clone().unwrap_or_else(|| "halted from Relay".into());
            let (cancelled, failed) = desk::halt(ctx.engine(), p.strategy_id, &reason)?;
            Ok(HaltOut { cancelled, failed })
        },
        |ctx, _, out| {
            ctx.emit("arbiter.changed", json!({"halted": true}));
            Ok(out)
        },
    );
    e.register::<Restart>(|ctx, p| {
        let engine = ctx.engine();
        if let Some(id) = p.strategy_id {
            if with_book(engine, |b| b.strategy(id))?.state != RunState::Halted {
                return Err(BusError::invalid("arbiter.invalid", "It is not halted"));
            }
        }
        // Past its daily loss limit still, it would be halted again on the runner's next pass.
        desk::check_restart(engine, p.strategy_id)?;
        with_book(engine, |b| match p.strategy_id {
            Some(id) => b.set_state(id, RunState::Running, None),
            None => {
                b.set_halted(None)?;
                b.log(None, "restart", "Lifted the halt on everything.", None).map(|_| ())
            }
        })?;
        ctx.emit("arbiter.changed", json!({"restarted": true}));
        Ok(Empty {})
    });
    e.register_staged::<Flatten, OrderView>(
        |ctx, p| {
            let engine = ctx.engine();
            let s = with_book(engine, |b| b.strategy(p.strategy_id))?;
            let intent = Intent { product: s.product.clone(), side: Side::Sell, quote: None, base: None, limit: false, source: Source::Person, cause: format!("s{}:flatten:{}", s.id, book::now()) };
            match desk::execute(engine, s.id, &intent, "sell everything (asked by you)")? {
                Outcome::Placed(o) => Ok(*o),
                Outcome::Refused(r) => Err(BusError::refused(r.code, r.message)),
                Outcome::Proposed(_) => Err(BusError::internal("a sell asked by the person became a proposal")),
            }
        },
        |ctx, _, o| {
            ctx.emit("arbiter.changed", json!({"order": o.id}));
            Ok(o)
        },
    );
    e.register_staged::<Propose, ProposalView>(
        |ctx, p| {
            let engine = ctx.engine();
            rules::validate(&p.draft.rule).map_err(|m| BusError::invalid("arbiter.invalid", m))?;
            check_product(engine, &p.draft)?;
            if p.why.trim().is_empty() {
                return Err(BusError::invalid("arbiter.invalid", "Say why, in a sentence the person reads on the card"));
            }
            let (title, changes, kind) = match p.strategy_id {
                Some(id) => {
                    let s = with_book(engine, |b| b.strategy(id))?;
                    let changes = book::changes(&s.draft(), &p.draft);
                    if changes.is_empty() {
                        return Err(BusError::invalid("arbiter.invalid", "That is the strategy as it already is"));
                    }
                    (format!("Change \"{}\" to version {}", s.name, s.version + 1), changes, "change")
                }
                None => (format!("New strategy \"{}\"", p.draft.name.trim()), rules::describe(&p.draft.rule, &p.draft.product, p.draft.granularity), "new_strategy"),
            };
            let new = book::NewProposal {
                kind: kind.into(),
                strategy_id: p.strategy_id,
                title,
                why: p.why.trim().to_string(),
                source: Some(Source::Agent),
                draft: Some(p.draft.clone()),
                changes,
                thread_id: p.thread_id,
                ..Default::default()
            };
            let id = with_book(engine, |b| b.add_proposal(&new))?;
            Ok(with_book(engine, |b| b.proposal(id))?.0)
        },
        |ctx, _, view| {
            ctx.emit("arbiter.changed", json!({"proposal": view.id}));
            Ok(view)
        },
    );
    e.register_staged::<OrderPlace, OrderPlaceOut>(
        |ctx, p| {
            let engine = ctx.engine();
            let s = with_book(engine, |b| b.strategy(p.strategy_id))?;
            if p.why.trim().is_empty() {
                return Err(BusError::invalid("arbiter.invalid", "Say why, in a sentence the person reads"));
            }
            let quote = match (p.side, p.quote.as_deref()) {
                (Side::Buy, Some(q)) => Some(amount(q, "The amount to spend")?),
                (Side::Buy, None) => return Err(BusError::invalid("arbiter.invalid", "A buy needs `quote`, the amount to spend")),
                (Side::Sell, _) => None,
            };
            let base = p.base.as_deref().map(|b| amount(b, "The amount to sell")).transpose()?;
            // One agent decision per minute and strategy: a retried call is the same order.
            let cause = format!("s{}:agent:{}:{}:{}", s.id, p.side.as_str(), p.quote.as_deref().or(p.base.as_deref()).unwrap_or("all"), book::now() / 60);
            let intent = Intent { product: s.product.clone(), side: p.side, quote, base, limit: p.limit.unwrap_or(false), source: Source::Agent, cause };
            Ok(place_out(desk::route(engine, s.id, &intent, p.why.trim(), p.thread_id)?))
        },
        |ctx, _, out| {
            ctx.emit("arbiter.changed", json!({"outcome": out.outcome}));
            Ok(out)
        },
    );
    e.register_staged::<ProposalResolve, ProposalView>(
        |ctx, p| desk::resolve(ctx.engine(), p.id, p.approve, p.draft_hash.as_deref()),
        |ctx, _, view| {
            ctx.emit("arbiter.changed", json!({"proposal": view.id}));
            Ok(view)
        },
    );
}
