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

/// Which of Tally's account types a bank's account can be, by Plaid's subtype: a card is only ever
/// a credit account, a chequing or savings account only a cash-like one, an investment only an
/// investment. Empty for a kind Tally can't place (a loan, a mortgage, nothing said), which is then
/// never guessed by name.
fn types_for(kind: &str) -> &'static [AccountType] {
    let k = kind.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| k.contains(w));
    if has(&["credit"]) {
        &[AccountType::Credit]
    } else if has(&["tfsa", "rrsp", "rrif", "resp", "fhsa", "lira", "brokerage", "investment", "401k", "ira", "retirement", "pension"]) {
        &[AccountType::Investment]
    } else if has(&["checking", "chequing", "savings", "prepaid", "cash", "money market", "depository", "hsa", "paypal"]) {
        &[AccountType::Chequing, AccountType::Savings, AccountType::Cash]
    } else {
        &[]
    }
}

/// The one Tally type a bank's subtype names exactly ("savings" is Savings), for choosing between
/// a chequing and a savings account at the same bank.
fn exact_type(kind: &str) -> Option<AccountType> {
    let k = kind.to_lowercase();
    if k.contains("saving") {
        Some(AccountType::Savings)
    } else if k.contains("checking") || k.contains("chequing") {
        Some(AccountType::Chequing)
    } else {
        types_for(kind).first().copied().filter(|_| types_for(kind).len() == 1)
    }
}

/// True when `hay` holds `word` as whole words: "td" is in "TD Visa", not in "Outdoor fund". A
/// multi-word name matches as a phrase (accounts_store::bank_name reads names the same way).
fn has_words(hay: &str, word: &str) -> bool {
    let hay = hay.to_lowercase();
    let words: Vec<&str> = hay.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let want: Vec<&str> = word.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    !want.is_empty() && words.windows(want.len()).any(|w| w == want.as_slice())
}

/// Words a bank's name may start with that don't say which bank it is: "Bank of America" is not
/// "My bank account", and "Caisse populaire Desjardins" is Desjardins.
const GENERIC: [&str; 14] = ["the", "bank", "banque", "caisse", "caisses", "populaire", "credit", "union", "of", "de", "du", "des", "la", "le"];

/// What an account's name or bank must hold to be at `institution`: its first word that says
/// which bank it is ("td" for "TD Canada Trust", "america" for "Bank of America"), else the whole
/// name as a phrase.
fn bank_word(institution: &str) -> String {
    let name = institution.to_lowercase();
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    match words.iter().find(|w| !GENERIC.contains(w)) {
        Some(w) => w.to_string(),
        None => words.join(" "),
    }
}

/// The Tally account a bank's account most likely is, among those of an agreeing type that no
/// other bank account already fills (`taken`, by uid): the one whose number ends in its mask, else
/// the one same-bank account of exactly its kind, else the one same-bank account of its kind's
/// family. Never an account of another type: a card's purchases landing in chequing would read as
/// money in.
pub fn match_account<'a>(institution: &str, a: &BankAccount, mine: &'a [Mine], taken: &[String]) -> Option<&'a Mine> {
    let bank = bank_word(institution);
    let types = types_for(&a.kind);
    let free: Vec<&Mine> = mine.iter().filter(|m| !m.archived && !taken.contains(&m.uid)).collect();
    // A kind Tally can't place still matches by number, as long as the number is the account's own.
    let agrees = |m: &Mine| types.is_empty() || types.contains(&m.r#type);
    if let Some(m) = free.iter().find(|m| !a.mask.trim().is_empty() && m.external_ref.ends_with(&a.mask) && agrees(m)) {
        return Some(m);
    }
    if types.is_empty() || bank.is_empty() {
        return None;
    }
    let same: Vec<&Mine> = free.iter().copied().filter(|m| types.contains(&m.r#type) && (has_words(&m.name, &bank) || has_words(&m.institution, &bank))).collect();
    if let Some(exact) = exact_type(&a.kind) {
        let by_kind: Vec<&Mine> = same.iter().copied().filter(|m| m.r#type == exact).collect();
        if by_kind.len() == 1 {
            return Some(by_kind[0]);
        }
    }
    if same.len() == 1 { Some(same[0]) } else { None }
}

/// A bank just connected, with the accounts Tally suggests following and where each lands. No two
/// of its accounts are suggested into the same Tally account.
pub fn suggested(bank: &Bank, mine: &[Mine]) -> Bank {
    suggested_beside(bank, mine, &[])
}

/// [`suggested`], never into a Tally account in `elsewhere`: the ones another bank connection
/// already follows into ([`followed_uids`]), which two banks writing into would count twice.
pub fn suggested_beside(bank: &Bank, mine: &[Mine], elsewhere: &[String]) -> Bank {
    let mut b = with_places_beside(bank, mine, elsewhere);
    for a in &mut b.accounts {
        a.follow = follows_by_default(a);
    }
    b
}

/// A bank with each account not yet placed given the Tally account it most likely is, what it
/// follows left as it is (a bank opened from the list). No two land in the same Tally account.
pub fn with_places(bank: &Bank, mine: &[Mine]) -> Bank {
    with_places_beside(bank, mine, &[])
}

/// [`with_places`], never placing one into a Tally account in `elsewhere` (another connection's
/// followed accounts, see [`suggested_beside`]).
pub fn with_places_beside(bank: &Bank, mine: &[Mine], elsewhere: &[String]) -> Bank {
    let mut b = bank.clone();
    // An account pointing into a Tally account since deleted, or archived, is placed afresh: left
    // as it was, the page would read "Into a new account" (the sheet lists neither) while Follow
    // still sent that uid.
    for a in &mut b.accounts {
        if a.into.as_ref().is_some_and(|u| !mine.iter().any(|m| &m.uid == u && !m.archived)) {
            a.into = None;
        }
    }
    let mut taken: Vec<String> = b.accounts.iter().filter_map(|a| a.into.clone()).collect();
    taken.extend(elsewhere.iter().cloned());
    for a in &mut b.accounts {
        if a.into.is_none() {
            a.into = match_account(&bank.institution, a, mine, &taken).map(|m| m.uid.clone());
            taken.extend(a.into.clone());
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

/// The preference holding the uids of the accounts the server last said it follows (a JSON list),
/// written each time the Banks page reads `money.bank.list`.
pub const FOLLOWED_PREF: &str = "bank_followed_uids";

/// The Tally accounts `banks` write into: each followed bank account's `into`.
pub fn followed_uids(banks: &[Bank]) -> Vec<String> {
    let mut out: Vec<String> = banks.iter().flat_map(|b| &b.accounts).filter(|a| a.follow).filter_map(|a| a.into.clone()).collect();
    out.sort();
    out.dedup();
    out
}

/// What the Banks page last read of [`followed_uids`], if it ever read one.
pub fn followed_pref(store: &crate::model::Store) -> Option<Vec<String>> {
    store.pref(FOLLOWED_PREF).and_then(|v| serde_json::from_str(&v).ok())
}

/// Keeps what the server just said it follows; true when that changed.
pub fn remember_followed(store: &mut crate::model::Store, banks: &[Bank]) -> bool {
    let now = followed_uids(banks);
    if followed_pref(store).as_ref() == Some(&now) {
        return false;
    }
    store.set_pref(FOLLOWED_PREF, &json!(now).to_string()).is_ok()
}

/// True for a reference the bank link names an account it made by: Plaid's mask, the last two to
/// four letters or digits of the bank's number. A Wealthsimple import's account number
/// ("HQ7XFMC41CAD") is longer, so an imported account never reads as connected by its number.
pub fn is_bank_mask(external_ref: &str) -> bool {
    let r = external_ref.trim();
    (2..=4).contains(&r.chars().count()) && r.chars().all(|c| c.is_ascii_alphanumeric())
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

/// What a bank that needs signing in again says when its Fix is tapped. `money.bank.hosted_link`
/// takes no bank id (docs/PLAID.md), so the server can only open a new connection, not Plaid's
/// update mode for this one; going through it would add the same bank a second time. Until the
/// server can reopen a sign-in, the honest way is to disconnect and connect again.
pub fn sign_in_again_line(institution: &str) -> String {
    format!("Your server can't reopen the sign-in for {institution} yet. Disconnect it below, then connect it again; its entries stay.")
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
