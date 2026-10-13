//! `relay cloud …` — this PC's link to a cloud engine (crates/relay-remote/src/cloud.rs,
//! docs/CLOUD.md). `connect` makes it through the owner's server with one tap on Discord (and lets
//! the Relay app pair with this PC through that server, connect.rs); `pair` makes it from a link
//! printed on the server; `sync` runs one round now, `status` says how the last went, `off`
//! forgets it. While `relay serve --remote` runs, the engine merges every minute by itself.

use anyhow::{anyhow, Result};
use clap::Subcommand;
use relay_core::Instance;
use relay_remote::cloud::{self, Link};
use relay_remote::connect::{Claim, Server};
use relay_remote::Registry;
use std::time::Duration;

#[derive(Subcommand)]
pub enum CloudCommand {
    /// Connect this PC through your server: you allow it with one tap on Discord. It then syncs
    /// with the cloud copy there, and the Relay app can pair with this PC through the same server
    Connect {
        /// The server's address, e.g. my-bot.onrender.com
        server: String,
    },
    /// Pair this PC with a cloud engine: the link `relay remote pair --via-only` printed on the server
    Pair { link: String },
    /// Merge with the cloud copy now (the engine does it every minute while `relay serve --remote` runs)
    Sync,
    /// Where the link points and how its last round went
    Status,
    /// Forget the cloud engine: this PC stops merging with it (revoke it there with `relay remote revoke`)
    Off,
}

/// How the cloud engine lists this PC.
pub fn pc_name(instance: Instance) -> String {
    let host = Registry::load(&Registry::path_for(instance)).map(|r| r.host_name).unwrap_or_else(|_| "Relay".into());
    cloud::device_name("PC", &host)
}

/// Runs a blocking call to the server off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    Ok(tokio::task::spawn_blocking(f).await?)
}

/// Asks the server to let this PC in, waits for the owner's tap, and returns the link and token.
async fn ask_and_wait(api: &Server, name: &str) -> Result<(String, Option<String>)> {
    let (a, n) = (api.clone(), name.to_string());
    let asked = blocking(move || a.ask("pc", &n)).await??;
    if asked.notified {
        println!("Your bot sent you a Discord message. Tap Allow if it shows the code {}.", asked.code);
    } else {
        println!("Your bot could not message you: allow the request on its dashboard, Devices (code {}).", asked.code);
    }
    println!("Waiting… (the request lasts 15 minutes; Ctrl+C stops waiting)");
    let mut troubles = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let (a, q) = (api.clone(), asked.clone());
        match blocking(move || a.claim(&q)).await? {
            Claim::Waiting | Claim::WaitingPc => {}
            Claim::Ready { link, host_token } => return Ok((link, host_token)),
            Claim::Denied => return Err(anyhow!("the request was denied")),
            Claim::Expired => return Err(anyhow!("the request expired: run `relay cloud connect` again")),
            Claim::Spent => return Err(anyhow!("the request was used up: run `relay cloud connect` again")),
            Claim::Trouble(reason) => {
                troubles += 1;
                if troubles >= 10 {
                    return Err(anyhow!("the server keeps failing: {reason}"));
                }
            }
        }
    }
}

/// Connecting, pairing and forgetting the link are the person's: each one hands this PC's data
/// to a cloud engine or cuts it off. An agent's shell carries `RELAY_SESSION` (providers.rs);
/// its exec gate refuses these too, and this is the same rule for a command it reaches some
/// other way. A seatbelt, not a boundary (BUS.md §9.5).
fn refuse_in_agent_session(cmd: &CloudCommand, session: Option<&str>) -> Result<()> {
    let what = match cmd {
        CloudCommand::Connect { .. } => "connect",
        CloudCommand::Pair { .. } => "pair",
        CloudCommand::Off => "off",
        CloudCommand::Sync | CloudCommand::Status => return Ok(()),
    };
    match session.filter(|s| !s.is_empty()) {
        Some(session) => Err(anyhow!("`relay cloud {what}` is the user's to run, not an agent's (this shell is session {session})")),
        None => Ok(()),
    }
}

pub async fn run(instance: Instance, cmd: CloudCommand) -> Result<u8> {
    refuse_in_agent_session(&cmd, std::env::var("RELAY_SESSION").ok().as_deref())?;
    let path = Link::path_for(instance);
    match cmd {
        CloudCommand::Connect { server } => {
            let api = Server::new(&server);
            let a = api.clone();
            let hello = blocking(move || a.hello()).await??;
            if !hello.cloud {
                return Err(anyhow!("{} takes requests, but its cloud copy is not running (RELAY_PUBLIC_URL is off there)", api.base()));
            }
            let existing = Link::load(&path)?;
            if let Some(old) = &existing {
                if old.server.as_deref() == Some(api.base()) && old.server_token.is_some() {
                    println!("This PC is already connected through {}.", api.base());
                    return Ok(0);
                }
            }
            let name = Registry::load(&Registry::path_for(instance)).map(|r| r.host_name).unwrap_or_else(|_| "this PC".into());
            println!("Asking {} to let {name} in…", api.base());
            let (pair_link, token) = ask_and_wait(&api, &name).await?;
            // Linked with the cloud copy already (`relay cloud pair`), this only adds the server.
            // Looked at again: `relay cloud off` may have run while this waited.
            if Link::load(&path)?.is_none() {
                println!("Allowed. Pairing with the cloud copy…");
                cloud::pair(&path, &pair_link, &pc_name(instance)).await?;
            }
            // The link as it is now, under the round lock: a round may have moved it on meanwhile.
            let _lock = cloud::lock(&path).await?;
            let Some(mut link) = Link::load(&path)? else {
                return Err(anyhow!("the cloud link was removed while this ran: run `relay cloud connect` again"));
            };
            link.server = Some(api.base().to_string());
            link.server_token = token;
            link.save(&path)?;
            println!("Connected to {} ({}) as device {}.", link.host, api.base(), link.device);
            println!("Restart `relay serve --remote` (or Relay) to start merging with the cloud copy and taking the Relay app's requests to pair; `relay cloud sync` merges now.");
            Ok(0)
        }
        CloudCommand::Pair { link } => {
            if let Some(old) = Link::load(&path)? {
                return Err(anyhow!("this PC is already linked to {} ({}); `relay cloud off` first", old.host, old.url));
            }
            println!("Pairing… if the server asks, answer y in its terminal.");
            let link = cloud::pair(&path, &link, &pc_name(instance)).await?;
            println!("Linked to {} ({}) as device {}.", link.host, link.url, link.device);
            println!("A running `relay serve --remote` starts merging when it restarts; `relay cloud sync` merges now.");
            Ok(0)
        }
        CloudCommand::Sync => {
            let report = cloud::round(&instance.socket_path(), &path, &pc_name(instance)).await?;
            match report.replaced_by {
                Some("pc") => println!("This PC's ledger was restored or replaced: it replaced the cloud copy ({} rows).", report.to_cloud),
                Some(_) => println!("The cloud copy was replaced (Tally's first sync, or a restore): it replaced this PC's ledger ({} rows).", report.to_pc),
                None => println!("Merged: {} rows came to this PC, {} went to the cloud.", report.to_pc, report.to_cloud),
            }
            if report.gym {
                println!("A newer Avex export came home.");
            }
            Ok(0)
        }
        CloudCommand::Status => {
            let Some(link) = Link::load(&path)? else {
                println!("no cloud link (`relay cloud connect <your server>` makes one)");
                return Ok(0);
            };
            println!("cloud:     {} ({})", link.host, link.url);
            println!("device:    {}", link.device);
            println!("last ok:   {}", link.last_ok.as_deref().unwrap_or("never"));
            if let Some(error) = &link.last_error {
                println!("last error: {error}");
            }
            println!("avex:      {}", if link.gym_digest.is_some() { "an export came home" } else { "nothing yet" });
            match (&link.server, &link.server_token) {
                (Some(server), Some(_)) => println!("server:    {server} (the Relay app can pair through it)"),
                _ => println!("server:    none (`relay cloud connect <server>` lets the Relay app pair through it)"),
            }
            Ok(0)
        }
        CloudCommand::Off => {
            // After any round under way, which would otherwise write the link back.
            let _lock = cloud::lock(&path).await?;
            match std::fs::remove_file(&path) {
                Ok(()) => println!("cloud link forgotten; revoke this PC on the server with `relay remote revoke <id>` (`relay remote devices` lists it)"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("no cloud link"),
                Err(e) => return Err(e.into()),
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{refuse_in_agent_session, CloudCommand};

    #[test]
    fn an_agent_session_cannot_connect_pair_or_forget_the_link() {
        let session = Some("steady-badger");
        for cmd in [CloudCommand::Connect { server: "x.test".into() }, CloudCommand::Pair { link: "x".into() }, CloudCommand::Off] {
            let refused = refuse_in_agent_session(&cmd, session).unwrap_err().to_string();
            assert!(refused.contains("the user's to run") && refused.contains("steady-badger"), "{refused}");
            refuse_in_agent_session(&cmd, None).expect("the person runs it");
            refuse_in_agent_session(&cmd, Some("")).expect("an empty RELAY_SESSION is no session");
        }
        for cmd in [CloudCommand::Sync, CloudCommand::Status] {
            refuse_in_agent_session(&cmd, session).expect("reading and merging are not the person's alone");
        }
    }
}
