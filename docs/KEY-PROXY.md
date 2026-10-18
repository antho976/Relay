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
  (`session.key_proxy`) while the token file is missing or nothing answers on the port.
- **The port** is held by PID 1 (`relay-key-proxy.socket`) and handed to the service, so no other
  program can take it, and collect the token, while the service is stopped or replaced.

## Setting it up

The install runs as root, and it is the one moment the key is typed, so nothing your account
controls may take part. Agents run as you: they can rewrite your checkout, your build, your shell,
your desktop (on Wayland the desktop itself handles Ctrl+Alt+F3, so a tampered one can draw a fake
console) and your terminals. So the install happens with no program of yours running at all, at a
root login on a real text console, from a root-owned clone of a commit you name in full.

**Before you start**

- **Make the key on your phone**, in the Anthropic console, with a monthly spending limit, and
  type it from the phone's screen. A key made, copied or pasted on this PC can be read there
  (browser, clipboard history). An ordinary key only: admin keys are refused.
- **Pick the commit on your phone too**: a commit on `main` you trust, and its full 40-character
  hash (on GitHub, open the commit and copy the hash from its page).
- **Root needs its own password**, set from a root shell, never typed in your desktop. If root has
  none ("account locked" at the console), set it at boot: in the boot menu, edit the entry, add
  `init=/bin/bash` to the kernel line, boot, then `mount -o remount,rw /`, `passwd root`, `sync`,
  `reboot -f`.
- **Check your sudo and polkit.** From your desktop, list what sudo lets you do (`sudo -l`): if it
  shows `NOPASSWD`, or sudo shares one timestamp across terminals, anything running as you can
  become root, and no setup on this PC can keep a secret from it. You are in `wheel`, which polkit
  treats as an administrator who answers prompts with your own password: never approve a password
  prompt you did not start, and keep your account's password non-empty.

**The install**

1. Reboot into a text console, without the desktop: in the boot menu, edit the entry and add
   `systemd.unit=multi-user.target` to the kernel line. (Your lingering user services still start;
   the script asks you to stop them.)
2. Log in as **root** on the console. Not as yourself followed by `su` or `sudo`: the script
   checks the login itself.
3. Stop everything of yours, then clone, read and run:

   ```sh
   systemctl stop user@1000.service; loginctl terminate-user <you>; pkill -KILL -u <you>
   git clone --no-tags https://github.com/antho976/Relay /root/relay-key-proxy
   cd /root/relay-key-proxy && git checkout --detach <full commit hash>
   less scripts/key-proxy/install.sh
   bash scripts/key-proxy/install.sh --user <you> --commit <full commit hash>
   ```

   The script refuses unless: it runs as root from a root login (`/proc/self/loginuid` is 0) at a
   text console that is root's; no process of yours is running; the clone is root's all the way up,
   writable by no one else and holds no symbolic link; it is exactly at the commit you named, clean,
   and that commit is on `origin/main`; and the build would read no `[patch]` section or build
   script. It then builds `relay-key-proxy` as root, from outside the clone (so no `.cargo/config`
   in it applies), with a root-owned cargo home and `--locked`; asks for the key at a hidden prompt
   (`systemd-ask-password`) and checks its shape before touching anything; installs the binary and
   both units to root-owned places; pipes the key straight into `systemd-creds`; writes a new pass
   token to your `~/.config/relay`, as you; starts the socket and the service; and makes one free
   call (`GET /v1/models`) through the proxy to tell you whether Anthropic accepts the key. On an
   update it shows what changed under the proxy since the commit installed last.
4. `exit`, reboot normally, and turn on **Use API key** when launching a Claude session.

### Afterwards

Each of these runs the same way: booted without the desktop, as root at the console, nothing of
yours running, from the root-owned clone (`git fetch` and check out the new full commit first):

- Update the proxy: `bash scripts/key-proxy/install.sh --user <you> --commit <hash>`. It also
  makes a new token.
- New key: add `--replace-key`.
- Remove everything: add `--uninstall`.

Status: `systemctl status relay-key-proxy.socket relay-key-proxy`, `journalctl -u relay-key-proxy`
(it never logs a request), `systemd-analyze security relay-key-proxy`.

**If you used the first version of this feature** (the key in the desktop keyring), remove it with
`secret-tool clear service relay account anthropic-api-key` and revoke that key in the console:
every agent could read it.

## What it does not stop

- **Spending.** The pass token is readable by anything running as you, so such a program can make
  API calls through the proxy, billed to the key. It can never learn the key. The spending limit
  bounds the cost; stopping the socket and the service, or `--uninstall`, ends it.
- **Your Anthropic console login.** If this PC's browser is signed in to console.anthropic.com, a
  program running as you can use that session to make keys of its own. Sign in there from your
  phone only.
- **Root**, and anyone who can become root: they can read the credential.
- **The code you choose.** The service runs what the commit you picked builds. The script prints
  the commit and the binary's sha256 for your records.
- **Anthropic's side**, including Cloudflare, which ends TLS for it.
- **Anywhere you paste the key yourself.** Relay blanks `sk-ant-` keys in terminal output,
  transcript replies and audit rows (`crates/relay-core/src/redact.rs`), as a best effort.

A paired phone cannot start an API-key session (`remote.api_key`); one started at the PC can be
used from the phone like any other, which spends but never shows the key.
