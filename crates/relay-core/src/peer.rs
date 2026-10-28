//! Who is at the other end of a socket connection (RA-096, D165).
//!
//! The socket door carries the user as well as the agents, and `user` has no token: same uid,
//! same trust (BUS.md §4.2). So until now an agent refused by a guardrail could answer its own
//! hold by writing `{"actor":"user","op":"guardrail.confirm",…}` to the socket. What every
//! connection does carry is a process: the kernel names the one that connected (`SO_PEERCRED`),
//! and its parent links say where it runs. A process inside an agent session's tree, or anywhere
//! else under the engine, may not speak as `user` or `test`. The engine is made a child
//! subreaper (`PR_SET_CHILD_SUBREAPER`) so a session descendant that double-forks or calls
//! `setsid` is reparented to the engine rather than to init, and stays below it.
//!
//! One exception, and a narrow one: a thread's agent (docs/THREADS.md) is the person's own
//! conversation and has no session, so Relay's MCP server under it speaks as `user`. A process in a
//! thread agent's tree may, but only for the ops that thread is given (`threads::AGENT_OPS`, or
//! `threads::ASSISTANT_OPS` for the Dev space's assistant); the engine checks the op on every
//! request, whatever the client in between lists.

use crate::engine::Engine;
use std::collections::{HashMap, HashSet};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the process behind a connection is to this engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Peer {
    /// Outside the engine's process tree: the desktop app, a terminal, `relay remote serve`.
    Outside,
    /// The engine's own process: an in-process door (the phone bridge of `relay serve --remote`).
    Engine,
    /// Inside this agent session's process tree.
    Session(String),
    /// Inside the process tree of this thread's agent.
    Thread(relay_bus::types::Id),
    /// Inside the process tree of this assistant thread's agent (the Dev space's assistant).
    Assistant(relay_bus::types::Id),
    /// Inside the process tree of this Aide thread's agent (docs/AIDE.md).
    Aide(relay_bus::types::Id),
    /// Below the engine but in no live session: a session's orphan, a build, a hook.
    EngineChild,
    /// Not identifiable: a peer in another pid namespace, an unreadable `/proc`, a process gone.
    Unknown(&'static str),
}

impl Peer {
    /// May this connection claim `user` (or `test`)? Fails closed: only a peer known to be
    /// outside every session and every engine child may.
    pub fn may_act_as_user(&self) -> bool {
        matches!(self, Peer::Outside | Peer::Engine)
    }

    /// May this connection claim `user` for `op`? As [`Peer::may_act_as_user`], and a thread's
    /// agent for its own ops alone.
    pub fn may_act_as_user_for(&self, op: &str) -> bool {
        match self {
            Peer::Thread(_) => crate::threads::peer_may_call(op),
            Peer::Assistant(_) => crate::threads::assistant_may_call(op),
            Peer::Aide(_) => crate::threads::aide_may_call(op),
            _ => self.may_act_as_user(),
        }
    }

    /// The typed refusal for a `user`/`test` claim from this peer.
    pub fn refusal(&self, actor: &str) -> relay_bus::BusError {
        let (whence, details) = match self {
            Peer::Session(name) => (
                format!("from inside agent session {name:?}'s process tree"),
                serde_json::json!({"peer": "session", "session": name}),
            ),
            Peer::Thread(id) => (
                format!("from thread {id}'s agent, which acts for you only through that thread's tools"),
                serde_json::json!({"peer": "thread", "thread": id}),
            ),
            Peer::Assistant(id) => (
                format!("from the assistant (thread {id}), which acts for you only through its own tools and asks you for the rest"),
                serde_json::json!({"peer": "assistant", "thread": id}),
            ),
            Peer::Aide(id) => (
                format!("from Aide (thread {id}), which acts for you only through its own tools"),
                serde_json::json!({"peer": "aide", "thread": id}),
            ),
            Peer::EngineChild => (
                "from a process the engine started outside every session (an orphan of a session, a build or a hook)".to_string(),
                serde_json::json!({"peer": "engine_child"}),
            ),
            Peer::Unknown(why) => (
                format!("from a process the engine could not identify ({why})"),
                serde_json::json!({"peer": "unknown", "why": why}),
            ),
            Peer::Outside | Peer::Engine => (String::new(), serde_json::json!({})),
        };
        relay_bus::BusError::refused(
            "actor.peer",
            format!(
                "this connection comes {whence}, so it cannot act as `{actor}`: user actions must come from the Relay app, \
                 the phone, or a terminal outside Relay's sessions"
            ),
        )
        .with_details(details)
        .with_hint("an agent's requests carry its own actor and token (RELAY_SESSION, RELAY_TOKEN); a guardrail hold is answered by the person")
    }
}

/// Identify the process behind a freshly accepted connection. Done once per connection: a
/// process cannot leave the tree it was born in, and every later request is answered from this.
pub fn identify(stream: &tokio::net::UnixStream, engine: &Engine) -> Peer {
    let pid = match stream.peer_cred().ok().and_then(|cred| cred.pid()) {
        Some(pid) if pid > 0 => pid as u32,
        // 0: the peer lives in a pid namespace this one cannot see.
        _ => return Peer::Unknown("no peer pid"),
    };
    // Taken before the walk, so the walk can be shown to have read the process that connected
    // and not a stranger that inherited its number (kernels before 6.5 have no SO_PEERPIDFD).
    let pidfd = peer_pidfd(stream.as_raw_fd());
    let peer = classify_with(pid, std::process::id(), &engine.session_pids(), &crate::threads::agent_pids(engine), proc_ppid);
    match pidfd {
        Some(fd) if !still_running(&fd) => Peer::Unknown("the process has exited"),
        _ => peer,
    }
}

/// Parent links are followed at most this far: deeper than any real tree, so a cycle (a pid
/// reused mid-walk) ends the walk instead of spinning.
const MAX_DEPTH: usize = 1024;

/// Walk `peer`'s ancestry. `sessions` maps each live session's PTY child pid to its name;
/// `ppid_of` reads one parent link (`None` when the process cannot be read).
#[cfg(test)]
pub(crate) fn classify(
    peer: u32,
    engine: u32,
    sessions: &HashMap<u32, String>,
    ppid_of: impl Fn(u32) -> Option<u32>,
) -> Peer {
    classify_with(peer, engine, sessions, &HashMap::new(), ppid_of)
}

/// [`classify`], also knowing each running thread agent's pid (`threads`, pid to
/// [`Peer::Thread`] or [`Peer::Assistant`]).
pub(crate) fn classify_with(
    peer: u32,
    engine: u32,
    sessions: &HashMap<u32, String>,
    threads: &HashMap<u32, Peer>,
    ppid_of: impl Fn(u32) -> Option<u32>,
) -> Peer {
    if peer == 0 {
        return Peer::Unknown("no peer pid");
    }
    if peer == engine {
        return Peer::Engine;
    }
    let mut pid = peer;
    for _ in 0..MAX_DEPTH {
        if let Some(name) = sessions.get(&pid) {
            return Peer::Session(name.clone());
        }
        if let Some(thread) = threads.get(&pid) {
            return thread.clone();
        }
        let Some(parent) = ppid_of(pid) else {
            return Peer::Unknown("its ancestry is unreadable");
        };
        if parent == engine {
            return Peer::EngineChild;
        }
        // Init (or a namespace's init) has no parent: the top, and the engine was not on the way.
        if parent == 0 {
            return Peer::Outside;
        }
        pid = parent;
    }
    Peer::Unknown("its ancestry is too deep")
}

/// The fields of `/proc/<pid>/stat` this module reads.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Stat {
    pub state: char,
    pub ppid: u32,
    pub sid: u32,
    pub start: u64,
}

/// `pid (comm) state ppid pgrp session … starttime …`. `comm` may hold spaces and parentheses,
/// so the fields are counted from the last `)`.
pub(crate) fn parse_stat(stat: &str) -> Option<Stat> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let fields: Vec<&str> = rest.split_ascii_whitespace().collect();
    Some(Stat {
        state: fields.first()?.chars().next()?,
        ppid: fields.get(1)?.parse().ok()?,
        sid: fields.get(3)?.parse().ok()?,
        start: fields.get(19)?.parse().ok()?,
    })
}

fn proc_stat(pid: u32) -> Option<Stat> {
    parse_stat(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

fn proc_ppid(pid: u32) -> Option<u32> {
    proc_stat(pid).map(|stat| stat.ppid)
}

fn peer_pidfd(socket: std::os::fd::RawFd) -> Option<OwnedFd> {
    let mut fd: libc::c_int = -1;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(socket, libc::SOL_SOCKET, libc::SO_PEERPIDFD, (&mut fd as *mut libc::c_int).cast(), &mut len)
    };
    (rc == 0 && fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) })
}

/// A pidfd polls readable once its process has exited.
fn still_running(pidfd: &OwnedFd) -> bool {
    let mut poll = libc::pollfd { fd: pidfd.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    unsafe { libc::poll(&mut poll, 1, 0) == 0 }
}

/// Make this process the reaper of every orphan below it, so a session descendant that
/// double-forks or calls `setsid` stays in the engine's tree, where the socket door can still
/// see it is not the user. Called wherever an engine is served (`serve::start`).
pub fn become_subreaper() -> bool {
    unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) == 0 }
}

/// How often the orphan reaper looks, and how long a zombie must have been one before it is
/// taken: two passes apart.
const REAP_EVERY: Duration = Duration::from_secs(30);

/// Reap the orphans a subreaper inherits. Nothing else waits for them, so each one that exits
/// would stay a zombie until the engine does.
///
/// Only a zombie in another session than the engine's is taken: every child the engine spawns
/// itself (git, gh, adb, a build shell) inherits the engine's session and has an owner that waits
/// for it — reaping one would hand that owner `ECHILD`, and a later `kill` a recycled pid. What
/// is left is a session's tree (each PTY child calls `setsid`) and anything that called `setsid`
/// itself; a live session's own PTY child is reaped by its waiter thread and is skipped here.
/// A daemon a build forks without `setsid` stays a zombie when it exits; there are few.
pub fn spawn_orphan_reaper(engine: &Arc<Engine>) {
    let weak = Arc::downgrade(engine);
    std::thread::Builder::new()
        .name("orphan-reaper".into())
        .spawn(move || {
            crate::background_priority();
            let me = std::process::id();
            let session = unsafe { libc::getsid(0) } as u32;
            let mut seen = HashSet::new();
            loop {
                let deadline = Instant::now() + REAP_EVERY;
                while Instant::now() < deadline {
                    std::thread::sleep(Duration::from_secs(1));
                    match weak.upgrade() {
                        Some(engine) if !engine.is_quitting() => {}
                        _ => return,
                    }
                }
                let Some(engine) = weak.upgrade() else { return };
                let ptys = engine.session_pids();
                drop(engine);
                seen = reap_pass(me, session, &ptys, seen);
            }
        })
        .ok();
}

/// One pass: zombies seen on the previous pass are reaped, new ones are remembered.
fn reap_pass(
    me: u32,
    session: u32,
    ptys: &HashMap<u32, String>,
    seen: HashSet<(u32, u64)>,
) -> HashSet<(u32, u64)> {
    let mut next = HashSet::new();
    let Ok(entries) = std::fs::read_dir("/proc") else { return next };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) else { continue };
        let Some(stat) = proc_stat(pid) else { continue };
        if stat.ppid != me || stat.state != 'Z' || stat.sid == session || ptys.contains_key(&pid) {
            continue;
        }
        // Keyed by start time too, so a pid recycled between passes starts over.
        let key = (pid, stat.start);
        if seen.contains(&key) {
            unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) };
        } else {
            next.insert(key);
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(links: &[(u32, u32)]) -> impl Fn(u32) -> Option<u32> + '_ {
        move |pid| links.iter().find(|(child, _)| *child == pid).map(|(_, parent)| *parent)
    }

    const ENGINE: u32 = 500;

    fn sessions() -> HashMap<u32, String> {
        HashMap::from([(600, "calm-otter".to_string())])
    }

    #[test]
    fn a_process_under_a_session_is_that_session() {
        // agent 700 → shell 650 → PTY child 600 → engine 500 → systemd 2 → init 1
        let links = [(700, 650), (650, 600), (600, ENGINE), (ENGINE, 2), (2, 1), (1, 0)];
        assert_eq!(classify(700, ENGINE, &sessions(), tree(&links)), Peer::Session("calm-otter".into()));
        assert_eq!(classify(600, ENGINE, &sessions(), tree(&links)), Peer::Session("calm-otter".into()));
    }

    #[test]
    fn an_orphan_reparented_to_the_engine_is_not_the_user() {
        // a double-forked client whose parent exited: the subreaper adopted it
        let links = [(800, ENGINE), (ENGINE, 1), (1, 0)];
        let peer = classify(800, ENGINE, &sessions(), tree(&links));
        assert_eq!(peer, Peer::EngineChild);
        assert!(!peer.may_act_as_user());
        // a build's grandchild, too
        let links = [(810, 805), (805, ENGINE), (ENGINE, 1), (1, 0)];
        assert_eq!(classify(810, ENGINE, &sessions(), tree(&links)), Peer::EngineChild);
    }

    #[test]
    fn a_terminal_outside_the_engine_is_the_user_and_so_is_the_engine() {
        // relay CLI 900 → fish 890 → terminal 880 → systemd --user 2 → init 1
        let links = [(900, 890), (890, 880), (880, 2), (2, 1), (1, 0), (ENGINE, 2)];
        let peer = classify(900, ENGINE, &sessions(), tree(&links));
        assert_eq!(peer, Peer::Outside);
        assert!(peer.may_act_as_user());
        assert_eq!(classify(ENGINE, ENGINE, &sessions(), tree(&links)), Peer::Engine);
        assert!(Peer::Engine.may_act_as_user());
    }

    #[test]
    fn what_cannot_be_read_fails_closed() {
        assert!(!classify(0, ENGINE, &sessions(), tree(&[])).may_act_as_user());
        // the chain breaks halfway (a process gone, /proc unreadable)
        let links = [(900, 890)];
        assert_eq!(classify(900, ENGINE, &sessions(), tree(&links)), Peer::Unknown("its ancestry is unreadable"));
        // a cycle never terminates by itself
        let links = [(900, 901), (901, 900)];
        assert!(matches!(classify(900, ENGINE, &sessions(), tree(&links)), Peer::Unknown(_)));
    }

    #[test]
    fn stat_fields_are_counted_from_the_last_parenthesis() {
        let line = "4242 (we(ird) n)ame) Z 17 4242 4242 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 987654 0 0";
        assert_eq!(parse_stat(line), Some(Stat { state: 'Z', ppid: 17, sid: 4242, start: 987654 }));
        let own = parse_stat(&std::fs::read_to_string("/proc/self/stat").unwrap()).unwrap();
        assert_eq!(own.ppid, unsafe { libc::getppid() } as u32);
        assert_eq!(own.sid, unsafe { libc::getsid(0) } as u32);
        assert_eq!(parse_stat("12 (truncated"), None);
    }

    #[test]
    fn the_reaper_takes_only_zombies_in_another_session_and_only_on_a_second_look() {
        use std::os::unix::process::CommandExt;
        fn zombie(own_session: bool) -> u32 {
            let mut command = std::process::Command::new("true");
            if own_session {
                unsafe { command.pre_exec(|| { libc::setsid(); Ok(()) }) };
            }
            // Dropped without a wait, as an orphan's parent never waits for it here.
            let pid = command.spawn().unwrap().id();
            for _ in 0..500 {
                if proc_stat(pid).is_some_and(|stat| stat.state == 'Z') {
                    return pid;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("{pid} never became a zombie");
        }
        let me = std::process::id();
        let session = unsafe { libc::getsid(0) } as u32;
        let orphan = zombie(true);
        let ours = zombie(false);
        let first = reap_pass(me, session, &HashMap::new(), HashSet::new());
        assert!(first.iter().any(|(pid, _)| *pid == orphan), "remembered, not yet reaped");
        assert!(!first.iter().any(|(pid, _)| *pid == ours), "a child in the engine's session has an owner");
        assert!(proc_stat(orphan).is_some());
        // A live session's PTY child is its waiter thread's to reap.
        let sessions = HashMap::from([(orphan, "calm-otter".to_string())]);
        assert!(reap_pass(me, session, &sessions, first.clone()).is_empty());
        assert!(proc_stat(orphan).is_some());
        reap_pass(me, session, &HashMap::new(), first);
        assert!(proc_stat(orphan).is_none(), "reaped on the second look");
        assert!(proc_stat(ours).is_some_and(|stat| stat.state == 'Z'));
        unsafe { libc::waitpid(ours as i32, std::ptr::null_mut(), 0) };
    }

    #[test]
    fn a_thread_agent_acts_for_the_person_only_through_its_tools() {
        // relay mcp 960 → claude 950 (thread 7's agent) → engine 500
        let links = [(960, 950), (950, ENGINE), (ENGINE, 1), (1, 0)];
        let threads = HashMap::from([(950, Peer::Thread(7))]);
        let peer = classify_with(960, ENGINE, &sessions(), &threads, tree(&links));
        assert_eq!(peer, Peer::Thread(7));
        assert!(!peer.may_act_as_user(), "never a blanket user");
        assert!(peer.may_act_as_user_for("money.summary"));
        assert!(peer.may_act_as_user_for("money.tx.add"));
        for op in ["money.invest.summary", "money.invest.list", "money.invest.add"] {
            assert!(peer.may_act_as_user_for(op), "{op}");
        }
        assert!(peer.may_act_as_user_for("bus.ops"), "to list its tools");
        assert!(peer.may_act_as_user_for("thread.propose"), "it asks the person first");
        for op in [
            "money.reset", "money.tx.delete", "guardrail.confirm", "thread.create", "session.input", "settings.set",
            "money.invest.import", "money.invest.preview", "money.invest.delete", "money.invest.room", "money.fx.fetch", "money.account.update",
            "money.budget.set", "money.account.add", "thread.proposal.resolve", "thread.proposal.undo", "thread.proposal.list",
        ] {
            assert!(!peer.may_act_as_user_for(op), "{op}");
        }
        // Another engine child is not a thread because a thread runs.
        let links = [(810, 805), (805, ENGINE), (ENGINE, 1), (1, 0)];
        let other = classify_with(810, ENGINE, &sessions(), &threads, tree(&links));
        assert_eq!(other, Peer::EngineChild);
        assert!(!other.may_act_as_user_for("money.summary"));
        assert!(!Peer::Session("calm-otter".into()).may_act_as_user_for("money.summary"));
        assert!(Peer::Outside.may_act_as_user_for("money.reset"));
        assert!(Peer::Thread(7).refusal("user").message.contains("thread 7"));
    }

    #[test]
    fn the_assistant_acts_for_the_person_only_through_its_own_tools() {
        // relay mcp 960 → claude 950 (assistant thread 9's agent) → engine 500
        let links = [(960, 950), (950, ENGINE), (ENGINE, 1), (1, 0)];
        let threads = HashMap::from([(950, Peer::Assistant(9))]);
        let peer = classify_with(960, ENGINE, &sessions(), &threads, tree(&links));
        assert_eq!(peer, Peer::Assistant(9));
        assert!(!peer.may_act_as_user());
        for op in ["session.list", "session.transcript", "task.list", "mailbox.send", "thread.ask", "bus.ops"] {
            assert!(peer.may_act_as_user_for(op), "{op}");
        }
        // What it asks for runs only when the person approves it, from the app.
        for op in [
            "session.create", "session.spawn", "session.close", "session.input", "workspace.create", "project.add",
            "thread.ask.resolve", "guardrail.confirm", "settings.set", "project.remove", "money.summary", "thread.propose",
        ] {
            assert!(!peer.may_act_as_user_for(op), "{op}");
        }
        assert!(peer.refusal("user").message.contains("assistant"));
    }

    #[test]
    fn refusals_say_where_user_actions_belong() {
        let error = Peer::Session("calm-otter".into()).refusal("user");
        assert_eq!(error.code, "actor.peer");
        assert!(error.message.contains("calm-otter") && error.message.contains("terminal outside Relay's sessions"), "{}", error.message);
    }
}
