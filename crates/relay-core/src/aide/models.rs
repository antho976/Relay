//! The models a job may run on, and how to reach each.
//!
//! - **Claude**, through the person's own Claude account: the CLI's sign-in, as every thread uses.
//! - **This PC**: a model Relay runs with llama.cpp (`local_models`), started when a run needs it.
//! - **The person's own server**: an endpoint in Aide's settings that answers Anthropic's
//!   Messages API (llama.cpp's server does; vLLM or LiteLLM in front of a farm can). The agent is
//!   still Claude Code, pointed at that address instead of Anthropic's.

use crate::engine::Engine;
use relay_aide::model::{Endpoint, Settings};
use relay_aide::policy::Host;
use relay_bus::ops::aide::ModelOption;
use relay_bus::types::Provider;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Claude's models a job may name: (id, name). Claude's default when a job names none.
pub const CLAUDE: &[(&str, &str)] = &[
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
];

/// How long a server is given to answer whether it is there.
pub const PROBE: Duration = Duration::from_millis(1500);

/// The ids of the models Relay can run on this PC now: their files are in the models folder.
pub fn pc_models(engine: &Engine) -> Vec<&'static str> {
    let dir = crate::local_models::dir(engine);
    crate::local_models::MODELS.iter().filter(|m| dir.join(m.file).is_file()).map(|m| m.id).collect()
}

fn claude_ready(engine: &Engine) -> Result<(), String> {
    let conn = engine.store.lock();
    crate::providers::executable(&conn, Provider::Claude).map(|_| ()).map_err(|e| format!("Claude is not available: {}", e.message))
}

/// The address an endpoint's URL names, for a quick look at whether it answers.
fn address(url: &str) -> Option<SocketAddr> {
    let (rest, default_port) = match url.strip_prefix("https://") {
        Some(r) => (r, 443),
        None => (url.strip_prefix("http://")?, 80),
    };
    let host_port = if rest.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) { rest.to_string() } else { format!("{rest}:{default_port}") };
    host_port.to_socket_addrs().ok()?.next()
}

/// Whether the endpoint's server takes a connection now.
pub fn reachable(endpoint: &Endpoint) -> Result<(), String> {
    let addr = address(&endpoint.url).ok_or_else(|| format!("{} is not an address", endpoint.url))?;
    TcpStream::connect_timeout(&addr, PROBE).map(|_| ()).map_err(|e| format!("{} does not answer: {e}", endpoint.url))
}

/// Whether `model` on `host` can run now, and why not.
pub fn available(engine: &Engine, settings: &Settings, model: Option<&str>, host: Host) -> Result<(), String> {
    match host {
        // Claude Code runs every model, so the server's model is checked as Claude's is too.
        Host::Claude => claude_ready(engine),
        Host::Pc => {
            claude_ready(engine)?;
            crate::local_models::server_bin(engine).map(|_| ())
        }
        Host::Server => {
            claude_ready(engine)?;
            let e = model.and_then(|m| settings.endpoint(m)).ok_or("no such server model")?;
            reachable(e)
        }
    }
}

/// Every model a job may name, each with whether it can run now. `probe` asks each server.
pub fn options(engine: &Engine, settings: &Settings, probe: bool) -> Vec<ModelOption> {
    let claude = claude_ready(engine);
    let mut out: Vec<ModelOption> = CLAUDE.iter().map(|(id, name)| ModelOption {
        id: id.to_string(),
        name: name.to_string(),
        host: Host::Claude.as_str().into(),
        available: claude.is_ok(),
        note: claude.clone().err(),
    }).collect();
    let dir = crate::local_models::dir(engine);
    let server = crate::local_models::server_bin(engine);
    for m in crate::local_models::MODELS {
        let present = dir.join(m.file).is_file();
        let note = if !present { Some(format!("{} is not in {}", m.file, dir.display())) } else { server.clone().err() };
        out.push(ModelOption { id: m.id.into(), name: format!("{} (this PC)", m.name), host: Host::Pc.as_str().into(), available: note.is_none(), note });
    }
    for e in &settings.endpoints {
        let note = if probe { reachable(e).err() } else { None };
        out.push(ModelOption { id: e.id.clone(), name: format!("{} (your server)", e.name), host: Host::Server.as_str().into(), available: note.is_none(), note });
    }
    out
}

/// The agent's environment for a thread on a server model: Claude Code pointed at the server.
pub fn server_env(endpoint: &Endpoint) -> Vec<(&'static str, String)> {
    let model = endpoint.model.clone().unwrap_or_else(|| endpoint.id.clone());
    let mut env = vec![
        ("ANTHROPIC_BASE_URL", endpoint.url.clone()),
        // A server on the person's own network takes any token; one that wants a key reads it
        // from its own configuration, never from Relay.
        ("ANTHROPIC_AUTH_TOKEN", "relay-aide".to_string()),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_string()),
        ("CLAUDE_CODE_ATTRIBUTION_HEADER", "0".to_string()),
        ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", endpoint.context.unwrap_or(32_768).to_string()),
    ];
    for var in ["ANTHROPIC_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"] {
        env.push((var, model.clone()));
    }
    env
}

/// The server model `model` names in Aide's settings, if it is one.
pub fn endpoint(engine: &Engine, model: Option<&str>) -> Option<Endpoint> {
    let m = model?;
    super::settings(engine).ok()?.endpoint(m).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_has_a_port() {
        assert_eq!(address("http://127.0.0.1:8080"), Some("127.0.0.1:8080".parse().unwrap()));
        assert_eq!(address("http://127.0.0.1"), Some("127.0.0.1:80".parse().unwrap()));
        assert_eq!(address("https://127.0.0.1"), Some("127.0.0.1:443".parse().unwrap()));
        assert_eq!(address("ftp://x"), None);
    }

    #[test]
    fn a_server_model_points_claude_code_at_it() {
        let e = Endpoint { id: "farm".into(), name: "Farm".into(), url: "http://10.0.0.9:8080".into(), model: Some("qwen-72b".into()), context: Some(131_072), vision: false };
        let env = server_env(&e);
        let get = |k: &str| env.iter().find(|(v, _)| *v == k).map(|(_, x)| x.as_str());
        assert_eq!(get("ANTHROPIC_BASE_URL"), Some("http://10.0.0.9:8080"));
        assert_eq!(get("ANTHROPIC_MODEL"), Some("qwen-72b"));
        assert_eq!(get("CLAUDE_CODE_MAX_CONTEXT_TOKENS"), Some("131072"));
    }

    /// A port on this machine that is bound and held, but not listening: a connection to it is
    /// refused, and no other test can take it while the socket is open. (A listener dropped to
    /// close its port leaves the port free for a test running beside this one.)
    #[cfg(unix)]
    fn held_closed_port() -> (std::os::fd::OwnedFd, u16) {
        use std::os::fd::FromRawFd;
        // SAFETY: a fresh socket, owned at once; the addresses are plain structs of the size given.
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
            assert!(fd >= 0, "socket: {}", std::io::Error::last_os_error());
            let socket = std::os::fd::OwnedFd::from_raw_fd(fd);
            let mut addr: libc::sockaddr_in = std::mem::zeroed();
            addr.sin_family = libc::AF_INET as libc::sa_family_t;
            addr.sin_addr.s_addr = u32::from(std::net::Ipv4Addr::LOCALHOST).to_be();
            let mut len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            assert_eq!(libc::bind(fd, (&raw const addr).cast(), len), 0, "bind: {}", std::io::Error::last_os_error());
            assert_eq!(libc::getsockname(fd, (&raw mut addr).cast(), &mut len), 0);
            (socket, u16::from_be(addr.sin_port))
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_closed_port_is_unreachable() {
        let endpoint = |port: u16| Endpoint { id: "x".into(), name: "x".into(), url: format!("http://127.0.0.1:{port}"), model: None, context: None, vision: false };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(reachable(&endpoint(listener.local_addr().unwrap().port())).is_ok());
        let (_held, closed) = held_closed_port();
        assert!(reachable(&endpoint(closed)).is_err());
    }
}
