//! Bank connections' pure half (Kotlin `data/banks/Banks.kt` and `BanksViewModel`'s rules): what
//! the server says a bank is, how a failure reads, which accounts are followed by default, which
//! Tally account a bank's account most likely is, and the server's Hosted Link answers
//! (docs/PLAID.md).

use relay_money::Date;
use relay_money::model::AccountType;
use serde_json::{Value, json};

use crate::sync::{Failure, NOT_PAIRED};

/// One account a connected bank holds, and whether Tally follows it into the ledger.
#[derive(Clone, Debug, PartialEq)]
pub struct BankAccount {
    pub id: String,
    pub name: String,
    pub mask: String,
    /// Plaid's subtype in words: "checking", "credit card", "tfsa".
    pub kind: String,
    pub follow: bool,
    /// The Tally account it fills (its uid), or none for "a new account named after it".
    pub into: Option<String>,
}

/// A bank connected through Plaid, as the server keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Bank {
    pub id: String,
    pub institution: String,
    pub accounts: Vec<BankAccount>,
    /// When the server last read new transactions from it, epoch ms.
    pub last_sync: Option<i64>,
    /// The connection's own state in words when it needs the person.
    pub problem: Option<String>,
}

/// What the server said about bank connections.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Ready(Vec<Bank>),
    /// The server's Relay has no bank connections yet.
    NotOnServer,
    NotPaired,
    Failed(String),
}

/// The codes a Relay without bank connections answers with.
pub const UNKNOWN: [&str; 2] = ["bus.unknown_op", "remote.op"];
pub const NOT_ON_SERVER: &str = "Your server does not connect banks yet. It needs the Plaid update.";
/// The door no longer knows this phone (sync_net's code).
const UNPAIRED: &str = "tally.unpaired";

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

pub fn parse_bank(v: &Value) -> Option<Bank> {
    let id = s(v, "id")?;
    let accounts = v
        .get("accounts")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(BankAccount {
                        id: s(x, "id")?,
                        name: s(x, "name").unwrap_or_else(|| "Account".into()),
                        mask: s(x, "mask").unwrap_or_default(),
                        kind: s(x, "kind").unwrap_or_default(),
                        follow: x.get("follow").and_then(Value::as_bool).unwrap_or(false),
                        into: s(x, "into"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Bank { id, institution: s(v, "institution").unwrap_or_else(|| "A bank".into()), accounts, last_sync: v.get("last_sync").and_then(Value::as_i64), problem: s(v, "problem") })
}

pub fn parse_banks(v: &Value) -> Vec<Bank> {
    v.get("banks").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_bank).collect()).unwrap_or_default()
}

/// How the list's answer reads.
pub fn answer(r: Result<Value, Failure>) -> Answer {
    match r {
        Ok(v) => Answer::Ready(parse_banks(&v)),
        Err(f) if f.code == NOT_PAIRED || f.code == UNPAIRED => Answer::NotPaired,
        Err(f) if UNKNOWN.contains(&f.code.as_str()) => Answer::NotOnServer,
        Err(f) => Answer::Failed(f.message),
    }
}

/// How an act's failure reads: the server's own words, except where it has no bank ops at all.
pub fn failure_words(f: &Failure) -> String {
    if UNKNOWN.contains(&f.code.as_str()) { NOT_ON_SERVER.into() } else { f.message.clone() }
}

/// Spending accounts are followed unless the person says otherwise; savings and investments not.
pub fn follows_by_default(a: &BankAccount) -> bool {
    let k = a.kind.to_lowercase();
    k.trim().is_empty() || ["checking", "credit", "cash", "prepaid", "chequing"].iter().any(|w| k.contains(w))
}

/// One of Tally's accounts, as matching and "Into" need it.
#[derive(Clone, Debug, PartialEq)]
pub struct Mine {
    pub id: i64,
    pub uid: String,
    pub name: String,
    pub r#type: AccountType,
    pub archived: bool,
    pub institution: String,
    pub external_ref: String,
}

/// The Tally account a bank's account most likely is: the one whose number ends in its mask, else
/// the one same-bank account of the same kind, else the one same-bank account.
pub fn match_account<'a>(institution: &str, a: &BankAccount, mine: &'a [Mine]) -> Option<&'a Mine> {
    let bank = institution.to_lowercase().split(' ').next().unwrap_or("").to_string();
    let credit = a.kind.to_lowercase().contains("credit");
    let live: Vec<&Mine> = mine.iter().filter(|m| !m.archived).collect();
    if let Some(m) = live.iter().find(|m| !a.mask.trim().is_empty() && m.external_ref.ends_with(&a.mask)) {
        return Some(m);
    }
    let same: Vec<&Mine> = live.iter().copied().filter(|m| !bank.is_empty() && (m.name.to_lowercase().contains(&bank) || m.institution.to_lowercase().contains(&bank))).collect();
    let by_kind: Vec<&Mine> = same.iter().copied().filter(|m| (m.r#type == AccountType::Credit) == credit).collect();
    if by_kind.len() == 1 {
        return Some(by_kind[0]);
    }
    if same.len() == 1 { Some(same[0]) } else { None }
}

/// A bank just connected, with the accounts Tally suggests following and where each lands.
pub fn suggested(bank: &Bank, mine: &[Mine]) -> Bank {
    let mut b = bank.clone();
    for a in &mut b.accounts {
        a.follow = follows_by_default(a);
        if a.into.is_none() {
            a.into = match_account(&bank.institution, a, mine).map(|m| m.uid.clone());
        }
    }
    b
}

/// How far back a newly followed bank is read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Since {
    #[default]
    Month,
    Year,
    Today,
}

impl Since {
    pub const ALL: [Since; 3] = [Since::Month, Since::Year, Since::Today];
    pub fn label(self) -> &'static str {
        match self {
            Since::Month => "Last 30 days",
            Since::Year => "Since January 1",
            Since::Today => "From today on",
        }
    }
    pub fn date(self, today: Date) -> String {
        use jiff::ToSpan;
        match self {
            Since::Month => today.checked_sub(30.days()).unwrap_or(today),
            Since::Year => today.first_of_year(),
            Since::Today => today,
        }
        .to_string()
    }
}

/// " •• 4411", or nothing without a mask.
pub fn mask(a: &BankAccount) -> String {
    if a.mask.trim().is_empty() { String::new() } else { format!(" •• {}", a.mask) }
}

/// "just now", "5m ago", "3h ago", "2d ago".
pub fn ago(epoch_ms: i64, now_ms: i64) -> String {
    let s = (now_ms - epoch_ms).max(0) / 1000;
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", s / 60),
        3600..86400 => format!("{}h ago", s / 3600),
        _ => format!("{}d ago", s / 86400),
    }
}

/// The line under a connected bank: "2 of 3 accounts followed · read 5m ago".
pub fn bank_line(b: &Bank, now_ms: i64) -> String {
    let followed = b.accounts.iter().filter(|a| a.follow).count();
    let read = b.last_sync.map(|t| format!(" · read {}", ago(t, now_ms))).unwrap_or_else(|| " · not read yet".into());
    format!("{followed} of {} accounts followed{read}", b.accounts.len())
}

/// The follow key: "Follow 2 accounts", "Follow nothing for now".
pub fn follow_label(followed: usize) -> String {
    match followed {
        0 => "Follow nothing for now".into(),
        1 => "Follow 1 account".into(),
        n => format!("Follow {n} accounts"),
    }
}

// Hosted Link (docs/PLAID.md, "Hosted Link")

/// Where the phone asks Plaid to send the browser once the person is done: back into Tally RS.
pub const COMPLETION_URI: &str = "tallyrs://banks";

pub fn hosted_link_payload() -> Value {
    json!({ "client_name": "Tally", "completion_redirect_uri": COMPLETION_URI })
}

/// The server's answer to `money.bank.hosted_link`: the link token and the page to open.
#[derive(Clone, Debug, PartialEq)]
pub struct HostedLink {
    pub link_token: String,
    pub url: String,
    /// When the page stops working, epoch ms; none when the server does not say.
    pub expires_at: Option<i64>,
}

pub fn parse_hosted_link(v: &Value) -> Option<HostedLink> {
    let url = s(v, "hosted_link_url")?;
    if !url.starts_with("https://") {
        return None;
    }
    Some(HostedLink { link_token: s(v, "link_token")?, url, expires_at: v.get("expires_at").and_then(Value::as_i64) })
}

/// Where a Hosted Link session stands, as `money.bank.link_status` says.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkState {
    /// The person has not finished on Plaid's page yet.
    Waiting,
    /// A bank is connected: its accounts, none followed yet.
    Connected(Bank),
    /// Plaid's page was closed, or failed, before a bank was connected.
    Exited(Option<String>),
    /// The link token ran out.
    Expired,
}

pub fn parse_link_state(v: &Value) -> Option<LinkState> {
    Some(match v.get("state").and_then(Value::as_str)? {
        "waiting" => LinkState::Waiting,
        "connected" => LinkState::Connected(parse_bank(v.get("bank")?)?),
        "exited" => LinkState::Exited(s(v, "problem")),
        "expired" => LinkState::Expired,
        _ => return None,
    })
}

/// `money.bank.accounts`' payload: which accounts to follow and where each lands.
pub fn follow_payload(bank: &Bank, since: &str) -> Value {
    let accounts: Vec<Value> = bank
        .accounts
        .iter()
        .map(|a| {
            let mut o = json!({ "id": a.id, "follow": a.follow });
            if let Some(into) = &a.into {
                o["into"] = json!(into);
            }
            o
        })
        .collect();
    json!({ "bank_id": bank.id, "since": since, "accounts": accounts })
}
