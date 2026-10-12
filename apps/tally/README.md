# Tally

The budget and spending tracker for Android, in Relay's look, synced with nothing to set up.
You log what you spend in two seconds, and the app answers one question at all times: how much
room is left this month, and is the month on pace.

Kotlin, Jetpack Compose, Room over SQLite, Hilt. This is the second Tally: the first lives in
antho976/RelayV4 (`apps/tally`) and is retired once its ledger has moved over (see "Moving from
the first Tally").

## Sync, with nothing to do

Tally keeps its ledger on the phone and syncs it with **Relay's cloud copy on the owner's
server**: the same Render service as the Discord bot (antho976/antho-s-bot, `deploy/relay/`).
The PC's Relay syncs with that copy too, so the phone, the server and the PC hold one ledger,
whether the phone is on the home Wi-Fi or not.

1. On first launch Tally asks the server to let it in (`POST /api/tally/enroll`), sending only
   the sha256 of a secret it keeps.
2. The bot DMs the server's owner on Discord: "Tally on Pixel 8 wants to sync with your server",
   with a four-character code the phone shows too, and **Allow** / **Deny**. That tap is the only
   step, once per phone. (DMs closed to the bot: the dashboard's **Tally sync** page has the same
   buttons.)
3. Tally collects a Relay pairing link (`POST /api/tally/claim`, proving the secret), pairs with
   the cloud engine through its rendezvous (`wss://<server>/relay`), and runs its first sync.

The first sync **merges**: a new phone takes the server's ledger, and a phone with entries keeps
them. After that it syncs at launch, when the app comes back to the front, three seconds after an
edit, the moment a network comes back, and every fifteen minutes in the background. Entries made
offline wait and go. Settings, Sync shows where it stands and holds the rare manual acts (send this
phone's ledger, take the server's, disconnect, another server address).

The server address is built in (`https://anthos-bot.onrender.com`; `-PtallyServer=…` or
`TALLY_SERVER` builds for another).

## What it does

- **Spend** (the top bar's switch): **Overview** (what is left with its pace, what each day can
  take, money in and out, net worth, the week, budgets furthest over pace, bills coming up, goals,
  recent entries, accounts), **Entries** by day month by month, **Insights** (the month's line
  against the pace with scrubbing, where it went, payees, largest, six-month trend, a calendar of
  days, net worth).
- **Plan**: **Budgets** (the monthly budget with a projection, envelopes, what was spent without a
  budget, budgets suggested from three months of averages), **Bills** that post themselves,
  **Goals** (pots, a balance by a date, a share of income invested or kept), **Accounts**,
  **Investments** (Wealthsimple files, holdings, room).
- **Add** from every page (and from the launcher icon's long-press): keypad, the usual entries one
  tap away, categories most used first, account, day, note with suggestions, repeat.
- **Search** every entry ever, from the top bar or the launcher.
- **Import** Desjardins, Wealthsimple or any bank's CSV; **Backup** weekly and to a folder;
  **Export** CSV; categories with over a hundred icons; three grounds (Relay's matte, dark,
  pure black).

## Moving from the first Tally

The two install side by side (this one is `com.quietsoftware.tally2`). If the first Tally already
synced with the PC or the server, there is nothing to move: this one takes the ledger on its first
sync. Otherwise, in the first Tally: Settings, Backup, Save a file; then in this one: Settings,
Backup, Restore from a file (or "Restore a Tally backup" on the first screen). Uninstall the first
one after.

## Build

```
./gradlew :core:test :link:test      # money rules and the link to the server, plain JVM
./gradlew :app:testDebugUnitTest     # Room, repositories, sync against a fake door, screens (Robolectric)
./gradlew :app:assembleDebug
```

Needs JDK 21 and the Android SDK with `platforms;android-37.0`. `:link` has a live test that runs
the whole way in against a real server: `TALLY_E2E_SERVER=http://127.0.0.1:3000 ./gradlew :link:test`
(see `CloudLiveTest`).

## CI

`.github/workflows/ci.yml`, on every push: Guard (wrapper, no keystores, network code only in the
link), Core (`:core` and `:link`), Verify (unit and screen tests, Lint, debug and minified release
APKs, uploaded as **tally-apks**), Instrumented (device SQLite and a cold launch of the minified
APK on an emulator). `record-screenshots.yml` records the screen goldens; `release.yml` turns a
`v*` tag into a GitHub Release.

## Money rules

`core/` is the first Tally's money rules, unchanged, with their tests; antho976/RelayV4's
`crates/relay-money` is its Rust port, and the two are held to the same tests. A money rule changed
here is changed there in the same change.
