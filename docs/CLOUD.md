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

Each side lists the link as a device (`PC (<name>)` on the cloud, `Cloud (<name>)` here), so
neither posts bills: Tally alone does, as when it syncs with the PC directly.

Large merges go in batches of 2,000 rows, in dependency order across batches, under the 8 MiB a
device may send the rendezvous in one message.

## Setting it up

On the server (the bot's container does all of this from `deploy/relay/start.sh` when
`RELAY_PUBLIC_URL` is set):

```fish
relay remote rendezvous --bind 127.0.0.1:7430 --queue-mib 64
relay --instance stable remote via ws://127.0.0.1:7430 --public wss://<server>/relay
relay --instance stable serve --remote --remote-bind 127.0.0.1:7420 --remote-ops cloud
```

Then, in a shell on the server (Render: the service's **Shell** tab), once per device:

```fish
relay --instance stable remote pair --via-only
```

It prints a `relay://pair` link and asks `[y/N]` before the device that presents it is paired.

- **The PC:** `relay cloud pair '<the link>'`, then restart `relay serve --remote` (or `relay
  cloud sync` to merge at once). `relay cloud status` says how the last round went; `relay cloud
  off` forgets the link.
- **Tally:** Settings, Relay on your PC, paste the link. Tally treats the cloud copy as its PC: its
  first sync makes the cloud's ledger the phone's, and the PC takes it on its next round.
- **Avex (personal build):** the "Avex → Relay" screen, paste the link.

## Security

- The server ends TLS: whoever runs it (Antho, and Render) can read the ledger and the training
  history in flight and on its disk. The phone link is not sealed after the handshake
  (docs/MOBILE.md §6); here the reach of an injected line is `CLOUD_OPS`: the data, never the PC.
- The link's credential on the PC is good for the cloud copy alone. Revoke it there with `relay
  remote revoke <id>` (`relay remote devices` lists it).
- `relay remote rendezvous --queue-mib` caps what the server queues toward phones; the default
  (512 MiB) is more than a 512 MB container holds.

## Verifying

`cargo test -p relay-remote` runs the round against two ledgers (`cloud.rs`), and end to end
(`tests/remote.rs`, `a_cloud_copy_takes_the_phones_data_and_the_pc_link_brings_it_home`): a cloud
engine behind a real rendezvous takes a phone's ledger and an Avex export, refuses a terminal, and a
PC engine's link brings both home and sends its own entry back. `crates/relay-core/tests/gym.rs`
covers `gym.upload` and `gym.raw`.
