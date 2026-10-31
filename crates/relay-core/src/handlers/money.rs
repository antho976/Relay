//! `money.*` — the Money space's ledger (docs/MONEY.md), and its investments
//! (docs/INVESTMENTS.md).
//!
//! The ledger is its own SQLite file, `money.db` beside the store, behind its own mutex: money has
//! its own schema versions and is what the phone will sync with. Its work is local and short, so
//! queries run unlocked and mutations inside the store's transaction like any other op; a money
//! write is committed in `money.db` whether or not the store transaction around it commits. A
//! Wealthsimple file or a Tally backup is read, and a backup written, before the transaction
//! opens, and the Bank of Canada's rates are fetched on a thread of their own after it closes:
//! nothing slow runs under the store's lock, and no file is touched under the ledger's.

use crate::engine::{Ctx, Engine, IntoBus};
use relay_bus::ops::money::*;
use relay_bus::{BusError, Empty};
use relay_money::backup::{self, BackupFile, BackupReadResult};
use relay_money::bank_statements::decode_text;
use relay_money::invest::{parse_scaled, RATE_SCALE, SOURCE_BANK_OF_CANADA, SOURCE_MANUAL};
use relay_money::ledger::{parse_date, Ledger, LedgerError, TxInput, TxPatch, TxQuery};
use relay_money::money::Locale;
use relay_money::views::{AccountPatch, ActivityInput, ActivityQuery};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A backup bigger than this is not one Tally wrote.
const MAX_BACKUP_BYTES: u64 = 64 << 20;
/// A Wealthsimple export bigger than this is not one: years of activity are a few megabytes.
const MAX_EXPORT_BYTES: u64 = 16 << 20;

/// The Bank of Canada's Valet API: the US dollar's daily rate in Canadian dollars over its last ten
/// business days. It needs no key, and the request carries nothing about the person.
const VALET_USD_CAD: &str = "https://www.bankofcanada.ca/valet/observations/FXUSDCAD/json?recent=10";
/// How long the Bank of Canada has to answer.
const FX_TIMEOUT: Duration = Duration::from_secs(15);
/// A fetch of the rates is on its way: a second one asked for meanwhile joins it.
static FX_FETCHING: AtomicBool = AtomicBool::new(false);

/// Holds [`FX_FETCHING`] for one fetch and lets it go however the fetch ends, a panic included, so
/// a failed fetch never leaves every later one answering `started: false`.
struct Fetching;

impl Fetching {
    fn start() -> Option<Fetching> {
        (!FX_FETCHING.swap(true, Ordering::SeqCst)).then_some(Fetching)
    }
}

impl Drop for Fetching {
    fn drop(&mut self) {
        FX_FETCHING.store(false, Ordering::SeqCst);
    }
}

fn bus(e: LedgerError) -> BusError {
    match e {
        LedgerError::Invalid(m) => BusError::invalid("money.invalid", m),
        LedgerError::NotFound(m) => BusError::not_found("money.not_found", m),
        LedgerError::Stale(m) => BusError::conflict("money.sync_stale", m),
        LedgerError::Sql(e) => BusError::internal(format!("money ledger: {e}")),
    }
}

/// Runs `f` on the ledger, opening `money.db` beside the store on first use (in memory when the
/// store is, as in tests).
pub(crate) fn ledger<T>(engine: &Engine, f: impl FnOnce(&mut Ledger) -> Result<T, LedgerError>) -> Result<T, BusError> {
    let mut slot = engine.money.lock().unwrap_or_else(|p| p.into_inner());
    if slot.is_none() {
        let store = engine.store.path();
        let opened = if store == Path::new(":memory:") {
            Ledger::open_in_memory()
        } else {
            Ledger::open(&store.with_file_name("money.db"))
        };
        *slot = Some(opened.map_err(bus)?);
    }
    f(slot.as_mut().expect("opened above")).map_err(bus)
}

/// The day a reading is for: the caller's, else this PC's. Within a few centuries of now, so the
/// periods and bill horizons computed from it stay inside the calendar.
fn today(day: Option<&str>) -> Result<jiff::civil::Date, BusError> {
    match day {
        Some(d) => parse_date(d).filter(|d| (1900..=2999).contains(&d.year())).ok_or_else(|| BusError::invalid("money.invalid", format!("Not a date: {d}"))),
        None => Ok(jiff::Zoned::now().date()),
    }
}

fn absolute(path: &str) -> Result<&Path, BusError> {
    let p = Path::new(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        Err(BusError::invalid("money.path", "Give the file's full path"))
    }
}

/// A file the person chose, read whole. Run before any lock is taken. Only a regular file is read,
/// and never more than `max` bytes of it: a pipe or a device (`/dev/zero` says it is empty) would
/// otherwise hold the request open forever, or fill memory.
fn read_bounded(path: &str, max: u64, too_big: &str) -> Result<Vec<u8>, BusError> {
    let path = absolute(path)?;
    let unreadable = |e: std::io::Error| BusError::invalid("money.path", format!("{}: {e}", path.display()));
    let too_big = || BusError::invalid("money.import", too_big);
    let meta = std::fs::metadata(path).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(BusError::invalid("money.path", format!("{}: not a file", path.display())));
    }
    if meta.len() > max {
        return Err(too_big());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path).and_then(|f| f.take(max + 1).read_to_end(&mut bytes)).map_err(unreadable)?;
    if bytes.len() as u64 > max {
        return Err(too_big());
    }
    Ok(bytes)
}

/// A Wealthsimple file, decoded as the phone decodes it (UTF-8, UTF-16 by its mark, else
/// Windows-1252), so both read the same text.
fn read_export(path: &str) -> Result<String, BusError> {
    read_bounded(path, MAX_EXPORT_BYTES, "That file is too big to be a Wealthsimple export").map(|b| decode_text(&b))
}

/// A Tally backup, read and decoded before any lock is taken.
fn read_backup(path: &str) -> Result<BackupFile, BusError> {
    let bytes = read_bounded(path, MAX_BACKUP_BYTES, "That file is too big to be a Tally backup")?;
    let text = String::from_utf8(bytes).map_err(|_| BusError::invalid("money.import", "This file is not a Tally backup."))?;
    match backup::decode(&text) {
        BackupReadResult::Ok(file) => Ok(file),
        BackupReadResult::Invalid(why) => Err(BusError::invalid("money.import", why)),
    }
}

/// Writes the ledger as a Tally backup at `path`, with no lock held but the ledger's own for the
/// read. A path that is there already must be a regular file: writing into a pipe blocks until
/// something reads it. The new backup is renamed in over the old one, so a crash or a full disk
/// mid-write leaves last week's whole, and only the user can read it (it is the whole ledger).
/// A symlink is written through, not replaced: a backup linked into a synced folder keeps syncing.
fn write_backup(engine: &Engine, path: &str) -> Result<ExportOut, BusError> {
    let path = absolute(path)?;
    let target = crate::private::resolve_links(path)
        .map_err(|e| BusError::invalid("money.path", format!("{}: {e}", path.display())))?;
    if let Ok(meta) = std::fs::symlink_metadata(&target) {
        if !meta.is_file() {
            return Err(BusError::invalid("money.path", format!("{}: not a file", path.display())));
        }
    }
    let file = ledger(engine, |l| l.export_backup(&crate::time::now()))?;
    crate::private::write_private_replace(&target, backup::encode(&file).as_bytes()).bus()?;
    Ok(ExportOut { path: path.display().to_string(), transactions: file.transactions.len() })
}

/// The rates in a Valet answer for `FXUSDCAD` (`{"observations": [{"d": "2026-10-08", "FXUSDCAD":
/// {"v": "1.3712"}}]}`): each day and its rate at [`RATE_SCALE`]. An observation that cannot be
/// read is left out; an answer with none is a refusal.
fn valet_rates(text: &str) -> Result<Vec<(String, i64)>, String> {
    let answer: Value = serde_json::from_str(text).map_err(|e| format!("The Bank of Canada's answer is not one Relay reads: {e}"))?;
    let rates: Vec<(String, i64)> = answer["observations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|o| {
            let day = o["d"].as_str().filter(|d| parse_date(d).is_some())?;
            let rate = match &o["FXUSDCAD"]["v"] {
                Value::String(v) => parse_scaled(v, RATE_SCALE.ilog10())?,
                Value::Number(v) => parse_scaled(&v.to_string(), RATE_SCALE.ilog10())?,
                _ => return None,
            };
            (rate.value > 0).then(|| (day.to_string(), rate.value))
        })
        .collect();
    if rates.is_empty() {
        return Err("The Bank of Canada sent no US dollar rate".into());
    }
    Ok(rates)
}

/// The recent USD → CAD rates, through `curl` under a deadline. Never called under a lock.
fn fetch_usd_cad(curl: &Path) -> Result<Vec<(String, i64)>, String> {
    let mut command = Command::new(curl);
    command.args(["--silent", "--show-error", "--fail", "--proto", "=https", VALET_USD_CAD]);
    match crate::proc::output_with_timeout(&mut command, FX_TIMEOUT) {
        Ok(Some(out)) if out.status.success() => valet_rates(&String::from_utf8_lossy(&out.stdout)),
        Ok(Some(out)) => Err(format!("The Bank of Canada could not be reached: {}", String::from_utf8_lossy(&out.stderr).trim())),
        Ok(None) => Err(format!("The Bank of Canada did not answer within {} seconds", FX_TIMEOUT.as_secs())),
        Err(e) => Err(format!("curl could not start: {e}")),
    }
}

pub fn register(e: &mut Engine) {
    e.register_unlocked::<SummaryOp>(|ctx, p| {
        let day = today(p.today.as_deref())?;
        // A reading for a later day is only a reading: bills are posted up to the real one, or a
        // `today` a year on would post a year of them and move each bill's next date for good.
        let due = day.min(jiff::Zoned::now().date());
        let (posted, summary) = ledger(ctx.engine(), |l| Ok((l.post_due(due)?, l.summary(day, &Locale::from_env())?)))?;
        // Bills that came due are entries now; whoever else is drawing the ledger re-reads it.
        if posted > 0 {
            ctx.emit("money.changed", json!({"posted": posted}));
        }
        Ok(summary)
    });
    e.register_unlocked::<ListsOp>(|ctx, _: Empty| ledger(ctx.engine(), |l| l.lists()));
    e.register_unlocked::<TxList>(|ctx, p| {
        let day = today(p.today.as_deref())?;
        let (from, to) = (p.from.as_deref().map(|d| today(Some(d))).transpose()?, p.to.as_deref().map(|d| today(Some(d))).transpose()?);
        let q = TxQuery { period_offset: p.period_offset, from, to, query: p.query, account_id: p.account_id, category_id: p.category_id, limit: p.limit };
        ledger(ctx.engine(), |l| l.tx_list(&q, day))
    });
    e.register_unlocked::<SeriesOp>(|ctx, p| {
        let day = today(p.today.as_deref())?;
        ledger(ctx.engine(), |l| l.series(&p, day))
    });
    e.register::<TxAdd>(|ctx, p| {
        let input = TxInput {
            r#type: p.r#type, amount: p.amount, date: p.date, account_id: p.account_id,
            to_account_id: p.to_account_id, category_id: p.category_id, note: p.note,
        };
        let tx = ledger(ctx.engine(), |l| l.tx_add(&input))?;
        ctx.emit("money.changed", json!({"tx": tx.id}));
        Ok(tx)
    });
    e.register::<TxUpdate>(|ctx, p| {
        let patch = TxPatch {
            r#type: p.r#type, amount: p.amount, date: p.date, account_id: p.account_id,
            to_account_id: p.to_account_id, category_id: p.category_id, note: p.note, clear_category: p.clear_category,
        };
        let (tx, previous) = ledger(ctx.engine(), |l| {
            let previous = l.tx(p.id)?;
            Ok((l.tx_update(p.id, &patch)?, previous))
        })?;
        ctx.emit("money.changed", json!({"tx": tx.id}));
        Ok(TxChanged { tx, previous })
    });
    e.register::<TxDelete>(|ctx, p| {
        ledger(ctx.engine(), |l| l.tx_delete(p.id))?;
        ctx.emit("money.changed", json!({"tx": p.id, "deleted": true}));
        Ok(Empty {})
    });
    e.register::<TxRestore>(|ctx, p| {
        let tx = ledger(ctx.engine(), |l| l.tx_restore(p.id))?;
        ctx.emit("money.changed", json!({"tx": tx.id}));
        Ok(tx)
    });
    e.register::<BudgetSet>(|ctx, p| {
        ledger(ctx.engine(), |l| l.budget_set(p.category_id, p.amount))?;
        ctx.emit("money.changed", json!({}));
        Ok(Empty {})
    });
    e.register::<AccountAdd>(|ctx, p| {
        let institution = p.institution.as_deref().unwrap_or("");
        let account = ledger(ctx.engine(), |l| l.account_add(&p.name, p.r#type, p.opening_balance.unwrap_or(0), p.registration, institution))?;
        ctx.emit("money.changed", json!({}));
        Ok(account)
    });
    e.register::<AccountUpdate>(|ctx, p| {
        let patch = AccountPatch { name: p.name, registration: p.registration, institution: p.institution, archived: p.archived };
        let account = ledger(ctx.engine(), |l| l.account_update(p.id, &patch))?;
        ctx.emit("money.changed", json!({}));
        Ok(account)
    });
    e.register::<ValueSet>(|ctx, p| {
        let account = ledger(ctx.engine(), |l| l.value_set(p.account_id, &p.date, p.value))?;
        ctx.emit("money.changed", json!({"account": account.id}));
        Ok(account)
    });
    e.register::<SettingsSet>(|ctx, p| {
        let settings = ledger(ctx.engine(), |l| {
            if let Some(c) = p.currency.as_deref() {
                l.set_currency(c.trim())?;
            }
            if let Some(d) = p.month_start_day {
                l.set_month_start_day(d)?;
            }
            l.settings()
        })?;
        ctx.emit("money.changed", json!({}));
        Ok(settings)
    });
    // The file is read and decoded, and the restore written, before the transaction opens: the
    // ledger has its own mutex, and a 64 MiB restore under the store's held every other request.
    e.register_staged::<Import, _>(
        |ctx, p| {
            let file: BackupFile = read_backup(&p.path)?;
            let accounts = file.accounts.len();
            let transactions = ledger(ctx.engine(), |l| l.import_backup(&file))?;
            Ok(ImportOut { kind: "backup".into(), transactions, accounts })
        },
        |ctx: &mut Ctx, _, out: ImportOut| {
            ctx.emit("money.changed", json!({"imported": out.transactions}));
            Ok(out)
        },
    );
    // The file is written before the transaction opens, which then only records that it was.
    e.register_staged::<Export, _>(|ctx, p| write_backup(ctx.engine(), &p.path), |_: &mut Ctx, _, out: ExportOut| Ok(out));
    e.register::<Sample>(|ctx, _: Empty| {
        let day = today(None)?;
        let transactions = ledger(ctx.engine(), |l| {
            let currency = l.settings()?.currency;
            l.load_sample(day, &currency)
        })?;
        ctx.emit("money.changed", json!({}));
        Ok(SampleOut { transactions })
    });
    // The ledger has its own mutex and needs nothing from the store: the whole sync runs before
    // the store's transaction opens. A replace's VACUUM INTO copies the whole money.db, and a
    // cloud round applies up to 2000 changes; neither holds every other request (keystrokes
    // included) behind it.
    e.register_staged::<Sync, _>(|ctx, p| {
        let device = p.device.trim();
        if device.is_empty() || device.chars().count() > 64 || device.contains(char::is_control) {
            return Err(BusError::invalid("money.invalid", "A device name is 1 to 64 printable characters"));
        }
        if p.limit == Some(0) {
            return Err(BusError::invalid("money.invalid", "A sync's limit is 1 or more changes"));
        }
        let limit = p.limit.map(|l| l as usize);
        // The ledger leaves out a change stamped below 0 or more than a day ahead of this clock;
        // the device hears it as skipped, and the log says which device sent it.
        let now = jiff::Timestamp::now().as_millisecond();
        let out_of_range = p.changes.iter().filter(|c| !relay_money::ledger::stamp_ok(c.updated_at, now)).count();
        if out_of_range > 0 {
            tracing::warn!(device, out_of_range, "money.sync: changes stamped below 0 or more than a day ahead were not applied");
        }
        ledger(ctx.engine(), |l| {
            l.sync_page(device, p.replace.unwrap_or(false), p.since, p.generation.as_deref(), &p.changes, limit, &crate::time::now())
        })
    }, |ctx: &mut Ctx, p, out: relay_money::views::SyncOut| {
        if out.applied > 0 || out.replaced {
            ctx.emit("money.changed", json!({"synced": p.device.trim(), "applied": out.applied}));
        }
        Ok(out)
    });
    // The ledger is copied aside (`VACUUM INTO`) and cleared before the transaction opens: it has
    // its own mutex, and the copy of a large ledger under the store's held every other request.
    e.register_staged::<Reset, _>(
        |ctx, _| ledger(ctx.engine(), |l| l.reset()),
        |ctx: &mut Ctx, _, ()| {
            ctx.emit("money.changed", json!({}));
            Ok(Empty {})
        },
    );
    register_invest(e);
}

/// Investments (docs/INVESTMENTS.md): the portfolio reading, activities, the Wealthsimple import,
/// room, prices and rates.
fn register_invest(e: &mut Engine) {
    e.register_unlocked::<InvestSummary>(|ctx, p| {
        let day = today(p.today.as_deref())?;
        ledger(ctx.engine(), |l| l.portfolio(day))
    });
    e.register_unlocked::<InvestList>(|ctx, p| {
        let q = ActivityQuery { account_id: p.account_id, r#type: p.r#type, since: p.since, limit: p.limit };
        Ok(InvestListOut { activities: ledger(ctx.engine(), |l| l.invest_list(&q))? })
    });
    e.register::<InvestAdd>(|ctx, p| {
        // An account Wealthsimple's files fill gets this activity from the next export: one a
        // thread's agent recorded too would count twice (INVESTMENTS.md). The person may still.
        if ctx.thread().is_some() {
            let filled = ledger(ctx.engine(), |l| l.accounts())?
                .into_iter()
                .any(|a| a.id == p.account_id && a.institution.trim().eq_ignore_ascii_case("wealthsimple"));
            if filled {
                return Err(BusError::refused(
                    "money.import_filled",
                    "This account is filled from Wealthsimple's files: the next holdings report or activities export brings this activity, and recording it here too would count it twice",
                ));
            }
        }
        let input = ActivityInput {
            account_id: p.account_id, r#type: p.r#type, date: p.date, symbol: p.symbol, currency: p.currency, quantity: p.quantity,
            amount: p.amount, fee: p.fee, note: p.note, to_amount: p.to_amount, to_currency: p.to_currency,
        };
        let activity = ledger(ctx.engine(), |l| l.invest_add(&input))?;
        ctx.emit("money.changed", json!({"activity": activity.id}));
        Ok(activity)
    });
    e.register::<InvestDelete>(|ctx, p| {
        ledger(ctx.engine(), |l| l.invest_delete(p.id))?;
        ctx.emit("money.changed", json!({"activity": p.id, "deleted": true}));
        Ok(Empty {})
    });
    e.register::<InvestRestore>(|ctx, p| {
        let activity = ledger(ctx.engine(), |l| l.invest_restore(p.id))?;
        ctx.emit("money.changed", json!({"activity": activity.id}));
        Ok(activity)
    });
    e.register_unlocked::<InvestPreview>(|ctx, p| {
        let text = read_export(&p.path)?;
        ledger(ctx.engine(), |l| l.invest_preview(&text, p.account_id))
    });
    // The file is read, and the plan written, before the transaction opens (the ledger's own
    // mutex is all it needs).
    e.register_staged::<InvestImport, _>(
        |ctx, p| {
            let text = read_export(&p.path)?;
            ledger(ctx.engine(), |l| l.invest_import(&text, &p.accounts, p.account_id))
        },
        |ctx: &mut Ctx, _, result| {
            ctx.emit("money.changed", json!({"imported": result.holdings + result.activities}));
            Ok(result)
        },
    );
    e.register::<InvestRoom>(|ctx, p| {
        ledger(ctx.engine(), |l| l.room_set(p.registration, p.year, p.amount))?;
        ctx.emit("money.changed", json!({}));
        Ok(Empty {})
    });
    e.register::<InvestPrice>(|ctx, p| {
        ledger(ctx.engine(), |l| l.price_set(&p.symbol, &p.date, p.price))?;
        ctx.emit("money.changed", json!({}));
        Ok(Empty {})
    });
    e.register::<FxSet>(|ctx, p| {
        ledger(ctx.engine(), |l| l.fx_set(&p.base, &p.quote, &p.date, p.rate, SOURCE_MANUAL))?;
        ctx.emit("money.changed", json!({}));
        Ok(Empty {})
    });
    // The github.connect shape: answer at once, fetch on a thread of its own once the transaction
    // is over, write the rates in one short ledger call, then say so with `money.changed`.
    e.register::<FxFetch>(|ctx, _: Empty| {
        let curl = which::which("curl").map_err(|_| {
            BusError::unavailable("money.curl_missing", "curl is not installed or not on PATH").with_hint("install curl, then fetch the rate again")
        })?;
        if FX_FETCHING.load(Ordering::SeqCst) {
            return Ok(FxFetchOut { started: false });
        }
        ctx.after_commit(move |engine| {
            let Some(fetching) = Fetching::start() else { return };
            let fetch = move || {
                let written = fetch_usd_cad(&curl).and_then(|rates| {
                    ledger(&engine, |l| {
                        for (day, rate) in &rates {
                            l.fx_set("USD", "CAD", day, *rate, SOURCE_BANK_OF_CANADA)?;
                        }
                        Ok(rates.len())
                    })
                    .map_err(|e| e.message)
                });
                drop(fetching);
                match written {
                    Ok(n) => engine.emit_system("money.changed", json!({"fx": n})),
                    Err(error) => {
                        tracing::warn!(error = %error, "fetching the Bank of Canada's rates");
                        engine.emit_system("money.changed", json!({"fx": 0, "error": error}));
                    }
                }
            };
            // A thread that cannot start drops the fetch, and with it the flag.
            if let Err(e) = std::thread::Builder::new().name("money-fx".into()).spawn(fetch) {
                tracing::warn!(error = %e, "starting the Bank of Canada fetch");
            }
        });
        Ok(FxFetchOut { started: true })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What copies or rewrites the whole ledger runs before the store's lock is taken.
    #[test]
    fn whole_ledger_work_runs_outside_the_store_lock() {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        for op in ["money.reset", "money.import", "money.export", "money.sync"] {
            assert!(engine.is_staged(op), "{op}");
        }
    }

    #[test]
    fn a_valet_answer_reads_as_days_and_rates() {
        let answer = r#"{"terms": {"url": "https://www.bankofcanada.ca/terms/"},
            "seriesDetail": {"FXUSDCAD": {"label": "USD/CAD", "description": "US dollar to Canadian dollar daily exchange rate"}},
            "observations": [
                {"d": "2026-10-07", "FXUSDCAD": {"v": "1.3712"}},
                {"d": "2026-10-08", "FXUSDCAD": {"v": 1.37125}},
                {"d": "2026-10-09", "FXUSDCAD": {"v": ""}},
                {"d": "yesterday", "FXUSDCAD": {"v": "1.37"}},
                {"d": "2026-10-10", "FXUSDCAD": {"v": "-1"}}
            ]}"#;
        assert_eq!(valet_rates(answer).unwrap(), vec![("2026-10-07".to_string(), 137_120_000), ("2026-10-08".to_string(), 137_125_000)]);
        assert!(valet_rates(r#"{"observations": []}"#).unwrap_err().contains("no US dollar rate"));
        assert!(valet_rates("<html>Service unavailable</html>").is_err());
    }
}
