# Tally RS review: what was and wasn't looked at (2026-11-20/21)

Eight rounds of blind code review over `native/`, each followed by fixes. Every round used five
fresh reviewers that never saw earlier findings, fixes or git history. Each one took one area:
sync, data and money logic, platform shell, screens A and screens B. Together the five areas
cover the whole app every round. The loop stopped after round 8, which came back nearly clean
(one high, fixed).

## Findings per round

| Round | Critical | High | Medium | Low |
|------:|---------:|-----:|-------:|----:|
| 1 | 0 | 4 | 15 | 19 |
| 2 | 0 | 6 | 13 | 11 |
| 3 | 0 | 4 | 17 | 18 |
| 4 | 0 | 2 | 12 | 13 |
| 5 | 0 | 4 | 8 | 15 |
| 6 | 0 | 4 | 8 | 10 |
| 7 | 1 | 4 | 8 | 11 |
| 8 | 0 | 1 | 7 | 17 |

Every finding was checked against the code and then fixed, except those listed under *Left
open* below. Counts are as reported; a few highs were found by two reviewers at once and
counted twice. Tests went from 141 to 318, all passing. The Android library checks clean
(`cargo check --target aarch64-linux-android`) and links with 16 KB page alignment.

## Coverage

| Part | Lines | Reviewed |
|---|---:|---|
| App source, `native/crates/tally-rs/src` | 31,780 | 31,363 lines, **98.7%** |
| Android shell (Java, manifest, res, Gradle module) and `tools/build-lib.sh` | 702 | All of it |
| Tests, `native/crates/tally-rs/tests` and unit tests | 6,645 | Read as context only, not reviewed as targets |
| `relay-money` (the money engine dependency, RelayV4) | 12,018 | Read to understand its APIs, not reviewed (out of scope) |
| Generator scripts, `native/tools/*.py` | 114 | Not reviewed |
| Root Gradle and Cargo manifests | 131 | Not reviewed |

**Across all of Tally's own files** (app, Android, tests, scripts, manifests: 39,372 lines),
32,065 lines, **81.4%**, were reviewed as a target. **Counting relay-money too** (51,390 lines), **62.4%**.

### The 1.3% of app source not reviewed (417 lines)

| File | Lines | What it is |
|---|---:|---|
| `shader.wgsl` | 150 | The GPU shader for every shape. A bug there would draw wrong, not lose data. |
| `glyphs.rs` | 131 | Generated icon tables (`tools/gen_glyphs.py`). |
| `category_icons.rs` | 123 | Generated icon tables (`tools/gen_category_icons.py`). |
| `clock.rs` | 10 | A monotonic clock. |
| `main.rs` | 3 | Calls the desktop entry point. |

## Not looked at, at all

- **Running it on a phone.** Nothing ran on a device or emulator. The Android-side fixes
  (launcher icon aliases, edge-to-edge insets, file picker tags and cancels, device-transfer
  rules, keyboard types, the paused loop waking, sharing) were only compiled.
- **Android 8 to 14 on a device.** The only emulator is API 36. The fixes for split screen,
  `Start`/`Stop` drawing and insets on API 26 to 34 were reasoned from AOSP and GameActivity
  4.4.0, not tested.
- **How the screens look.** No screenshots were compared. Layout fixes (text fitted, toasts
  placed above keys) are from reading the drawing code.
- **The server side (RelayV4):** pairing, `money.sync`, the door and Plaid Hosted Link, as the
  server runs them. Reviewers read the protocol from the phone's side only.
- **relay-money** itself (above).
- **The tests as tests.** A test asserting the wrong thing would only have been caught if it got
  in the way of a finding.
- **Performance and battery** beyond the redraw and polling bugs reviewers happened to find.
- **Accessibility.**

## Left open (decisions or server work)

- **Bank "Sign in" again** can't reopen a bank's sign-in: the server's
  `money.bank.hosted_link` takes no bank id (Plaid update mode). The app now says to disconnect
  and connect the bank again. Server follow-up: an optional `bank_id`.
- **The opening-balance move for an older statement** writes the lines, then the opening, on two
  connections, because relay-money has no call that changes an opening. A kill between the two
  leaves the lines in, and the next read of the file offers the move again. To make it one
  transaction, `AccountPatch` in relay-money needs an `opening_balance` field.
- **A Wealthsimple activities file mapped by hand** can't show its duplicates in the preview:
  relay-money's `invest_preview` takes no account mapping.
- **Two Desjardins accounts of the same kind in one file** share the bank's remembered target
  unless the file names its accounts.

## Behaviour changed on purpose

- **Plain `ws://`** is used only when the server offers no `wss://` route (loopback always). A
  refusal heard over a clear route reads as unreachable, not unpaired.
- **The phone asks before replacing its ledger** whenever entries were made during a pull, the
  server's ledger is stale with changes not sent, the server's ledger is far smaller than the
  phone's, or categories were changed before the first pull.
- **Pairing again with the same server** merges from where sync stood. Only a new server starts
  with a pull-only first sync.
- **The launcher icon** follows an explicit Light or Dark choice only, never "System": Pixel
  Launcher drops the home-screen shortcut when its alias is switched off. It switches when the
  app is left with Back.
- **Net worth:**
  - Transfers between the person's own accounts never move net worth.
  - A card always counts what it owes, even opened at zero.
  - An account given no starting balance reads its balance day by day, never below zero from
    spending alone.
  - An investment account priced from holdings adds the transfers made after its report, and a
    newer value recorded by hand wins over an older report.
  - Settings, goals, account pages and the Investments page all read the same figure as Accounts.
- **Changing currency** to one with other decimals is refused once anything is kept, budgets and
  goals included.
- **The sync secret** (`files/sync-secret.json`) is left out of device-to-device transfer, so a
  new phone pairs again instead of sharing the old phone's device id.
