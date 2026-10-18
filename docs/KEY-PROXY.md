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
- **Make your password stop being root's.** You are in `wheel`: sudo and polkit let your own
  password act as root's, and you type it into things agents can replace (the lock screen, the
  polkit prompt, `sudo` in a terminal). An agent that learns it can become root, and root can
  decrypt the key. Once, as root at the console, make elevation ask for **root's** password
  instead (this changes elevation only, nothing else):

  ```sh
  echo 'Defaults rootpw' > /etc/sudoers.d/10-rootpw && chmod 0440 /etc/sudoers.d/10-rootpw && visudo -c
  printf '%s\n' 'polkit.addAdminRule(function(action, subject) { return ["unix-user:root"]; });' \
      > /etc/polkit-1/rules.d/40-admin-root.rules
  ```

  Then never type root's password anywhere but a root console: that means no more `sudo` or admin
  prompts from your desktop; do admin work at a root console. If your sudo also allows
  `NOPASSWD` or shares one timestamp across terminals (list it with `sudo -l`), anything running as
  you can already become root: fix that too.

**The install**

1. Reboot into a text console with nothing of yours running: in the boot menu, edit the entry and
   add `systemd.unit=multi-user.target systemd.mask=user@1000.service` to the kernel line (1000 is
   your user id). The desktop does not start, and neither do your lingering user services, so
   nothing of yours can watch the screen, listen, or answer for the console.
2. Log in as **root** on the console. Not as yourself followed by `su` or `sudo`: the script
   checks the login itself.
3. Clone, read and run:

   ```sh
   git clone --no-tags https://github.com/antho976/Relay /root/relay-key-proxy
   cd /root/relay-key-proxy && git checkout --detach <full commit hash>
   less scripts/key-proxy/install.sh
   bash scripts/key-proxy/install.sh --user <you> --commit <full commit hash>
   ```

   The script refuses unless: it runs as root from a root login (`/proc/self/loginuid` is 0) at a
   text console that is root's; no process of yours runs, your user manager is stopped and your
   slice is empty (it says how to stop them); the clone is root's all the way up, writable by no
   one else and holds no symbolic link; it is exactly at the commit you named, clean, and on
   `origin/main`; nothing patches or replaces a dependency, the proxy has no build script, and every
   crate it builds from comes from crates.io at the version `Cargo.lock` pins. It keeps your user
   manager masked until it ends, so nothing of yours starts meanwhile.

   Before building, it shows you what is about to run as root: on an update, the full diff under
   the proxy since the commit installed last; always, every crate it builds from. Read it, and
   type `yes` only if it is what you expect: agents can push to `main`, so this, and the commit
   you picked, are what stands between their code and root.

   It then builds `relay-key-proxy` as root, from outside the clone (so no `.cargo/config` in it
   applies), with a root-owned cargo home and `--locked`; asks for the key with no time limit and
   nothing shown as you type (`systemd-ask-password`), and checks its shape before touching
   anything; installs the binary and both units to root-owned places; pipes the key straight into
   `systemd-creds`; writes a new pass token to your `~/.config/relay`, as you but without a login,
   so nothing of yours starts; starts the socket and the service; and makes one free call
   (`GET /v1/models`) through the proxy to tell you whether Anthropic accepts the key.
4. `exit`, reboot normally, and turn on **Use API key** when launching a Claude session.

### Afterwards

Each of these runs the same way: booted as in step 1, as root at the console, from the root-owned
clone (`git fetch` and check out the new full commit first):

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
- **The code you choose.** The service runs what the commit you picked builds, with its crates.
  The script shows you that before it builds, and prints the binary's sha256 for your records.
- **Anthropic's side**, including Cloudflare, which ends TLS for it.
- **Anywhere you paste the key yourself.** Relay blanks `sk-ant-` keys in terminal output,
  transcript replies and audit rows (`crates/relay-core/src/redact.rs`), as a best effort.

A paired phone cannot start an API-key session (`remote.api_key`); one started at the PC can be
used from the phone like any other. That refusal limits spending, not more: anything that can
write a Claude settings file or type into a session (the phone among them) can point a session at
the proxy, as anything running as you can. Each of these spends; none shows the key.
