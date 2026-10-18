# The API-key proxy

A Claude session launched with **Use API key** bills an Anthropic API key instead of the signed-in
subscription. The key is held by `relay key-proxy`, a small service that runs as its own throwaway
system user. Nothing that runs as you can read it: not an agent, a repository's hooks, an MCP
server, or a command injected over the phone link.

## How it works

```
Claude (you) ──x-api-key: <pass token>──▶ 127.0.0.1:7445  relay key-proxy (its own user)
                                                │  swaps the token for the key
                                                └──TLS──▶ api.anthropic.com
```

- **The key** is stored once in `/etc/credstore.encrypted/relay-anthropic-key`, encrypted by
  `systemd-creds` with the host's root-only key (and the TPM, where there is one). systemd decrypts
  it for `relay-key-proxy.service` alone, which runs with `DynamicUser=yes`: a user id no other
  process has. Your user cannot read that service's files, its `/proc` entries or its memory.
- **The pass token** in `~/.config/relay/key-proxy-token` is what Claude sends. The proxy forwards
  a request only when it carries exactly that token, only to `api.anthropic.com`, and only over
  TLS checked against roots built into the binary (a certificate added to the system store cannot
  intercept it). It replaces the token with the key, and drops the caller's `authorization`.
- **Relay** stores only the session's on/off choice (`use_api_key`). An API-key session's
  `--settings` point Claude at the proxy (`env.ANTHROPIC_BASE_URL`) and give it
  `apiKeyHelper: /usr/bin/cat ~/.config/relay/key-proxy-token`. The launch's own settings outrank
  the checkout's, and a checkout that redirected Claude anyway would capture only the token, which
  works nowhere but on this machine.

## Setting it up

From your own account, in a terminal **outside Relay** (Konsole, not a Relay pane):

```fish
cargo build --release -p relay-cli
sudo scripts/key-proxy/install.sh
```

Read `scripts/key-proxy/install.sh` first: it runs as root. It copies the binary and the unit to
root-owned places (`/usr/local/libexec/relay/`, `/etc/systemd/system/`), asks for the key at a
hidden prompt (`systemd-ask-password`) and pipes it straight into `systemd-creds`, makes a new pass
token, and starts the service. Then turn on **Use API key** in the launch form.

- New key: `sudo scripts/key-proxy/install.sh --replace-key`.
- New binary (after an update): run the script again. It also makes a new pass token.
- Remove everything: `sudo scripts/key-proxy/install.sh --uninstall`.
- Status: `systemctl status relay-key-proxy`, `journalctl -u relay-key-proxy`. The log never holds
  a request's headers.

Use a key made for this, with a monthly spending limit, in the Anthropic console.

## What it does not stop

- **Spending.** The pass token is readable by anything running as you, so such a program can make
  API calls through the proxy, billed to the key. It can never learn the key. The spending limit
  bounds the cost; `--uninstall`, or stopping the service, ends it.
- **Root.** Anything running as root can read the credential.
- **The code it was built from.** The service runs the binary you installed. Agents write this
  repository, so install from a commit you trust; the script prints the commit, warns about
  uncommitted changes and prints the binary's sha256.
- **Anywhere you paste the key yourself.** Relay blanks `sk-ant-` keys in terminal output,
  transcript replies and audit rows (`crates/relay-core/src/redact.rs`), as a best effort.

A paired phone cannot start an API-key session (`remote.api_key`); one started at the PC can be
used from the phone like any other.
