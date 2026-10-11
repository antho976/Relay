//! The PC's cloud link (docs/CLOUD.md): this engine paired, like a phone, with a cloud engine — a
//! headless `relay serve --remote --remote-ops cloud` on an always-on server, reached through a
//! rendezvous — that Tally and Avex sync with while the PC is off. Every round, the link merges
//! the two ledgers and brings the newest training history home.
//!
//! The link keeps no ledger of its own. It is a device to both sides, speaking the same
//! `money.sync` Tally does (docs/MONEY.md, "Sync"): it asks this PC for its changes, hands them to
//! the cloud with its own cursor there, and hands the cloud's answer back to this PC. Newest edit
//! wins per row on each side, so the two converge. The wholesale acts keep their meaning: a ledger
//! restored, erased or replaced on one side (`money.sync_stale` on the next round) replaces the
//! other side's whole, as it would a phone's.

use crate::registry::{normalize_code, random_hex, sha256_hex};
use crate::wire::{self, Greeting, Hello, Welcome, WIRE_V};
use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use relay_bus::envelope::{Actor, Request, Response};
use relay_money::views::{Change, SyncOut};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// How often a running engine merges with its cloud copy.
pub const EVERY: Duration = Duration::from_secs(60);

/// Changes per `money.sync` request. A first merge carries the whole ledger, and the rendezvous
/// takes at most 8 MiB from a device in one message; a few hundred bytes a row keeps a batch well
/// under that.
const BATCH: usize = 2000;

/// How many times one round passes changes back and forth before leaving the rest to the next
/// round: an edit made on the PC while a round runs is sent on in the same round.
const PASSES: usize = 4;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);
/// A pairing may wait on a person approving it at the cloud's terminal: its door gives them 75 s.
const PAIR_TIMEOUT: Duration = Duration::from_secs(90);

/// What one side last gave the link: the cursor and generation of its last `money.sync` answer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Side {
    pub since: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
}

/// `cloud.json`, beside `remote.json`: the cloud engine this PC is paired with, its credential,
/// and where the last round left each side.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub v: u32,
    /// `wss://server/relay/join/<room>`: the cloud engine's room on its rendezvous.
    pub url: String,
    /// The cloud engine's name, as its greeting gave it.
    pub host: String,
    pub host_id: String,
    pub device: String,
    /// The credential the cloud engine minted at pairing; never sent again (a proof is).
    pub token: String,
    #[serde(default)]
    pub pc: Side,
    #[serde(default)]
    pub cloud: Side,
    /// sha256 of the last Avex export brought home.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gym_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ok: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Link {
    pub fn path_for(instance: relay_core::Instance) -> PathBuf {
        crate::Registry::path_for(instance).with_file_name("cloud.json")
    }

    /// The link, or `None` when this PC has none.
    pub fn load(path: &Path) -> Result<Option<Link>> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Written whole and renamed in, readable by this user alone: it holds a credential.
    pub fn save(&self, path: &Path) -> Result<()> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let tmp = dir.join(format!(".cloud.json.{}.{}.tmp", std::process::id(), random_hex(6)));
        let written = (|| -> Result<()> {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
            f.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            std::fs::rename(&tmp, path)?;
            Ok(())
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written.with_context(|| format!("writing {}", path.display()))
    }
}

/// Why a call failed: the other side refused it (a bus error), or the link itself broke.
#[derive(Debug, thiserror::Error)]
pub enum Fail {
    #[error("{code}: {message}")]
    Refused { code: String, message: String },
    #[error(transparent)]
    Link(#[from] anyhow::Error),
}

impl Fail {
    fn is(&self, wanted: &str) -> bool {
        matches!(self, Fail::Refused { code, .. } if code == wanted)
    }
}

/// One side of the link: something that answers bus requests as the person.
#[allow(async_fn_in_trait)]
pub trait Bus {
    async fn call(&mut self, op: &str, payload: Value) -> std::result::Result<Value, Fail>;
}

fn answer(resp: Response) -> std::result::Result<Value, Fail> {
    match resp.into_result() {
        Ok(v) => Ok(v),
        Err(e) => Err(Fail::Refused { code: e.code.to_string(), message: e.message.to_string() }),
    }
}

/// This PC's engine, through its socket door.
pub struct Local(relay_core::socket::Client);

impl Local {
    pub async fn connect(socket: &Path) -> Result<Local> {
        let client = relay_core::socket::Client::connect(socket)
            .await
            .with_context(|| format!("no engine at {}", socket.display()))?;
        Ok(Local(client))
    }
}

impl Bus for Local {
    async fn call(&mut self, op: &str, payload: Value) -> std::result::Result<Value, Fail> {
        let req = Request::new(Actor::User, op, payload);
        let resp = tokio::time::timeout(REQUEST_TIMEOUT, self.0.call(&req, |_| {}))
            .await
            .map_err(|_| anyhow!("this PC's engine did not answer {op} in {}s", REQUEST_TIMEOUT.as_secs()))??;
        answer(resp)
    }
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The cloud engine, through its rendezvous: an admitted connection, as a phone holds one.
pub struct Remote {
    ws: Ws,
    pub greeting: Greeting,
}

impl Remote {
    /// Dial `url`, read the greeting, answer it with what `hello` makes of it, and wait for the
    /// welcome. A denial is final; the caller says what it means.
    async fn open(url: &str, hello: impl FnOnce(&Greeting) -> Hello, wait: Duration) -> Result<(Remote, Welcome)> {
        crate::tunnel::ensure_crypto_provider();
        let (mut ws, _) = tokio::time::timeout(HANDSHAKE_TIMEOUT, tokio_tungstenite::connect_async(url))
            .await
            .map_err(|_| anyhow!("{url} did not answer in {}s", HANDSHAKE_TIMEOUT.as_secs()))?
            .with_context(|| format!("dialing {url}"))?;
        let first = next_text(&mut ws, HANDSHAKE_TIMEOUT).await?;
        let greeting: Greeting = match serde_json::from_str(&first) {
            Ok(g) => g,
            Err(_) => {
                // A rendezvous with no host in the room answers `{ok:false, error}` instead.
                let refusal = serde_json::from_str::<Value>(&first).ok().and_then(|v| v["error"].as_str().map(String::from));
                return Err(match refusal.as_deref() {
                    Some("host.offline") => anyhow!("the cloud engine is not connected to its rendezvous right now"),
                    Some(code) => anyhow!("the rendezvous refused: {code}"),
                    None => anyhow!("something answered at {url}, but not Relay"),
                });
            }
        };
        ws.send(Message::text(serde_json::to_string(&hello(&greeting))?)).await?;
        let welcome: Welcome = serde_json::from_str(&next_text(&mut ws, wait).await?).context("reading the welcome")?;
        if !welcome.ok {
            return Err(anyhow!("{}", denial(welcome.error.as_deref())));
        }
        Ok((Remote { ws, greeting }, welcome))
    }

    /// Connect with the link's credential.
    pub async fn connect(link: &Link) -> Result<Remote> {
        let (device, token) = (link.device.clone(), link.token.clone());
        let hello = move |g: &Greeting| Hello { v: WIRE_V, device: Some(device), proof: Some(wire::proof(&g.challenge, &token)), ..Default::default() };
        Ok(Remote::open(&link.url, hello, HANDSHAKE_TIMEOUT).await?.0)
    }
}

/// What the door's refusal codes mean here (`bridge.rs`).
fn denial(code: Option<&str>) -> String {
    match code {
        Some("pair.invalid") => "the pairing code was wrong or has expired: run `relay remote pair --via-only` on the server again".into(),
        Some("pair.declined") => "the server declined the pairing".into(),
        Some("pair.unconfirmed") => "nobody approved the pairing on the server in time".into(),
        Some("auth.unknown_device") | Some("auth.bad_proof") => {
            "the cloud engine no longer knows this PC: pair it again with `relay cloud pair`".into()
        }
        Some(code) => format!("the cloud engine refused the connection ({code})"),
        None => "the cloud engine refused the connection".into(),
    }
}

async fn next_text(ws: &mut Ws, wait: Duration) -> Result<String> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let msg = tokio::time::timeout_at(deadline, ws.next())
            .await
            .map_err(|_| anyhow!("no answer in {}s", wait.as_secs()))?;
        match msg {
            None | Some(Ok(Message::Close(_))) => anyhow::bail!("the connection closed"),
            Some(Err(e)) => return Err(e.into()),
            Some(Ok(Message::Text(t))) => return Ok(t.to_string()),
            Some(Ok(Message::Binary(b))) => return Ok(String::from_utf8_lossy(&b).into_owned()),
            Some(Ok(_)) => {}
        }
    }
}

impl Bus for Remote {
    async fn call(&mut self, op: &str, payload: Value) -> std::result::Result<Value, Fail> {
        let req = Request::new(Actor::User, op, payload);
        self.ws.send(Message::text(serde_json::to_string(&req).map_err(anyhow::Error::from)?)).await.map_err(anyhow::Error::from)?;
        let deadline = tokio::time::Instant::now() + REQUEST_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            let text = next_text(&mut self.ws, left).await.with_context(|| format!("waiting for {op} from the cloud engine"))?;
            // A frame may hold several lines; events and other answers are skipped.
            for line in text.lines() {
                let Ok(resp) = serde_json::from_str::<Response>(line) else { continue };
                if resp.id == Some(req.id) || resp.id.is_none() {
                    return answer(resp);
                }
            }
        }
    }
}

/// What a `relay://pair` link says about where to reach an engine. Only the rendezvous is used:
/// a cloud engine's own addresses are its container's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairTarget {
    pub code: String,
    pub via: String,
}

impl PairTarget {
    pub fn parse(link: &str) -> Result<PairTarget> {
        let rest = link
            .trim()
            .strip_prefix("relay://pair?")
            .ok_or_else(|| anyhow!("that is not a relay://pair link: copy the whole link `relay remote pair --via-only` printed"))?;
        let mut code = None;
        let mut via = None;
        for pair in rest.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            match k {
                "code" => code = Some(decode(v)),
                "via" => via = Some(decode(v)),
                _ => {}
            }
        }
        let code = code.filter(|c| !c.trim().is_empty()).ok_or_else(|| anyhow!("the link has no pairing code"))?;
        let via = via
            .filter(|v| v.starts_with("ws://") || v.starts_with("wss://"))
            .ok_or_else(|| anyhow!("the link has no rendezvous (`via=`): set one on the server with `relay remote via` first"))?;
        Ok(PairTarget { code, via })
    }
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        match (hex, bytes[i]) {
            (Some(b), _) => {
                out.push(b);
                i += 3;
            }
            (None, b'+') => {
                out.push(b' ');
                i += 1;
            }
            (None, b) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Pair this PC with the cloud engine whose pairing link this is, and write `cloud.json`.
pub async fn pair(path: &Path, link: &str, name: &str) -> Result<Link> {
    let target = PairTarget::parse(link)?;
    let hello = |_: &Greeting| Hello { v: WIRE_V, pair: Some(normalize_code(&target.code)), device_name: Some(name.to_string()), ..Default::default() };
    let (remote, welcome) = Remote::open(&target.via, hello, PAIR_TIMEOUT).await?;
    let (Some(device), Some(token)) = (welcome.device, welcome.token) else {
        anyhow::bail!("the cloud engine admitted this PC without a credential");
    };
    let link = Link {
        v: 1,
        url: target.via,
        host: remote.greeting.host.clone(),
        host_id: remote.greeting.host_id.clone(),
        device,
        token,
        pc: Side::default(),
        cloud: Side::default(),
        gym_digest: None,
        last_ok: None,
        last_error: None,
    };
    link.save(path)?;
    Ok(link)
}

/// What one round did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Rows this PC took from the cloud copy, and the cloud copy from this PC.
    pub to_pc: usize,
    pub to_cloud: usize,
    /// `pc` or `cloud`: that side's ledger was replaced wholesale since the last round, and
    /// replaced the other's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<&'static str>,
    /// A newer Avex export came home.
    pub gym: bool,
}

/// `money.sync` with the changes split into batches, in dependency order across them. Answers
/// with the last batch's cursor and generation and every change the side sent back.
async fn send(bus: &mut impl Bus, device: &str, replace: bool, side: &Side, mut changes: Vec<Change>) -> std::result::Result<SyncOut, Fail> {
    changes.sort_by_key(|c| relay_money::sync::order(&c.table));
    let mut since = side.since;
    let mut generation = side.generation.clone();
    let mut first = true;
    let mut answers: Vec<Change> = Vec::new();
    let (mut applied, mut skipped) = (0, 0);
    let mut chunks: Vec<Vec<Change>> = changes.chunks(BATCH).map(<[Change]>::to_vec).collect();
    if chunks.is_empty() {
        chunks.push(Vec::new());
    }
    let mut last = None;
    for chunk in chunks {
        let payload = json!({
            "device": device,
            "replace": replace && first,
            "since": since,
            "generation": if replace && first { None } else { generation.clone() },
            "changes": chunk,
        });
        let out: SyncOut = serde_json::from_value(bus.call("money.sync", payload).await?).map_err(anyhow::Error::from)?;
        since = out.cursor;
        generation = Some(out.generation.clone());
        applied += out.applied;
        skipped += out.skipped;
        answers.extend(out.changes.iter().cloned());
        first = false;
        last = Some(out);
    }
    let mut out = last.expect("one batch at least");
    out.changes = answers;
    out.applied = applied;
    out.skipped = skipped;
    Ok(out)
}

fn side_of(out: &SyncOut) -> Side {
    Side { since: out.cursor, generation: Some(out.generation.clone()) }
}

/// `from`'s whole ledger replaces `to`'s: the wholesale act made on `from` reaches `to`. Each
/// side is asked under the name it lists the link by. Answers both sides' new places and how many
/// rows went.
async fn replace_whole(
    from: &mut impl Bus,
    from_device: &str,
    to: &mut impl Bus,
    to_device: &str,
) -> std::result::Result<(Side, Side, usize), Fail> {
    let whole = send(from, from_device, false, &Side::default(), Vec::new()).await?;
    let n = whole.changes.len();
    let taken = send(to, to_device, true, &Side::default(), whole.changes.clone()).await?;
    Ok((side_of(&whole), side_of(&taken), n))
}

/// One merge of the two ledgers. `pc_name` is how the cloud lists this PC, `cloud_name` how this
/// PC lists the cloud copy. `link`'s sides move only as far as the round got.
pub async fn merge_money(pc: &mut impl Bus, cloud: &mut impl Bus, link: &mut Link, pc_name: &str, cloud_name: &str) -> std::result::Result<Report, Fail> {
    let mut report = Report::default();
    // This PC's changes since the last round.
    let ours = match send(pc, cloud_name, false, &link.pc, Vec::new()).await {
        Ok(out) => out,
        Err(e) if e.is("money.sync_stale") => {
            // Restored, erased or replaced on this PC: it goes to the cloud whole.
            let (pc_side, cloud_side, n) = replace_whole(pc, cloud_name, cloud, pc_name).await?;
            (link.pc, link.cloud) = (pc_side, cloud_side);
            report.to_cloud = n;
            report.replaced_by = Some("pc");
            return Ok(report);
        }
        Err(e) => return Err(e),
    };
    let mut pc_side = side_of(&ours);
    let mut changes = ours.changes;
    for pass in 0..PASSES {
        let theirs = match send(cloud, pc_name, false, &link.cloud, changes).await {
            Ok(out) => out,
            Err(e) if pass == 0 && e.is("money.sync_stale") => {
                // Tally's first sync, or a restore, replaced the cloud's ledger: it comes here whole.
                let (cloud_side, pc_side, n) = replace_whole(cloud, pc_name, pc, cloud_name).await?;
                (link.pc, link.cloud) = (pc_side, cloud_side);
                report.to_pc = n;
                report.replaced_by = Some("cloud");
                return Ok(report);
            }
            Err(e) => return Err(e),
        };
        report.to_cloud += theirs.applied;
        link.cloud = side_of(&theirs);
        if theirs.changes.is_empty() {
            break;
        }
        let back = send(pc, cloud_name, false, &pc_side, theirs.changes).await?;
        report.to_pc += back.applied;
        // What this PC answers with is an edit made here while the round ran. It goes on to the
        // cloud in the next pass; past the last pass, this PC's cursor stays where it was, so the
        // next round asks for it again (what it sends twice, the other side already has).
        if !back.changes.is_empty() && pass + 1 == PASSES {
            break;
        }
        pc_side = side_of(&back);
        if back.changes.is_empty() {
            break;
        }
        changes = back.changes;
    }
    link.pc = pc_side;
    Ok(report)
}

/// Bring the newest Avex export home, when the cloud holds one this PC has not taken.
pub async fn fetch_gym(pc: &mut impl Bus, cloud: &mut impl Bus, link: &mut Link) -> std::result::Result<bool, Fail> {
    let raw = match cloud.call("gym.raw", json!({"have": link.gym_digest})).await {
        Ok(raw) => raw,
        // A cloud engine from before the upload: nothing to bring.
        Err(e) if e.is("bus.unknown_op") => return Ok(false),
        Err(e) => return Err(e),
    };
    let (Some(digest), Some(text)) = (raw["digest"].as_str(), raw["json"].as_str()) else {
        return Ok(false);
    };
    if link.gym_digest.as_deref() == Some(digest) || sha256_hex(text) != digest {
        return Ok(false);
    }
    pc.call("gym.upload", json!({"json": text, "name": "avex_export.json"})).await?;
    link.gym_digest = Some(digest.to_string());
    Ok(true)
}

/// Serializes rounds between this engine's loop and `relay cloud sync`.
fn lock(path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let lock_path = path.with_file_name("cloud.json.lock");
    let f = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(&lock_path)
        .with_context(|| format!("opening {}", lock_path.display()))?;
    f.lock().with_context(|| format!("locking {}", lock_path.display()))?;
    Ok(f)
}

/// One round against the engine at `socket`: money both ways, then the training history home.
/// The outcome is written to `cloud.json` either way, for `relay cloud status`.
pub async fn round(socket: &Path, path: &Path, pc_name: &str) -> Result<Report> {
    let _lock = tokio::task::spawn_blocking({
        let path = path.to_path_buf();
        move || lock(&path)
    })
    .await??;
    let Some(mut link) = Link::load(path)? else {
        anyhow::bail!("this PC has no cloud link: `relay cloud pair '<relay://pair link>'` makes one");
    };
    let outcome = async {
        let mut pc = Local::connect(socket).await?;
        let mut cloud = Remote::connect(&link).await?;
        let cloud_name = format!("Cloud ({})", link.host);
        let mut report = merge_money(&mut pc, &mut cloud, &mut link, pc_name, &cloud_name).await.map_err(anyhow::Error::from)?;
        report.gym = fetch_gym(&mut pc, &mut cloud, &mut link).await.map_err(anyhow::Error::from)?;
        anyhow::Ok(report)
    }
    .await;
    let now = jiff::Timestamp::now().to_string();
    match &outcome {
        Ok(_) => {
            link.last_ok = Some(now);
            link.last_error = None;
        }
        Err(e) => link.last_error = Some(format!("{now}: {e:#}")),
    }
    link.save(path)?;
    outcome
}

/// Merge every [`EVERY`] while the engine runs, starting now. Failures are logged and kept in
/// `cloud.json`; the next round tries again.
pub fn spawn(socket: PathBuf, path: PathBuf, pc_name: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut failing = false;
        loop {
            tick.tick().await;
            match Link::load(&path) {
                Ok(Some(_)) => {}
                // Unpaired since (`relay cloud off`): nothing to do until the engine restarts.
                Ok(None) => return,
                Err(e) => {
                    tracing::warn!(error = %format!("{e:#}"), "cloud link unreadable; not merging");
                    continue;
                }
            }
            match round(&socket, &path, &pc_name).await {
                Ok(report) => {
                    if failing || report.to_pc + report.to_cloud > 0 || report.gym || report.replaced_by.is_some() {
                        tracing::info!(to_pc = report.to_pc, to_cloud = report.to_cloud, gym = report.gym, replaced_by = ?report.replaced_by, "cloud link merged");
                    }
                    failing = false;
                }
                Err(e) => {
                    // Once per outage at warn; the rest at debug.
                    if failing {
                        tracing::debug!(error = %format!("{e:#}"), "cloud link round failed");
                    } else {
                        tracing::warn!(error = %format!("{e:#}"), "cloud link round failed; retrying every minute");
                    }
                    failing = true;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pairing_link_gives_its_code_and_rendezvous() {
        let t = PairTarget::parse("relay://pair?v=1&code=ABCD-EFGH&host=cloud&id=x&instance=stable&via=wss%3A%2F%2Fbot.example.org%2Frelay%2Fjoin%2Fr00m").unwrap();
        assert_eq!(t, PairTarget { code: "ABCD-EFGH".into(), via: "wss://bot.example.org/relay/join/r00m".into() });
        assert!(PairTarget::parse("relay://pair?v=1&code=X&direct=ws%3A%2F%2F10.0.0.2%3A7420").is_err(), "a cloud engine is reached through its rendezvous");
        assert!(PairTarget::parse("https://example.org").is_err());
        assert_eq!(decode("a%2Fb%"), "a/b%");
    }

    use relay_money::ledger::{Ledger, LedgerError};
    use relay_money::model::AccountType;

    /// A side of the link that is just a ledger: what the engine's `money.sync` handler does.
    struct Ledgered(Ledger);

    impl Bus for Ledgered {
        async fn call(&mut self, op: &str, p: Value) -> std::result::Result<Value, Fail> {
            assert_eq!(op, "money.sync");
            let changes: Vec<Change> = serde_json::from_value(p["changes"].clone()).unwrap();
            let out = self.0.sync(
                p["device"].as_str().unwrap(),
                p["replace"].as_bool().unwrap_or(false),
                p["since"].as_i64().unwrap(),
                p["generation"].as_str(),
                &changes,
                "2026-10-11T12:00:00Z",
            );
            match out {
                Ok(out) => Ok(serde_json::to_value(out).unwrap()),
                Err(LedgerError::Stale(m)) => Err(Fail::Refused { code: "money.sync_stale".into(), message: m }),
                Err(e) => panic!("{e}"),
            }
        }
    }

    fn link() -> Link {
        Link {
            v: 1, url: "wss://x/join/r".into(), host: "cloud".into(), host_id: "h".into(), device: "d".into(), token: "t".into(),
            pc: Side::default(), cloud: Side::default(), gym_digest: None, last_ok: None, last_error: None,
        }
    }

    fn names(l: &Ledger) -> Vec<String> {
        let mut v: Vec<String> = l.accounts().unwrap().into_iter().map(|a| a.name).collect();
        v.sort();
        v
    }

    async fn merge(pc: &mut Ledgered, cloud: &mut Ledgered, link: &mut Link) -> Report {
        merge_money(pc, cloud, link, "PC (desk)", "Cloud (render)").await.unwrap()
    }

    #[tokio::test]
    async fn the_two_ledgers_converge_and_then_stay_quiet() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        pc.0.account_add("Chequing", AccountType::Chequing, 10_000, None, "").unwrap();
        let mut link = link();

        let first = merge(&mut pc, &mut cloud, &mut link).await;
        assert!(first.to_cloud > 0, "{first:?}");
        assert_eq!(names(&cloud.0), ["Chequing"]);

        // Tally adds one on the cloud while the PC is off; the PC adds one of its own.
        cloud.0.account_add("Savings", AccountType::Savings, 0, None, "").unwrap();
        pc.0.account_add("Cash", AccountType::Cash, 0, None, "").unwrap();
        let second = merge(&mut pc, &mut cloud, &mut link).await;
        assert!(second.to_pc > 0 && second.to_cloud > 0, "{second:?}");
        assert_eq!(names(&pc.0), ["Cash", "Chequing", "Savings"]);
        assert_eq!(names(&cloud.0), names(&pc.0));

        // Nothing changed: nothing moves, either way.
        let quiet = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.replaced_by), (0, 0, None), "{quiet:?}");

        // Both sides now count the link as a device, so neither posts bills behind Tally's back.
        assert!(!pc.0.synced_devices().unwrap().is_empty());
        assert!(!cloud.0.synced_devices().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_ledger_replaced_on_either_side_replaces_the_other() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        pc.0.account_add("Old", AccountType::Cash, 0, None, "").unwrap();
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(names(&cloud.0), ["Old"]);

        // Tally pairs with the cloud: its first sync makes the cloud's ledger the phone's.
        let mut phone = Ledger::open_in_memory().unwrap();
        phone.account_add("Phone", AccountType::Chequing, 0, None, "").unwrap();
        let everything = phone.sync("probe", false, 0, None, &[], "t").unwrap().changes;
        cloud.0.sync("Pixel", true, 0, None, &everything, "t").unwrap();
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        assert_eq!(names(&pc.0), ["Phone"], "the phone's ledger reached the PC whole");

        // A restore or an erase on the PC goes the other way.
        pc.0.reset().unwrap();
        pc.0.account_add("Restored", AccountType::Cash, 0, None, "").unwrap();
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("pc"));
        assert_eq!(names(&cloud.0), ["Restored"]);

        // And afterwards they merge as before.
        cloud.0.account_add("Later", AccountType::Cash, 0, None, "").unwrap();
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, None);
        assert_eq!(names(&pc.0), ["Later", "Restored"]);
    }
}
