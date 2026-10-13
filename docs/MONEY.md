# Money in Relay

Relay gains a second, casual space beside the dev workspace. Money management comes first: Tally,
the budget and spending tracker, moves into this repository and the PC gets the same ledger.
Investments (holdings, their value and registered room) are designed in `docs/INVESTMENTS.md`;
AI-planned trades come later and are not designed anywhere yet.

Decided with Antho on 2026-10-07:

- **Two apps, one repository.** Tally stays its own Android app in `apps/tally` (Kotlin, Compose,
  Room). `apps/relay-android` is the PC remote. Folding them into one phone app is a later idea,
  not this plan.
- **Both devices hold the whole ledger and work as one.** The phone is where nearly everything is
  logged; the PC is used rarely but must never be read-only. Either can add or change anything
  offline, and they catch each other up when they reach each other.
- **The start screen is a select screen,** not a chat: a greeting for the time of day, then a card
  per space with one live line each (Money: "$412 left · 3 days ahead"; Dev: agents and reviews).
- **The switcher sits at the top left of the top bar,** where the brand is now: Dev | Money.

## Where this is going (Antho, 2026-10-07)

The Money pages are a first step, not the destination. What Antho wants instead of a Money tab is
a **threads page**: agent first, data second. A thread is a conversation with an agent that works
on the data of Antho's own apps, and the data shows beside it to read and edit. The apps to bring
in: money (Tally's budget), investing, AI-run investing later, and the gym app's data. The ledger,
its ops and sync here are what such a thread reads and writes: see `docs/THREADS.md`.

## Try it

From this branch's checkout, the `test` instance keeps its own store and ledger
(`~/.local/share/relay-v4/test/money.db`) and leaves the `dev` engine and its agents alone:

```fish
./run.sh test
```

Pick **Money** on the start screen (or the Dev | Money switch at the top left). An empty ledger
offers three ways in: add an account, load Tally's sample household, or import a Tally backup
(Tally: Settings, Backup, export, then copy the `.json` to the PC).

To sync the phone with the `test` engine, open its phone door on a free port (the `dev`
engine's door holds 7420) and pair from Tally's Settings, "Relay on your PC":

```fish
target/debug/relay --instance test remote serve --bind 0.0.0.0:7421 &
target/debug/relay --instance test remote pair
```

The first sync makes the PC's ledger the phone's; after that both sides merge.

## Steps

1. **Tally in the repository** (done). `apps/tally`, squashed from
   [antho976/Tally](https://github.com/antho976/Tally) at `cfae7c3`; its full history stays there.
   Its workflows run from the root as `.github/workflows/tally-*.yml`, only for changes under
   `apps/tally`; releases are `tally-v*` tags.
2. **The money rules in Rust** (`crates/relay-money`). A module-for-module port of
   `apps/tally/core` with its tests: amounts, periods, pace, recurrence, payees, CSV and bank
   statements, the backup file. A rule that changes on one side changes on the other in the same
   commit; the shared tests are what prove they agree.
3. **A ledger that can sync.** On both sides: every row gets a permanent id (a UUID, not Room's
   1, 2, 3, which collide across devices), a change time, and deletes become tombstones. A change
   log records each edit. Tally needs a Room migration; the engine gets `store::MIGRATIONS` entries
   and `money.*` bus ops.
4. **The desktop Money space.** The space switcher, the start screen, and Money pages that mirror
   Tally's Home, Transactions, Plan and Insights, drawn with Tally's tokens (`apps/tally/DESIGN.md`)
   as Relay's second theme.
5. **Sync.** Tally pairs with the PC the way Relay's phone app does (`crates/relay-remote`, LAN or
   Tailscale; no cloud) and exchanges changes since the last sync. Newest edit wins per row; for one
   person, conflicts are rare. Totals are never synced: balances and pace are recomputed from the
   entries, so equal entries always mean equal numbers. Tally gains the INTERNET permission for
   this, and its promise becomes "nothing leaves your own devices". (2026-10-11: a device may
   also be a cloud copy of Relay on a server Antho runs, which Tally syncs with while the PC is
   off; docs/CLOUD.md.)
6. **Warnings.** Over pace, a bill due soon, a low balance, an unusual expense: Android
   notifications on the phone (WorkManager is already there), Relay's notification center on the
   PC.

## Duplicates sync must not create

- **Recurring bills.** Both devices post rent on its due date. The posted entry's id is derived
  from the bill and the date, so the two copies are the same row and merge.
- **Bank imports.** An imported line's id is derived from the statement line, so importing the
  same CSV on both devices adds it once.
- **Investment imports.** A Wealthsimple file's lines, snapshot, prices and values take uids derived
  from what they are (`docs/INVESTMENTS.md`), so the same file imported on both devices adds it once.

## The ledger on the PC

The engine keeps the ledger in its own SQLite file, `money.db` beside `store.db`, owned by
`relay_money::ledger`. It is apart from the dev store on purpose: money has its own schema
versions, its own backups, and is what the phone will sync with. Every row has a local integer
`id`, a permanent `uid` (UUID text) that sync will use, `updated_at` (ms) and `deleted` (0/1).

Amounts are minor units (`i64`). Dates are `YYYY-MM-DD`. Enum values are Tally's names
(`EXPENSE`, `CHEQUING`, `OVER_PACE`). Every mutation emits `money.changed`.

### Ops (phase 12, scope global)

`money.summary {today?}` → the Home reading:

```
{ currency, fraction_digits, month_start_day, empty,
  period: { start, end_exclusive, days, days_left },
  pace: { budget, spent, expected, remaining, pace_delta, days_left, daily_allowance,
          spent_fraction, pace_fraction, status },
  income, spent,                       // this period, transfers excluded
  lines: { margin, pace, versus_last },  // Tally's sentences, from relay_money::copy
  budgets: [ { category_id, name, icon, color, budget, spent, status } ],   // per category
  accounts: [ { id, name, type, balance } ], net_worth,
  bills: [ { id, name, amount, type, next_date, days_until, due_line, auto_post } ], // next 30 days
  goals: [ { id, name, kind, target, saved, line } ],
  recent: [ Tx ] }                      // last 8
```

`Tx` = `{ id, uid, type, amount, date, account_id, account, to_account_id, to_account,
category_id, category, icon, color, note }`.

| op | payload | result |
|---|---|---|
| `money.summary` | `{ today? }` | above |
| `money.lists` | `{}` | `{ currency, fraction_digits, accounts: [{id,name,type,balance,archived,registration,institution}], categories: [{id,name,kind,icon,color,archived}], devices }` |
| `money.tx.list` | `{ period_offset?, query?, account_id?, category_id?, limit? }` | `{ period, transactions: [Tx], income, spent }` |
| `money.tx.add` | `{ type, amount, date, account_id, to_account_id?, category_id?, note? }` | `Tx` |
| `money.tx.update` | `{ id, type?, amount?, date?, account_id?, to_account_id?, category_id?, note? }` | `Tx` |
| `money.tx.delete` | `{ id }` | `{}` |
| `money.budget.set` | `{ category_id?, amount }` (no category: the overall budget; 0 removes) | `{}` |
| `money.import` | `{ path }`: a Tally backup (`.json`) replaces the ledger, as a restore does on the phone | `{ kind, transactions, accounts }` |
| `money.export` | `{ path }` | `{ path, transactions }` |
| `money.sample` | `{}`: Tally's sample household, only into an empty ledger | `{ transactions }` |
| `money.reset` | `{}`: erase everything | `{}` |
| `money.account.add` | `{ name, type, opening_balance?, registration?, institution? }` (a registration on an `INVESTMENT` account only) | `AccountView` |
| `money.account.update` | `{ id, name?, registration?, institution?, archived? }` | `AccountView` |
| `money.value.set` | `{ account_id, date, value }`: what an investment account was worth that day; a second value the same day replaces the first | `AccountView` |
| `money.invest.summary` | `{ today? }` | `Portfolio` (`docs/INVESTMENTS.md`) |
| `money.invest.list` | `{ account_id?, type?, since?, limit? }` | `{ activities: [Activity] }`, newest first |
| `money.invest.add` | `{ account_id, type, date, symbol?, currency?, quantity?, amount, fee?, note?, to_amount?, to_currency? }` | `Activity` |
| `money.invest.delete` | `{ id }` | `{}` |
| `money.invest.restore` | `{ id }` | `Activity` |
| `money.invest.preview` | `{ path, account_id? }`: a Wealthsimple file, read without importing (the person's alone) | `ImportPreview` |
| `money.invest.import` | `{ path, accounts: [{ number, account_id? }], account_id? }` (the person's alone) | `ImportResult` |
| `money.invest.room` | `{ registration, year, amount }` (TFSA, RRSP or FHSA; ≤ 0 removes) | `{}` |
| `money.invest.price` | `{ symbol, date, price }` (price at 1e-8) | `{}` |
| `money.fx.set` | `{ base, quote, date, rate }` (rate at 1e-8) | `{}` |
| `money.fx.fetch` | `{}`: USD→CAD from the Bank of Canada in the background, then `money.changed` (the person's alone) | `{ started }` |

`AccountView` = `{ id, name, type, balance, archived, registration, institution }`, as
`money.lists` lists them. The investment ops' shapes, and which of them the thread agent may call,
are in `docs/INVESTMENTS.md`.

Clients format amounts themselves with `relay_money::money::MoneyFormatter`
(`currency` from the result, `Locale::from_env()`).

## Sync

Tally talks to the PC through the same door Relay's phone app uses (`crates/relay-remote`, wire v1:
greeting, pairing code once, then a proof per connection; docs/MOBILE.md §2, §6). It sends one
bus request, `money.sync`, which is in `PHONE_OPS`.

```
money.sync { device, replace?, since, generation?, changes: [Change] }  →  { cursor, generation, changes: [Change], replaced, applied, skipped }
Change = { table, uid, updated_at, deleted, row }
```

- `since` is the PC's `cursor` from the phone's last sync with this PC (0 the first time). The PC
  answers with every row it changed after `since`, and the new `cursor`.
- `changes` are the phone's rows changed since it last synced (by the phone's `updatedAt`), and
  its tombstones. "Since" is a watermark the phone takes when a sync starts, and no edit after it
  is stamped under it: the phone stamps `max(now, watermark)` (a floor kept in `sync_state` for
  the ledger, in DataStore for settings), so a phone that synced with its clock fast and was set
  back still sends what it edits next.
- **First sync: `replace: true`.** The PC's ledger becomes the phone's: everything on the PC is
  erased and the phone's rows are applied, and the PC answers with no changes, just the cursor.
  The phone sends its tombstones of derived uids (`bill:`, `imp:`, `px:`…) with them, so a line
  deleted before pairing is not posted or imported again on the PC.
  The phone is where the ledger lives; the PC is a second view of it. After that, both sides
  merge. Tally says so on its pairing page before the first sync runs.
- A large first sync may come in batches: the first carries `replace: true`, the rest
  `replace: false` with the cursor the one before answered, tables in order across them.
- **Newest edit wins, per row**, by `updated_at` (ms since the epoch). An equal or older change
  is ignored. A tombstone (`deleted: true`) wins like any edit.
- **A stamp never goes backwards.** Every edit on either side is stamped
  `max(now, the row's stamp + 1)`, so a device whose clock runs behind still wins with the edit
  it made last. Settings do the same.
- **Generation.** The PC returns a `generation` with every answer; the phone keeps it and sends it
  back. It changes when the PC's ledger is restored, erased, sampled or replaced by a device's
  first sync. A phone whose generation is not the PC's, or whose `since` is above the PC's
  cursor (the PC's `money.db` was lost), is refused with `money.sync_stale`, and merges nothing.
  It then takes the PC's ledger whole: `since: 0`, no generation, no changes; it replaces its own
  ledger with the answer, keeps the new cursor and generation, and merges from then on. The
  latest wholesale act wins: restoring a backup on the PC reaches the phone, and a reinstalled
  phone that pairs again makes an older install take the new ledger instead of mixing two.
- References travel as uids, never local ids: a transaction's `row.account` is the account's
  uid. Budgets are matched by their category (`row.category`, null for the overall budget), not
  by uid, because each category has one budget.
- A tombstone's `row` is `{}`, except a budget's, which carries `{ category }` both ways (null
  for the overall budget); the PC matches a budget tombstone with no category by its uid. Tally keeps the category's uid in its
  tombstone for that, and sends nothing for a budget whose category is already gone (the
  category's own tombstone takes it on the PC). A row whose reference the receiving side does
  not have is skipped (counted in `skipped`), not fatal.
- A posted bill's uid is `bill:<recurring uid>:<date>` on both devices. **Once a phone syncs, the
  phone alone posts bills**: two devices posting the same bill at different times would each stamp
  it with their own time, and the later post would undo an edit or a delete made in between. A
  PC with no phone posts them itself, and never posts a uid it holds a tombstone for. It posts as
  Tally's `RecurringPoster` does: at most 60 dates of one bill a pass, the rest on the next, and a
  reminder whose day is over moves to its next date (the phone does that once it syncs).
- The PC changes the currency only when the decimals stay the same (CAD to USD); Tally converts
  every amount when they differ (CAD to JPY), so that change is made on the phone.
- `table` is one of `settings`, `accounts`, `categories`, `recurring`, `goals`, `transactions`,
  `budgets`, `contributions`, `account_values`, `securities`, `holdings`, `activities`, `prices`,
  `fx_rates`, `room_facts`, applied in that order. `settings` rows have
  the setting's name as uid (`currency`, `month_start_day`, `week_starts_monday`) and
  `row: { value }`, a string as the PC stores it (`"CAD"`, `"15"`, `"1"`/`"0"`). On the phone
  they live in DataStore, each with the time it last changed.

Row fields, camelCase as Tally's backup writes them:

| table | row |
|---|---|
| accounts | `name, type, openingBalance, archived, sortOrder, registration, institution, externalRef` (the last three read as null, "" and "" when an older device leaves them out) |
| categories | `name, kind, color, icon, archived, sortOrder` |
| recurring | `name, type, amount, account, toAccount, category, frequency, interval, anchorDate, nextDate, endDate, autoPost, active` |
| goals | `name, target, targetDate, color, archived, kind, account, percent, startDate, startAmount` |
| transactions | `type, amount, date, account, toAccount, category, note, recurring, createdAt` |
| budgets | `category, amount` |
| contributions | `goal, amount, date, note` |
| account_values | `account, date, value` |
| securities | `symbol, name, currency, kind, exchange` |
| holdings | `account, security, date, quantity, book, bookMarket` |
| activities | `account, security, type, date, quantity, amount, fee, currency, toAmount, toCurrency, note, source, createdAt` |
| prices | `security, date, price, source` |
| fx_rates | `base, quote, date, rate, source` |
| room_facts | `registration, year, amount` |

## Investments

Holdings, activities, prices, FX rates and the CRA's room figures live in the six tables above,
with the rules for reading them (positions, value, gains, returns, room) and the Wealthsimple
import in `docs/INVESTMENTS.md`: `relay_money::invest` and `relay_money::wealthsimple` on the PC,
`Invest.kt` and `Wealthsimple.kt` on the phone, held to the same shared tests. Positions and totals
are derived, never stored or synced. The ledger's writes are in `crates/relay-money/src/portfolio.rs`.
Rows an import or a write derives a uid for (`sec:`, `hold:`, `imp:`, `px:`, `fx:`, `room:`,
`val:`, `ws:`) are the same rows on both devices, so a file imported on each merges on sync; an
imported line is added only when its uid is neither present nor deleted, as a posted bill is.
