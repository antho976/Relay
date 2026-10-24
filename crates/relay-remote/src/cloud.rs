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
//! other side's whole, as it would a phone's; but a cloud ledger far smaller than this PC's is held
//! for the person to decide rather than copied over this PC's ([`Held`]).

use crate::registry::{normalize_code, random_hex, sha256_hex};
use crate::wire::{self, Greeting, Hello, Welcome, WIRE_V};
use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use relay_bus::envelope::{Actor, Request, Response};
use relay_money::views::{Change, SyncOut};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// How often a running engine merges with its cloud copy when nothing wakes it sooner: between
/// rounds it listens to the cloud copy and merges as soon as a phone's data lands there ([`spawn`]).
pub const EVERY: Duration = Duration::from_secs(60);

/// How long a change the cloud copy announced waits for the rest of its burst (Tally's pages, an
/// upload right behind a sync) before the round it starts.
const SETTLE: Duration = Duration::from_millis(500);

/// How often the listening connection pings; the rendezvous drops a device silent for 90 s, and a
/// connection with no answer to three pings in a row is taken for dead.
const LISTEN_PING: Duration = Duration::from_secs(30);
const LISTEN_SILENCE: Duration = Duration::from_secs(90);

/// The longest the listener waits before dialing again after its connection failed.
const LISTEN_BACKOFF: Duration = Duration::from_secs(60);

/// Changes per `money.sync` request. A first merge carries the whole ledger, and the rendezvous
/// takes at most 8 MiB from a device in one message; a few hundred bytes a row keeps a batch well
/// under that.
const BATCH: usize = 2000;

/// Changes per page of a `money.sync` answer. A whole ledger in one answer is one line, and the
/// rendezvous closes a lane on a line past `MAX_LANE_LINE` (15 MiB): a ledger of tens of thousands
/// of prices and rates could never be taken whole off the home network otherwise.
const PAGE: usize = 2000;

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
    /// The owner's server this PC connected through (`relay cloud connect`; connect.rs), and its
    /// token there, which a running engine polls with for Relay phones the owner allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_token: Option<String>,
    /// A wholesale replace begun and not finished: the side whose ledger is going over whole.
    /// Written before the other side is cleared, so a round that stops partway is finished by the
    /// next one, never read as a restore of the half-copied ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacing: Option<Whole>,
    /// Where the side being filled stood once the replace's first batch landed there: its new
    /// generation and cursor (the side's own field moves there too, and the source's to the
    /// whole ledger it sent). A replace cut short after that is finished by merging the source's
    /// whole ledger into that side under this generation, newest edit winning, so what was edited
    /// there since stays. `None` before the first batch landed, and in a marker an older build
    /// wrote (it may not know this field): the replace then runs again whole, as it always did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed: Option<Side>,
    /// A replace from the cloud this PC would not make by itself, waiting on the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<Held>,
}

/// The cloud's ledger was replaced wholesale by one empty, or under half this PC's in accounts and
/// entries, while this PC's is not empty. Any device the cloud copy lets sync can replace its
/// ledger (an Avex included), and that would otherwise reach this PC and every phone after it, so
/// the link holds it until the person chooses (`relay cloud accept cloud|pc`, [`decide`]); each
/// round meanwhile fails with `cloud.replace_held`, kept in `last_error`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Held {
    /// The cloud ledger's generation when it was held: a later wholesale act there is looked at
    /// afresh.
    pub generation: String,
    /// Live accounts and entries on each side then.
    pub pc_rows: usize,
    pub cloud_rows: usize,
    /// Held changes rather than a replace ([`sweeping`]): how many of this PC's accounts and
    /// entries they would delete or rewrite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lost: Option<usize>,
    /// The person's choice, which the next round carries out: `cloud` takes the cloud's ledger
    /// here, `pc` sends this PC's to the cloud in its place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept: Option<Whole>,
}

impl Held {
    fn refusal(&self) -> Fail {
        if let Some(lost) = self.lost {
            return Fail::Refused {
                code: "cloud.changes_held".into(),
                message: format!(
                    "the cloud copy sent changes that would delete or rewrite {lost} of this PC's {} accounts and entries; they were not applied here. \
                     `relay cloud accept cloud` takes the cloud's ledger here; `relay cloud accept pc` sends this PC's to the cloud instead",
                    self.pc_rows
                ),
            };
        }
        Fail::Refused {
            code: "cloud.replace_held".into(),
            message: format!(
                "the cloud copy's ledger was replaced by one with {} accounts and entries, against {} on this PC; it was not copied over this PC's. \
                 `relay cloud accept cloud` takes the cloud's here; `relay cloud accept pc` sends this PC's to the cloud instead",
                self.cloud_rows, self.pc_rows
            ),
        }
    }
}

/// One side of the link, as the side whose whole ledger replaces the other's.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Whole {
    Pc,
    Cloud,
}

/// A device name the money.sync handler takes: at most 64 characters, none of them control
/// characters. The host part gives way.
pub fn device_name(label: &str, host: &str) -> String {
    let room = 64usize.saturating_sub(label.chars().count() + 3);
    let host: String = host.chars().filter(|c| !c.is_control()).take(room).collect();
    format!("{label} ({})", host.trim())
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
        // A door names its host only once the device is in; an older one named it in the greeting.
        let mut greeting = greeting;
        for (theirs, ours) in [(&welcome.host, &mut greeting.host), (&welcome.host_id, &mut greeting.host_id), (&welcome.version, &mut greeting.version)] {
            if let Some(v) = theirs.as_ref().filter(|v| !v.is_empty()) {
                ours.clone_from(v);
            }
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
        server: None,
        server_token: None,
        replacing: None,
        landed: None,
        held: None,
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

/// `money.sync` with the changes split into batches, in dependency order across them, and the
/// answer asked for in pages until the side has no more. Answers with the last request's cursor
/// and generation and every change the side sent back, the newest of each row once. A side from
/// before pages (it refuses `limit`) is asked whole, as before.
async fn send(bus: &mut impl Bus, device: &str, replace: bool, side: &Side, changes: Vec<Change>) -> std::result::Result<SyncOut, Fail> {
    send_noting(bus, device, replace, side, changes, &mut |_| Ok(())).await
}

/// [`send`], telling `first` where the side stands once the first request's answer is in: for a
/// replace, the cleared side's new generation, with the first batch in it.
async fn send_noting(
    bus: &mut impl Bus,
    device: &str,
    replace: bool,
    side: &Side,
    mut changes: Vec<Change>,
    first_landed: &mut impl FnMut(&Side) -> Result<()>,
) -> std::result::Result<SyncOut, Fail> {
    changes.sort_by_key(|c| relay_money::sync::order(&c.table));
    // What goes out, by row: a page asked for after a batch was applied hands that batch back.
    let sent: HashMap<(&str, &str), i64> = changes.iter().map(|c| ((c.table.as_str(), c.uid.as_str()), c.updated_at)).collect();
    let mut since = side.since;
    let mut generation = side.generation.clone();
    let mut first = true;
    let mut paged = true;
    let mut answers: Vec<Change> = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let (mut applied, mut skipped) = (0, 0);
    let mut chunks = changes.chunks(BATCH).peekable();
    let mut out = loop {
        let chunk = chunks.next().unwrap_or(&[]);
        let mut payload = json!({
            "device": device,
            "replace": replace && first,
            "since": since,
            "generation": if replace && first { None } else { generation.clone() },
            "changes": chunk,
        });
        if paged {
            payload["limit"] = json!(PAGE);
        }
        let answer = match bus.call("money.sync", payload.clone()).await {
            // An engine from before pages refuses the field it does not know, before applying anything.
            Err(e) if paged && e.is("bus.schema") => {
                paged = false;
                if let Some(p) = payload.as_object_mut() {
                    p.remove("limit");
                }
                bus.call("money.sync", payload).await?
            }
            answer => answer?,
        };
        let out: SyncOut = serde_json::from_value(answer).map_err(anyhow::Error::from)?;
        if out.more && out.cursor <= since {
            return Err(anyhow!("the other side's money.sync said more without moving its cursor").into());
        }
        since = out.cursor;
        generation = Some(out.generation.clone());
        if first {
            first_landed(&Side { since, generation: generation.clone() })?;
        }
        applied += out.applied;
        skipped += out.skipped;
        for c in &out.changes {
            if sent.get(&(c.table.as_str(), c.uid.as_str())) == Some(&c.updated_at) {
                continue;
            }
            // A row changed between pages comes again: the newest stands.
            match seen.get(&(c.table.clone(), c.uid.clone())) {
                Some(&i) if answers[i].updated_at >= c.updated_at => {}
                Some(&i) => answers[i] = c.clone(),
                None => {
                    seen.insert((c.table.clone(), c.uid.clone()), answers.len());
                    answers.push(c.clone());
                }
            }
        }
        first = false;
        if !out.more && chunks.peek().is_none() {
            break out;
        }
    };
    out.changes = answers;
    out.applied = applied;
    out.skipped = skipped;
    out.more = false;
    Ok(out)
}

fn side_of(out: &SyncOut) -> Side {
    Side { since: out.cursor, generation: Some(out.generation.clone()) }
}

/// Accounts and entries in a whole ledger: what the person would miss. Categories are left out,
/// since every new ledger is given some.
fn weight(changes: &[Change]) -> usize {
    changes.iter().filter(|c| !c.deleted && (c.table == "accounts" || c.table == "transactions")).count()
}

/// The generation `bus`'s ledger has now, asked for with one change at most.
async fn generation_of(bus: &mut impl Bus, device: &str) -> std::result::Result<String, Fail> {
    let mut payload = json!({"device": device, "replace": false, "since": 0, "changes": [], "limit": 1});
    let answer = match bus.call("money.sync", payload.clone()).await {
        Err(e) if e.is("bus.schema") => {
            if let Some(p) = payload.as_object_mut() {
                p.remove("limit");
            }
            bus.call("money.sync", payload).await?
        }
        answer => answer?,
    };
    Ok(serde_json::from_value::<SyncOut>(answer).map_err(anyhow::Error::from)?.generation)
}

/// What [`fill`] did: both sides' new places, the rows that went over, and the rows edited on the
/// filled side meanwhile that went back.
struct Filled {
    from: Side,
    to: Side,
    rows: usize,
    back: usize,
}

/// `whole`, `from`'s whole ledger, goes to `to`: replacing `to`'s (`at` is `None`), or, to finish
/// a replace cut short, merged into it under the place `to` took when the first batch landed, so
/// what was edited there since stays (newest edit wins, as in any round). Each side is asked under
/// the name it lists the link by. `landed` hears both sides' places once a replace's first batch is
/// in. Rows `to` answers with were edited there while it was being filled: they go back to `from`,
/// whose cursor stays at `whole`, so `from`'s own edits made meanwhile come over next round (with
/// what was just sent back, which `to` already has). `None`: `to` is not that ledger any more.
async fn fill(
    from: &mut impl Bus,
    from_device: &str,
    to: &mut impl Bus,
    to_device: &str,
    whole: SyncOut,
    at: Option<&Side>,
    landed: &mut impl FnMut(&Side, &Side) -> Result<()>,
) -> std::result::Result<Option<Filled>, Fail> {
    let rows = whole.changes.len();
    let from_side = side_of(&whole);
    let taken = match at {
        None => send_noting(to, to_device, true, &Side::default(), whole.changes, &mut |t| landed(&from_side, t)).await?,
        Some(at) => match send(to, to_device, false, at, whole.changes).await {
            Ok(taken) => taken,
            Err(e) if e.is("money.sync_stale") => return Ok(None),
            Err(e) => return Err(e),
        },
    };
    let back = if taken.changes.is_empty() { 0 } else { send(from, from_device, false, &from_side, taken.changes.clone()).await?.applied };
    Ok(Some(Filled { from: from_side, to: side_of(&taken), rows, back }))
}

/// `from`'s whole ledger goes to the other side: a replace (`at` is `None`, with [`Link::replacing`]
/// saved first), or the end of one cut short after its first batch landed (`at`, the place
/// [`Link::landed`] kept). Clears the marker once it is done. `whole` is `from`'s whole ledger when
/// the caller has it. `None`: a side's ledger was replaced again since the cut, and the marker is
/// dropped for the usual rules to decide which way the latest wholesale act goes.
#[allow(clippy::too_many_arguments)]
async fn carry(
    from: Whole,
    at: Option<Side>,
    pc: &mut impl Bus,
    cloud: &mut impl Bus,
    link: &mut Link,
    (pc_name, cloud_name): (&str, &str),
    save: &mut impl FnMut(&Link) -> Result<()>,
    whole: Option<SyncOut>,
) -> std::result::Result<Option<Report>, Fail> {
    if at.is_none() {
        link.replacing = Some(from);
        link.landed = None;
        save(link)?;
    }
    let whole = match (whole, from) {
        (Some(whole), _) => whole,
        (None, Whole::Pc) => send(pc, cloud_name, false, &Side::default(), Vec::new()).await?,
        (None, Whole::Cloud) => send(cloud, pc_name, false, &Side::default(), Vec::new()).await?,
    };
    // Finishing: the source's ledger must still be the one whose first batch landed.
    let source = match from {
        Whole::Pc => &link.pc,
        Whole::Cloud => &link.cloud,
    };
    let filled = if at.is_some() && source.generation.as_deref() != Some(whole.generation.as_str()) {
        None
    } else {
        match from {
            Whole::Pc => {
                let mut landed = |f: &Side, t: &Side| {
                    (link.pc, link.cloud, link.landed) = (f.clone(), t.clone(), Some(t.clone()));
                    save(link)
                };
                fill(pc, cloud_name, cloud, pc_name, whole, at.as_ref(), &mut landed).await?
            }
            Whole::Cloud => {
                let mut landed = |f: &Side, t: &Side| {
                    (link.cloud, link.pc, link.landed) = (f.clone(), t.clone(), Some(t.clone()));
                    save(link)
                };
                fill(cloud, pc_name, pc, cloud_name, whole, at.as_ref(), &mut landed).await?
            }
        }
    };
    link.replacing = None;
    link.landed = None;
    let Some(f) = filled else { return Ok(None) };
    let mut report = Report::default();
    match from {
        Whole::Pc => {
            (link.pc, link.cloud) = (f.from, f.to);
            (report.to_cloud, report.to_pc) = (f.rows, f.back);
            report.replaced_by = Some("pc");
        }
        Whole::Cloud => {
            (link.cloud, link.pc) = (f.from, f.to);
            (report.to_pc, report.to_cloud) = (f.rows, f.back);
            report.replaced_by = Some("cloud");
        }
    }
    link.held = None;
    Ok(Some(report))
}

/// `from`'s whole ledger replaces the other side's.
async fn replace(
    from: Whole,
    pc: &mut impl Bus,
    cloud: &mut impl Bus,
    link: &mut Link,
    names: (&str, &str),
    save: &mut impl FnMut(&Link) -> Result<()>,
    whole: Option<SyncOut>,
) -> std::result::Result<Report, Fail> {
    // A replace from scratch sends with `replace: true`, which no ledger answers as stale.
    Ok(carry(from, None, pc, cloud, link, names, save, whole).await?.unwrap_or_default())
}

/// The cloud's ledger was replaced wholesale: it replaces this PC's, unless that is held ([`Held`]),
/// or the person chose already.
async fn from_cloud(
    pc: &mut impl Bus,
    cloud: &mut impl Bus,
    link: &mut Link,
    names: (&str, &str),
    save: &mut impl FnMut(&Link) -> Result<()>,
) -> std::result::Result<Report, Fail> {
    let (pc_name, cloud_name) = names;
    if let Some(held) = link.held.clone() {
        match held.accept {
            Some(choice) => return replace(choice, pc, cloud, link, names, save, None).await,
            // Still the ledger held: no need to read either side whole again.
            None if generation_of(cloud, pc_name).await? == held.generation => return Err(held.refusal()),
            None => link.held = None,
        }
    }
    let whole = send(cloud, pc_name, false, &Side::default(), Vec::new()).await?;
    let ours = send(pc, cloud_name, false, &Side::default(), Vec::new()).await?;
    let (cloud_rows, pc_rows) = (weight(&whole.changes), weight(&ours.changes));
    if pc_rows > 0 && cloud_rows * 2 < pc_rows {
        let held = Held { generation: whole.generation.clone(), pc_rows, cloud_rows, lost: None, accept: None };
        let refusal = held.refusal();
        link.held = Some(held);
        return Err(refusal);
    }
    replace(Whole::Cloud, pc, cloud, link, names, save, Some(whole)).await
}

/// Fewest of this PC's accounts and entries a round from the cloud must take away to be held.
const SWEEP_MIN: usize = 20;

/// Whether the cloud's `theirs` would delete or blank at least half of this PC's accounts and
/// entries ([`SWEEP_MIN`] at least). An ordinary edit (a recategorised entry, a renamed account)
/// takes nothing away and is not counted: a young ledger's first weeks of fixes in Tally while
/// the PC was off held every round. A rewrite that empties a row ([`blanked`]) is. A replace from scratch is held when it would leave this PC
/// with too little ([`from_cloud`]), but the same loss sent as ordinary changes, a tombstone or a
/// rewrite at a time, from any credential the cloud lets sync or from the server itself, passed
/// unchecked and left no copy behind. No phone's own editing comes near it. Held, it waits on the
/// same choice, since applying it would leave this PC's ledger the cloud's.
async fn sweeping(pc: &mut impl Bus, cloud_name: &str, theirs: &SyncOut) -> std::result::Result<Option<Held>, Fail> {
    let counted = |c: &&Change| c.table == "accounts" || c.table == "transactions";
    if theirs.changes.iter().filter(counted).count() < SWEEP_MIN {
        return Ok(None);
    }
    let ours = send(pc, cloud_name, false, &Side::default(), Vec::new()).await?;
    let here: HashSet<(&str, &str)> = ours.changes.iter().filter(counted).filter(|c| !c.deleted).map(|c| (c.table.as_str(), c.uid.as_str())).collect();
    let (mut lost, mut gone, mut new) = (0, 0, 0);
    for c in theirs.changes.iter().filter(counted) {
        match (here.contains(&(c.table.as_str(), c.uid.as_str())), c.deleted) {
            (true, true) => {
                lost += 1;
                gone += 1;
            }
            (true, false) => lost += usize::from(blanked(c)),
            (false, false) => new += 1,
            (false, true) => {}
        }
    }
    Ok((lost >= SWEEP_MIN && lost * 2 >= here.len()).then(|| Held {
        generation: theirs.generation.clone(),
        pc_rows: here.len(),
        cloud_rows: here.len() - gone + new,
        lost: Some(lost),
        accept: None,
    }))
}

/// A row rewritten to nothing: every field but its identity empty, zero or null.
fn blanked(c: &Change) -> bool {
    c.row.iter().filter(|(key, _)| !matches!(key.as_str(), "uid" | "id" | "updated_at" | "updatedAt")).all(|(_, value)| match value {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.trim().is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    })
}

/// Writes the person's choice for a replace the link holds ([`Held`]) into `cloud.json`, for the
/// next round to carry out: `Whole::Cloud` takes the cloud's ledger here, `Whole::Pc` sends this
/// PC's to the cloud in its place. False when nothing is held.
pub async fn decide(path: &Path, choice: Whole) -> Result<bool> {
    let _lock = lock(path).await?;
    let Some(mut link) = Link::load(path)? else {
        anyhow::bail!("this PC has no cloud link");
    };
    let Some(held) = link.held.as_mut() else { return Ok(false) };
    held.accept = Some(choice);
    link.save(path)?;
    Ok(true)
}

/// One merge of the two ledgers. `pc_name` is how the cloud lists this PC, `cloud_name` how this
/// PC lists the cloud copy. `link`'s sides move only as far as the round got. `save` writes the
/// link out mid-round, before a wholesale replace clears either side.
pub async fn merge_money(
    pc: &mut impl Bus,
    cloud: &mut impl Bus,
    link: &mut Link,
    pc_name: &str,
    cloud_name: &str,
    save: &mut impl FnMut(&Link) -> Result<()>,
) -> std::result::Result<Report, Fail> {
    let names = (pc_name, cloud_name);
    // A replace the last round left partway goes on from the same side, before anything else: the
    // side it was clearing holds part of the ledger and a new generation, and would read as stale.
    if let Some(from) = link.replacing {
        match link.landed.clone() {
            None => return replace(from, pc, cloud, link, names, save, None).await,
            Some(at) => {
                if let Some(report) = carry(from, Some(at), pc, cloud, link, names, save, None).await? {
                    return Ok(report);
                }
                // A side was replaced again since: the round goes on as any other.
            }
        }
    }
    // Changes held for the person ([`sweeping`]), now answered: carried out as the replace they
    // amount to. Unanswered, the round goes on and holds them again.
    if let Some(Held { lost: Some(_), accept: Some(choice), .. }) = link.held {
        return replace(choice, pc, cloud, link, names, save, None).await;
    }
    let mut report = Report::default();
    // This PC's changes since the last round.
    let ours = match send(pc, cloud_name, false, &link.pc, Vec::new()).await {
        Ok(out) => out,
        // Restored, erased or replaced on this PC: it goes to the cloud whole.
        Err(e) if e.is("money.sync_stale") => return replace(Whole::Pc, pc, cloud, link, names, save, None).await,
        Err(e) => return Err(e),
    };
    let mut pc_side = side_of(&ours);
    let mut changes = ours.changes;
    for pass in 0..PASSES {
        let theirs = match send(cloud, pc_name, false, &link.cloud, changes).await {
            Ok(out) => out,
            // Tally's first sync, or a restore, replaced the cloud's ledger: it comes here whole.
            Err(e) if pass == 0 && e.is("money.sync_stale") => return from_cloud(pc, cloud, link, names, save).await,
            Err(e) => return Err(e),
        };
        report.to_cloud += theirs.applied;
        // The cloud has this PC's changes up to here.
        link.pc = pc_side.clone();
        if theirs.changes.is_empty() {
            link.cloud = side_of(&theirs);
            break;
        }
        let cloud_side = side_of(&theirs);
        if let Some(held) = sweeping(pc, cloud_name, &theirs).await? {
            let refusal = held.refusal();
            link.held = Some(held);
            return Err(refusal);
        }
        let back = send(pc, cloud_name, false, &pc_side, theirs.changes).await?;
        report.to_pc += back.applied;
        // And this PC the cloud's; had that failed, the next round would ask the cloud again.
        link.cloud = cloud_side;
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
    // A clean round settles whatever was held: the changes no longer sweep, or the ledger is
    // not the replaced one any more. Left set, `relay cloud status` kept showing a hold, and a
    // later `relay cloud accept` carried out a wholesale replace on that stale record.
    link.held = None;
    Ok(report)
}

/// How many of the exports this PC's training history came from [`fetch_gym`] keeps.
pub const GYM_KEPT: usize = 5;

/// Before an upload replaces this PC's training history, the export it was last uploaded from
/// (this PC's `gym.raw`) is kept in `dir` (0700) as `avex-<time>-<n>.json` (0600), the newest
/// [`GYM_KEPT`]: `gym.import` of that file (the Gym page's import) puts it back. A history
/// imported from a file has no such text here; that file is wherever it was imported from.
async fn keep_gym(pc: &mut impl Bus, dir: &Path) -> std::result::Result<(), Fail> {
    use std::io::Write as _;
    use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
    let raw = match pc.call("gym.raw", json!({})).await {
        Ok(raw) => raw,
        Err(e) if e.is("bus.unknown_op") => return Ok(()),
        Err(e) => return Err(e),
    };
    let Some(text) = raw["json"].as_str() else { return Ok(()) };
    let kept = (|| -> std::io::Result<()> {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        let name = format!("avex-{}-{}.json", jiff::Timestamp::now().strftime("%Y%m%dT%H%M%SZ"), random_hex(4));
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(dir.join(name))?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        let mut all: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("avex-") && n.ends_with(".json")))
            .collect();
        all.sort();
        for old in all.iter().rev().skip(GYM_KEPT) {
            let _ = std::fs::remove_file(old);
        }
        Ok(())
    })();
    kept.with_context(|| format!("keeping this PC's training history in {} before replacing it", dir.display()))?;
    Ok(())
}

/// Bring the newest Avex export home, when the cloud holds one this PC has not taken. With `keep`,
/// the export this PC's history came from is kept there first ([`keep_gym`]).
pub async fn fetch_gym(pc: &mut impl Bus, cloud: &mut impl Bus, link: &mut Link, keep: Option<&Path>) -> std::result::Result<bool, Fail> {
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
    if let Some(dir) = keep {
        keep_gym(pc, dir).await?;
    }
    pc.call("gym.upload", json!({"json": text, "name": "avex_export.json"})).await?;
    link.gym_digest = Some(digest.to_string());
    Ok(true)
}

/// The round lock for the link at `path`, held until the file is dropped. A round holds it, and so
/// does anything else that writes or removes `cloud.json` (`relay cloud connect`, `off`), so none
/// writes over another's.
pub async fn lock(path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let lock_path = path.with_file_name("cloud.json.lock");
    tokio::task::spawn_blocking(move || {
        let f = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(&lock_path)
            .with_context(|| format!("opening {}", lock_path.display()))?;
        f.lock().with_context(|| format!("locking {}", lock_path.display()))?;
        Ok(f)
    })
    .await?
}

/// Writes `link` over the file a round read it from, unless that file is gone: a link removed
/// while a round ran stays removed. False when it was.
fn keep(link: &Link, path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    link.save(path)?;
    Ok(true)
}

/// One round against the engine at `socket`: money both ways, then the training history home.
/// The outcome is written to `cloud.json` either way, for `relay cloud status`.
pub async fn round(socket: &Path, path: &Path, pc_name: &str) -> Result<Report> {
    let _lock = lock(path).await?;
    let Some(mut link) = Link::load(path)? else {
        anyhow::bail!("this PC has no cloud link: `relay cloud pair '<relay://pair link>'` makes one");
    };
    let outcome = async {
        let mut pc = Local::connect(socket).await?;
        let mut cloud = Remote::connect(&link).await?;
        let cloud_name = device_name("Cloud", &link.host);
        let mut save = |l: &Link| if keep(l, path)? { Ok(()) } else { Err(anyhow!("the cloud link was removed during the round")) };
        // The two halves are independent: a held or refused money round must not keep Avex's
        // newest export sitting on the cloud copy every minute until the money is settled.
        let money = merge_money(&mut pc, &mut cloud, &mut link, pc_name, &cloud_name, &mut save).await.map_err(anyhow::Error::from);
        let gym = fetch_gym(&mut pc, &mut cloud, &mut link, Some(&path.with_file_name("cloud-gym"))).await.map_err(anyhow::Error::from);
        both(money, gym)
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
    keep(&link, path)?;
    outcome
}

/// A round's outcome from its two halves, each run whatever the other did: the money report
/// with whether a newer Avex export came home, or what failed (both, when both did).
fn both(money: Result<Report>, gym: Result<bool>) -> Result<Report> {
    match (money, gym) {
        (Ok(mut report), Ok(gym)) => {
            report.gym = gym;
            Ok(report)
        }
        (Err(money), Ok(_)) => Err(money),
        (Ok(_), Err(gym)) => Err(gym.context("bringing the training history home")),
        (Err(money), Err(gym)) => Err(money.context(format!("and bringing the training history home: {gym:#}"))),
    }
}

/// Whether a line from the listening connection asks for a round: a change the cloud copy took
/// from anything but this PC's own rounds (whose `money.sync` it names `pc_name`), or word that
/// events were dropped.
fn wakes(line: &str, pc_name: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return false };
    match v["ev"].as_str() {
        Some("money.changed") => v["payload"]["synced"].as_str() != Some(pc_name),
        Some("gym.changed" | "bus.lagged") => true,
        _ => false,
    }
}

/// One listening connection: subscribe to the cloud copy's ledger and training history, then wake
/// the merging loop for each change, until the connection fails. `reconnected` wakes it once on
/// connecting too, for what landed while there was no connection.
async fn listen_once(link: &Link, pc_name: &str, wake: &tokio::sync::mpsc::Sender<()>, reconnected: bool) -> Result<()> {
    let mut cloud = Remote::connect(link).await?;
    cloud.call("bus.subscribe", json!({"events": ["money.changed", "gym.changed"]})).await?;
    tracing::debug!("cloud link listening for changes");
    if reconnected {
        let _ = wake.try_send(());
    }
    let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + LISTEN_PING, LISTEN_PING);
    let mut heard = tokio::time::Instant::now();
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if heard.elapsed() > LISTEN_SILENCE {
                    anyhow::bail!("the cloud engine went silent");
                }
                cloud.ws.send(Message::Ping(Vec::new().into())).await?;
            }
            msg = cloud.ws.next() => {
                heard = tokio::time::Instant::now();
                let text = match msg {
                    None | Some(Ok(Message::Close(_))) => anyhow::bail!("the connection closed"),
                    Some(Err(e)) => return Err(e.into()),
                    Some(Ok(Message::Text(t))) => t.to_string(),
                    Some(Ok(Message::Binary(b))) => String::from_utf8_lossy(&b).into_owned(),
                    Some(Ok(_)) => continue,
                };
                if text.lines().any(|line| wakes(line, pc_name)) {
                    // A round already waiting will take this change too.
                    let _ = wake.try_send(());
                }
            }
        }
    }
}

/// Holds a connection to the cloud engine subscribed to its changes, so a round starts within a
/// second of Tally's sync or Avex's upload rather than on the minute. Dials again with a growing
/// wait when the connection fails; the minute's round runs regardless, so a dropped connection
/// costs speed, never data. Returns once the link is gone.
async fn listen(path: PathBuf, pc_name: String, wake: tokio::sync::mpsc::Sender<()>) {
    let mut backoff = Duration::from_secs(1);
    let mut reconnected = false;
    loop {
        let link = match Link::load(&path) {
            Ok(Some(link)) => link,
            Ok(None) => return,
            // The merging loop says so at warn.
            Err(_) => {
                tokio::time::sleep(LISTEN_BACKOFF).await;
                continue;
            }
        };
        let started = tokio::time::Instant::now();
        if let Err(e) = listen_once(&link, &pc_name, &wake, reconnected).await {
            tracing::debug!(error = %format!("{e:#}"), "cloud link stopped listening; dialing again");
        }
        reconnected = true;
        // A connection that held a while starts the wait over.
        if started.elapsed() > LISTEN_BACKOFF {
            backoff = Duration::from_secs(1);
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(LISTEN_BACKOFF);
    }
}

/// Aborts the task when dropped: the listener goes with the merging loop.
struct Aborting(tokio::task::JoinHandle<()>);

impl Drop for Aborting {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Merge while the engine runs, starting now: as soon as the cloud copy says it took a change
/// ([`listen`]), and every [`EVERY`] without one. Failures are logged and kept in `cloud.json`;
/// the next round tries again.
pub fn spawn(socket: PathBuf, path: PathBuf, pc_name: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (wake, mut woken) = tokio::sync::mpsc::channel(1);
        let _listener = Aborting(tokio::spawn(listen(path.clone(), pc_name.clone(), wake)));
        let mut tick = tokio::time::interval(EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut failing = false;
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                Some(()) = woken.recv() => {
                    tokio::time::sleep(SETTLE).await;
                    // What arrived while settling is in this round.
                    while woken.try_recv().is_ok() {}
                }
            }
            // The minute counts from the last round, whatever started it.
            tick.reset();
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
    fn a_change_wakes_the_link_unless_it_was_the_links_own() {
        let ev = |ev: &str, payload: Value| json!({"v": 1, "ev": ev, "ts": "2026-10-14T00:00:00Z", "actor": "user", "payload": payload}).to_string();
        assert!(wakes(&ev("money.changed", json!({"synced": "Pixel", "applied": 1})), "PC (desk)"), "Tally's sync");
        assert!(!wakes(&ev("money.changed", json!({"synced": "PC (desk)", "applied": 3})), "PC (desk)"), "this PC's own round");
        assert!(wakes(&ev("money.changed", json!({})), "PC (desk)"), "a reset or restore there");
        assert!(wakes(&ev("gym.changed", json!({"imported": 4})), "PC (desk)"), "Avex's upload");
        assert!(wakes(&ev("bus.lagged", json!({"dropped": 2})), "PC (desk)"), "dropped events may have been anything");
        assert!(!wakes(r#"{"v":1,"id":"x","ok":true,"result":{}}"#, "PC (desk)"), "an answer");
        assert!(!wakes("not json", "PC (desk)"));
    }

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
            let out = self.0.sync_page(
                p["device"].as_str().unwrap(),
                p["replace"].as_bool().unwrap_or(false),
                p["since"].as_i64().unwrap(),
                p["generation"].as_str(),
                &changes,
                p["limit"].as_u64().map(|l| l as usize),
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
            pc: Side::default(), cloud: Side::default(), gym_digest: None, last_ok: None, last_error: None, server: None, server_token: None,
            replacing: None, landed: None, held: None,
        }
    }

    fn names(l: &Ledger) -> Vec<String> {
        let mut v: Vec<String> = l.accounts().unwrap().into_iter().map(|a| a.name).collect();
        v.sort();
        v
    }

    async fn merge(pc: &mut Ledgered, cloud: &mut Ledgered, link: &mut Link) -> Report {
        merge_money(pc, cloud, link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap()
    }

    /// A side whose link breaks after `left` more calls.
    struct Flaky<'a> {
        bus: &'a mut Ledgered,
        left: usize,
    }

    impl Bus for Flaky<'_> {
        async fn call(&mut self, op: &str, p: Value) -> std::result::Result<Value, Fail> {
            if self.left == 0 {
                return Err(Fail::Link(anyhow!("the connection closed")));
            }
            self.left -= 1;
            self.bus.call(op, p).await
        }
    }

    /// A side that keeps count: requests, rows sent to it, the longest answer, and pages that
    /// said more. `old` is an engine from before pages, which refuses `limit` as it refuses any
    /// field it does not know.
    struct Watched<'a> {
        bus: &'a mut Ledgered,
        old: bool,
        calls: usize,
        rows_in: usize,
        longest: usize,
        more: usize,
    }

    impl<'a> Watched<'a> {
        fn new(bus: &'a mut Ledgered, old: bool) -> Watched<'a> {
            Watched { bus, old, calls: 0, rows_in: 0, longest: 0, more: 0 }
        }
    }

    impl Bus for Watched<'_> {
        async fn call(&mut self, op: &str, p: Value) -> std::result::Result<Value, Fail> {
            if self.old && p.get("limit").is_some() {
                return Err(Fail::Refused { code: "bus.schema".into(), message: "unknown field `limit`".into() });
            }
            self.calls += 1;
            self.rows_in += p["changes"].as_array().map_or(0, Vec::len);
            let out = self.bus.call(op, p).await?;
            self.longest = self.longest.max(out["changes"].as_array().map_or(0, Vec::len));
            self.more += usize::from(out["more"].as_bool().unwrap_or(false));
            Ok(out)
        }
    }

    fn accounts(l: &mut Ledger, prefix: &str, n: usize) {
        for i in 0..n {
            l.account_add(&format!("{prefix}{i:04}"), AccountType::Cash, 0, None, "").unwrap();
        }
    }

    /// A held money round still says the gym half ran; a failed gym half is named.
    #[test]
    fn a_round_reports_each_half() {
        let held: Result<Report> = Err(anyhow!("cloud.changes_held"));
        assert!(format!("{:#}", both(held, Ok(true)).unwrap_err()).contains("cloud.changes_held"));
        let report = both(Ok(Report { to_pc: 2, ..Report::default() }), Ok(true)).unwrap();
        assert!(report.gym && report.to_pc == 2);
        let gym = format!("{:#}", both(Ok(Report::default()), Err(anyhow!("gym.raw timed out"))).unwrap_err());
        assert!(gym.contains("training history") && gym.contains("gym.raw timed out"), "{gym}");
        let two = format!("{:#}", both(Err(anyhow!("cloud.replace_held")), Err(anyhow!("gym.raw timed out"))).unwrap_err());
        assert!(two.contains("cloud.replace_held") && two.contains("gym.raw timed out"), "{two}");
    }

    #[tokio::test]
    async fn a_ledger_longer_than_a_page_is_taken_in_pages_and_each_row_crosses_once() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "P", PAGE + 300);
        accounts(&mut cloud.0, "C", PAGE + 300);
        let mut link = link();
        let (mut p, mut c) = (Watched::new(&mut pc, false), Watched::new(&mut cloud, false));
        let r = merge_money(&mut p, &mut c, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap();
        // Each side's accounts, and the categories a new ledger starts with.
        assert!(r.to_pc > PAGE + 300 && r.to_cloud == r.to_pc, "{r:?}");
        assert!(p.longest <= PAGE && c.longest <= PAGE, "{} {}", p.longest, c.longest);
        assert!(p.more > 0 && c.more > 0);
        // The cloud's later pages hand back the batch this PC sent first; none of it comes home.
        assert_eq!(p.rows_in, r.to_pc, "every row sent here was news here");
        assert_eq!(names(&pc.0).len(), 2 * (PAGE + 300));
        assert_eq!(names(&cloud.0), names(&pc.0));
        let quiet = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.replaced_by), (0, 0, None), "{quiet:?}");

        // Tally's first sync replaces the cloud's ledger: it comes here whole, in pages.
        let mut phone = Ledger::open_in_memory().unwrap();
        accounts(&mut phone, "T", PAGE + 300);
        let everything = phone.sync("probe", false, 0, None, &[], "t").unwrap().changes;
        cloud.0.sync("Pixel", true, 0, None, &everything, "t").unwrap();
        let mut c = Watched::new(&mut cloud, false);
        let r = merge_money(&mut pc, &mut c, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap();
        assert_eq!(r.replaced_by, Some("cloud"));
        assert!(c.longest <= PAGE && c.more > 0);
        assert_eq!(names(&pc.0).len(), PAGE + 300);
        assert_eq!(names(&pc.0), names(&cloud.0));
    }

    #[tokio::test]
    async fn a_cloud_engine_from_before_pages_is_asked_whole() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "P", 10);
        accounts(&mut cloud.0, "C", PAGE + 10);
        let mut link = link();
        let mut c = Watched::new(&mut cloud, true);
        let r = merge_money(&mut pc, &mut c, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap();
        assert_eq!(r.to_pc - r.to_cloud, PAGE, "{r:?}");
        assert!(c.longest > PAGE, "one answer, as before");
        assert_eq!(names(&cloud.0), names(&pc.0));
    }

    /// A whole ledger of `n` accounts, as a phone's first sync sends it.
    fn phone_ledger(n: usize) -> Vec<Change> {
        let mut phone = Ledger::open_in_memory().unwrap();
        accounts(&mut phone, "A", n);
        phone.sync("probe", false, 0, None, &[], "t").unwrap().changes
    }

    fn has(l: &Ledger, name: &str) -> bool {
        names(l).iter().any(|n| n == name)
    }

    #[tokio::test]
    async fn a_replace_cut_short_is_finished_from_the_same_side() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        pc.0.account_add("Old", AccountType::Cash, 0, None, "").unwrap();
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;

        // Tally's first sync gives the cloud a ledger bigger than one batch.
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(BATCH + 100), "t").unwrap();

        // The round stops after this PC took the first batch (its changes, its whole ledger, the
        // batch): it holds part of the phone's ledger.
        let mut saved = Vec::new();
        let mut flaky = Flaky { bus: &mut pc, left: 3 };
        let cut = merge_money(&mut flaky, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |l| {
            saved.push(l.clone());
            Ok(())
        })
        .await;
        assert!(cut.is_err());
        assert_eq!(names(&pc.0).len(), BATCH);
        // What reached the disk once the first batch landed says which way the replace went, and
        // where this PC's ledger stood then.
        let mut on_disk = saved.pop().expect("the link was saved before the replace");
        assert_eq!(on_disk.replacing, Some(Whole::Cloud));
        assert_eq!(on_disk.landed.as_ref().and_then(|l| l.generation.clone()), Some(pc.0.generation().unwrap()));

        // The cloud is out of reach for a while, and the person adds an account on the PC.
        pc.0.account_add("Edited while the cloud was down", AccountType::Cash, 0, None, "").unwrap();

        // The next round finishes the replace, rather than sending the half-copied ledger to the
        // cloud, and keeps what was edited here since.
        let r = merge(&mut pc, &mut cloud, &mut on_disk).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        // The account, and the categories adding one gave a ledger that had none yet.
        assert!(r.to_cloud > 0, "{r:?}");
        assert_eq!(names(&pc.0).len(), BATCH + 101);
        assert!(has(&pc.0, "Edited while the cloud was down") && !has(&pc.0, "Old"));
        assert_eq!(names(&cloud.0), names(&pc.0));
        assert_eq!((on_disk.replacing, on_disk.landed.as_ref()), (None, None));
        let quiet = merge(&mut pc, &mut cloud, &mut on_disk).await;
        assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.replaced_by), (0, 0, None), "{quiet:?}");
    }

    #[tokio::test]
    async fn finishing_a_replace_keeps_what_a_phone_did_on_the_half_copied_cloud() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        // The PC's ledger is restored: it goes to the cloud whole, and the link breaks after the
        // first batch.
        pc.0.reset().unwrap();
        accounts(&mut pc.0, "R", BATCH + 100);
        let mut flaky = Flaky { bus: &mut cloud, left: 1 };
        assert!(merge_money(&mut pc, &mut flaky, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        assert_eq!((link.replacing, names(&cloud.0).len()), (Some(Whole::Pc), BATCH));

        // Tally, stale, takes the cloud's ledger whole and merges an edit into it, for days.
        let whole = cloud.0.sync("Tally", false, 0, None, &[], "t").unwrap();
        let mut phone = Ledger::open_in_memory().unwrap();
        phone.account_add("Phone edit", AccountType::Cash, 0, None, "").unwrap();
        let mine: Vec<Change> = phone.sync("p", false, 0, None, &[], "t").unwrap().changes.into_iter().filter(|c| c.table == "accounts").collect();
        cloud.0.sync("Tally", false, whole.cursor, Some(&whole.generation), &mine, "t").unwrap();

        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("pc"));
        assert!(has(&cloud.0, "Phone edit") && has(&pc.0, "Phone edit"), "the phone's edit stays and comes to the PC");
        assert_eq!(names(&cloud.0).len(), BATCH + 101);
        assert_eq!(names(&cloud.0), names(&pc.0));
        // Tally merges on with the generation it took: the cloud's ledger is still that one.
        assert!(cloud.0.sync("Tally", false, whole.cursor, Some(&whole.generation), &[], "t").is_ok());
    }

    #[tokio::test]
    async fn a_marker_without_where_it_landed_runs_the_replace_again_whole() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        pc.0.account_add("Old", AccountType::Cash, 0, None, "").unwrap();
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(BATCH + 100), "t").unwrap();
        // An older build's marker: the direction only, after part of the ledger landed.
        let mut flaky = Flaky { bus: &mut pc, left: 3 };
        assert!(merge_money(&mut flaky, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        let old: Link = serde_json::from_str(&serde_json::to_string(&link).unwrap().replace(r#""landed""#, r#""landed_by_a_newer_build""#)).unwrap();
        let mut link = old;
        assert_eq!((link.replacing, link.landed.as_ref()), (Some(Whole::Cloud), None));
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        assert_eq!(names(&pc.0), names(&cloud.0));
        assert_eq!(names(&pc.0).len(), BATCH + 100);
    }

    #[tokio::test]
    async fn a_side_replaced_again_after_the_cut_wins_by_the_usual_rules() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(BATCH + 100), "t").unwrap();
        let mut flaky = Flaky { bus: &mut pc, left: 3 };
        assert!(merge_money(&mut flaky, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        assert!(link.landed.is_some());
        // Another phone pairs with the cloud before the PC is back: the newest wholesale act.
        let mut other = Ledger::open_in_memory().unwrap();
        accounts(&mut other, "New", BATCH + 50);
        cloud.0.sync("Other phone", true, 0, None, &other.sync("probe", false, 0, None, &[], "t").unwrap().changes, "t").unwrap();
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        assert_eq!(names(&pc.0), names(&other));
        assert_eq!(names(&cloud.0), names(&other));
        assert_eq!(link.replacing, None);
    }

    /// A PC on which the person adds an account right after a replace's first batch lands here.
    struct EditsBetween<'a> {
        bus: &'a mut Ledgered,
        done: bool,
    }

    impl Bus for EditsBetween<'_> {
        async fn call(&mut self, op: &str, p: Value) -> std::result::Result<Value, Fail> {
            let replacing = p["replace"].as_bool() == Some(true);
            let out = self.bus.call(op, p).await;
            if replacing && !self.done {
                self.done = true;
                self.bus.0.account_add("Between", AccountType::Cash, 0, None, "").unwrap();
            }
            out
        }
    }

    #[tokio::test]
    async fn an_edit_made_between_two_batches_of_a_replace_reaches_the_other_side() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(BATCH + 100), "t").unwrap();
        let mut between = EditsBetween { bus: &mut pc, done: false };
        let r = merge_money(&mut between, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap();
        assert!(r.replaced_by == Some("cloud") && r.to_cloud > 0, "{r:?}");
        assert!(has(&cloud.0, "Between"), "it went to the cloud in the same round");
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.to_pc, 0, "{r:?}");
        assert_eq!(names(&cloud.0), names(&pc.0));
        let quiet = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.replaced_by), (0, 0, None), "{quiet:?}");
    }

    #[tokio::test]
    async fn an_empty_ledger_any_credential_put_on_the_cloud_is_held_not_copied_here() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "Mine", 50);
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        // Avex's credential may sync: it sends a replace with nothing in it.
        cloud.0.sync("Avex (S21)", true, 0, None, &[], "t").unwrap();
        let held = merge_money(&mut pc, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap_err();
        assert!(held.is("cloud.replace_held"), "{held}");
        assert!(held.to_string().contains("relay cloud accept cloud"), "{held}");
        assert_eq!(names(&pc.0).len(), 50, "this PC's ledger is as it was");
        assert_eq!(link.held.as_ref().map(|h| (h.pc_rows, h.cloud_rows)), Some((50, 0)));
        assert_eq!(link.replacing, None);

        // Held, a round reads neither ledger whole again.
        let mut c = Watched::new(&mut cloud, false);
        let again = merge_money(&mut pc, &mut c, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap_err();
        assert!(again.is("cloud.replace_held"));
        assert_eq!((c.calls, c.longest), (2, 0), "the stale answer, then one look at the generation");

        // The person keeps this PC's: it goes to the cloud whole.
        link.held.as_mut().unwrap().accept = Some(Whole::Pc);
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("pc"));
        assert_eq!(names(&cloud.0).len(), 50);
        assert_eq!(link.held, None);
        let quiet = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.replaced_by), (0, 0, None), "{quiet:?}");

        // Or takes the cloud's, small as it is.
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(3), "t").unwrap();
        assert!(merge_money(&mut pc, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        link.held.as_mut().unwrap().accept = Some(Whole::Cloud);
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        assert_eq!(names(&pc.0).len(), 3);

        // A ledger as big as this PC's (a phone pairing with the cloud) replaces it as before.
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(2), "t").unwrap();
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        assert_eq!(names(&pc.0).len(), 2);
    }

    /// The same loss as an empty replace, sent as ordinary changes: tombstones from a credential
    /// that leaked, or the server. Held; a few deletes, as a phone makes them, still go through.
    #[tokio::test]
    async fn changes_that_would_take_away_most_of_this_pcs_ledger_are_held() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "Mine", 50);
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(names(&cloud.0).len(), 50);
        let tombstones = |cloud: &mut Ledgered, skip: usize, n: usize| -> Vec<Change> {
            let rows = cloud.0.sync("probe", false, 0, None, &[], "t").unwrap().changes;
            rows.into_iter().filter(|c| c.table == "accounts" && !c.deleted).skip(skip).take(n)
                .map(|c| Change { updated_at: c.updated_at + 1, deleted: true, row: Default::default(), ..c }).collect()
        };
        let few = tombstones(&mut cloud, 0, 5);
        cloud.0.sync("Pixel", false, 0, None, &few, "t").unwrap();
        merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(names(&pc.0).len(), 45, "a phone's deletes reach this PC");

        let most = tombstones(&mut cloud, 0, 30);
        cloud.0.sync("Avex (S21)", false, 0, None, &most, "t").unwrap();
        let held = merge_money(&mut pc, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap_err();
        assert!(held.is("cloud.changes_held"), "{held}");
        assert!(held.to_string().contains("delete or rewrite 30 of this PC's 45"), "{held}");
        assert_eq!(names(&pc.0).len(), 45, "this PC's ledger is as it was");
        assert_eq!(link.held.as_ref().map(|h| (h.lost, h.cloud_rows)), Some((Some(30), 15)));
        let again = merge_money(&mut pc, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.unwrap_err();
        assert!(again.is("cloud.changes_held"), "held until the person answers");

        // The person keeps this PC's: it goes to the cloud whole, the deletes undone there.
        link.held.as_mut().unwrap().accept = Some(Whole::Pc);
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!((r.replaced_by, link.held.as_ref()), (Some("pc"), None));
        assert_eq!(names(&cloud.0), names(&pc.0));
        assert_eq!(names(&pc.0).len(), 45);
        let quiet = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!((quiet.to_pc, quiet.to_cloud, quiet.replaced_by), (0, 0, None), "{quiet:?}");
    }

    /// Held changes that stop sweeping (this PC's ledger grew past twice the loss) go through on
    /// the next clean round, and the hold goes with them: no stale hold for `accept` to act on.
    /// A young ledger's ordinary fixes from the phone (most of its entries recategorised while
    /// the PC was off) go through; the same rows blanked are held.
    #[tokio::test]
    async fn ordinary_edits_are_not_a_sweep_but_blanked_rows_are() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "Mine", 40);
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        let rows = cloud.0.sync("probe", false, 0, None, &[], "t").unwrap().changes;
        let live: Vec<Change> = rows.into_iter().filter(|c| c.table == "accounts" && !c.deleted).take(30).collect();
        let renamed: Vec<Change> = live.iter().cloned().map(|mut c| {
            c.updated_at += 1;
            for (key, value) in c.row.iter_mut() {
                if key == "name" { *value = json!(format!("{} (fixed)", value.as_str().unwrap_or(""))); }
            }
            c
        }).collect();
        cloud.0.sync("Pixel", false, 0, None, &renamed, "t").unwrap();
        merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(link.held, None, "edits are not a loss");
        assert_eq!(names(&pc.0).iter().filter(|n| n.ends_with("(fixed)")).count(), 30);
        let blank: Vec<Change> = live.into_iter().map(|mut c| {
            c.updated_at += 2;
            for (key, value) in c.row.iter_mut() {
                if !matches!(key.as_str(), "uid" | "updated_at" | "updatedAt") { *value = Value::Null; }
            }
            c
        }).collect();
        assert!(blank.iter().all(blanked));
        // A ledger refuses such rows, so only a server sending them itself gets this far.
        let theirs = SyncOut { cursor: 0, generation: "g".into(), changes: blank, replaced: false, applied: 0, skipped: 0, more: false };
        let held = sweeping(&mut pc, "Cloud (render)", &theirs).await.unwrap();
        assert_eq!(held.map(|h| h.lost), Some(Some(30)), "rows blanked by the cloud are held");
    }

    #[tokio::test]
    async fn a_clean_round_clears_a_hold_it_no_longer_needs() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "Mine", 45);
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        let rows = cloud.0.sync("probe", false, 0, None, &[], "t").unwrap().changes;
        let most: Vec<Change> = rows.into_iter().filter(|c| c.table == "accounts" && !c.deleted).take(30)
            .map(|c| Change { updated_at: c.updated_at + 1, deleted: true, row: Default::default(), ..c }).collect();
        cloud.0.sync("Avex (S21)", false, 0, None, &most, "t").unwrap();
        assert!(merge_money(&mut pc, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        assert!(link.held.is_some());
        accounts(&mut pc.0, "More", 30);
        merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(link.held, None, "the round that went through settled the hold");
        assert_eq!(names(&pc.0).len(), 45);
    }

    #[tokio::test]
    async fn a_held_replace_is_looked_at_afresh_once_the_cloud_is_replaced_again() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        accounts(&mut pc.0, "Mine", 10);
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;
        cloud.0.sync("Avex (S21)", true, 0, None, &[], "t").unwrap();
        assert!(merge_money(&mut pc, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        // Tally pairs with the cloud and sends its whole ledger.
        cloud.0.sync("Pixel", true, 0, None, &phone_ledger(12), "t").unwrap();
        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert_eq!(r.replaced_by, Some("cloud"));
        assert_eq!((names(&pc.0).len(), link.held.as_ref()), (12, None));
    }

    #[test]
    fn a_held_link_says_so_and_reads_back() {
        let mut l = link();
        l.held = Some(Held { generation: "g".into(), pc_rows: 50, cloud_rows: 0, lost: None, accept: None });
        let text = serde_json::to_string(&l).unwrap();
        assert!(text.contains(r#""held":{"generation":"g","pc_rows":50,"cloud_rows":0}"#), "{text}");
        assert_eq!(serde_json::from_str::<Link>(&text).unwrap(), l);
    }

    #[tokio::test]
    async fn the_clouds_changes_are_asked_again_when_this_pc_did_not_take_them() {
        let (mut pc, mut cloud) = (Ledgered(Ledger::open_in_memory().unwrap()), Ledgered(Ledger::open_in_memory().unwrap()));
        pc.0.account_add("Chequing", AccountType::Chequing, 0, None, "").unwrap();
        let mut link = link();
        merge(&mut pc, &mut cloud, &mut link).await;

        cloud.0.account_add("Savings", AccountType::Savings, 0, None, "").unwrap();
        let before = link.cloud.clone();
        let mut flaky = Flaky { bus: &mut pc, left: 1 };
        assert!(merge_money(&mut flaky, &mut cloud, &mut link, "PC (desk)", "Cloud (render)", &mut |_| Ok(())).await.is_err());
        assert_eq!(link.cloud, before, "the cloud's cursor stays where this PC last caught up");

        let r = merge(&mut pc, &mut cloud, &mut link).await;
        assert!(r.to_pc > 0, "{r:?}");
        assert_eq!(names(&pc.0), ["Chequing", "Savings"]);
    }

    #[test]
    fn a_device_name_fits_the_sixty_four_characters_money_sync_takes() {
        assert_eq!(device_name("Cloud", "render"), "Cloud (render)");
        let long = device_name("PC", &"é".repeat(100));
        assert_eq!(long.chars().count(), 64);
        assert!(long.starts_with("PC (é") && long.ends_with("é)"));
        assert_eq!(device_name("Cloud", "a\nb"), "Cloud (ab)");
    }

    #[test]
    fn a_link_from_before_the_replace_marker_still_reads() {
        let old = r#"{"v":1,"url":"wss://x/join/r","host":"cloud","host_id":"h","device":"d","token":"t"}"#;
        let link: Link = serde_json::from_str(old).unwrap();
        assert_eq!(link.replacing, None);
        let mut marked = link.clone();
        marked.replacing = Some(Whole::Pc);
        let text = serde_json::to_string(&marked).unwrap();
        assert!(text.contains(r#""replacing":"pc""#), "{text}");
        assert!(!serde_json::to_string(&link).unwrap().contains("replacing"));
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
