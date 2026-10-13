//! `remote.json`: which phones may talk to this engine, the pairing codes that are open right
//! now, and where the rendezvous server is. One small file per instance next to the store,
//! mode 0600, re-read on every handshake so `relay remote pair` in a second terminal and the
//! serving process never have to talk to each other.
//!
//! Several processes change it (the door, `relay remote pair`, `revoke`, …), and the door changes
//! it from many connections at once, so every change is a `Registry::update`: a read-modify-write
//! under an exclusive lock on a sibling `remote.json.lock`, saved through a temp file of its own.

use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A phone that completed pairing. The token is the whole credential: it is sent once, at
/// pairing, and afterwards only proven with a per-connection challenge (`wire.rs`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub token: String,
    pub created_at: String,
    #[serde(default)]
    pub last_seen: Option<String>,
}

/// A pairing code somebody typed `relay remote pair` for. Short-lived and single use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pending {
    pub code: String,
    pub created_at: String,
    pub expires_at: String,
    /// The PC must approve the device that presents this code before it gets a token: the
    /// `relay remote pair` that opened the window asks. Without it the first presenter wins.
    #[serde(default)]
    pub confirm: bool,
    /// The device that presented the code and is waiting for the PC's answer. A code answers
    /// one device: a second presenter is refused while this one waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<PairRequest>,
}

/// A device waiting at the door with a code, as the PC is asked about it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairRequest {
    pub device_name: String,
    /// Which door and from where: `direct 192.0.2.7` or `rendezvous`.
    pub origin: String,
    pub at: String,
    /// The PC's answer; `None` until someone gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved: Option<bool>,
}

/// What presenting a pairing code got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presented {
    /// No such open code, or another device is already waiting on it.
    Invalid,
    /// A first-come code: the device is paired.
    Paired(Device),
    /// The PC has to approve; ask `outcome` until it has.
    Waiting,
}

/// Where a waiting pairing stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Waiting,
    Approved(Device),
    /// Declined, cancelled or expired: the code is gone either way.
    Declined,
}

/// The self-hosted rendezvous this engine dials out to (`tunnel.rs`), if any.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rendezvous {
    /// `wss://relay.example.org` or `ws://192.0.2.1:7430`; paths are appended.
    pub url: String,
    /// What a phone asks the server for. Derived from the secret, never guessable.
    pub room: String,
    /// What the host proves to the server. Never leaves this machine except to that server.
    pub secret: String,
    /// Where phones reach the server, when that is not `url`: an engine beside its rendezvous
    /// dials it on loopback, while phones come in through the public proxy (docs/CLOUD.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registry {
    pub v: u32,
    pub host_id: String,
    pub host_name: String,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub pending: Vec<Pending>,
    #[serde(default)]
    pub rendezvous: Option<Rendezvous>,
    /// The port `relay remote serve` last listened on, so `relay remote pair` can print a link
    /// that points at it without being told.
    #[serde(default)]
    pub direct_port: Option<u16>,
}

pub const DEFAULT_PORT: u16 = 7420;
pub const DEFAULT_RENDEZVOUS_PORT: u16 = 7430;
const PAIR_TTL_MINUTES: i64 = 10;

/// Serializes `Registry::update` between threads of one process; the file lock does the same
/// between processes (and between descriptors, so it alone would do — this keeps the wait off
/// the kernel when the door's own connections are the ones racing).
static UPDATE: Mutex<()> = Mutex::new(());

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buf);
    hex(&buf)
}

pub fn sha256_hex(input: &str) -> String {
    use sha2::Digest as _;
    hex(&sha2::Sha256::digest(input.as_bytes()))
}

/// The room a secret opens: the rendezvous server checks `sha256(secret)` against the room it
/// is asked to host, so it stores nothing and a restart forgets nothing.
pub fn room_for(secret: &str) -> String {
    sha256_hex(secret)[..32].to_string()
}

fn now() -> Timestamp {
    Timestamp::now()
}

fn host_name() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "relay".to_string())
}

/// A pairing code a person can read aloud: eight characters from an alphabet without 0/O, 1/I.
fn pair_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut buf = [0u8; 8];
    rand::rng().fill_bytes(&mut buf);
    let chars: Vec<char> = buf
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect();
    format!(
        "{}-{}",
        chars[..4].iter().collect::<String>(),
        chars[4..].iter().collect::<String>()
    )
}

/// A name a device chose for itself, made safe to store and to print in a terminal prompt.
fn device_label(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(64).collect();
    let name = name.trim();
    if name.is_empty() { "phone".to_string() } else { name.to_string() }
}

/// Codes are compared without case, dashes or spaces: what a person types is never exact.
pub fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

impl Registry {
    pub fn path_for(instance: relay_core::Instance) -> PathBuf {
        instance.data_dir().join("remote.json")
    }

    pub fn fresh() -> Registry {
        Registry {
            v: 1,
            host_id: random_hex(8),
            host_name: host_name(),
            devices: Vec::new(),
            pending: Vec::new(),
            rendezvous: None,
            direct_port: None,
        }
    }

    /// Read the file, or start a fresh registry when there is none yet. A registry that cannot
    /// be parsed is an error, not a silent reset: it holds every phone's credential.
    pub fn load(path: &Path) -> Result<Registry> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut reg: Registry = serde_json::from_str(&text)
                    .with_context(|| format!("parsing {}", path.display()))?;
                reg.prune();
                Ok(reg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry::fresh()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Replace the file atomically: a temp file of this write's own, flushed, renamed over the
    /// old one, and the directory flushed so the rename survives a crash. A reader sees the old
    /// file or the new one, never a mix. This does not lock: a read-modify-write is `update`.
    pub fn save(&self, path: &Path) -> Result<()> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let dir = parent_dir(path);
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("remote.json");
        let tmp = dir.join(format!(".{name}.{}.{}.tmp", std::process::id(), random_hex(6)));
        let written = (|| -> Result<()> {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)
                .with_context(|| format!("writing {}", tmp.display()))?;
            f.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all().with_context(|| format!("flushing {}", tmp.display()))?;
            std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
            Ok(())
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    /// The lock file `update` holds: next to the registry, never renamed, so every process
    /// locks the same inode.
    pub fn lock_path(path: &Path) -> PathBuf {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("remote.json");
        parent_dir(path).join(format!("{name}.lock"))
    }

    /// Load, change and save under an exclusive lock, so two writers — the door admitting a
    /// phone while `relay remote revoke` runs, two phones at once — never undo each other.
    /// `f` returning an error saves nothing. Nothing slow belongs in `f`: the lock is held
    /// throughout, and the door's connections wait on it.
    pub fn update<R, E>(path: &Path, f: impl FnOnce(&mut Registry) -> std::result::Result<R, E>) -> std::result::Result<R, E>
    where
        E: From<anyhow::Error>,
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let _thread = UPDATE.lock().unwrap_or_else(|p| p.into_inner());
        let dir = parent_dir(path);
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let lock_path = Registry::lock_path(path);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&lock_path)
            .with_context(|| format!("opening {}", lock_path.display()))?;
        lock.lock().with_context(|| format!("locking {}", lock_path.display()))?;
        let mut registry = Registry::load(path)?;
        let before = registry.clone();
        let out = f(&mut registry)?;
        if registry != before {
            registry.save(path)?;
        }
        // Dropping `lock` closes the descriptor, which releases the lock.
        Ok(out)
    }

    /// Drop pairing codes that expired.
    pub fn prune(&mut self) {
        let now = now();
        self.pending.retain(|p| {
            p.expires_at
                .parse::<Timestamp>()
                .map(|t| t > now)
                .unwrap_or(false)
        });
    }

    /// Open a pairing window: one code, ten minutes, single use. It replaces any window still
    /// open, so a code shown earlier — on a screen share, in scrollback — stops working.
    /// `confirm` makes the PC approve the device that presents it (`Presented::Waiting`).
    pub fn begin_pair(&mut self, confirm: bool) -> Pending {
        self.pending.clear();
        self.add_pair(confirm)
    }

    /// Open a pairing window beside any already open. For windows nobody sees but the device
    /// they are handed to (`connect.rs` opens one per phone allowed on Discord): closing the
    /// others would decline a phone that is mid-pairing, or a code the owner just printed.
    pub fn add_pair(&mut self, confirm: bool) -> Pending {
        let created = now();
        let pending = Pending {
            code: pair_code(),
            created_at: created.to_string(),
            expires_at: (created + PAIR_TTL_MINUTES.minutes()).to_string(),
            confirm,
            request: None,
        };
        self.pending.push(pending.clone());
        pending
    }

    fn pending_index(&mut self, code: &str) -> Option<usize> {
        self.prune();
        let wanted = normalize_code(code);
        if wanted.is_empty() {
            return None;
        }
        self.pending.iter().position(|p| normalize_code(&p.code) == wanted)
    }

    fn mint(&mut self, device_name: &str) -> Device {
        let device = Device {
            id: random_hex(6),
            name: device_label(device_name),
            token: random_hex(32),
            created_at: now().to_string(),
            last_seen: None,
        };
        self.devices.push(device.clone());
        device
    }

    /// A device presents a code. A first-come code is consumed and pairs it at once; a code
    /// that needs the PC's approval records the device as waiting, and answers no one else.
    pub fn present(&mut self, code: &str, device_name: &str, origin: &str) -> Presented {
        let Some(idx) = self.pending_index(code) else { return Presented::Invalid };
        let pending = &mut self.pending[idx];
        if !pending.confirm {
            self.pending.remove(idx);
            return Presented::Paired(self.mint(device_name));
        }
        if pending.request.is_some() {
            return Presented::Invalid;
        }
        pending.request = Some(PairRequest {
            device_name: device_label(device_name),
            origin: origin.chars().filter(|c| !c.is_control()).take(80).collect(),
            at: now().to_string(),
            approved: None,
        });
        Presented::Waiting
    }

    /// Where the device waiting on `code` stands. An answer, either way, consumes the code.
    pub fn outcome(&mut self, code: &str) -> Outcome {
        let Some(idx) = self.pending_index(code) else { return Outcome::Declined };
        match self.pending[idx].request.as_ref().and_then(|r| r.approved.map(|a| (a, r.device_name.clone()))) {
            None => Outcome::Waiting,
            Some((false, _)) => {
                self.pending.remove(idx);
                Outcome::Declined
            }
            Some((true, name)) => {
                self.pending.remove(idx);
                Outcome::Approved(self.mint(&name))
            }
        }
    }

    /// The PC's answer to the device waiting on `code`. `false` when no device is waiting.
    pub fn decide(&mut self, code: &str, approve: bool) -> bool {
        let Some(idx) = self.pending_index(code) else { return false };
        match self.pending[idx].request.as_mut() {
            Some(request) if request.approved.is_none() => {
                request.approved = Some(approve);
                true
            }
            _ => false,
        }
    }

    /// Close a pairing window, answered or not.
    pub fn abandon(&mut self, code: &str) {
        if let Some(idx) = self.pending_index(code) {
            self.pending.remove(idx);
        }
    }

    pub fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub fn touch(&mut self, id: &str) {
        if let Some(d) = self.devices.iter_mut().find(|d| d.id == id) {
            d.last_seen = Some(now().to_string());
        }
    }

    pub fn revoke(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        self.devices.len() != before
    }

    /// Point this engine at a rendezvous server, minting a room secret the first time.
    pub fn set_rendezvous(&mut self, url: &str) -> &Rendezvous {
        let url = url.trim().trim_end_matches('/').to_string();
        match self.rendezvous.as_mut() {
            Some(r) => r.url = url,
            None => {
                let secret = random_hex(32);
                self.rendezvous = Some(Rendezvous {
                    url,
                    room: room_for(&secret),
                    secret,
                    public: None,
                });
            }
        }
        self.rendezvous.as_ref().expect("just set")
    }
}

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_pairs_once_and_then_is_gone() {
        let mut reg = Registry::fresh();
        let pending = reg.begin_pair(false);
        assert_eq!(pending.code.len(), 9);
        let Presented::Paired(device) = reg.present(&pending.code.to_lowercase().replace('-', " "), "  Pixel 8\u{1b}[2J ", "test") else {
            panic!("a first-come code pairs at once")
        };
        assert_eq!(device.name, "Pixel 8[2J", "control characters never reach a terminal");
        assert_eq!(device.token.len(), 64);
        assert_eq!(reg.present(&pending.code, "again", "test"), Presented::Invalid, "codes are single use");
        assert_eq!(reg.device(&device.id).map(|d| &d.token), Some(&device.token));
        assert!(reg.revoke(&device.id));
        assert!(reg.device(&device.id).is_none());
    }

    #[test]
    fn a_confirmed_code_waits_for_the_pc_and_answers_one_device() {
        let mut reg = Registry::fresh();
        let code = reg.begin_pair(true).code;
        assert_eq!(reg.present(&code, "Pixel", "direct 192.0.2.7"), Presented::Waiting);
        assert_eq!(reg.present(&code, "Imposter", "rendezvous"), Presented::Invalid, "one device per code");
        assert_eq!(reg.outcome(&code), Outcome::Waiting);
        assert!(reg.devices.is_empty(), "no token before the PC says yes");
        assert_eq!(reg.pending[0].request.as_ref().unwrap().device_name, "Pixel");
        assert!(reg.decide(&code, true));
        assert!(!reg.decide(&code, false), "an answer is final");
        let Outcome::Approved(device) = reg.outcome(&code) else { panic!("approved") };
        assert_eq!(device.name, "Pixel");
        assert!(reg.pending.is_empty());

        let code = reg.begin_pair(true).code;
        assert!(!reg.decide(&code, true), "nobody is waiting yet");
        assert_eq!(reg.present(&code, "Stranger", "rendezvous"), Presented::Waiting);
        assert!(reg.decide(&code, false));
        assert_eq!(reg.outcome(&code), Outcome::Declined);
        assert_eq!(reg.present(&code, "Stranger", "rendezvous"), Presented::Invalid, "a declined code is spent");
        assert_eq!(reg.devices.len(), 1);
    }

    #[test]
    fn a_new_window_closes_the_old_one() {
        let mut reg = Registry::fresh();
        let old = reg.begin_pair(false).code;
        let new = reg.begin_pair(false).code;
        assert_eq!(reg.present(&old, "late", "test"), Presented::Invalid);
        assert!(matches!(reg.present(&new, "phone", "test"), Presented::Paired(_)));
    }

    #[test]
    fn an_added_window_leaves_the_others_open() {
        let mut reg = Registry::fresh();
        let printed = reg.begin_pair(true).code;
        let a = reg.add_pair(true).code;
        assert_eq!(reg.present(&a, "Phone A", "rendezvous"), Presented::Waiting);
        let b = reg.add_pair(true).code;
        assert_eq!(reg.pending.len(), 3, "a window for phone B closes neither A's nor the printed one");
        assert_eq!(reg.outcome(&a), Outcome::Waiting, "phone A is still waiting, not declined");
        assert!(reg.decide(&a, true));
        assert!(matches!(reg.outcome(&a), Outcome::Approved(_)));
        assert_eq!(reg.present(&b, "Phone B", "rendezvous"), Presented::Waiting);
        assert_eq!(reg.present(&printed, "Owner's phone", "direct 192.0.2.7"), Presented::Waiting);
        // `relay remote pair` still closes every window open before it.
        let fresh = reg.begin_pair(false).code;
        assert_eq!(reg.outcome(&b), Outcome::Declined);
        assert_eq!(reg.pending.len(), 1);
        assert!(matches!(reg.present(&fresh, "phone", "test"), Presented::Paired(_)));
    }

    #[test]
    fn expired_codes_are_pruned_on_load_and_redeem() {
        let mut reg = Registry::fresh();
        reg.pending.push(Pending {
            code: "AAAA-BBBB".into(),
            created_at: "2020-01-01T00:00:00Z".into(),
            expires_at: "2020-01-01T00:10:00Z".into(),
            confirm: false,
            request: None,
        });
        assert_eq!(reg.present("AAAA-BBBB", "late", "test"), Presented::Invalid);
        assert!(reg.pending.is_empty());
    }

    #[test]
    fn the_file_round_trips_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("remote.json");
        let mut reg = Registry::fresh();
        reg.begin_pair(true);
        reg.set_rendezvous("wss://relay.example.org/");
        reg.save(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let back = Registry::load(&path).unwrap();
        assert_eq!(back, reg);
        assert_eq!(back.rendezvous.as_ref().unwrap().url, "wss://relay.example.org");
        assert_eq!(
            back.rendezvous.as_ref().unwrap().room,
            room_for(&back.rendezvous.as_ref().unwrap().secret)
        );
    }

    #[test]
    fn a_missing_file_is_a_fresh_registry_but_a_broken_one_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("remote.json");
        assert!(Registry::load(&path).unwrap().devices.is_empty());
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Registry::load(&path).is_err());
    }
}
