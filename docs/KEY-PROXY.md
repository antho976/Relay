# The API-key proxy

A Claude session launched with **Use API key** bills an Anthropic API key instead of the signed-in
subscription. The key is held by `relay-key-proxy`, a small service that runs as its own throwaway
system user. Nothing that runs as you can read it: not an agent, a repository's hooks, an MCP
server, or a command injected over the phone link.

## How it works

```
Claude (you) ──x-api-key: <pass token>──▶ 127.0.0.1:7445  relay-key-proxy (its own user)
                                                │  checks the token, swaps it for the key
                                                └──TLS──▶ api.anthropic.com
```

- **The key** is stored once in `/etc/credstore.encrypted/relay-anthropic-key`, encrypted by
  `systemd-creds` with the host's root-only key (and the TPM, where there is one). systemd decrypts
  it for `relay-key-proxy.service` alone, which runs with `DynamicUser=yes`: a user id no other
  process has. Your account cannot read that service's credential, its memory or its environment
  (another user's, guarded by the kernel and Yama), so no agent can. It can see that the process
  exists, nothing more.
- **The pass token** in `~/.config/relay/key-proxy-token` is what Claude sends. The proxy serves one
  request per connection and forwards it only when it carries exactly that token, is addressed to
  `127.0.0.1:7445`, and goes to one of the endpoints Claude Code uses (`POST /v1/messages`,
  `POST /v1/messages/count_tokens`, `GET /v1/models`) with a body of a stated length. It sends it to
  `api.anthropic.com` only, over TLS checked against roots built into the binary (a certificate
  added to the system store cannot intercept it), with a fixed list of headers and the token
  swapped for the key. Everything else is answered by the proxy and goes nowhere. Admin keys
  (`sk-ant-admin…`) are refused, so the token is never worth more than API calls.
- **Relay** stores only the session's on/off choice (`use_api_key`). An API-key session's
  `--settings` point Claude at the proxy (`env.ANTHROPIC_BASE_URL`) and give it
  `apiKeyHelper: /usr/bin/cat ~/.config/relay/key-proxy-token`. A launch is refused
  (`session.key_proxy`) while the token file is missing or the proxy is not answering.

## Setting it up

The install runs as root, and it is the one moment the key is typed, so it must not touch anything
your account controls. Agents run as you: they can rewrite your checkout, your build, your shell
configuration and your desktop's terminals, any of which could catch the key or turn root's work
against you. So the install happens at a root text console, from a root-owned clone:

1. Pick a commit on `main` that you trust, on github.com (your phone will do), not in a terminal on
   this PC.
2. Press **Ctrl+Alt+F3** and log in as **root**. (Not `sudo` or `su` from your desktop.)
3. Run:

   ```sh
   git clone https://github.com/antho976/Relay /root/relay-key-proxy
   cd /root/relay-key-proxy && git checkout --detach <that commit>
   less scripts/key-proxy/install.sh
   bash scripts/key-proxy/install.sh --user <your login>
   ```

   The script refuses to run anywhere else: through sudo, outside a text console, or from a clone
   any part of which is not root's. It builds `relay-key-proxy` as root with a root-owned cargo
   home and `--locked`, installs it and the unit to root-owned places, asks for the key at a hidden
   prompt (`systemd-ask-password`) and pipes it straight into `systemd-creds`, writes a new pass
   token to your `~/.config/relay`, as you, and starts the service. It needs a root-owned Rust
   (`pacman -S rust`).
4. Log out (`exit`), go back with **Ctrl+Alt+F1** (or F2), and turn on **Use API key** when
   launching a Claude session.

Use a key made for this, with a monthly spending limit, in the Anthropic console.

**Check your sudo first.** Run `sudo -l`. If it shows `NOPASSWD`, or your sudo keeps one timestamp
for every terminal (`timestamp_type=global`), anything running as you can become root, and no
setup on this PC can keep a secret from it. Fix that first.

**If you used the first version of this feature** (the key in the desktop keyring), remove it with
`secret-tool clear service relay account anthropic-api-key` and revoke that key in the console:
every agent could read it.

### Afterwards

Each of these runs the same way, as root at the console, from the root-owned clone (`git fetch`
and check out the new commit first to update):

- Update the proxy: `bash scripts/key-proxy/install.sh --user <you>`. It also makes a new token.
- New key: `bash scripts/key-proxy/install.sh --user <you> --replace-key`.
- Remove everything: `bash scripts/key-proxy/install.sh --user <you> --uninstall`.

Status: `systemctl status relay-key-proxy`, `journalctl -u relay-key-proxy` (it never logs a
request), `systemd-analyze security relay-key-proxy`.

## What it does not stop

- **Spending.** The pass token is readable by anything running as you, so such a program can make
  API calls through the proxy, billed to the key. It can never learn the key. The spending limit
  bounds the cost; stopping the service, or `--uninstall`, ends it. While the service is down, a
  program could listen on its port and collect the token: still not the key.
- **Root**, and anyone who can become root: they can read the credential.
- **The code you choose.** The service runs what the commit you picked builds. The script prints
  the commit and the binary's sha256 for your records.
- **Anthropic's side**, including Cloudflare, which ends TLS for it.
- **Anywhere you paste the key yourself.** Relay blanks `sk-ant-` keys in terminal output,
  transcript replies and audit rows (`crates/relay-core/src/redact.rs`), as a best effort.

A paired phone cannot start an API-key session (`remote.api_key`); one started at the PC can be
used from the phone like any other, which spends but never shows the key.
