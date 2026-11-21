//! The client side of `money.sync` (Kotlin: data/sync/RelaySync.kt, `exchange` and `send`), over
//! relay-money's own sync on this phone's ledger.
//!
//! The server's `money.sync` erases its ledger and takes the device's when asked with
//! `replace: true` ("the phone is where the ledger lives"). The owner's real ledger lives on that
//! server, so Tally RS never asks that, and cannot:
//!
//! - [`SyncIn`], the only payload this module sends, has no `replace` field. The server reads a
//!   missing one as false.
//! - What a sync asks is an [`Ask`]. [`Ask::Whole`] sends nothing and takes the server's ledger
//!   in place of this phone's. [`Ask::Merge`] is the only way to send rows, and it needs a
//!   [`Generation`], which only an answer from the server makes ([`Generation::saved`] reads
//!   back one kept after this phone took the server's ledger whole). So the first sync of a
//!   phone can only pull.
//! - An answer saying the server's ledger was replaced is refused as a fault.
//! - `State::request` refuses `money.sync` (and the ops that erase a ledger) from any screen.
//!
//! On this phone the same rules run the other way: the server's rows are applied with
//! relay-money's `sync_page` on the local ledger (newest `updated_at` wins per row), and the rows
//! changed here since the last push are read with it too (`since` is this ledger's own cursor).

use relay_money::views::Change;
use serde_json::Value;

use crate::model::Store;
use crate::sync::Failure;
use crate::sync_net::{Requester, code, fail};

/// Changes per request: a few hundred kilobytes, far under the door's 8 MiB message.
pub const BATCH: usize = 1_500;
/// Changes per answer at most, as Relay's own cloud link asks.
pub const PAGE: u32 = 2_000;
/// A server from before pages refuses a field it does not know with this, `limit` included.
pub const SCHEMA: &str = "bus.schema";
/// The server's ledger is not the one this phone last merged with.
pub const STALE: &str = "money.sync_stale";
/// This phone's ledger is not the one it last merged with: it sends nothing.
pub const STALE_HERE: &str = "tally.sync_stale_here";
/// The server's ledger was replaced since the last sync and this phone holds changes it never
/// sent: taking the server's ledger would drop them, so the person says so first.
pub const STALE_UNSENT: &str = "tally.sync_stale_unsent";

/// The name the server is recorded under in this phone's ledger (`sync.device.*`).
const LOCAL: &str = "Your server";

pub const TOOK_SERVER_LEDGER: &str = "Your server's ledger had been replaced, so this phone took it";
pub const TOOK_SERVER_LEDGER_ASKED: &str = "This phone now holds your server's ledger";

/// The generation of the server's ledger this phone holds a copy of.
#[derive(Clone, Debug, PartialEq)]
pub struct Generation(String);

impl Generation {
    /// The one kept after the last sync, once this phone has taken the server's ledger whole.
    pub fn saved(store: &Store) -> Option<Generation> {
        store.pref(PULLED)?;
        store.pref(GENERATION).filter(|g| !g.is_empty()).map(Generation)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Prefs this module keeps (this phone's own, never synced).
pub const PULLED: &str = "sync.pulled";
pub const GENERATION: &str = "sync.generation";
pub const CURSOR: &str = "sync.cursor";
pub const PUSHED: &str = "sync.pushed";
pub const LOCAL_GENERATION: &str = "sync.local_generation";

/// What one sync asks the server for.
#[derive(Debug)]
pub enum Ask {
    /// The server's whole ledger, to take in place of this phone's: `since` 0, no generation,
    /// nothing sent. A phone's first sync, a ledger the server replaced since, or the person's ask.
    Whole,
    /// This phone's changes since the last push, and the server's since `since`.
    Merge { since: i64, generation: Generation, changes: Vec<Change> },
}

/// `money.sync`'s payload as Tally RS sends it. There is no `replace`.
#[derive(serde::Serialize)]
struct SyncIn<'a> {
    device: &'a str,
    since: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<&'a str>,
    changes: &'a [Change],
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
}

/// The server's answer to a whole sync.
#[derive(Debug)]
pub struct Answer {
    /// The server's ledger, whole, to take in place of this phone's.
    pub whole: bool,
    pub cursor: i64,
    pub generation: Generation,
    /// The server's rows this phone has not seen, newest of each.
    pub changes: Vec<Change>,
    /// This phone's changes the server took.
    pub sent: usize,
    /// This phone's changes the server did not take and holds nothing newer of (a stamp too far
    /// ahead of its clock, a row it cannot place): they go again next time.
    pub not_taken: usize,
    pub note: Option<String>,
}

/// What [`not_taken`](Answer::not_taken) says on the Sync page.
pub fn not_taken_note(n: usize) -> String {
    let what = if n == 1 { "1 change here was".to_string() } else { format!("{n} changes here were") };
    format!("{what} not taken by your server. They go again with the next sync; if this stays, check this phone's date and time.")
}

/// What [`Applied::ahead`] says on the Sync page.
pub fn ahead_note(n: usize) -> String {
    let what = if n == 1 { "1 change from your server is".to_string() } else { format!("{n} changes from your server are") };
    format!("{what} dated more than a day ahead of this phone's clock, so it was not taken here. It comes again with the next sync; check this phone's date and time.")
}

fn unreadable() -> Failure {
    fail(code::ANSWER, "Your server's answer did not read")
}

/// One sync on an admitted connection.
pub fn exchange(r: &mut dyn Requester, device: &str, ask: Ask) -> Result<Answer, Failure> {
    match ask {
        Ask::Whole => {
            let s = send(r, device, 0, None, &[])?;
            Ok(Answer { whole: true, cursor: s.cursor, generation: s.generation, changes: s.changes, sent: 0, not_taken: 0, note: None })
        }
        Ask::Merge { since, generation, changes } => match send(r, device, since, Some(&generation), &changes) {
            Ok(s) => Ok(Answer {
                whole: false,
                cursor: s.cursor,
                generation: s.generation,
                changes: s.changes,
                sent: changes.len().saturating_sub(s.skipped),
                not_taken: s.not_taken,
                note: (s.not_taken > 0).then(|| not_taken_note(s.not_taken)),
            }),
            // The server's ledger was restored, erased or replaced since: it is taken whole, and
            // nothing here is sent (the latest wholesale act wins). With nothing unsent here that
            // loses nothing, so it happens by itself; changes made here and never sent would go
            // with it, so the person is asked first. So is a ledger that came back far smaller
            // than this phone's (`apply`): the server's may have been lost.
            Err(f) if f.code == STALE && changes.is_empty() => {
                let s = send(r, device, 0, None, &[])?;
                Ok(Answer { whole: true, cursor: s.cursor, generation: s.generation, changes: s.changes, sent: 0, not_taken: 0, note: Some(TOOK_SERVER_LEDGER.into()) })
            }
            Err(f) if f.code == STALE => Err(fail(STALE_UNSENT, "Your server's ledger was replaced since this phone last synced, and changes made here never went to it")),
            Err(f) => Err(f),
        },
    }
}

/// What one [`send`] came back with.
struct Sent {
    cursor: i64,
    generation: Generation,
    changes: Vec<Change>,
    /// The server's count of the rows sent that it did not take.
    skipped: usize,
    /// Of those, the ones it holds nothing as new as (its answer did not bring them back as new
    /// or newer): not taken, and not outdated either.
    not_taken: usize,
}

/// A table, whether the row is matched by category, and the category's uid or the row's.
type MatchKey<'a> = (&'a str, bool, &'a str);

/// What the server matches a sent row by: its uid, but a budget by its category (each category
/// has one budget, whatever uid each device gave it; no category is the overall budget). A budget
/// tombstone that does not say its category is matched by uid, as the server matches it.
fn match_key(c: &Change) -> MatchKey<'_> {
    if c.table == "budgets" && (!c.deleted || c.row.contains_key("category")) {
        match c.row.get("category") {
            None | Some(Value::Null) => return (c.table.as_str(), true, ""),
            Some(Value::String(category)) => return (c.table.as_str(), true, category.as_str()),
            Some(_) => {}
        }
    }
    (c.table.as_str(), false, c.uid.as_str())
}

/// One `money.sync` of `changes` in batches, and the server's answer to it whole. The server's
/// changes are read first, in pages while the server says `more`, with nothing sent; then each
/// batch goes in a request of its own, answered whole (only what changed on the server since the
/// last page). A row the server takes is numbered after every page, so a page read after a batch
/// went would bring that batch back as it went, which would read as the server holding it as new
/// and hide a row it refused. A row edited while the pages are read comes again, newer: the newest
/// of each is kept, and what the server hands back exactly as sent is no news. A server from
/// before pages refuses `limit` before applying anything and is asked again whole.
fn send(r: &mut dyn Requester, device: &str, since: i64, generation: Option<&Generation>, changes: &[Change]) -> Result<Sent, Failure> {
    // Each row sent, by what the server matches it by: its stamp, where it is, and its uid.
    let mut sent: std::collections::HashMap<MatchKey, Vec<(i64, usize, &Change)>> = std::collections::HashMap::new();
    for (i, c) in changes.iter().enumerate() {
        sent.entry(match_key(c)).or_default().push((c.updated_at, i, c));
    }
    // A row sent that the server skips as older than its own comes back in the answer as new or
    // newer (a budget under the uid the server gave its own); one that does not come back was
    // refused.
    let mut outdated: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut skipped = 0usize;
    let mut newest: Vec<Change> = Vec::new();
    let mut at: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
    let batches: Vec<&[Change]> = changes.chunks(BATCH).collect();
    let mut cursor = since;
    let mut gen_: Option<String> = generation.map(|g| g.0.clone());
    let mut paged = true;
    // Reading the server's pages, before (or between) the batches.
    let mut pulling = true;
    let mut next_batch = 0;
    // A server that pages an answer to a batch unasked (Relay's does not) may bring the batch back
    // as it went: what it skipped cannot be told from what it holds newer, so every skip counts.
    let mut unsure = false;
    loop {
        let batch: &[Change] = if pulling { &[] } else { batches[next_batch] };
        let limit = (pulling && paged).then_some(PAGE);
        let payload = |limit: Option<u32>| serde_json::to_value(SyncIn { device, since: cursor, generation: gen_.as_deref(), changes: batch, limit }).unwrap_or(Value::Null);
        let result = match r.request("money.sync", &payload(limit), crate::sync_net::REQUEST_TIMEOUT) {
            Err(f) if limit.is_some() && f.code == SCHEMA => {
                paged = false;
                r.request("money.sync", &payload(None), crate::sync_net::REQUEST_TIMEOUT)?
            }
            other => other?,
        };
        if result.get("replaced").and_then(Value::as_bool) == Some(true) {
            // Never asked for; a server that did it anyway is not one to merge with.
            return Err(fail(code::ANSWER, "Your server says it replaced its ledger, which Tally RS never asks. Nothing here was changed."));
        }
        let next = result.get("cursor").and_then(Value::as_i64).ok_or_else(unreadable)?;
        let more = result.get("more").and_then(Value::as_bool) == Some(true);
        if more && next <= cursor {
            return Err(unreadable());
        }
        unsure |= more && !batch.is_empty();
        cursor = next;
        if let Some(g) = result.get("generation").and_then(Value::as_str) {
            gen_ = Some(g.to_string());
        }
        skipped += result.get("skipped").and_then(Value::as_u64).map_or(0, |n| n as usize);
        for v in result.get("changes").and_then(Value::as_array).into_iter().flatten() {
            let Ok(c) = serde_json::from_value::<Change>(v.clone()) else { continue };
            let mut echo = false;
            for &(mine, i, row) in sent.get(&match_key(&c)).into_iter().flatten() {
                // Newer there, or as new and held before this push went (a page read first, or
                // the very row sent): the server skips the one sent. A budget the server takes
                // onto its own (another uid, one per category) comes back in that batch's answer
                // under its uid with the stamp it went with: that is it taken, not outdated, and
                // counting it so would hide a row refused in the same push.
                if c.updated_at > mine || (c.updated_at == mine && (c.uid == row.uid || batch.is_empty())) {
                    outdated.insert(i);
                }
                echo |= c.updated_at == mine && c.uid == row.uid;
            }
            if echo {
                continue;
            }
            match at.get(&(c.table.clone(), c.uid.clone())) {
                Some(&k) if newest[k].updated_at >= c.updated_at => {}
                Some(&k) => newest[k] = c,
                None => {
                    at.insert((c.table.clone(), c.uid.clone()), newest.len());
                    newest.push(c);
                }
            }
        }
        if !batch.is_empty() {
            next_batch += 1;
        }
        // The rest of the server's answer first, then the next batch.
        pulling = more;
        if !more && next_batch >= batches.len() {
            break;
        }
    }
    let generation = gen_.filter(|g| !g.is_empty()).map(Generation).ok_or_else(unreadable)?;
    let not_taken = if unsure { skipped } else { skipped.saturating_sub(outdated.len()) };
    Ok(Sent { cursor, generation, changes: newest, skipped, not_taken })
}

fn now_text() -> String {
    jiff::Timestamp::now().to_string()
}

fn err(e: impl std::fmt::Display) -> Failure {
    fail("tally.ledger", format!("This phone's ledger could not take it: {e}"))
}

/// Whether this phone's ledger is still the one the last sync saw (not restored, erased or
/// replaced since).
fn same_ledger(store: &Store) -> bool {
    store.pref(LOCAL_GENERATION).is_some_and(|g| !g.is_empty() && store.ledger.generation().is_ok_and(|mine| mine == g))
}

/// relay-money's `sync_page` on this phone's own ledger. It records the server as a device synced
/// with, which would stop bills posting here: the record goes again at once.
fn local_page(store: &mut Store, replace: bool, since: i64, changes: &[Change]) -> Result<relay_money::views::SyncOut, Failure> {
    let out = store.ledger.sync_page(LOCAL, replace, since, None, changes, None, &now_text()).map_err(err);
    store.forget_sync_records();
    out
}

/// The rows changed on this phone since the last push, and the local cursor they were read at.
///
/// Only from the ledger the last sync saw. One replaced here since (an erase, a restore, the
/// sample) holds a tombstone for every row it had: sent, they would erase the server's ledger. So
/// such a ledger sends nothing, ever; the next sync takes the server's whole instead
/// ([`next_ask`]).
pub fn collect(store: &mut Store) -> Result<(Vec<Change>, i64), Failure> {
    if !same_ledger(store) {
        return Err(fail(STALE_HERE, "This phone's ledger was replaced since it last synced, so it sends nothing; the next sync takes your server's"));
    }
    let pushed = store.pref(PUSHED).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let cursor = store.ledger.cursor().map_err(err)?;
    let since = if pushed <= cursor { pushed } else { 0 };
    let out = local_page(store, false, since, &[])?;
    Ok((out.changes, out.cursor))
}

/// Where this phone's ledger stood when a sync read it: its counter and its generation. One
/// that moved by the time the answer comes was changed here meanwhile.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The local cursor (for a merge, the one the sent rows were read at).
    pub cursor: i64,
    generation: String,
}

impl Mark {
    /// Where this phone's ledger stands now.
    pub fn now(store: &Store) -> Result<Mark, Failure> {
        // The generation first: the first read of it writes it, which moves the counter.
        let generation = store.ledger.generation().map_err(err)?;
        Ok(Mark { cursor: store.ledger.cursor().map_err(err)?, generation })
    }

    fn moved(&self, store: &Store) -> Result<bool, Failure> {
        Ok(Mark::now(store)? != *self)
    }
}

/// What applying an answer did here.
#[derive(Debug, PartialEq)]
pub struct Applied {
    pub received: usize,
    /// Rows changed here while the sync ran: they go next time, so sync again soon.
    pub again: bool,
    /// A whole answer was not applied: this phone's ledger changed while it came, and taking it
    /// would drop what was made here meanwhile. The person says whether to take it.
    pub ask_first: bool,
    /// The server's rows this ledger did not take, stamped more than a day ahead of this phone's
    /// clock: where sync stands is kept so they come again ([`ahead_note`]).
    pub ahead: usize,
    /// Why a whole answer nobody asked for was not applied: it holds under half the accounts and
    /// entries this phone does, `(the server's, this phone's)`. The person says whether to take it.
    pub fewer: Option<(usize, usize)>,
}

/// The accounts and entries among `changes`, as Relay's own PC link weighs a ledger.
fn weight(changes: &[Change]) -> usize {
    changes.iter().filter(|c| !c.deleted && (c.table == "accounts" || c.table == "transactions")).count()
}

/// The accounts and entries this phone's ledger holds.
fn own_weight(store: &Store) -> Result<usize, Failure> {
    match store.sql() {
        Ok(db) => db
            .query_row("SELECT (SELECT COUNT(*) FROM accounts WHERE deleted = 0) + (SELECT COUNT(*) FROM transactions WHERE deleted = 0)", [], |r| r.get::<_, i64>(0))
            .map(|n| n.max(0) as usize)
            .map_err(err),
        // In memory there is no second connection.
        Err(_) => store.ledger.export_backup("").map(|f| f.accounts.len() + f.transactions.len()).map_err(err),
    }
}

/// Whether this phone's categories are other than the ones every new ledger is given: added,
/// renamed, recoloured, archived or deleted here. `Store::holds_anything` (and relay-money's own,
/// which decides whether a replace keeps a copy first) leaves categories out.
pub fn categories_changed(store: &Store) -> bool {
    use relay_money::model::{CategoryKind, DEFAULT_CATEGORIES};
    // Unreadable, they count as changed: asking is the safe side.
    let Ok(here) = store.ledger.categories() else { return true };
    let mut here: Vec<(String, bool, String, i64, bool)> = here.into_iter().map(|c| (c.name, c.kind == CategoryKind::Income, c.icon, c.color, c.archived)).collect();
    let mut seeded: Vec<(String, bool, String, i64, bool)> =
        DEFAULT_CATEGORIES.iter().map(|c| (c.name.to_string(), c.kind == CategoryKind::Income, c.icon.to_string(), i64::from(c.color), false)).collect();
    here.sort();
    seeded.sort();
    here != seeded
}

/// Whether this phone holds anything a pull of the server's whole ledger would replace: the
/// owner's rows, or categories they changed.
pub fn holds_own(store: &Store) -> bool {
    store.holds_anything() || categories_changed(store)
}

/// Applies the server's answer to this phone's ledger and keeps where both stand. A whole answer
/// replaces this phone's ledger (relay-money keeps a copy of it in `money-backups/` first), unless
/// the ledger moved from `mark` since the sync started, or, when the person did not ask for it
/// (`asked` false), it holds under half the accounts and entries this phone does. `mark` is where
/// the ledger stood when the sync read it ([`next_ask`]).
pub fn apply(store: &mut Store, answer: &Answer, mark: &Mark, asked: bool) -> Result<Applied, Failure> {
    // The server's rows relay-money will not take here: stamped more than a day ahead of this
    // phone's clock (a clock behind, or the server's ahead). Counted before applying, on a clock
    // no later than the one `sync_page` reads, so none it skips goes uncounted.
    let now = jiff::Timestamp::now().as_millisecond();
    let ahead = answer.changes.iter().filter(|c| c.updated_at > now.saturating_add(relay_money::ledger::STAMP_AHEAD_MS)).count();
    // Where sync stands with the server is kept only when everything sent was taken and nothing
    // changed here meanwhile: otherwise this sync's rows go again, and the server's answer comes
    // again with them, so each row sent again is matched to the server's copy of it. Nor when a
    // row of the server's was not taken here for its stamp: it would never come again.
    let (received, pushed, again, cursor) = if answer.whole {
        if mark.moved(store)? {
            // Entries made here while the server's ledger was on its way: replacing would erase
            // them unannounced. Nothing is applied; the person is asked, with them in view.
            return Ok(Applied { received: 0, again: false, ask_first: true, ahead: 0, fewer: None });
        }
        if !asked {
            // A server whose ledger was lost or made again empty (a restart on a disk that does
            // not last, a reinstall, an erase on the PC) answers stale, and this phone may hold
            // the only copy: taken by itself, it would go. Far fewer rows there than here, the
            // person says first, as Relay's own PC link holds such a replace.
            let (there, here) = (weight(&answer.changes), own_weight(store)?);
            if here > 0 && there * 2 < here {
                return Ok(Applied { received: 0, again: false, ask_first: true, ahead: 0, fewer: Some((there, here)) });
            }
        }
        // relay-money keeps a copy before a replace only of a ledger holding more than categories:
        // categories changed here are kept too.
        if !store.holds_anything() && categories_changed(store) {
            store.ledger.snapshot().map_err(err)?;
        }
        let out = local_page(store, true, 0, &answer.changes)?;
        // A whole pull that left rows out is not where this phone stands: the next sync asks for
        // the server's rows from the start, as a merge, until they are taken.
        (out.applied, Some(out.cursor), false, Some(if ahead > 0 { 0 } else { answer.cursor }))
    } else {
        let at = mark.cursor;
        if !same_ledger(store) || at > store.ledger.cursor().map_err(err)? {
            // This ledger was replaced while the sync ran (an erase, a restore): it is not the one
            // the answer is for, and what it holds must never go up. Nothing is applied; the next
            // sync takes the server's ledger whole.
            forget(store);
            return Ok(Applied { received: 0, again: true, ask_first: false, ahead: 0, fewer: None });
        }
        let out = local_page(store, false, at, &answer.changes)?;
        if answer.not_taken > 0 {
            // Rows the server did not take: they go again next time, from where this push began.
            (out.applied, None, !out.changes.is_empty(), None)
        } else if !out.changes.is_empty() {
            // Rows numbered after `at` that this answer did not bring were edited here meanwhile.
            (out.applied, Some(at), true, None)
        } else {
            (out.applied, Some(out.cursor), false, (ahead == 0).then_some(answer.cursor))
        }
    };
    let local_gen = store.ledger.generation().map_err(err)?;
    store.holding_prefs(|store| {
        let first = store.pref(CURSOR).is_none();
        let mut save = |k: &str, v: &str| store.set_pref(k, v).map_err(err);
        if let Some(cursor) = cursor.or(first.then_some(answer.cursor)) {
            save(CURSOR, &cursor.to_string())?;
        }
        save(GENERATION, answer.generation.as_str())?;
        if let Some(pushed) = pushed {
            save(PUSHED, &pushed.to_string())?;
        }
        save(LOCAL_GENERATION, &local_gen)?;
        // Last: a merge needs it, and it says the server's ledger is here.
        save(PULLED, "1")
    })?;
    Ok(Applied { received, again, ask_first: false, ahead, fewer: None })
}

/// The next sync's ask: a merge of what changed here once this phone holds the server's ledger,
/// its whole ledger before, or after this phone's ledger was replaced here. Returns where the
/// local ledger stood when it was read.
pub fn next_ask(store: &mut Store) -> Result<(Ask, Mark), Failure> {
    let Some(generation) = Generation::saved(store) else { return Ok((Ask::Whole, Mark::now(store)?)) };
    if !same_ledger(store) {
        forget(store);
        return Ok((Ask::Whole, Mark::now(store)?));
    }
    let since = store.pref(CURSOR).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let (changes, at) = collect(store)?;
    let mark = Mark { cursor: at, generation: store.ledger.generation().map_err(err)? };
    Ok((Ask::Merge { since, generation, changes }, mark))
}

/// Forget where sync stood with the server (a new pairing, or a disconnect): the next sync takes
/// the server's ledger whole again.
pub fn forget(store: &mut Store) {
    store.holding_prefs(|store| {
        for k in [PULLED, GENERATION, CURSOR, PUSHED, LOCAL_GENERATION] {
            let _ = store.clear_pref(k);
        }
    });
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use relay_money::ledger::Ledger;
    use std::time::Duration;
    use relay_money::views::{TxInput, TxQuery};
    use relay_money::model::TxType;

    /// Relay's `money.sync` handler, as the server runs it, over a ledger of its own.
    pub struct FakeServer {
        pub ledger: Ledger,
        pub replaces: usize,
        pub requests: Vec<Value>,
        pub pages_known: bool,
    }

    impl Default for FakeServer {
        fn default() -> FakeServer {
            FakeServer::new()
        }
    }

    impl FakeServer {
        pub fn new() -> FakeServer {
            FakeServer { ledger: Ledger::open_in_memory().unwrap(), replaces: 0, requests: Vec::new(), pages_known: true }
        }

        /// A server holding the owner's ledger: the sample household plus one entry of theirs.
        pub fn with_owners_ledger() -> FakeServer {
            let mut s = FakeServer::new();
            s.ledger.ensure_categories().unwrap();
            s.ledger.load_sample(crate::model::today(), "CAD").unwrap();
            let account = s.ledger.accounts().unwrap()[0].id;
            s.ledger
                .tx_add(&TxInput { r#type: TxType::Expense, amount: 12_34, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: Some("The owner's".into()) })
                .unwrap();
            s
        }

        pub fn handle(&mut self, op: &str, p: &Value) -> Result<Value, Failure> {
            assert_eq!(op, "money.sync");
            self.requests.push(p.clone());
            if !self.pages_known && p.get("limit").is_some() {
                return Err(fail(SCHEMA, "unknown field limit"));
            }
            // As relay-core reads it: `replace` absent is false.
            let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(false);
            if replace {
                self.replaces += 1;
            }
            let changes: Vec<Change> = serde_json::from_value(p["changes"].clone()).unwrap();
            let limit = p.get("limit").and_then(Value::as_u64).map(|l| l as usize);
            let out = self
                .ledger
                .sync_page(p["device"].as_str().unwrap(), replace, p["since"].as_i64().unwrap(), p.get("generation").and_then(Value::as_str), &changes, limit, "t")
                .map_err(|e| match e {
                    relay_money::ledger::LedgerError::Stale(m) => fail(STALE, m),
                    e => fail("money.invalid", e.to_string()),
                })?;
            Ok(serde_json::to_value(out).unwrap())
        }

        pub fn entries(&self) -> Vec<String> {
            all_uids(&self.ledger)
        }
    }

    impl Requester for FakeServer {
        fn request(&mut self, op: &str, payload: &Value, _wait: Duration) -> Result<Value, Failure> {
            self.handle(op, payload)
        }
    }

    /// Every live entry's uid, sorted.
    pub fn all_uids(l: &Ledger) -> Vec<String> {
        let q = TxQuery { limit: Some(100_000), from: Some(relay_money::Date::constant(2000, 1, 1)), to: Some(relay_money::Date::constant(2100, 1, 1)), ..Default::default() };
        let mut v: Vec<String> = l.tx_list(&q, crate::model::today()).unwrap().transactions.into_iter().map(|t| t.uid).collect();
        v.sort();
        v
    }

    fn sync_once(phone: &mut Store, server: &mut FakeServer) -> Applied {
        let (ask, at) = next_ask(phone).unwrap();
        let answer = exchange(server, "Tally RS", ask).unwrap();
        apply_store(phone, &answer, at)
    }

    pub fn apply_store(store: &mut Store, answer: &Answer, at: Mark) -> Applied {
        apply(store, answer, &at, false).unwrap()
    }

    fn phone_with_sample() -> Store {
        let mut s = Store::in_memory().unwrap();
        s.load_sample(crate::model::today()).unwrap();
        s
    }

    #[test]
    fn the_payload_never_carries_replace() {
        for (generation, changes) in [(None, vec![]), (Some("g"), vec![Change { table: "accounts".into(), uid: "a".into(), updated_at: 1, deleted: false, row: Default::default() }])] {
            let v = serde_json::to_value(SyncIn { device: "d", since: 0, generation, changes: &changes, limit: Some(PAGE) }).unwrap();
            assert!(v.get("replace").is_none(), "{v}");
        }
    }

    #[test]
    fn a_first_sync_pulls_and_leaves_the_servers_ledger_as_it_was() {
        let mut server = FakeServer::with_owners_ledger();
        let before = server.entries();
        let before_cursor = server.ledger.cursor().unwrap();
        let mut phone = phone_with_sample();
        let (ask, _) = next_ask(&mut phone).unwrap();
        assert!(matches!(ask, Ask::Whole), "nothing pulled yet: the only ask is the whole ledger");
        let applied = sync_once(&mut phone, &mut server);
        assert!(applied.received > 50);
        assert_eq!(server.replaces, 0);
        assert!(server.requests.iter().all(|r| r.get("replace").is_none() && r["changes"].as_array().unwrap().is_empty()), "the first sync sends nothing");
        assert_eq!(server.entries(), before, "the server's ledger is untouched");
        // Only the device record moves the server's counter.
        assert!(server.ledger.cursor().unwrap() <= before_cursor + 1);
        assert_eq!(all_uids(&phone.ledger), all_uids(&server.ledger), "this phone now holds the server's ledger, its sample gone");
        assert!(phone.pref(PULLED).is_some());
    }

    #[test]
    fn after_the_pull_a_change_here_reaches_the_server_and_one_there_comes_back() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = phone_with_sample();
        sync_once(&mut phone, &mut server);
        // Nothing changed either side: nothing goes, nothing comes.
        let quiet = sync_once(&mut phone, &mut server);
        assert_eq!(quiet, Applied { received: 0, again: false, ask_first: false, ahead: 0, fewer: None });
        assert!(server.requests.last().unwrap()["changes"].as_array().unwrap().is_empty(), "the rows the pull wrote are not sent back");
        let account = phone.ledger.accounts().unwrap()[0].id;
        let mine = phone.add(&TxInput { r#type: TxType::Expense, amount: 5_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: Some("From Tally RS".into()) }).unwrap();
        let theirs_account = server.ledger.accounts().unwrap()[0].id;
        let theirs = server.ledger.tx_add(&TxInput { r#type: TxType::Income, amount: 7_00, date: crate::model::today().to_string(), account_id: theirs_account, to_account_id: None, category_id: None, note: None }).unwrap();
        let n = server.entries().len();
        let applied = sync_once(&mut phone, &mut server);
        assert_eq!(applied.received, 1);
        assert_eq!(server.replaces, 0);
        assert_eq!(server.entries().len(), n + 1);
        assert!(server.entries().contains(&mine.uid));
        assert!(all_uids(&phone.ledger).contains(&theirs.uid));
        assert_eq!(all_uids(&phone.ledger), all_uids(&server.ledger));
        // A delete here travels too.
        phone.delete(mine.id).unwrap();
        sync_once(&mut phone, &mut server);
        assert!(!server.entries().contains(&mine.uid));
        assert_eq!(server.replaces, 0);
    }

    /// Another device replaces the server's ledger (the Kotlin app, re-paired).
    fn replace_servers_ledger(server: &mut FakeServer) {
        let mut other = FakeServer::new();
        other.ledger.ensure_categories().unwrap();
        other.ledger.load_sample(crate::model::today(), "USD").unwrap();
        let rows = other.ledger.sync("x", false, 0, None, &[], "t").unwrap().changes;
        server.ledger.sync("Kotlin", true, 0, None, &rows, "t").unwrap();
    }

    #[test]
    fn a_server_whose_ledger_was_replaced_is_taken_whole_not_merged_into() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        replace_servers_ledger(&mut server);
        // Nothing unsent here: nothing is lost, so it is taken by itself.
        let (ask, at) = next_ask(&mut phone).unwrap();
        let answer = exchange(&mut server, "Tally RS", ask).unwrap();
        assert!(answer.whole);
        assert_eq!(answer.note.as_deref(), Some(TOOK_SERVER_LEDGER));
        apply_store(&mut phone, &answer, at);
        assert_eq!(all_uids(&phone.ledger), all_uids(&server.ledger));
    }

    #[test]
    fn a_replaced_server_ledger_is_not_taken_over_changes_never_sent() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        replace_servers_ledger(&mut server);
        let account = phone.ledger.accounts().unwrap()[0].id;
        let mine = phone.add(&TxInput { r#type: TxType::Expense, amount: 1_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None }).unwrap();
        let (ask, _) = next_ask(&mut phone).unwrap();
        let refused = exchange(&mut server, "Tally RS", ask).unwrap_err();
        assert_eq!(refused.code, STALE_UNSENT, "the person is asked; nothing here is replaced by itself");
        assert!(all_uids(&phone.ledger).contains(&mine.uid));
        assert!(!server.entries().contains(&mine.uid));
        assert_eq!(server.replaces, 0);
    }

    #[test]
    fn a_long_ledger_comes_in_pages_and_an_old_server_whole() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = Store::in_memory().unwrap();
        let mut r = PagedBy(&mut server, 7);
        let answer = exchange(&mut r, "Tally RS", Ask::Whole).unwrap();
        assert!(server.requests.len() > 5);
        let mark = Mark::now(&phone).unwrap();
        apply_store(&mut phone, &answer, mark);
        assert_eq!(all_uids(&phone.ledger), all_uids(&server.ledger));

        let mut old = FakeServer::with_owners_ledger();
        old.pages_known = false;
        let answer = exchange(&mut old, "Tally RS", Ask::Whole).unwrap();
        assert_eq!(old.requests.len(), 2, "refused with a limit, then asked whole");
        let mut phone = Store::in_memory().unwrap();
        let mark = Mark::now(&phone).unwrap();
        apply_store(&mut phone, &answer, mark);
        assert_eq!(all_uids(&phone.ledger), all_uids(&old.ledger));
    }

    /// A server that pages at `limit` whatever the device asked.
    struct PagedBy<'a>(&'a mut FakeServer, u64);

    impl Requester for PagedBy<'_> {
        fn request(&mut self, op: &str, payload: &Value, _wait: Duration) -> Result<Value, Failure> {
            let mut p = payload.clone();
            p["limit"] = Value::from(self.1);
            self.0.handle(op, &p)
        }
    }

    #[test]
    fn an_edit_made_while_a_sync_runs_goes_next_time() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        let (ask, at) = next_ask(&mut phone).unwrap();
        let answer = exchange(&mut server, "Tally RS", ask).unwrap();
        // The person adds an entry while the answer is on its way.
        let account = phone.ledger.accounts().unwrap()[0].id;
        let meanwhile = phone.add(&TxInput { r#type: TxType::Expense, amount: 3_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None }).unwrap();
        let applied = apply_store(&mut phone, &answer, at);
        assert!(applied.again);
        sync_once(&mut phone, &mut server);
        assert!(server.entries().contains(&meanwhile.uid));
    }

    /// How many rows, and how many deletions, the phone has sent the server so far.
    fn sent(server: &FakeServer) -> (usize, usize) {
        let all: Vec<&Value> = server.requests.iter().flat_map(|r| r["changes"].as_array().unwrap().iter()).collect();
        (all.len(), all.iter().filter(|c| c["deleted"] == Value::Bool(true)).count())
    }

    /// A ledger in a file of its own: the second connection is there, as on the phone.
    pub fn file_store() -> (Store, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("tally-sync-ledger-{}", uuid::Uuid::new_v4()));
        (Store::open(&dir.join("tally.db")).unwrap(), dir)
    }

    #[test]
    fn an_erase_on_a_synced_phone_sends_nothing_and_takes_the_servers_ledger_back() {
        let mut server = FakeServer::with_owners_ledger();
        let before = server.entries();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        let (rows_before, _) = sent(&server);
        // Erase everything, as Settings does, and sync without being told: the clear's
        // tombstones never go.
        phone.ledger.reset().unwrap();
        phone.ledger.ensure_categories().unwrap();
        let (ask, _) = next_ask(&mut phone).unwrap();
        assert!(matches!(ask, Ask::Whole), "a ledger replaced here only pulls");
        assert!(collect(&mut phone).is_err() || phone.pref(PULLED).is_none());
        sync_once(&mut phone, &mut server);
        sync_once(&mut phone, &mut server);
        assert_eq!(sent(&server), (rows_before, 0), "not one row, and not one tombstone, went up");
        assert_eq!(server.entries(), before);
        assert_eq!(server.replaces, 0);
        assert_eq!(all_uids(&phone.ledger), before, "the server's ledger came back here");
    }

    #[test]
    fn a_restore_on_a_synced_phone_sends_nothing() {
        let mut server = FakeServer::with_owners_ledger();
        let before = server.entries();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        let mut other = Store::in_memory().unwrap();
        other.load_sample(crate::model::today()).unwrap();
        let file = other.ledger.export_backup("t").unwrap();
        phone.ledger.import_backup(&file).unwrap();
        // As Settings does after it, and without it.
        for forget_first in [true, false] {
            if forget_first {
                forget(&mut phone);
            }
            sync_once(&mut phone, &mut server);
            assert_eq!(sent(&server).1, 0, "no tombstone went up");
            assert_eq!(server.entries(), before);
            phone.ledger.import_backup(&file).unwrap();
        }
        assert_eq!(sent(&server).0, 0, "nothing at all went up");
        assert_eq!(server.replaces, 0);
    }

    #[test]
    fn an_erase_while_a_sync_runs_drops_its_answer_and_sends_nothing() {
        let mut server = FakeServer::with_owners_ledger();
        let before = server.entries();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        let (ask, at) = next_ask(&mut phone).unwrap();
        let answer = exchange(&mut server, "Tally RS", ask).unwrap();
        phone.ledger.reset().unwrap();
        let applied = apply_store(&mut phone, &answer, at);
        assert_eq!(applied, Applied { received: 0, again: true, ask_first: false, ahead: 0, fewer: None });
        sync_once(&mut phone, &mut server);
        assert_eq!(sent(&server), (0, 0));
        assert_eq!(server.entries(), before);
        assert_eq!(all_uids(&phone.ledger), before);
    }

    #[test]
    fn bills_still_post_here_after_a_sync() {
        let mut server = FakeServer::with_owners_ledger();
        let (mut phone, dir) = file_store();
        sync_once(&mut phone, &mut server);
        sync_once(&mut phone, &mut server);
        assert!(phone.ledger.synced_devices().unwrap().is_empty(), "the server is not recorded as a device here");
        let today = crate::model::today();
        let account = phone.ledger.accounts().unwrap()[0].id;
        let bill = crate::plan_store::BillInput {
            name: "Phone plan".into(),
            kind: TxType::Expense,
            amount: 55_00,
            account_id: account,
            to_account_id: None,
            category_id: None,
            frequency: relay_money::recurrence::Frequency::Monthly,
            interval: 1,
            anchor: today,
            auto_post: true,
            active: true,
        };
        crate::plan_store::bill_save(phone.sql().unwrap(), None, &bill, today).unwrap();
        assert!(phone.ledger.post_due(today).unwrap() >= 1, "the bill due today posted");
        // And the posted entry goes up on the next sync, as an edit here does.
        let n = server.entries().len();
        sync_once(&mut phone, &mut server);
        assert!(server.entries().len() > n);
        drop(phone);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_ledger_synced_before_this_build_posts_bills_again_once_opened() {
        let (phone, dir) = file_store();
        phone.sql().unwrap().execute("INSERT INTO settings (key, value, updated_at) VALUES ('sync.device.Your server', 't', 0)", []).unwrap();
        assert!(!phone.ledger.synced_devices().unwrap().is_empty());
        drop(phone);
        let phone = Store::open(&dir.join("tally.db")).unwrap();
        assert!(phone.ledger.synced_devices().unwrap().is_empty());
        drop(phone);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_answer_that_says_replaced_is_refused() {
        struct Liar;
        impl Requester for Liar {
            fn request(&mut self, _op: &str, _p: &Value, _w: Duration) -> Result<Value, Failure> {
                Ok(serde_json::json!({"cursor": 5, "generation": "g", "changes": [], "replaced": true, "applied": 0, "skipped": 0}))
            }
        }
        assert!(exchange(&mut Liar, "Tally RS", Ask::Whole).is_err());
    }
}
