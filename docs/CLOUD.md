# A cloud copy: Tally and Avex sync while the PC is off

Tally syncs its ledger with the PC (docs/MONEY.md) and Relay reads Avex's training history
(docs/GYM.md), but only while the PC is on. A **cloud copy** is a second, headless Relay engine on
an always-on server. Tally and Avex send their data to it whenever they have a connection, and the
PC's **cloud link** merges that copy into its own every minute while it runs.

```
Tally ──money.sync──▶ ┐                              ┌──── PC: relay serve --remote
                      │  server: rendezvous + cloud  │     (cloud link, every minute:
Avex ──gym.upload───▶ ┘  engine (--remote-ops cloud) └────  money.sync both ways, gym.raw home)
```

Decided with Antho on 2026-10-11:

- **One server for everything.** The cloud copy runs in the same container as Antho's Discord bot
  on Render (antho976/antho-s-bot, `deploy/relay/`), behind the bot's own address: the server's
  rendezvous at `/relay`, the bot's dashboard everywhere else. The rendezvous and the engine take
  about 35 MB between them, and the Caddy in front of both about 40 MB.
- **Avex uploads only in Antho's own build.** Avex's public build keeps its promise that it
  "physically cannot upload anything": the uploader is a separate `personal` build type in Avex
  (`forge-android/app/src/personal/`), the only place that holds the INTERNET permission.
- **The PC stays where the dev side lives.** Agents, terminals, files and git need the PC's
  repositories; the cloud copy holds none of them and offers no op that reaches them. The phone
  app reaches those through the PC as before (docs/MOBILE.md).

## What the cloud copy is

`relay serve --remote --remote-ops cloud`: the same engine, whose door admits a paired device to
`CLOUD_OPS` (`crates/relay-remote/src/wire.rs`) and nothing else: `money.sync` and the ledger's
reads, `gym.upload`, `gym.raw` and the training history's reads, and `bus.ping` /
`bus.subscribe`. A terminal, a file, a git command, a thread, a reset or an import from a path is
refused with `remote.op`, whatever the credential. Its data is the engine's usual files on the
server's disk: `money.db`, `gym.db`, and `gym-upload.json`, the text of the last upload.

It is reached through a rendezvous only. The engine dials its rendezvous on loopback, while its
pairing links name the public address (`relay remote via ws://127.0.0.1:7430 --public
wss://<server>/relay`), and `relay remote pair --via-only` leaves the container's own addresses out
of the link.

## The two new ops

| op | who | what |
| --- | --- | --- |
| `gym.upload` | the person | Avex's whole `avex_export.json` as text: read as `gym.import` reads a file, replaces the copy, and keeps the text |
| `gym.raw` | the person | the last uploaded text and its sha256; with `have` set to that digest, the text is left out |

Both are in `CLOUD_OPS` and not in `PHONE_OPS`: on the PC, only the cloud link uploads, through the
socket door. An import from a path, or a reset, drops the kept text, so `gym.raw` never hands out
an export the copy no longer holds.

## The cloud link (`crates/relay-remote/src/cloud.rs`)

The PC pairs with the cloud copy like a phone, and keeps the credential in `cloud.json` beside
`remote.json` (mode 0600). While `relay serve --remote` runs, it merges every minute; `relay cloud
sync` merges now.

A round:

1. Ask this PC for its changes since the link's cursor here (`money.sync` with no changes).
2. Send them to the cloud with the link's cursor there; take the cloud's changes from its answer.
3. Send those to this PC. What this PC answers with is an edit made here while the round ran: it
   goes on to the cloud, for up to four passes; past that, the PC's cursor stays put and the next
   round asks again (what is sent twice, the other side already has: an equal stamp is ignored).
4. Ask the cloud for `gym.raw` with the digest last taken; a newer export is `gym.upload`ed here.

The link keeps no ledger of its own, only a cursor and a generation for each side. Newest edit wins
per row on each side, as with Tally, so the two converge. The wholesale acts keep their meaning:

- **The cloud's ledger was replaced** (Tally's first sync with the cloud, which makes the cloud's
  ledger the phone's; or a restore there): the cloud answers `money.sync_stale`, and its whole
  ledger replaces this PC's.
- **This PC's ledger was restored, erased or replaced**: this PC answers `money.sync_stale`, and
  its whole ledger replaces the cloud's, and from there Tally's on its next sync.

Before a replace clears either side, `cloud.json` records which side is going over (`replacing`).
A round that stops partway (a batch failed, the engine shut down) leaves the other side holding part
of the ledger, which would read as stale next time; the next round instead finishes the same
replace from the same side, before anything else, and clears the mark. Otherwise a side's cursor
moves only once the other side has taken its changes: a round that fails partway is asked again.

Each side lists the link as a device (`PC (<name>)` on the cloud, `Cloud (<name>)` here), so
neither posts bills: Tally alone does, as when it syncs with the PC directly. A long name is cut to
the 64 characters `money.sync` takes.

Large merges go in batches of 2,000 rows, in dependency order across batches, under the 8 MiB a
device may send the rendezvous in one message. Answers come back in pages of 2,000 rows
(`money.sync`'s `limit` and `more`, docs/MONEY.md), so the cloud's whole ledger never has to fit
the 15 MiB line the rendezvous carries to a device; the link reads every page before it hands the
rows on, keeps the newest of each, drops what the other side handed back unchanged, and moves a
cursor only as it did before. A cloud engine from before pages refuses `limit` (`bus.schema`) and
is asked whole, as before.

## Setting it up

On the server (the bot's container does all of this from `deploy/relay/start.sh` when
`RELAY_PUBLIC_URL` is set):

```fish
relay remote rendezvous --bind 127.0.0.1:7430 --queue-mib 64
relay --instance stable remote via ws://127.0.0.1:7430 --public wss://<server>/relay
relay --instance stable serve --remote --remote-bind 127.0.0.1:7420 --remote-ops cloud
```

Then each device connects with one tap on Discord (next section). Without the bot, a shell on the
server (Render: the service's **Shell** tab) still pairs a device, once per device:

```fish
relay --instance stable remote pair --via-only
```

It prints a `relay://pair` link and asks `[y/N]` before the device that presents it is paired:
`relay cloud pair '<the link>'` on the PC, or the link pasted into the device.

## Connecting through your server (`crates/relay-remote/src/connect.rs`)

The bot beside the cloud copy (antho976/antho-s-bot, `/api/connect`, `deploy/relay/README.md`)
takes a request from a device, DMs the owner **Allow** / **Deny** with a four-character code the
device shows too, and hands the device a pairing link once allowed. The device proves it is the one
that asked: it sent only the sha256 of a secret it keeps, and collects the link with the secret.

- **The PC:** `relay cloud connect <server>` (e.g. `relay cloud connect antho-s-bot-8dve.onrender.com`),
  tap Allow, then restart `relay serve --remote` (or Relay). It pairs with the cloud copy as `relay
  cloud pair` did, and keeps a token for the server in `cloud.json` (`server`, `server_token`). A PC
  already linked with `relay cloud pair` keeps its link and only adds the token.
- **Tally** asks the first time it opens; **Avex** (personal build) from "Avex → Relay".
- **The Relay app** (personal build only, `apps/relay-android/app/src/personal`): Pair, Connect
  through my server. The bot cannot reach the PC, so the PC comes to it: while `relay serve
  --remote` runs on a PC connected this way, it asks the server every five seconds for Relay phones
  the owner allowed, opens a pairing window that **this desktop confirms** for each, and hands the
  phone that window's link through the server. When the phone presents the code, the PC asks with a
  notification carrying Accept and Deny (`notify-send --wait`; `RELAY_ASK` names another program
  that prints `accept` or `deny`). With nothing to ask with, a toast says to run `relay remote
  approve` (or `decline`) while the phone waits, about a minute and a half. The Discord tap alone
  never gives a phone the PC: the phone gets in only once both answered yes.

`relay cloud status` shows the server; forgetting the PC on the dashboard (Devices) stops its token
working, and `relay cloud off` forgets the link here.

## Security

- The server ends TLS: whoever runs it (Antho, and Render) can read the ledger and the training
  history in flight and on its disk. The phone link is not sealed after the handshake
  (docs/MOBILE.md §6); here the reach of an injected line is `CLOUD_OPS`: the data, never the PC.
- The link's credential on the PC is good for the cloud copy alone. Revoke it there with `relay
  remote revoke <id>` (`relay remote devices` lists it).
- Connecting through the server: the bot sees the PC's pairing link for a Relay phone, code
  included, so whoever runs the server could present that code first. That is why the PC opens the
  window with confirmation on: a phone gets in only after the person at the PC accepts the device
  the notification names. The PC's token for the server opens nothing on the PC; it only lets the
  PC ask for waiting phones and hand them links.
- `relay remote rendezvous --queue-mib` caps what the server queues toward phones; the default
  (512 MiB) is more than a 512 MB container holds.
- The rendezvous hosts any room whose secret is presented unless it is given a list (docs/MOBILE.md
  §4). On this server, list the two rooms it is for: the PC's and the cloud engine's
  (`RELAY_RENDEZVOUS_ROOMS=<pc room>,<cloud room>`; each is the `room:` that `relay remote via`
  printed, or `rendezvous.room` in that engine's `remote.json`). Without the list it still works and
  warns at start: rooms it has seen hosted are let back in ahead of strangers' made-up ones, until
  it restarts. Behind Caddy every connection comes from loopback, so the per-address caps count
  nothing unless `RELAY_RENDEZVOUS_TRUST_PROXY` says how many proxies append to
  `X-Forwarded-For`: 2 only if Caddy is set to pass on the header from Render's proxy in front of
  it (Caddy's `trusted_proxies`). Otherwise leave it unset: 1 would count Render's proxy, which
  every phone shares, as one address. A phone that joins and never says hello is cut after 15 s
  either way.

## Verifying

`cargo test -p relay-remote` runs the round against two ledgers (`cloud.rs`), and end to end
(`tests/remote.rs`, `a_cloud_copy_takes_the_phones_data_and_the_pc_link_brings_it_home`): a cloud
engine behind a real rendezvous takes a phone's ledger and an Avex export, refuses a terminal, and a
PC engine's link brings both home and sends its own entry back. `crates/relay-core/tests/gym.rs`
covers `gym.upload` and `gym.raw`.
