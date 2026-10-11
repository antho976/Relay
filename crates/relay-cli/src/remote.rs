//! `relay remote …` — the phone door (crates/relay-remote), driven from the CLI.
//!
//! `serve` fronts a running engine on the LAN and, when a rendezvous is configured, dials out
//! to it. `pair` opens a ten-minute window, prints the code as a QR for the phone's camera, and
//! stays to ask before the phone that presents it is paired. Everything else edits
//! `remote.json`, always through `Registry::update` so concurrent writers never undo each other.

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use relay_core::Instance;
use relay_remote::direct::{is_tailnet, lan_addresses, DirectServer};
use relay_remote::pairlink::PairLink;
use relay_remote::rendezvous::RendezvousServer;
use relay_remote::registry::normalize_code;
use relay_remote::{Ctx, Registry, DEFAULT_PORT, DEFAULT_RENDEZVOUS_PORT};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Subcommand)]
pub enum RemoteCommand {
    /// Front the running engine for paired phones: on the LAN, and through the rendezvous if one is set
    Serve {
        /// Address to listen on for phones on the same network
        #[arg(long, default_value_t = format!("0.0.0.0:{DEFAULT_PORT}"))]
        bind: String,
        /// Also open a pairing window and print its QR code at start
        #[arg(long)]
        pair: bool,
        /// With --pair: let the first phone with the code pair without asking here
        #[arg(long)]
        no_confirm: bool,
        /// Do not dial the configured rendezvous this time
        #[arg(long)]
        no_rendezvous: bool,
        /// Start the engine when a paired phone arrives and none is running, and serve phones
        /// while it is down: the login service for a PC whose desktop app is closed
        /// (deploy/relay-door.service)
        #[arg(long)]
        start_engine: bool,
    },
    /// Open a ten-minute pairing window, print the code and QR for the phone, and ask before it pairs
    Pair {
        /// Let the first phone with the code pair without asking (for a terminal you cannot answer in)
        #[arg(long)]
        no_confirm: bool,
        /// Leave this machine's own addresses out of the link: only the rendezvous reaches it. For
        /// an engine in a container, whose addresses no phone can reach (docs/CLOUD.md)
        #[arg(long)]
        via_only: bool,
    },
    /// List paired phones
    Devices,
    /// Forget a paired phone by id
    Revoke { id: String },
    /// Rename this machine as phones see it
    Name { name: String },
    /// Set (or with `off`, clear) the rendezvous server this engine dials out to
    Via {
        url: String,
        /// Where phones reach that server, when it is not `url`: for an engine that dials its
        /// rendezvous on loopback behind a public proxy (docs/CLOUD.md)
        #[arg(long)]
        public: Option<String>,
    },
    /// Run a rendezvous server: what `via` points at. Host it anywhere you trust, behind TLS.
    Rendezvous {
        #[arg(long, default_value_t = format!("0.0.0.0:{DEFAULT_RENDEZVOUS_PORT}"))]
        bind: String,
        /// Bytes queued toward every phone together, in MiB (512 by default). Lower it on a small
        /// server that hosts other things too; never below what one lane may queue (30 MiB)
        #[arg(long)]
        queue_mib: Option<usize>,
    },
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("RELAY_LOG").unwrap_or_else(|_| "info".into()),
        )
        .init();
}

fn print_pair_link(link: &PairLink, confirm: bool) {
    println!();
    if let Some(qr) = link.qr() {
        println!("{qr}");
    }
    println!("Pairing code: {}   (valid for 10 minutes, one phone; any earlier code no longer works)", link.code);
    if confirm {
        println!("This terminal asks before the phone that presents it is paired.");
    } else {
        println!("Not confirmed here: the first phone that presents this code is paired.");
    }
    println!("Host: {} ({})", link.host, link.instance);
    if link.direct.is_empty() {
        println!("Direct: no LAN address found; the phone can only reach this engine through a rendezvous");
    } else {
        println!("Direct: {}", link.direct.join("  "));
    }
    // A tailnet address is still the direct door, reachable wherever the phone is on the tailnet.
    let tailnet: Vec<&String> = link
        .direct
        .iter()
        .filter(|url| {
            url.trim_start_matches("ws://").split(':').next().and_then(|ip| ip.parse().ok()).is_some_and(is_tailnet)
        })
        .collect();
    if !tailnet.is_empty() {
        println!("Tailscale: {} — reachable from anywhere the phone is on your tailnet", tailnet[0]);
    }
    match &link.via {
        Some(via) => println!("Via:    {via}"),
        None => println!("Via:    none — set one with `relay remote via wss://your-server` to reach this PC away from home"),
    }
    println!("\nScan the code from the Relay app's PC tab, or type the link:\n{}\n", link.to_url());
}

/// Whether pairing can be confirmed here: it needs a person at this terminal.
fn confirm_wanted(no_confirm: bool) -> Result<bool> {
    use std::io::IsTerminal as _;
    if no_confirm {
        return Ok(false);
    }
    if !std::io::stdin().is_terminal() {
        return Err(anyhow!("no terminal to confirm the pairing in; pass --no-confirm to let the first phone with the code pair"));
    }
    Ok(true)
}

/// Stay with the open window and ask about the phone that presents its code. Returns once the
/// window is answered, expires, or is replaced by a newer one.
async fn confirm_pairing(path: PathBuf, code: String) -> Result<()> {
    let wanted = normalize_code(&code);
    println!("Waiting for a phone… (Ctrl+C closes the window)");
    loop {
        let registry = Registry::load(&path)?;
        let Some(pending) = registry.pending.iter().find(|p| normalize_code(&p.code) == wanted) else {
            println!("The pairing window closed (it expired, or a newer `relay remote pair` replaced it).");
            return Ok(());
        };
        if let Some(request) = pending.request.as_ref().filter(|r| r.approved.is_none()) {
            let question = format!("Pair \"{}\" ({}) with this PC? It gets full control as you. [y/N] ", request.device_name, request.origin);
            let yes = tokio::task::spawn_blocking(move || {
                use std::io::Write as _;
                print!("{question}");
                let _ = std::io::stdout().flush();
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer).map(|_| matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes"))
            })
            .await??;
            let decided = Registry::update(&path, |r| Ok::<_, anyhow::Error>(r.decide(&code, yes)))?;
            match (decided, yes) {
                (false, _) => println!("Too late: the phone stopped waiting. Run `relay remote pair` again."),
                (true, true) => println!("Approved. `relay remote devices` lists it; `relay remote revoke <id>` removes it."),
                (true, false) => println!("Declined. The code is spent; nothing was paired."),
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Close a pairing window nobody is going to answer.
fn abandon_pairing(path: &Path, code: &str) {
    let _ = Registry::update(path, |r| {
        r.abandon(code);
        Ok::<_, anyhow::Error>(())
    });
}

/// `relay serve --remote`: the engine and its phone door in one process, so a machine that
/// starts Relay at login is reachable from the phone without a second command.
pub async fn serve_with_door(
    instance: Instance,
    store: Option<std::path::PathBuf>,
    bind: &str,
    profile: relay_remote::wire::OpProfile,
) -> std::result::Result<(), relay_core::socket::BindError> {
    let addr: SocketAddr = bind
        .parse()
        .with_context(|| format!("bad --remote-bind {bind:?}"))
        .map_err(relay_core::socket::BindError::Other)?;
    let served = relay_core::serve::start(instance, store).await?;
    tracing::info!(instance = %instance, store = %served.engine.store.path().display(), "engine up");
    let ctx = Arc::new(Ctx {
        instance,
        registry_path: Registry::path_for(instance),
        socket_path: served.socket.path.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        // The door lives inside this engine and ends with it.
        start_engine: None,
        ops: profile,
    });
    // The door is a convenience on top of the engine, never a reason for the engine not to
    // start: a port already taken by another instance's door is logged and skipped, and so is a
    // `remote.json` that cannot be read or written — it is never reset, since it holds every
    // phone's credential, so the door stays shut until a person fixes it.
    let door = match DirectServer::bind(ctx.clone(), addr).await {
        Ok(door) => Some(door),
        Err(e) => {
            eprintln!("relay serve: phone door not opened ({e:#}); the engine runs without it — `relay remote serve --bind <addr>` opens one later");
            None
        }
    };
    // A door that did not open clears the port: a stale one would send `relay remote pair`'s
    // link to whichever door holds it now, often another instance's.
    let port = door.as_ref().map(|d| d.local_addr.port());
    let registry = Registry::update(&ctx.registry_path, |r| {
        r.direct_port = port;
        Ok::<_, anyhow::Error>(r.clone())
    });
    let (door, registry) = match registry {
        Ok(registry) => (door, Some(registry)),
        Err(e) => {
            eprintln!("relay serve: {} is unusable ({e:#}); the engine runs without the phone door or rendezvous — fix or move the file, then restart", ctx.registry_path.display());
            tracing::warn!(error = %format!("{e:#}"), path = %ctx.registry_path.display(), "remote.json unusable; phone door disabled");
            (None, None)
        }
    };
    let rendezvous = registry.as_ref().and_then(|r| r.rendezvous.clone());
    let tunnel = rendezvous
        .as_ref()
        .map(|r| relay_remote::tunnel::spawn(ctx.clone(), r.clone()));
    if let Some(door) = &door {
        println!("relay serve: phone door on {} ({}); `relay remote pair` adds a phone", door.local_addr,
            match &rendezvous { Some(r) => format!("dialing {}", r.url), None => "no rendezvous".to_string() });
    }
    // A PC linked to a cloud engine merges with it while it runs (docs/CLOUD.md).
    let cloud_path = relay_remote::cloud::Link::path_for(instance);
    let cloud = match relay_remote::cloud::Link::load(&cloud_path) {
        Ok(Some(link)) => {
            println!("relay serve: merging with the cloud copy at {} every minute", link.host);
            Some(relay_remote::cloud::spawn(ctx.socket_path.clone(), cloud_path, crate::cloud::pc_name(instance)))
        }
        Ok(None) => None,
        Err(e) => {
            eprintln!("relay serve: {} is unreadable ({e:#}); not merging with the cloud copy", cloud_path.display());
            None
        }
    };
    let quit = served.engine.clone();
    tokio::select! {
        _ = quit.wait_quit() => tracing::info!("app.quit received"),
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT"),
        _ = sigterm() => tracing::info!("SIGTERM"),
    }
    if let Some(t) = tunnel {
        t.abort();
    }
    if let Some(c) = cloud {
        c.abort();
    }
    drop(door);
    // Quitting first and the socket door closed before the sessions go (RA-333).
    served.stop();
    Ok(())
}

async fn sigterm() {
    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        Ok(mut s) => {
            s.recv().await;
        }
        Err(_) => std::future::pending::<()>().await,
    }
}

/// The desktop launcher runs the `dev` engine by default and `relay` defaults to `stable`, so
/// the first thing a person types would otherwise be answered with "no engine". When the
/// asked-for instance has no engine and exactly one other does, use that one and say so;
/// `remote.json` is per instance, so pairing follows the engine it belongs to.
async fn resolve_instance(requested: Instance) -> Instance {
    if relay_core::socket::probe(&requested.socket_path()).await {
        return requested;
    }
    let mut alive = Vec::new();
    for candidate in [Instance::Stable, Instance::Dev, Instance::Test] {
        if candidate != requested && relay_core::socket::probe(&candidate.socket_path()).await {
            alive.push(candidate);
        }
    }
    match alive.as_slice() {
        [only] => {
            eprintln!("relay remote: no {requested} engine is running; using the {only} engine (pass --instance to choose)");
            *only
        }
        _ => requested,
    }
}

pub async fn run(requested: Instance, cmd: RemoteCommand) -> Result<u8> {
    let instance = match cmd {
        // A door that starts its own engine fronts the instance it was asked for, never another
        // that happens to be running.
        RemoteCommand::Rendezvous { .. } | RemoteCommand::Serve { start_engine: true, .. } => requested,
        _ => resolve_instance(requested).await,
    };
    let ctx = Arc::new(Ctx::for_instance(instance));
    match cmd {
        RemoteCommand::Serve { bind, pair, no_confirm, no_rendezvous, start_engine } => {
            init_logging();
            let addr: SocketAddr = bind.parse().with_context(|| format!("bad --bind {bind:?}"))?;
            let ctx = if start_engine {
                let relay = std::env::current_exe().context("finding this relay binary to start the engine with")?;
                Arc::new(Ctx { start_engine: Some(relay), ..(*ctx).clone() })
            } else {
                ctx
            };
            if !start_engine && !relay_core::socket::probe(&ctx.socket_path).await {
                eprintln!(
                    "relay remote: no engine answering at {} — start one with `./run.sh`, `relay serve`, or `relay serve --remote` (which needs no second command)",
                    ctx.socket_path.display()
                );
                return Ok(5);
            }
            let confirm = pair && confirm_wanted(no_confirm)?;
            let door = DirectServer::bind(ctx.clone(), addr).await?;
            let port = door.local_addr.port();
            let (registry, link_code) = Registry::update(&ctx.registry_path, |r| {
                r.direct_port = Some(port);
                let code = pair.then(|| r.begin_pair(confirm).code);
                Ok::<_, anyhow::Error>((r.clone(), code))
            })?;

            let tunnel = match (&registry.rendezvous, no_rendezvous) {
                (Some(r), false) => Some(relay_remote::tunnel::spawn(ctx.clone(), r.clone())),
                _ => None,
            };
            println!("relay remote: listening on {} for phones on this network", door.local_addr);
            match (&registry.rendezvous, tunnel.is_some()) {
                (Some(r), true) => println!("relay remote: dialing rendezvous {} (room {})", r.url, r.room),
                (Some(_), false) => println!("relay remote: rendezvous configured but skipped (--no-rendezvous)"),
                (None, _) => println!("relay remote: no rendezvous configured; phones must be on this network"),
            }
            if let Some(code) = link_code {
                // A door bound to one address is reachable at that address only.
                let addresses = match door.local_addr.ip() {
                    std::net::IpAddr::V4(ip) if !ip.is_unspecified() => vec![ip],
                    _ => lan_addresses(),
                };
                let link = PairLink::build(&registry, instance.as_str(), &code, door.local_addr.port(), &addresses);
                print_pair_link(&link, confirm);
                if confirm {
                    let path = ctx.registry_path.clone();
                    tokio::spawn(async move {
                        if let Err(e) = confirm_pairing(path, code).await {
                            eprintln!("relay remote: pairing confirmation ended: {e:#}");
                        }
                    });
                }
            } else {
                println!("relay remote: run `relay remote pair` in another terminal to add a phone");
            }
            tokio::signal::ctrl_c().await?;
            if let Some(t) = tunnel {
                t.abort();
            }
            drop(door);
            Ok(0)
        }
        RemoteCommand::Pair { no_confirm, via_only } => {
            let confirm = confirm_wanted(no_confirm)?;
            let (registry, code) = Registry::update(&ctx.registry_path, |r| {
                let code = r.begin_pair(confirm).code;
                Ok::<_, anyhow::Error>((r.clone(), code))
            })?;
            let port = registry.direct_port.unwrap_or(DEFAULT_PORT);
            if via_only && registry.rendezvous.is_none() {
                abandon_pairing(&ctx.registry_path, &code);
                return Err(anyhow!("--via-only needs a rendezvous: set one with `relay remote via wss://…` first"));
            }
            let lan = if via_only { Vec::new() } else { lan_addresses() };
            let link = PairLink::build(&registry, instance.as_str(), &code, port, &lan);
            if registry.direct_port.is_none() && !via_only {
                println!("note: the {instance} engine has no phone door open: `relay remote serve` has not run for it, or `relay serve --remote` could not open its port (another instance's door may hold it). The link assumes port {DEFAULT_PORT}, which may be another instance's door; `relay remote serve --bind 0.0.0.0:<port>` opens one on a free port.");
            }
            print_pair_link(&link, confirm);
            if confirm {
                tokio::select! {
                    done = confirm_pairing(ctx.registry_path.clone(), code.clone()) => done?,
                    _ = tokio::signal::ctrl_c() => {
                        abandon_pairing(&ctx.registry_path, &code);
                        println!("\nPairing window closed.");
                    }
                }
            }
            Ok(0)
        }
        RemoteCommand::Devices => {
            let registry = Registry::load(&ctx.registry_path)?;
            if registry.devices.is_empty() {
                println!("no paired phones (run `relay remote pair`)");
                return Ok(0);
            }
            println!("{:<14} {:<24} {:<26} last seen", "id", "name", "paired");
            for d in &registry.devices {
                println!("{:<14} {:<24} {:<26} {}", d.id, d.name, d.created_at, d.last_seen.as_deref().unwrap_or("never"));
            }
            if !registry.pending.is_empty() {
                println!("\n{} pairing window(s) open", registry.pending.len());
            }
            Ok(0)
        }
        RemoteCommand::Revoke { id } => {
            if !Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.revoke(&id)))? {
                return Err(anyhow!("no paired phone with id {id:?}"));
            }
            println!("revoked {id}; a connection it has open is closed within a few seconds, and the next is refused");
            Ok(0)
        }
        RemoteCommand::Name { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(anyhow!("a name cannot be empty"));
            }
            let host_name: String = name.chars().take(64).collect();
            Registry::update(&ctx.registry_path, |r| {
                r.host_name = host_name.clone();
                Ok::<_, anyhow::Error>(())
            })?;
            println!("phones will see this machine as {host_name:?}");
            Ok(0)
        }
        RemoteCommand::Via { url, public } => {
            if url.eq_ignore_ascii_case("off") {
                Registry::update(&ctx.registry_path, |r| {
                    r.rendezvous = None;
                    Ok::<_, anyhow::Error>(())
                })?;
                println!("rendezvous cleared; phones must be on this network");
                return Ok(0);
            }
            if !(url.starts_with("ws://") || url.starts_with("wss://")) {
                return Err(anyhow!("the rendezvous URL must start with ws:// or wss:// (wss:// is what you want on the internet)"));
            }
            // Behind a public `wss://` the plain hop is loopback, where it reaches no network.
            if url.starts_with("ws://") && public.is_none() {
                eprintln!("warning: ws:// is unencrypted; a pairing through it exposes the phone's token to the network in between. Put TLS in front (see docs/MOBILE.md).");
            }
            if let Some(public) = &public {
                if !public.starts_with("wss://") {
                    return Err(anyhow!("--public must start with wss://: phones pair through it, and their token must not cross the internet in the clear"));
                }
            }
            let public = public.map(|p| p.trim().trim_end_matches('/').to_string());
            let r = Registry::update(&ctx.registry_path, |r| {
                let mut set = r.set_rendezvous(&url).clone();
                set.public = public.clone();
                r.rendezvous = Some(set.clone());
                Ok::<_, anyhow::Error>(set)
            })?;
            println!("rendezvous: {}\nroom:       {}\nphones join at {}", r.url, r.room, relay_remote::tunnel::join_url(&r));
            println!("restart `relay remote serve` to dial it");
            Ok(0)
        }
        RemoteCommand::Rendezvous { bind, queue_mib } => {
            init_logging();
            let addr: SocketAddr = bind.parse().with_context(|| format!("bad --bind {bind:?}"))?;
            let mut limits = relay_remote::rendezvous::Limits::default();
            if let Some(mib) = queue_mib {
                // One lane's budget must still fit, or its largest line could never be queued.
                limits.queued_total = (mib << 20).max(limits.lane_budget);
            }
            let server = RendezvousServer::bind_with(addr, limits).await?;
            println!("relay rendezvous: listening on {} (health: GET /health)", server.local_addr);
            tokio::signal::ctrl_c().await?;
            drop(server);
            Ok(0)
        }
    }
}
