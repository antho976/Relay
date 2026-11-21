# Bank connections through Plaid

Asked for by the owner (2026-11-19): connect Desjardins and Wealthsimple once, and let new purchases
arrive in Tally by themselves instead of importing CSVs. The owner already uses Plaid (Wealthsimple
links Desjardins through it), so sending the same values to Plaid is accepted (PRODUCT.md,
principle 4).

## Who does what

- **The phone** never holds Plaid's secret or a bank's access token. It asks the owner's server for
  a *link token*, opens Plaid's own sign-in (Plaid Link Android SDK, `com.plaid.link:sdk-core`) with
  it, and hands the one-time *public token* back. Its screen is `ui/banks/BanksScreen.kt`; its
  requests are `data/banks/Banks.kt`.
- **The server** (the cloud Relay that Tally already syncs with, RelayV4) keeps the Plaid keys and
  each bank's access token, reads transactions on a schedule (read only), and writes them into the
  ledger as ordinary entries. They reach the phone, the PC and every other device by the existing
  `money.sync`. Nothing about sync changes.

Until the server has these ops, the phone shows "Your server does not connect banks yet" (it reads
`bus.unknown_op` / `remote.op` as that) and everything else in Tally works as before.

## What the owner sets up once

1. A Plaid account at dashboard.plaid.com. The free **Trial** plan covers up to 10 bank logins with
   real data; Tally needs two (Desjardins, Wealthsimple).
2. In the dashboard: country **CA**, product **Transactions** (Investments optional, for the TFSA,
   RRSP and FHSA), and under *Allowed Android package names* both
   `com.quietsoftware.tally2` (release) and `com.quietsoftware.tally2.debug` (debug builds).
3. On the server (the owner's home server, anthobot.bid, since 2026-11-04; no longer Render), in
   the bot's environment: `PLAID_CLIENT_ID`, `PLAID_SECRET`, `PLAID_ENV` (`production`, the
   default, or `sandbox`) and `PLAID_TOKEN_KEY`, 64 hex digits (`openssl rand -hex 32`), which
   seals the access tokens in the server's `plaid.db`. Never in the APK, never in a repository.

As built in RelayV4 (2026-11-19, branch relay/rusty-quail; its docs/MONEY.md, "Banks (Plaid)"): a
server without these set answers `money.bank.list` with `{ banks: [] }` and every other op with
code `money.plaid_setup` and a readable message ("This server is not set up for Plaid yet: it
needs PLAID_CLIENT_ID and PLAID_SECRET"), which the phones show as is.

## The ops (phone → server, over the door, actor `user`)

All are remote-allowed for a paired Tally device. Errors are ordinary bus errors with a readable
`message`; the phone shows it as is.

| op | payload | result |
| --- | --- | --- |
| `money.bank.list` | `{}` | `{ banks: [Bank] }` |
| `money.bank.link_token` | `{ android_package_name, client_name }` | `{ link_token }` from Plaid `/link/token/create` (`products: ["transactions"]`, `country_codes: ["CA"]`, `language: "en"`, `user.client_user_id`: `"relay-owner"` (the door does not tell the server which device asks), `transactions.days_requested: 730` so a `since` far back finds data, `android_package_name` as given) |
| `money.bank.exchange` | `{ public_token, institution_id?, institution_name? }` | `{ bank: Bank }`: `/item/public_token/exchange`, then `/accounts/get`; every account `follow: false` |
| `money.bank.accounts` | `{ bank_id, since?: "YYYY-MM-DD", accounts: [{ id, follow, into?: account uid }] }` | `{ bank: Bank }`; starts the first read in the background |
| `money.bank.refresh` | `{}` | `{}`: reads every bank now |
| `money.bank.remove` | `{ bank_id }` | `{}`: `/item/remove`, drops the token; entries already written stay |

| `money.bank.hosted_link` | `{ client_name, completion_redirect_uri?, bank_id? }`; with `bank_id`, a sign-in again to that bank (see "Signing in again") | `{ link_token, hosted_link_url, expires_at?: epoch ms }`: Plaid `/link/token/create` as for `link_token`, but with no `android_package_name` and with `hosted_link: { completion_redirect_uri }`; `hosted_link_url` is Plaid's page for that token |
| `money.bank.link_status` | `{ link_token }` | `{ state: "waiting" \| "connected" \| "exited" \| "expired", bank?: Bank, problem?: string }`; see "Hosted Link" below |

`Bank` is `{ id, institution, last_sync?: epoch ms, problem?: string, expires_at?: epoch ms,
accounts: [{ id, name, mask, kind, follow, into? }] }`. `expires_at` is when the bank's consent runs
out and it asks for a sign-in again (Plaid's `consent_expiration_time` from `/item/get`, asked on
every read); absent when the bank sets no end. `kind` is Plaid's subtype in words ("checking", "credit card", "tfsa").
`problem` is set when the person must act ("Sign in to Desjardins again"), from Plaid's
`ITEM_LOGIN_REQUIRED` and the like; otherwise Plaid's `display_message`, else "Plaid said: …".
Ids: a bank's `id` is Plaid's `item_id`, an account's `id` Plaid's `account_id`, and `into` the
ledger account's uid. A followed account gets its ledger account when `money.bank.accounts` runs,
so `into` is set in that answer.

## Hosted Link (Tally RS)

Tally RS (the Rust app, `native/`) has no Java Plaid SDK, so it uses Plaid's **Hosted Link**: a
Link session on a web page Plaid hosts, which the phone opens in the browser. The server finds out
what came of it, so the phone only ever asks.

1. The phone calls `money.bank.hosted_link` with `completion_redirect_uri: "tallyrs://banks"`. The
   server creates the link token with the hosted-link config and keeps it (with the device that
   asked) until it expires. Plaid's hosted pages live 30 minutes unless the token says otherwise;
   the server returns that as `expires_at`.
2. The phone opens `hosted_link_url` (an `https://` page; the phone opens nothing else). The person
   signs in to the bank on Plaid's page. At the end Plaid sends the browser to
   `tallyrs://banks`, which brings Tally RS back to the front (an intent filter on its activity).
3. While the page is out, and again when it comes back, the phone calls `money.bank.link_status`
   every 3 seconds with the link token. The server reads `/link/token/get` on each call
   (`link_sessions[].results.item_add_results[].public_token`; there is no webhook yet), and on
   success makes the exchange itself, once per token (a second call while it runs answers
   `waiting`), exactly as
   `money.bank.exchange` does: `/item/public_token/exchange`, `/accounts/get`, every account
   `follow: false`. It must be idempotent per link token: asked again, it answers the same bank and
   exchanges nothing twice.
   - `waiting`: the person has not finished on Plaid's page.
   - `connected`: `bank` is the new connection; the phone shows its accounts to pick, then calls
     `money.bank.accounts` as before.
   - `exited`: the page closed or failed before a bank was connected; `problem` is Plaid's
     `display_message` when it gave one.
   - `expired`: the token ran out before a bank was connected (the phone stops asking too).
4. As built, `link_status` answers whoever holds the link token (the door does not say which
   device asks); an unknown token is `money.not_found`.

The Kotlin app keeps using the SDK and `money.bank.link_token` / `money.bank.exchange`; both ways
end in the same `Bank` and the same `money.bank.accounts`.

### Signing in again

A bank asks for a sign-in again when the person changes their password, when it wants a new
security check, or when its consent runs out (some banks set an end, often around a year). The bank
decides; nobody can skip it. What Tally RS makes short is everything around the sign-in:

- **Fix is one sign-in.** `money.bank.hosted_link` with `bank_id` opens Plaid's **update mode** for
  that bank on the same hosted page: the link token is made with the bank's own access token and no
  products. Nothing is exchanged: the server keeps the same access token, cursor, accounts and
  choices. `money.bank.link_status` answers `connected` with the bank once a session finished with
  no exit (update mode may hand back no public token); the server then asks `/item/get` how the
  bank stands, clears `problem` unless Plaid still holds an error, keeps the new `expires_at`, and
  reads the banks at once.
- **A week's warning.** Banks with `problem`, or with `expires_at` within 7 days (Plaid's own
  `PENDING_EXPIRATION` comes a week ahead), get Fix on their row, a line on Overview under the
  month, and a count beside Connected banks in the drawer. Tally lists the banks
  (`money.bank.list`, which reaches neither Plaid nor a bank) on its Banks page and in the
  background every 6 hours while open, and keeps the result in `prefs.json`.
- **An older server** refuses `bank_id` (`bus.schema`); Fix then says to disconnect the bank and
  connect it again, as before.

## Turning a transaction into an entry

The server reads with `/transactions/sync` and keeps its cursor per bank: a minute after it
starts, every 4 hours, on `money.bank.refresh`, and after `money.bank.accounts` (no webhooks yet).
Following an account not followed before, or moving `since` earlier, resets that bank's cursor and
reads it again from the start; uids keep anything from being written twice.

- **uid** `plaid:<transaction_id>`. A uid already present or tombstoned is never written again, as
  for imports and posted bills (docs/MONEY.md in RelayV4). A `removed` transaction tombstones its
  entry only if the person never edited it.
- **Skip `pending`** transactions. Write the posted one when it arrives; it has its own id, and the
  pending one's id is in `pending_transaction_id`.
- **Amount and type.** Plaid's amount is positive for money out: a positive amount is an EXPENSE of
  that amount in cents, and a negative one is INCOME. A payment to a followed credit card, from a
  followed account, is a TRANSFER between the two (match by amount and date within 3 days, as the
  CSV import does).
- **Account.** The `into` Tally account, or a new account named after the bank's (type from the
  subtype: credit card → CREDIT, savings → SAVINGS, else CHEQUING), with `institution` and
  `externalRef` set to the institution name and the mask, so later CSV imports find it.
- **Category.** Plaid's `personal_finance_category.primary` mapped to the ledger's categories by
  name (FOOD_AND_DRINK → Dining or Groceries by `detailed`, TRANSPORTATION → Transport, and so on);
  unknown leaves it uncategorised for the person to file, as an import does.
- **Note.** `merchant_name`, else `name`.
- **Currency.** Desjardins sends CAD amounts labelled `USD` through Plaid (Monarch's status page,
  2025-11). Treat Desjardins' amounts as the ledger's currency. Amounts in any other currency
  than the ledger's are left out, for every other bank.
- **Duplicates with CSV imports.** Before writing, skip a transaction that matches an existing
  entry on the same account with the same amount within 2 days and no `plaid:` uid of its own.
  This is the "Entries you already have are matched and skipped" the phone promises.

## Privacy

Plaid's access is read only. The access tokens sit in the server's own store, encrypted at rest
with a key from the environment, and leave it only to talk to Plaid. Disconnecting removes the item
at Plaid as well.
