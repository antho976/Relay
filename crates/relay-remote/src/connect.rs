//! Connecting through the owner's server (docs/CLOUD.md, "Connecting through your server"): the
//! bot beside the cloud copy (antho976/antho-s-bot, `/api/connect`) takes a request, the owner
//! allows it with one tap on Discord, and the device collects a `relay://pair` link.
//!
//! A PC connects this way once (`relay cloud connect`): it pairs with the cloud copy, and keeps a
//! token for the server. While `relay serve --remote` runs it polls with that token for Relay
//! phones the owner allowed, opens a pairing window of its own for each, hands the phone its link
//! through the server, and asks on this desktop before it pairs the phone. The Discord tap alone
//! never gives a phone this PC.

use crate::pairlink::PairLink;
use crate::registry::{normalize_code, random_hex, sha256_hex, Registry};
use crate::{Ctx, DEFAULT_PORT};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often a running PC asks its server for Relay phones waiting on it.
pub const POLL: Duration = Duration::from_secs(5);
/// How long the desktop question stays up: the phone waits 90 s for the PC's answer.
const ASK_FOR: Duration = Duration::from_secs(75);
/// A pairing window lasts ten minutes (registry.rs); past that nobody is coming.
const WINDOW: Duration = Duration::from_secs(10 * 60);

/// The server's address as it is given: `my-bot.onrender.com`, its https URL, or the cloud
/// copy's `wss://…/relay` address all name the same server, which answers at its root.
pub fn normalize(server: &str) -> String {
    let s = server.trim();
    let (scheme, rest) = match s.split_once("://") {
        Some(("ws", rest)) | Some(("http", rest)) => ("http", rest),
        Some((_, rest)) => ("https", rest),
        None => ("https", s),
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    format!("{scheme}://{host}")
}

/// What `GET /api/connect` says.
#[derive(Debug, Clone, Deserialize)]
pub struct Hello {
    pub service: String,
    #[serde(default)]
    pub cloud: bool,
    /// Whether a PC checks in there for Relay phones.
    #[serde(default)]
    pub pc: bool,
}

/// A request the owner has not answered yet, and the secret that collects its link.
#[derive(Debug, Clone)]
pub struct Asked {
    pub id: String,
    /// The four characters the DM shows too.
    pub code: String,
    pub expires_at: i64,
    /// Whether the bot reached the owner on Discord; if not, the dashboard lists the request.
    pub notified: bool,
    verifier: String,
}

/// Where a request stands.
#[derive(Debug, Clone, PartialEq)]
pub enum Claim {
    Waiting,
    /// Allowed; the owner's PC has not opened its window yet (a Relay phone's request).
    WaitingPc,
    Denied,
    Expired,
    Spent,
    /// The pairing link, and for a PC its token for the server.
    Ready { link: String, host_token: Option<String> },
    Trouble(String),
}

/// A Relay phone the owner allowed, waiting for this PC.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WaitingPhone {
    pub id: String,
    /// As people read it: "the Relay app on Pixel 8".
    pub device: String,
    pub code: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
}

/// The owner's server. Every call blocks (ureq): from async code, run it on `spawn_blocking`.
#[derive(Clone)]
pub struct Server {
    base: String,
    agent: ureq::Agent,
}

impl Server {
    pub fn new(server: &str) -> Server {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            // Render wakes a sleeping service on the first request; give it the time it takes.
            .timeout_global(Some(Duration::from_secs(45)))
            .http_status_as_error(false)
            .build()
            .into();
        Server { base: normalize(server), agent }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn post(&self, path: &str, body: Value) -> Result<(u16, Value)> {
        let mut response = self
            .agent
            .post(&format!("{}{path}", self.base))
            .send_json(&body)
            .with_context(|| format!("reaching {}", self.base))?;
        let status = response.status().as_u16();
        let value = response.body_mut().read_json::<Value>().unwrap_or(Value::Null);
        Ok((status, value))
    }

    /// Whether the address answers as a server that takes requests to connect.
    pub fn hello(&self) -> Result<Hello> {
        let mut response = self
            .agent
            .get(&format!("{}/api/connect", self.base))
            .call()
            .with_context(|| format!("reaching {}", self.base))?;
        let status = response.status().as_u16();
        let hello = (status == 200).then(|| response.body_mut().read_json::<Hello>().ok()).flatten();
        match hello {
            Some(h) if h.service == "relay-connect" => Ok(h),
            _ => Err(anyhow!("{} does not answer as your server (no /api/connect there; is the bot up to date?)", self.base)),
        }
    }

    /// Asks the owner to let `device` connect, as `kind` (`pc` for this PC). A phone on the same
    /// network may have asked a moment before (the server takes one request per address every few
    /// seconds): that waits and asks again.
    pub fn ask(&self, kind: &str, device: &str) -> Result<Asked> {
        let verifier = random_hex(32);
        let challenge = sha256_hex(&verifier);
        let body = json!({"kind": kind, "device": device, "challenge": challenge});
        let mut tries = 0;
        let (status, o) = loop {
            let (status, o) = self.post("/api/connect/enroll", body.clone())?;
            tries += 1;
            if status == 429 && o["error"] == "slow_down" && tries < 4 {
                std::thread::sleep(Duration::from_secs(6));
                continue;
            }
            break (status, o);
        };
        if status == 201 {
            let id = o["id"].as_str().ok_or_else(|| anyhow!("the server's answer had no request id"))?;
            return Ok(Asked {
                id: id.to_string(),
                code: o["code"].as_str().unwrap_or_default().to_string(),
                expires_at: o["expiresAt"].as_i64().unwrap_or_default(),
                notified: o["notified"].as_bool().unwrap_or(false),
                verifier,
            });
        }
        Err(anyhow!(match o["error"].as_str() {
            Some("off") => "your server does not take requests to connect: it is turned off there".to_string(),
            Some("cloud_off") => "your server's cloud copy is not running (RELAY_PUBLIC_URL is off)".to_string(),
            Some("no_pc") => "no PC is connected to your server yet".to_string(),
            Some("busy") => "your server has requests waiting already: answer those, then try again".to_string(),
            Some("slow_down") => "something on your network keeps asking: wait a minute and try again".to_string(),
            _ => format!("your server could not take the request ({status})"),
        }))
    }

    /// Where `asked` stands; once allowed, its link.
    pub fn claim(&self, asked: &Asked) -> Claim {
        match self.post("/api/connect/claim", json!({"id": asked.id, "verifier": asked.verifier})) {
            Ok((status, o)) => parse_claim(status, &o),
            Err(e) => Claim::Trouble(format!("{e:#}")),
        }
    }

    /// Relay phones waiting for this PC. `None` when the server no longer knows this PC's token
    /// (it was forgotten on the dashboard).
    pub fn phones(&self, token: &str) -> Result<Option<Vec<WaitingPhone>>> {
        let (status, o) = self.post("/api/connect/host/phones", json!({"token": token}))?;
        match status {
            200 => Ok(Some(serde_json::from_value(o["phones"].clone()).context("reading the waiting phones")?)),
            401 => Ok(None),
            _ => Err(anyhow!("the server answered {status}")),
        }
    }

    /// Hands the phone waiting on request `id` this PC's pairing link. False when it is no longer
    /// waiting (it expired, or another PC answered first).
    pub fn answer(&self, token: &str, id: &str, link: &str) -> Result<bool> {
        let (status, _) = self.post("/api/connect/host/answer", json!({"token": token, "id": id, "link": link}))?;
        match status {
            200 => Ok(true),
            409 => Ok(false),
            401 => Err(anyhow!("the server no longer knows this PC: run `relay cloud connect` again")),
            _ => Err(anyhow!("the server answered {status}")),
        }
    }
}

/// A `/api/connect/claim` answer, read.
pub fn parse_claim(status: u16, o: &Value) -> Claim {
    match o["status"].as_str() {
        Some("waiting") => Claim::Waiting,
        Some("waiting_pc") => Claim::WaitingPc,
        Some("denied") => Claim::Denied,
        Some("expired") => Claim::Expired,
        Some("spent") => Claim::Spent,
        Some("ready") => match o["link"].as_str() {
            Some(link) if link.starts_with("relay://pair?") => Claim::Ready {
                link: link.to_string(),
                host_token: o["hostToken"].as_str().map(str::to_string),
            },
            _ => Claim::Trouble("the server's link did not read".into()),
        },
        Some("error") => Claim::Trouble(o["reason"].as_str().unwrap_or("the server could not answer").to_string()),
        _ => Claim::Trouble(format!("the server answered {status}")),
    }
}

/// While `relay serve --remote` runs on a PC connected to its server: ask every few seconds for
/// Relay phones the owner allowed, and open a confirmed pairing window for each.
pub fn spawn_host(ctx: Arc<Ctx>, server: String, token: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let api = Server::new(&server);
        let mut seen: HashSet<String> = HashSet::new();
        let mut forgotten = false;
        loop {
            let (a, t) = (api.clone(), token.clone());
            let next = match tokio::task::spawn_blocking(move || a.phones(&t)).await {
                Ok(Ok(Some(phones))) => {
                    forgotten = false;
                    for phone in phones {
                        if seen.insert(phone.id.clone()) {
                            let (ctx, api, token) = (ctx.clone(), api.clone(), token.clone());
                            tokio::spawn(async move {
                                if let Err(e) = open_for_phone(ctx, api, token, phone).await {
                                    tracing::warn!(error = %format!("{e:#}"), "opening a pairing window for a phone failed");
                                }
                            });
                        }
                    }
                    POLL
                }
                Ok(Ok(None)) => {
                    if !forgotten {
                        tracing::warn!(server = %api.base(), "this PC was forgotten on its server; `relay cloud connect` connects it again");
                        forgotten = true;
                    }
                    Duration::from_secs(600)
                }
                Ok(Err(e)) => {
                    tracing::debug!(error = %format!("{e:#}"), server = %api.base(), "asking the server for waiting phones failed");
                    Duration::from_secs(30)
                }
                Err(_) => POLL,
            };
            tokio::time::sleep(next).await;
        }
    })
}

/// One allowed phone: a pairing window that this desktop confirms, its link handed over through
/// the server, then the question once the phone presents the code.
async fn open_for_phone(ctx: Arc<Ctx>, api: Server, token: String, phone: WaitingPhone) -> Result<()> {
    let (registry, code) = Registry::update(&ctx.registry_path, |r| {
        let code = r.begin_pair(true).code;
        Ok::<_, anyhow::Error>((r.clone(), code))
    })?;
    let port = registry.direct_port.unwrap_or(DEFAULT_PORT);
    let link = PairLink::build(&registry, ctx.instance.as_str(), &code, port, &crate::direct::lan_addresses()).to_url();
    let id = phone.id.clone();
    let delivered = tokio::task::spawn_blocking(move || api.answer(&token, &id, &link)).await?;
    if !matches!(delivered, Ok(true)) {
        abandon(&ctx, &code);
        return delivered.map(|_| ());
    }
    tracing::info!(device = %phone.device, "opened a pairing window for a phone allowed on Discord");
    crate::bridge::toast(
        ctx.socket_path.clone(),
        format!("{} (code {}), allowed on Discord, is pairing with this PC. Accept it when asked.", capitalized(&phone.device), phone.code),
        "info",
        20_000,
    )
    .await;
    confirm(&ctx, &code, &phone).await
}

/// Waits for the phone to present the code, then asks on the desktop. With nobody answering
/// there, `relay remote approve` still can, until the phone stops waiting.
async fn confirm(ctx: &Ctx, code: &str, phone: &WaitingPhone) -> Result<()> {
    let wanted = normalize_code(code);
    let until = Instant::now() + WINDOW;
    while Instant::now() < until {
        let registry = Registry::load(&ctx.registry_path)?;
        // Gone: answered with `relay remote approve`, replaced by a newer window, or expired.
        let Some(pending) = registry.pending.iter().find(|p| normalize_code(&p.code) == wanted) else { return Ok(()) };
        if let Some(request) = &pending.request {
            if request.approved.is_some() {
                return Ok(());
            }
            let body = format!(
                "\"{}\" ({}) asks to pair with this PC. You allowed {} on Discord with the code {}. It gets full control of this PC as you.",
                request.device_name, request.origin, phone.device, phone.code
            );
            let answer = tokio::task::spawn_blocking(move || ask_desktop("Pair a phone with Relay?", &body)).await.ok().flatten();
            match answer {
                Some(yes) => {
                    let decided = Registry::update(&ctx.registry_path, |r| Ok::<_, anyhow::Error>(r.decide(code, yes)))?;
                    tracing::info!(device = %request.device_name, approved = yes, decided, "phone pairing answered on the desktop");
                }
                None => {
                    crate::bridge::toast(
                        ctx.socket_path.clone(),
                        format!("\"{}\" waits to pair with this PC. Run `relay remote approve` within a minute to let it in.", request.device_name),
                        "warn",
                        60_000,
                    )
                    .await;
                }
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    abandon(ctx, code);
    Ok(())
}

fn abandon(ctx: &Ctx, code: &str) {
    let _ = Registry::update(&ctx.registry_path, |r| {
        r.abandon(code);
        Ok::<_, anyhow::Error>(())
    });
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Asks at the desktop, with a notification carrying Accept and Deny (`notify-send --wait`,
/// libnotify 0.7.9 or later). `RELAY_ASK` names another program to ask with: it is given the
/// title and the question, and prints `accept` or `deny`. `None` when nobody answered, or there is
/// nothing to ask with.
fn ask_desktop(title: &str, body: &str) -> Option<bool> {
    let mut cmd = match std::env::var_os("RELAY_ASK") {
        Some(program) => {
            let mut cmd = std::process::Command::new(program);
            cmd.args([title, body]);
            cmd
        }
        None => {
            let mut cmd = std::process::Command::new("notify-send");
            cmd.args(["--app-name=Relay", "--urgency=critical", "--wait"])
                .arg(format!("--expire-time={}", ASK_FOR.as_millis()))
                .args(["--action=accept=Accept", "--action=deny=Deny", title, body]);
            cmd
        }
    };
    let out = relay_core::proc::output_with_timeout(&mut cmd, ASK_FOR + Duration::from_secs(5)).ok().flatten()?;
    match String::from_utf8_lossy(&out.stdout).trim() {
        "accept" => Some(true),
        "deny" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_way_of_naming_the_server_reaches_its_root() {
        for s in [
            "antho-s-bot-8dve.onrender.com",
            "https://antho-s-bot-8dve.onrender.com/",
            " https://antho-s-bot-8dve.onrender.com/dashboard/devices ",
            "wss://antho-s-bot-8dve.onrender.com/relay/join/abc",
        ] {
            assert_eq!(normalize(s), "https://antho-s-bot-8dve.onrender.com", "{s}");
        }
        assert_eq!(normalize("http://127.0.0.1:3901/api/connect"), "http://127.0.0.1:3901");
        assert_eq!(normalize("ws://127.0.0.1:7430/relay"), "http://127.0.0.1:7430");
    }

    #[test]
    fn a_claim_reads_every_state_and_keeps_a_pc_token() {
        assert_eq!(parse_claim(200, &json!({"status": "waiting"})), Claim::Waiting);
        assert_eq!(parse_claim(200, &json!({"status": "waiting_pc"})), Claim::WaitingPc);
        assert_eq!(parse_claim(200, &json!({"status": "denied"})), Claim::Denied);
        assert_eq!(parse_claim(200, &json!({"status": "spent"})), Claim::Spent);
        assert_eq!(
            parse_claim(200, &json!({"status": "ready", "link": "relay://pair?v=1&code=AB", "hostToken": "t"})),
            Claim::Ready { link: "relay://pair?v=1&code=AB".into(), host_token: Some("t".into()) }
        );
        assert_eq!(
            parse_claim(200, &json!({"status": "ready", "link": "https://elsewhere"})),
            Claim::Trouble("the server's link did not read".into())
        );
        assert_eq!(parse_claim(503, &json!({"status": "error", "reason": "later"})), Claim::Trouble("later".into()));
        assert_eq!(parse_claim(502, &Value::Null), Claim::Trouble("the server answered 502".into()));
    }
}
