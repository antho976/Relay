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
    pub sent: usize,
    pub note: Option<String>,
}

fn unreadable() -> Failure {
    fail(code::ANSWER, "Your server's answer did not read")
}

/// One sync on an admitted connection.
pub fn exchange(r: &mut dyn Requester, device: &str, ask: Ask) -> Result<Answer, Failure> {
    match ask {
        Ask::Whole => {
            let (cursor, generation, changes) = send(r, device, 0, None, &[])?;
            Ok(Answer { whole: true, cursor, generation, changes, sent: 0, note: None })
        }
        Ask::Merge { since, generation, changes } => match send(r, device, since, Some(&generation), &changes) {
            Ok((cursor, generation, back)) => Ok(Answer { whole: false, cursor, generation, changes: back, sent: changes.len(), note: None }),
            // The server's ledger was restored, erased or replaced since: take it whole. What
            // changed here since the last sync is not sent: the latest wholesale act wins.
            Err(f) if f.code == STALE => {
                let (cursor, generation, changes) = send(r, device, 0, None, &[])?;
                Ok(Answer { whole: true, cursor, generation, changes, sent: 0, note: Some(TOOK_SERVER_LEDGER.into()) })
            }
            Err(f) => Err(f),
        },
    }
}

/// One `money.sync` of `changes` in batches, and the server's answer to it whole, asked for in
/// pages while the server says `more`. A row edited while the pages are read comes again, newer:
/// the newest of each is kept, and what the server hands back exactly as sent is no news. A
/// server from before pages refuses `limit` before applying anything and is asked again whole.
fn send(r: &mut dyn Requester, device: &str, since: i64, generation: Option<&Generation>, changes: &[Change]) -> Result<(i64, Generation, Vec<Change>), Failure> {
    let sent: std::collections::HashMap<(&str, &str), i64> = changes.iter().map(|c| ((c.table.as_str(), c.uid.as_str()), c.updated_at)).collect();
    let mut newest: Vec<Change> = Vec::new();
    let mut at: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
    let batches: Vec<&[Change]> = changes.chunks(BATCH).collect();
    let mut cursor = since;
    let mut gen_: Option<String> = generation.map(|g| g.0.clone());
    let mut paged = true;
    let mut i = 0;
    loop {
        let batch: &[Change] = batches.get(i).copied().unwrap_or(&[]);
        let payload = |limit: Option<u32>| serde_json::to_value(SyncIn { device, since: cursor, generation: gen_.as_deref(), changes: batch, limit }).unwrap_or(Value::Null);
        let result = match r.request("money.sync", &payload(Some(PAGE)), crate::sync_net::REQUEST_TIMEOUT) {
            Err(f) if paged && f.code == SCHEMA => {
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
        let more = paged && result.get("more").and_then(Value::as_bool) == Some(true);
        if more && next <= cursor {
            return Err(unreadable());
        }
        cursor = next;
        if let Some(g) = result.get("generation").and_then(Value::as_str) {
            gen_ = Some(g.to_string());
        }
        for v in result.get("changes").and_then(Value::as_array).into_iter().flatten() {
            let Ok(c) = serde_json::from_value::<Change>(v.clone()) else { continue };
            if sent.get(&(c.table.as_str(), c.uid.as_str())) == Some(&c.updated_at) {
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
        i += 1;
        if !more && i >= batches.len() {
            break;
        }
    }
    let generation = gen_.filter(|g| !g.is_empty()).map(Generation).ok_or_else(unreadable)?;
    Ok((cursor, generation, newest))
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

/// What applying an answer did here.
#[derive(Debug, PartialEq)]
pub struct Applied {
    pub received: usize,
    /// Rows changed here while the sync ran: they go next time, so sync again soon.
    pub again: bool,
}

/// Applies the server's answer to this phone's ledger and keeps where both stand. A whole answer
/// replaces this phone's ledger (relay-money keeps a copy of it in `money-backups/` first).
/// `collected_at` is the local cursor the sent rows were read at (None for a whole sync).
pub fn apply(store: &mut Store, answer: &Answer, collected_at: Option<i64>) -> Result<Applied, Failure> {
    let (received, pushed, again) = if answer.whole {
        let out = local_page(store, true, 0, &answer.changes)?;
        (out.applied, out.cursor, false)
    } else {
        let at = collected_at.unwrap_or(0);
        if !same_ledger(store) || at > store.ledger.cursor().map_err(err)? {
            // This ledger was replaced while the sync ran (an erase, a restore): it is not the one
            // the answer is for, and what it holds must never go up. Nothing is applied; the next
            // sync takes the server's ledger whole.
            forget(store);
            return Ok(Applied { received: 0, again: true });
        }
        let out = local_page(store, false, at, &answer.changes)?;
        // Rows numbered after `at` that this answer did not bring were edited here meanwhile.
        if out.changes.is_empty() { (out.applied, out.cursor, false) } else { (out.applied, at, true) }
    };
    let local_gen = store.ledger.generation().map_err(err)?;
    store.holding_prefs(|store| {
        let mut save = |k: &str, v: &str| store.set_pref(k, v).map_err(err);
        save(CURSOR, &answer.cursor.to_string())?;
        save(GENERATION, answer.generation.as_str())?;
        save(PUSHED, &pushed.to_string())?;
        save(LOCAL_GENERATION, &local_gen)?;
        // Last: a merge needs it, and it says the server's ledger is here.
        save(PULLED, "1")
    })?;
    Ok(Applied { received, again })
}

/// The next sync's ask: a merge of what changed here once this phone holds the server's ledger,
/// its whole ledger before, or after this phone's ledger was replaced here. Returns the local
/// cursor the rows were read at.
pub fn next_ask(store: &mut Store) -> Result<(Ask, Option<i64>), Failure> {
    let Some(generation) = Generation::saved(store) else { return Ok((Ask::Whole, None)) };
    if !same_ledger(store) {
        forget(store);
        return Ok((Ask::Whole, None));
    }
    let since = store.pref(CURSOR).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let (changes, at) = collect(store)?;
    Ok((Ask::Merge { since, generation, changes }, Some(at)))
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

    pub fn apply_store(store: &mut Store, answer: &Answer, at: Option<i64>) -> Applied {
        apply(store, answer, at).unwrap()
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
        assert_eq!(quiet, Applied { received: 0, again: false });
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

    #[test]
    fn a_server_whose_ledger_was_replaced_is_taken_whole_not_merged_into() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = Store::in_memory().unwrap();
        sync_once(&mut phone, &mut server);
        // Another device replaces the server's ledger (the Kotlin app, re-paired).
        let mut other = FakeServer::new();
        other.ledger.ensure_categories().unwrap();
        other.ledger.load_sample(crate::model::today(), "USD").unwrap();
        let rows = other.ledger.sync("x", false, 0, None, &[], "t").unwrap().changes;
        server.ledger.sync("Kotlin", true, 0, None, &rows, "t").unwrap();
        let account = phone.ledger.accounts().unwrap()[0].id;
        phone.add(&TxInput { r#type: TxType::Expense, amount: 1_00, date: crate::model::today().to_string(), account_id: account, to_account_id: None, category_id: None, note: None }).unwrap();
        let (ask, at) = next_ask(&mut phone).unwrap();
        let answer = exchange(&mut server, "Tally RS", ask).unwrap();
        assert!(answer.whole);
        assert_eq!(answer.note.as_deref(), Some(TOOK_SERVER_LEDGER));
        apply_store(&mut phone, &answer, at);
        assert_eq!(all_uids(&phone.ledger), all_uids(&server.ledger));
    }

    #[test]
    fn a_long_ledger_comes_in_pages_and_an_old_server_whole() {
        let mut server = FakeServer::with_owners_ledger();
        let mut phone = Store::in_memory().unwrap();
        let mut r = PagedBy(&mut server, 7);
        let answer = exchange(&mut r, "Tally RS", Ask::Whole).unwrap();
        assert!(server.requests.len() > 5);
        apply_store(&mut phone, &answer, None);
        assert_eq!(all_uids(&phone.ledger), all_uids(&server.ledger));

        let mut old = FakeServer::with_owners_ledger();
        old.pages_known = false;
        let answer = exchange(&mut old, "Tally RS", Ask::Whole).unwrap();
        assert_eq!(old.requests.len(), 2, "refused with a limit, then asked whole");
        let mut phone = Store::in_memory().unwrap();
        apply_store(&mut phone, &answer, None);
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
        assert_eq!(applied, Applied { received: 0, again: true });
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
