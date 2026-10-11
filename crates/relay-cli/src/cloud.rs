//! `relay cloud …` — this PC's link to a cloud engine (crates/relay-remote/src/cloud.rs,
//! docs/CLOUD.md). `pair` makes it, `sync` runs one round now, `status` says how the last went,
//! `off` forgets it. While `relay serve --remote` runs, the engine merges every minute by itself.

use anyhow::{anyhow, Result};
use clap::Subcommand;
use relay_core::Instance;
use relay_remote::cloud::{self, Link};
use relay_remote::Registry;

#[derive(Subcommand)]
pub enum CloudCommand {
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
    format!("PC ({host})")
}

pub async fn run(instance: Instance, cmd: CloudCommand) -> Result<u8> {
    let path = Link::path_for(instance);
    match cmd {
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
                println!("no cloud link (`relay cloud pair '<relay://pair link>'` makes one)");
                return Ok(0);
            };
            println!("cloud:     {} ({})", link.host, link.url);
            println!("device:    {}", link.device);
            println!("last ok:   {}", link.last_ok.as_deref().unwrap_or("never"));
            if let Some(error) = &link.last_error {
                println!("last error: {error}");
            }
            println!("avex:      {}", if link.gym_digest.is_some() { "an export came home" } else { "nothing yet" });
            Ok(0)
        }
        CloudCommand::Off => {
            match std::fs::remove_file(&path) {
                Ok(()) => println!("cloud link forgotten; revoke this PC on the server with `relay remote revoke <id>` (`relay remote devices` lists it)"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("no cloud link"),
                Err(e) => return Err(e.into()),
            }
            Ok(0)
        }
    }
}
