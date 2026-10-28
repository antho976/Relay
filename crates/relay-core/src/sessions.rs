//! Session rows, names and tokens (BUS.md §2, §4.2, §11.3). Names are adjective-animal
//! handles, unique among sessions that are not closed; tokens bind the agent actor.

use anyhow::Result;
use relay_bus::error::BusError;
use relay_bus::types::{Id, Provider, Role, Session, SessionState};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

const ADJECTIVES: &[&str] = &[
    "brisk", "calm", "clever", "bold", "quiet", "swift", "keen", "gentle", "lucky", "merry", "nimble", "plucky",
    "proud", "quick", "sharp", "sly", "snug", "spry", "steady", "sunny", "tidy", "vivid", "witty", "zesty",
    "amber", "cobalt", "coral", "ivory", "jade", "olive", "rusty", "silver", "teal", "umber", "violet", "golden",
];
const ANIMALS: &[&str] = &[
    "otter", "heron", "lynx", "badger", "falcon", "gecko", "ibis", "jackal", "koala", "lemur", "marten", "newt",
    "osprey", "puffin", "quail", "raven", "seal", "tapir", "urchin", "vole", "walrus", "yak", "zebra", "bison",
    "crane", "dingo", "egret", "ferret", "gannet", "hare", "iguana", "kestrel", "llama", "moose", "narwhal", "ocelot",
];

pub fn state_str(s: SessionState) -> &'static str {
    match s {
        SessionState::Created => "created",
        SessionState::Spawning => "spawning",
        SessionState::Running => "running",
        SessionState::Idle => "idle",
        SessionState::Blocked => "blocked",
        SessionState::Parked => "parked",
        SessionState::Restorable => "restorable",
        SessionState::Exited => "exited",
        SessionState::Closed => "closed",
    }
}

pub fn parse_state(s: &str) -> SessionState {
    match s {
        "created" => SessionState::Created,
        "spawning" => SessionState::Spawning,
        "running" => SessionState::Running,
        "idle" => SessionState::Idle,
        "blocked" => SessionState::Blocked,
        "parked" => SessionState::Parked,
        "restorable" => SessionState::Restorable,
        "exited" => SessionState::Exited,
        _ => SessionState::Closed,
    }
}

pub fn provider_str(p: Provider) -> &'static str {
    match p {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Shell => "shell",
    }
}
pub fn parse_provider(s: &str) -> Provider {
    match s {
        "codex" => Provider::Codex,
        "shell" => Provider::Shell,
        _ => Provider::Claude,
    }
}
pub fn role_str(r: Role) -> &'static str {
    match r {
        Role::Builder => "builder",
        Role::Reviewer => "reviewer",
        Role::Docs => "docs",
    }
}
pub fn parse_role(s: &str) -> Role {
    match s {
        "reviewer" => Role::Reviewer,
        "docs" => Role::Docs,
        _ => Role::Builder,
    }
}

/// A fresh name not used by any non-closed session.
pub fn new_name(conn: &Connection) -> Result<String> {
    use rand::seq::IndexedRandom;
    let mut rng = rand::rng();
    for _ in 0..200 {
        let name = format!("{}-{}", ADJECTIVES.choose(&mut rng).unwrap(), ANIMALS.choose(&mut rng).unwrap());
        let taken: bool = conn
            .prepare_cached("SELECT 1 FROM sessions WHERE name = ?1 AND state != 'closed'")?
            .query_row([&name], |_| Ok(()))
            .optional()?
            .is_some();
        if !taken {
            return Ok(name);
        }
    }
    anyhow::bail!("could not find a free session name after 200 tries")
}

pub fn new_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 24];
    rand::rng().fill_bytes(&mut b);
    crate::hex(&b)
}

/// A session row plus the columns the bus type does not carry.
#[derive(Debug, Clone)]
pub struct Row_ {
    pub session: Session,
    pub token: String,
    pub epoch: u64,
    pub launch_prompt: Option<String>,
}

pub fn row(r: &Row) -> rusqlite::Result<Row_> {
    let usage: Option<String> = r.get("usage")?;
    Ok(Row_ {
        session: Session {
            id: r.get("id")?,
            name: r.get("name")?,
            project_id: r.get("project_id")?,
            provider: parse_provider(&r.get::<_, String>("provider")?),
            role: parse_role(&r.get::<_, String>("role")?),
            model: r.get("model")?,
            effort: r.get("effort")?,
            branch: r.get("branch")?,
            worktree: r.get("worktree")?,
            task_id: r.get("task_id")?,
            module_id: r.get("module_id")?,
            pair_with: r.get("pair_with")?,
            bus_writes: r.get::<_, i64>("bus_writes")? != 0,
            allow_ui: r.get::<_, i64>("allow_ui")? != 0,
            use_api_key: r.get::<_, i64>("use_api_key")? != 0,
            state: parse_state(&r.get::<_, String>("state")?),
            pid: r.get("pid")?,
            exit_code: r.get("exit_code")?,
            provider_ref: r.get("provider_ref")?,
            spawned_at: r.get("spawned_at")?,
            last_output_at: r.get("last_output_at")?,
            intent: r.get("intent")?,
            usage: usage.and_then(|s| serde_json::from_str::<Value>(&s).ok()),
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
            closed_at: r.get("closed_at")?,
        },
        token: r.get("token")?,
        epoch: r.get::<_, i64>("epoch")? as u64,
        launch_prompt: r.get("launch_prompt")?,
    })
}

// Every request that carries an agent actor looks a session up at least twice — to
// authenticate it, then again to authorize the op. `Connection::query_row` re-compiles its SQL
// on each call; these go through the connection's prepared-statement cache instead.
pub fn by_id(conn: &Connection, id: Id) -> Result<Option<Row_>, BusError> {
    conn.prepare_cached("SELECT * FROM sessions WHERE id = ?1")
        .map_err(crate::engine::internal)?
        .query_row([id], row)
        .optional()
        .map_err(crate::engine::internal)
}

/// The non-closed session with this name (names are unique among live sessions).
pub fn by_name(conn: &Connection, name: &str) -> Result<Row_, BusError> {
    conn.prepare_cached("SELECT * FROM sessions WHERE name = ?1 AND state != 'closed' ORDER BY id DESC LIMIT 1")
        .map_err(crate::engine::internal)?
        .query_row([name], row)
        .optional()
        .map_err(crate::engine::internal)?
        .ok_or_else(|| BusError::not_found("session.not_found", format!("no live session named {name:?}")))
}

/// The open sessions that review and advance through tasks together with `session`, itself
/// included, as (id, role), oldest first.
///
/// A pooled checkout (`.relay/worktrees/<name>`) is made for one session and shared only by
/// its PAIR or review group, so everyone on it is in the group. Any other checkout — the
/// primary above all, where every session of an Unreal-plugin project lands (D160) — is shared
/// by independent agents: there the group is only the sessions joined by `pair_with`.
pub fn review_group(conn: &Connection, session: &Session) -> Result<Vec<(Id, Role)>, BusError> {
    let project: String = conn
        .prepare_cached("SELECT path FROM projects WHERE id=?1").map_err(crate::engine::internal)?
        .query_row([session.project_id], |row| row.get(0)).map_err(crate::engine::internal)?;
    let mut stmt = conn
        .prepare_cached("SELECT id, name, role, pair_with FROM sessions WHERE worktree=?1 AND state!='closed' ORDER BY id")
        .map_err(crate::engine::internal)?;
    let open = stmt
        .query_map([&session.worktree], |row| Ok((
            row.get::<_, Id>(0)?, row.get::<_, String>(1)?, parse_role(&row.get::<_, String>(2)?), row.get::<_, Option<String>>(3)?,
        )))
        .map_err(crate::engine::internal)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::engine::internal)?;
    let pooled = std::path::Path::new(&session.worktree).starts_with(crate::worktree::pool_dir(std::path::Path::new(&project)));
    let mut members: Vec<String> = vec![session.name.clone()];
    if pooled {
        members = open.iter().map(|(_, name, _, _)| name.clone()).collect();
    } else {
        // `pair_with` links, either direction, until nothing new joins.
        loop {
            let before = members.len();
            for (_, name, _, pair) in &open {
                if members.contains(name) {
                    if let Some(pair) = pair.as_ref().filter(|pair| !members.contains(pair)) {
                        if open.iter().any(|(_, other, _, _)| other == pair) { members.push(pair.clone()); }
                    }
                } else if pair.as_ref().is_some_and(|pair| members.contains(pair)) {
                    members.push(name.clone());
                }
            }
            if members.len() == before { break; }
        }
    }
    Ok(open.into_iter().filter(|(_, name, _, _)| members.contains(name)).map(|(id, _, role, _)| (id, role)).collect())
}

pub fn is_live(s: SessionState) -> bool {
    matches!(s, SessionState::Spawning | SessionState::Running | SessionState::Idle | SessionState::Blocked)
}

pub fn save_scrollback(
    conn: &Connection,
    session_id: Id,
    text: &str,
    epoch: u64,
    seq: u64,
    now: &str,
) -> Result<(), BusError> {
    conn.prepare_cached(
        "INSERT INTO session_scrollback(session_id,text,epoch,seq,updated_at) VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(session_id) DO UPDATE SET text=excluded.text,epoch=excluded.epoch,seq=excluded.seq,updated_at=excluded.updated_at",
    )
    .map_err(crate::engine::internal)?
    .execute(params![session_id, text.as_bytes(), epoch as i64, seq as i64, now])
    .map(|_| ())
    .map_err(crate::engine::internal)
}

pub fn load_scrollback(conn: &Connection, session_id: Id) -> Result<Option<(String, u64, u64)>, BusError> {
    conn.prepare_cached("SELECT text,epoch,seq FROM session_scrollback WHERE session_id=?1")
        .map_err(crate::engine::internal)?
        .query_row(
        [session_id],
        |row| {
            let bytes: Vec<u8> = row.get(0)?;
            Ok((String::from_utf8_lossy(&bytes).into_owned(), row.get::<_, i64>(1)? as u64, row.get::<_, i64>(2)? as u64))
        },
    ).optional().map_err(crate::engine::internal)
}
