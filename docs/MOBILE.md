# Relay from a phone

Relay's phone app (`apps/relay-android`, "Relay" on the phone; [ANDROID.md](ANDROID.md)) reaches
the engine on this machine through `crates/relay-remote`: the same bus lines the desktop client
and the CLI speak (`BUS.md` §6.2), carried over a WebSocket to a phone that paired once.

Two routes, tried in privacy order by the phone:

| route | when | what travels where |
| --- | --- | --- |
| **direct** | phone and PC on the same network, or the same Tailscale tailnet | LAN only (plain `ws://`, readable on the network), or Tailscale's end-to-end encrypted tunnel |
| **via a rendezvous** | phone anywhere | PC dials *out* to a server you host; the phone joins there; the server copies lines between the two and can read them (§6) |

Both end in `bridge.rs` on the PC, which checks the phone's credential and forwards its
requests to the socket door as actor `user` — the same door, the same guardrails and audit
rows as the CLI, but not the whole bus: a phone may call only the ops the app uses
(`PHONE_OPS` in `crates/relay-remote/src/wire.rs`, plus `bus.unsubscribe`). Anything else,
`settings.set` or `app.quit` for example, is refused at the door with `refused` /
`remote.op` and never reaches the engine; run it on the PC. There is no third route:
Tailscale, if you use it, only carries the direct link, and sees encrypted packets, not bus lines.

## 1. Tonight, in three steps

On the PC, from the RelayV4 checkout, `./run.sh` as usual. The engine it starts carries the
phone door (`relay serve --remote`), so there is nothing else to keep running. Then:

```fish
./target/debug/relay remote pair              # prints the QR code (finds the dev engine on its own)
```

Leave it running: when the phone presents the code, it asks `Pair "<phone>" (…) with this PC?
[y/N]`, and only a yes pairs it. The phone waits up to 90 seconds for that answer; the PC gives
up after 75, and the phone then says nobody approved it (`pair.unconfirmed`). A no tells the
phone the PC declined (`pair.declined`). Either way the code is spent: run `relay remote pair`
again. A phone that gives up sooner withdraws its request, so a late yes pairs nothing.

On the phone: install the app (the `relay-android-apk` artifact of the **Relay Android**
workflow, or `./gradlew :app:installDebug` in `apps/relay-android` with the phone plugged in),
open it, scan. If the engine was running from before the
door existed, restart it once (`./target/debug/relay --instance dev cmd app.quit '{}'`, then
`./run.sh`), or run `./target/debug/relay remote serve --pair` alongside it. The quit is refused
while agents are live (`app.sessions_live`); `app.quit '{"force":true}'` stops them, and their
sessions come back as restorable. For the phone to
reach the PC when you are out, see §3 (Tailscale) or §4 (your own server).

## 2. On the PC

With an engine running (`relay serve`, or the desktop app):

```fish
relay remote serve --pair       # front the engine for phones, open a pairing window, print the QR
```

Or both in one process, which is what a login service wants:

```fish
relay serve --remote            # engine + phone door; `deploy/relay-remote.service` runs this
```

The unit runs the checkout's `target/debug/relay` on the `dev` instance, the engine `./run.sh`
and the desktop app use; edit its `ExecStart` path to your checkout before installing it.

The door listens on `0.0.0.0:7420` for phones on the network. Until a phone is paired or a
pairing window is open it answers nobody: connections are closed unread, `GET /info` included.
A pairing window lasts ten minutes and admits one phone; to pair another later,
`relay remote pair` in a second terminal. Opening a window replaces any older code still open,
and a code answers one phone at a time. Each pairing is confirmed at the terminal that opened
the window (`--no-confirm` on `pair` or `serve --pair` lets the first phone with the code pair
without asking; without a terminal, `pair` refuses unless it is given). Every new pairing is
logged at warn in the engine log, naming the phone, and shown as a toast in the desktop app if
it is open (`ui.toast`). Nothing lasting lands in the notification centre yet: no bus op lets a
client create a notification, so that needs one in the engine.

Every connection has ten seconds to finish its WebSocket upgrade and thirty more to say hello.
The door holds at most 64 connections, of which at most 16 may be unproven and at most 4 from
any one address, so a stranger on the network cannot exhaust the engine's descriptors or lock
a paired phone out. A browser page on another site cannot open it (the `Origin` must match).

| command | does |
| --- | --- |
| `relay remote serve --bind 192.168.1.20:7420` | listen on one address only |
| `relay remote serve --start-engine` | serve phones with no engine running, and start one when a paired phone arrives (the login unit `deploy/relay-door.service`; ANDROID.md) |
| `relay remote devices` | list paired phones |
| `relay remote revoke <id>` | forget a phone; a connection it has open closes within seconds and its next one is refused (once no phone is left, the door goes quiet, so that phone sees the PC as unreachable rather than "no longer paired") |
| `relay remote name "antho desktop"` | the name phones show |
| `RELAY_INSTANCE=dev relay remote serve` | serve the dev engine instead of stable |

State lives in `~/.local/share/relay-v4/<instance>/remote.json`, mode 0600. It holds each
phone's token and the rendezvous room secret (§4), so treat it like a private key.

## 3. From anywhere without a server: Tailscale

The simplest way to reach the PC away from home needs no server at all. Install
[Tailscale](https://tailscale.com) on the PC and on the phone and sign both in to the same
account. The PC then has a `100.x.y.z` address that the phone can reach from any network, and
the direct door already listens on it: `relay remote pair` prints it on a `Tailscale:` line and
puts it in the pairing link, so a phone paired after Tailscale was set up just works.

For a phone paired before that, open the PC's page (tap the PC's line in the status bar), and
under **Routes** enter the PC's Tailscale address (`100.x.y.z`, or its MagicDNS name
`my-pc.tail1234.ts.net`). Added addresses are tried with the WiFi ones, and the status bar says
**Tailscale** when that is the link in use. Traffic travels
over Tailscale's encrypted WireGuard tunnel; nothing on the PC listens to the internet.

## 4. From anywhere: your own rendezvous

The PC never listens to the internet. Instead it keeps one outbound WebSocket open to a small
server you run, and phones join through it — the shape of a remote-control service, with the
service being yours.

On any machine with a public address (a VPS, a Raspberry Pi behind a port forward, a Tailscale
node):

```fish
relay remote rendezvous --bind 0.0.0.0:7430
```

`deploy/rendezvous.Dockerfile` builds it as a container and `deploy/relay-rendezvous.service`
runs the binary under systemd; both are reached on loopback, expect a TLS proxy in front that
writes `X-Forwarded-For`, and so trust one proxy (`--trust-proxy 1`, under "Per address" below).
The unit also keeps what the server remembers (`--state`) in its state directory. The PC
pings the server every 30 s and redials after 90 s of silence, so a dropped NAT flow does not
leave phones told "offline" for long.

Put TLS in front of it. The phone sends its pairing code and, once, receives its token over
this link; `wss://` keeps that from anyone *between* the phone and the server, but TLS ends at
the proxy, so the rendezvous process itself sees the token and every line (§6). A Caddyfile is
enough:

```
relay.example.org {
    reverse_proxy 127.0.0.1:7430
}
```

Then on the PC:

```fish
relay remote via wss://relay.example.org
relay --instance dev cmd app.quit '{"force":true}'    # then ./run.sh
relay remote pair
```

`via` mints a room secret and stores it. The PC presents it to the server as
`Authorization: Bearer <secret>`, so it stays out of URLs and proxy access logs; a server
that predates the header refuses that with its own "room secret does not open this room", and
only then does the PC dial it once more with `?secret=` in the URL (and log a warning to update
the server). Any other 403, from a proxy or a firewall, gets no retry. The process that carries the phone door
dials the server only when it starts, so restart it: for the `./run.sh` engine (`relay serve --remote`),
quit it and run `./run.sh` again. `force` stops live agents, whose sessions come back as
restorable; without it the quit is refused while any is live. (`via` says to restart `relay
remote serve`; that applies only when a separate `relay remote serve` carries the door. It
cannot run beside a `serve --remote` engine, which already holds the port.) Every pairing link
from then on carries the join address as well as the LAN ones. The phone tries the LAN first
and the server second; a PC card in the app can pin either. `relay remote via off` clears it.

A room is named by the digest of its secret, so only the PC that holds the secret can host it.
But anyone can make up a secret and host a room of their own, and anyone who knows a room id can
join it, so the server guards what strangers can hold:

- **Rooms.** List the rooms your server is for, and it hosts and joins no others:

  ```fish
  relay remote rendezvous --bind 127.0.0.1:7430 --room <id> --room <id>
  # or: set -x RELAY_RENDEZVOUS_ROOMS <id>,<id>
  ```

  A room's id is what `relay remote via` prints (`room:`), what `relay remote serve` prints when it
  dials, `rendezvous.room` in that engine's `remote.json`, and `room=` in the server's own
  `host joined` log line. A host for an unlisted room is refused with 403 ("this rendezvous does not
  host that room"). Without a list the server warns at start and still works, less surely. Rooms
  it does not remember are capped at 16 hosted at once. A room is remembered (up to 64) only once
  a host has held it for five minutes, so rooms a stranger opens and closes are never remembered
  and never crowd yours out; a remembered room is let in past the cap whenever its PC comes back.
  With 64 remembered, a newly proven room takes the place of the one useful least recently that
  has no host at the moment, never of one whose host is there. What a stranger can still do is
  hold all 16 new-room places while your PC is a room the server does not remember: just after a
  restart without `--state`, before your PC has held its room five minutes, or after strangers'
  rooms, each held five minutes, took its place while it was off. Your PC is then refused until
  they let go. Only the list makes sure your PC gets in. What the server remembers lasts until it
  exits, or across restarts with `--state <file>` (`RELAY_RENDEZVOUS_STATE`), a small JSON map
  of room ids to when each was last useful. The unit in `deploy/` keeps it in its state directory;
  the container has none unless you mount a volume and point `RELAY_RENDEZVOUS_STATE` into it.
- **Phones that never say hello.** A phone that joins has 15 s to send its first line (the app
  answers the greeting at once), or its lane is closed.
- **Per address.** One address may hold 16 connections still finishing their upgrade and 8
  joined phones that have not said anything yet. Loopback, private (10/8, 172.16/12,
  192.168/16, fc00::/7) and link-local addresses are never counted: such a peer is a proxy on the
  same machine or network, which every phone comes through. Caddy on the host reaching the
  container comes from the Docker bridge's gateway (172.17.0.1), for one. 100.64/10 is counted: on
  a server it is a Tailscale peer, one device per address. So behind a proxy these caps count
  phones only once you say how many proxies in front append to `X-Forwarded-For`, with
  `--trust-proxy <n>` (`RELAY_RENDEZVOUS_TRUST_PROXY`). The server then counts the n-th address
  from the right, the one the outermost proxy saw; whatever a phone wrote further left is never
  read. 1 for a Caddy (or an nginx with `proxy_add_x_forwarded_for`) that phones reach directly:
  the container image and the unit in `deploy/` ship with 1. 2 when that Caddy sits behind another
  proxy *and* trusts it (Caddy's `trusted_proxies`): without that, Caddy drops the outer proxy's
  header and writes only the outer proxy's own address, and 1 would count that one address for
  every phone, so eight quiet joins from anyone would turn every phone away. A header with fewer
  entries than n is counted by the connection's own peer instead (the server warns once). Set n
  only when nothing reaches the server except through those proxies, since anyone else can write
  the header: run the container with `-e RELAY_RENDEZVOUS_TRUST_PROXY=0` if you expose it without
  a proxy. To check n, run with `RELAY_LOG=info,relay_remote::rendezvous=debug`: each join logs the
  header it came with and the address counted.

`GET /health` reports how many hosts and lanes are up.

The server never waits on one phone. Each phone may have 8 MiB queued toward it; a phone that
drops off a mobile link without closing fills that and is cut off (it reconnects), and a phone
silent for 90 s is closed, so it cannot stall the other phones in the room. A room carries at
most 32 phones at once.

For Tally and Avex to sync while the PC is **off**, the same server can also run a cloud copy
of Relay that keeps their data and that the PC merges with: docs/CLOUD.md.

## 5. On the phone

The app looks like the desktop: the Dev | Threads switch, the sidebar as a drawer, the status bar
with the link, the route in use and what waits in the outbox. It keeps a copy of the PC's data and
works with the PC away; [ANDROID.md](ANDROID.md) says how, and what it does when the PC is on,
asleep or off.

**Pairing.** Scan the QR that `relay remote pair` prints. Without a camera, paste the
`relay://pair?…` link, or type the address the PC printed and the eight-character code, then
approve the phone in the terminal on the PC when it asks. The code is spent on use, whether
the PC said yes or no. A `relay://pair` link opened from another app only fills the field; pairing
takes a tap.

**Routes.** The PC's page shows where it can be reached and lets you pin a route: **Auto** tries
the direct addresses (WiFi and Tailscale) first and the server second; **Direct only** never
uses the server; **Server only** always goes through the rendezvous.

**What you can do.** Everything is an existing bus op, limited to the ones the door lets a
phone call (see the top of this page):

- **Agents**: the project's agents as terminal plates with a live miniature; a tap opens the
  terminal, which borrows the PTY at the phone's own size (`session.resize {until_detach}`), with
  a key row and a composer. A drag scrolls the phone's own scrollback; in the alternate screen
  (Claude Code), which keeps none, it turns the program's wheel instead, as the PC's terminal
  does: mouse reports at the finger's cell, or arrow keys when the program asked for no mouse.
  **New agent** starts one (`session.create` + `session.spawn`), queued
  until the PC is back if it is away.
- **Inbox**: guardrail holds with the exact held action and **Allow once** / **Deny**
  (`guardrail.confirm` / `guardrail.reject`), tasks in review, and the notification feed.
- **Board, tasks, modules, notes and mail**: read from the phone's copy, edited through the
  outbox.
- **Files and Git**: the tree, files, changes, commits, pushes and pull requests, with the PC.
- **Threads**: the PC's conversations, streamed as they are written, and Tally, Arbiter and Avex.

With the app in the background, a held, blocked or finished agent shows up as a notification; it
comes from the PC link (**Stay connected**), not from a push service. The link reconnects by
itself after a drop and when the app comes back to the foreground; **Disconnect** on the PC's page
stops that until you connect again.

## 6. Security model, honestly

- A phone is the user at the keyboard. The door refuses any envelope whose actor is not
  `user` (or `test` on a dev/test instance) before the engine sees it, so a phone cannot
  borrow an agent's identity even with a guessed token.
- A phone also gets fewer ops than the user at the keyboard: only those in `PHONE_OPS`
  (`crates/relay-remote/src/wire.rs`). Ops that reconfigure what the engine runs
  (`settings.set`, which holds provider and adb paths), quit it, or that no screen of the app
  uses are refused (`refused` / `remote.op`). A WebSocket frame holding several lines is
  gated line by line, so a permitted first line cannot carry a refused second one through.
- A paired phone is trusted to execute on the PC as you: it starts agents and shells, edits
  files and runs builds, so a lost phone's credential can run code here until you revoke that
  phone (the table in §2). What it may not do is weaken the guardrails that hold agents back:
  a `guardrail.config.set` patch that touches what agents may run or where they may write
  (`denied_commands`, `allowed_write_roots`, `roles`, `shape_gates`, `agent_builds`), or a
  `settings.reset` at or above one of them (the whole tree, `guardrails`, a whole layer), is
  refused (`remote.command`). So, as defence in depth, is `project.update` with a `build_cmd`
  or `run_cmd` (other than `null`, back to the default), which the PC runs with no agent in
  between; it is not a wall, since a phone that edits the repository can change what a build
  runs in other ways. The app shows those values; they change on the PC. A phone also
  publishes no release (`remote.publish`) and starts no session on the API key
  (`remote.api_key`).
- **App developers:** an op the phone starts calling must be added to `PHONE_OPS` (it is
  sorted; the gate binary-searches it), or the door refuses it. `wire.rs`'s
  `every_op_the_app_sends_is_open_to_a_phone` reads the app's sources and fails when one is
  missing.
- Pairing codes live ten minutes and are single use, a new code replaces any older one, and the
  PC approves each phone that presents one before it gets a token (unless `--no-confirm`), so
  a code seen by someone else — a screen share, scrollback — pairs nothing on its own. Every
  pairing is logged and toasted on the desktop. A device token is 256 bits, stored on
  the PC in `remote.json` and on the phone sealed with a key in its keystore.
- After pairing, the token is never sent again: each connection gets a random challenge and
  the phone answers with `sha256(challenge:token)`. Listening on the LAN yields proofs that
  are only good for that connection.
- The one message that carries the token is the pairing reply. On your own WiFi that is a
  LAN packet; through a rendezvous it should ride `wss://`, which is why `via` warns on
  `ws://`. Anyone who captures the pairing reply holds the phone's credential for good.
- The rendezvous server sees the same bytes the LAN would — encrypted by TLS in flight if
  you set it up as above, but readable by whoever runs the server. Run it yourself.
- Before a phone has proved itself, the door says only what the handshake needs: the protocol
  version, the instance, whether the engine is running, and the challenge. Through a rendezvous
  that greeting goes to anyone who knows the room id, so the PC's name and id, its Relay version
  and its network cards' MAC and broadcast addresses (for Wake-on-LAN) come in the welcome, once
  the phone is admitted. The greeting still carries `host`, `host_id` and `version`, empty, for
  older readers that require them.
- **The channel is not authenticated after the handshake, and the PC never proves itself.**
  The proof shows the PC that the phone holds its token, once, at the start of the
  connection; nothing seals the lines that follow. Someone *on the path* — an active attacker
  on the same WiFi (ARP spoofing, a rogue access point) or whoever runs the rendezvous — can
  read every line, and can inject bus requests as `user` into a connection the phone already
  opened. That is command execution on the PC: `session.spawn` and `session.input` are among
  the phone's ops. The phone also cannot tell its PC from an
  impostor answering at a stored address. Passive listening alone gets proofs that are no
  good on another connection, and nothing reaches the engine from a peer that has no proof.
- Until the link is sealed end to end (a planned wire v2: the PC's public key in the pairing
  QR, an ephemeral key exchange with the token as a pre-shared key, every line encrypted and
  authenticated), the route that resists an active attacker is **Tailscale**: bind the door
  to the tailnet address only, `relay serve --remote --remote-bind <100.x.y.z>:7420` (or
  `relay remote serve --bind …`), and pair over it. Use the LAN route only on a network you
  trust, and treat a rendezvous you do not run as able to act as you.
- To cut a phone off from the PC side: `relay remote revoke <device id>` (the id is on the
  PC card in the app). An open connection closes within a few seconds.
- The op list narrows what a lost phone or an injected line can do; it is not a sandbox for a
  hostile phone. A paired phone can still start agents and type into their terminals, which
  is acting as you.

## 7. Verifying

```fish
cargo test -p relay-remote
```

covers pairing, proof, revocation, the actor gate, the op gate (`settings.set` refused),
`GET /info`, event interleaving and a
two-phone rendezvous session against a real engine. It runs in CI with the rest of the
headless crates. The app's own tests, Lint and APK builds run in `relay-android.yml`, and
`LiveDoorTest` runs its link and sync code against a real door (ANDROID.md).
