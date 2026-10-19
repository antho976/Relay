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
  `127.0.0.1:7445` (or `localhost:7445`), and goes to one of the endpoints Claude Code uses (`POST /v1/messages`,
  `POST /v1/messages/count_tokens`, `GET /v1/models`) with a body of a stated length. It sends it to
  `api.anthropic.com` only, over TLS checked against roots built into the binary (a certificate
  added to the system store cannot intercept it), with a fixed list of headers and the token
  swapped for the key. Everything else is answered by the proxy and goes nowhere. Admin keys
  (`sk-ant-admin…`) are refused, so the token is never worth more than API calls, including the
  server-side tools those calls can use on the key's workspace (files, skills, containers): make
  the key in a workspace of its own, with nothing else in it.
- **Relay** stores only the session's on/off choice (`use_api_key`). An API-key session's
  `--settings` point Claude at the proxy (`env.ANTHROPIC_BASE_URL`) and give it
  `apiKeyHelper: /usr/bin/cat ~/.config/relay/key-proxy-token`. A launch is refused
  (`session.key_proxy`) while the token file is missing or nothing answers on the port.
- **The port** is held by PID 1 (`relay-key-proxy.socket`) and handed to the service, so no other
  program can take it, and collect the token, while the service is stopped or replaced.
- **The service** may connect only to loopback and Anthropic's published API ranges
  (`160.79.104.0/23`, `2607:6bc0::/48`), and marks itself undumpable. That narrows where a
  tampered build could send anything, but it is no defence on its own (it could still answer its
  caller, reach a local listener or use DNS): reading the code before it is installed is. Should
  Anthropic move its API, the service fails closed; widen `IPAddressAllow=` in the unit.
- **The program** is `crates/relay-key-proxy`, its own workspace with its own `Cargo.lock`: about
  700 lines (about 40% of them tests), 22 crates from crates.io, no proc macros, and no build script of
  its own (four of its crates have one: `httparse`, `libc`, `ring`, `rustls`). Root builds that
  folder alone, copied out of a clean clone.

## Setting it up

The install runs as root, and it is the one moment the key is typed, so nothing your account
controls may take part. Agents run as you: they can rewrite your checkout, your build, your shell,
your desktop (on Wayland the desktop itself handles Ctrl+Alt+F3, so a tampered one can draw a fake
console) and your terminals. So the install happens with no program of yours running at all, at a
root login on a real text console, from a root-owned clone of a commit you name in full.

**Before you start**

- **Use a device agents have never reached** to make the key and to read the commit hash. Agents
  can read this PC's adb key, and your phone trusts it: over adb they can install apps (Relay's
  own app among them), turn on an accessibility service that reads the screen, or grant an app
  the right to draw over others, and none of that is undone by revoking adb later. Best is another
  device that has never been plugged into this PC. If it has to be this phone: in Developer options
  turn off USB debugging and Wireless debugging and tap **Revoke USB debugging authorisations**;
  then check Settings > Accessibility (no service you did not turn on yourself), Special app access
  > Display over other apps and Device admin apps, and the installed apps, before you go on.
- **Make the key on that device**, in the Anthropic console, with a monthly spending limit, and
  type it from its screen. A key made, copied or pasted on this PC can be read there (browser,
  clipboard history). An ordinary key only: admin keys are refused. Sign out of the console
  afterwards.
- **Remove Bluetooth pairings you do not use** (any program of yours can pair a device, and a
  paired phone app can pose as a keyboard). The console boot below also keeps Bluetooth off.
- **Pick the commit on your phone too**: a commit on `main` you trust, and its full 40-character
  hash (on GitHub, open the commit and copy the hash from its page).
- **Restart only with the power button.** Every step below that needs a real console starts from
  powered off: hold the power button until the PC is off, then turn it on. A restart begun from
  your desktop, or from a terminal in it, runs code agents can replace, which could show a fake
  boot menu or console.
- **Root needs its own password**, set from a root shell, never typed in your desktop. If root has
  none ("account locked" at the console), set it at boot: power off with the button, turn on, in
  the boot menu edit the entry and add `init=/bin/bash` to the kernel line, boot, then
  `mount -o remount,rw /`, `passwd root`, `sync`, and hold the power button again.
- **Take yourself out of `wheel`.** As a member, your own password acts as root's (sudo, and
  polkit, whose vendor rules also let `wheel` change network connections, flash firmware and
  install system-wide apps without asking), and you type it into things agents can replace: the
  lock screen, a polkit prompt, `sudo` in a terminal. An agent that learns it can become root, and
  root can decrypt the key. As root at the console: `gpasswd -d <you> wheel`. Then check that no
  other rule lets you in: `sudo -l -U <you>` must say you may not run sudo, and nothing in
  `/etc/sudoers.d` may name you. From then on, do admin work at a root console, and never type
  root's password anywhere else. Out of `wheel`, your desktop's admin prompts (network, firmware,
  apps) would ask for root's password instead. Make them name an account that cannot log in, so
  no prompt can ever take root's password:

  ```sh
  printf '%s\n' 'polkit.addAdminRule(function(action, subject) { return ["unix-user:nobody"]; });' \
      > /etc/polkit-1/rules.d/40-admin-nobody.rules
  ```

  `nobody` must be locked, or an empty password could satisfy such a prompt: at the root console,
  `passwd -S nobody` must show `L`. Check it from your desktop afterwards: an admin prompt should
  name `nobody`, and no password can satisfy it. Never type root's password into one.
- **Close the other ways to root.** Nothing outside your home may be yours where root could run
  it, on any filesystem (this PC has several):

  ```sh
  find / \( -path /proc -o -path /sys -o -path /dev -o -path /run -o -path /home -o -path /tmp \) -prune \
      -o -user <you> -print 2>/dev/null
  ```

  Give anything it lists in a system folder to root (on this PC, `/usr/local/bin/lific` is yours:
  `chown root:root /usr/local/bin/lific`; caches under `/var/tmp` are expected and harmless).
  Also: `id <you>` should list no group that a rule in `/etc/polkit-1/rules.d`,
  `/run/polkit-1/rules.d`, `/usr/local/share/polkit-1/rules.d` or `/usr/share/polkit-1/rules.d`
  grants actions to; `systemctl is-enabled
  sshd` should say disabled (agents can write your `~/.ssh/authorized_keys`); and if you use
  Tailscale, you must not be its operator (`tailscale set --operator=`), with Tailscale SSH off.
- **Make sure the console boot has network.** The clone and the build need it. A Wi-Fi password
  kept in your desktop keyring is not there without your desktop: use a cable, or a connection
  saved for all users.
- **The commit must be on `main`.** The installer refuses a commit that is not.

**The first install**

1. Power off with the button, turn on, and in the boot menu edit the entry and add
   `systemd.unit=multi-user.target systemd.mask=user@1000.service systemd.mask=bluetooth.service
   systemd.mask=sshd.service systemd.mask=tailscaled.service systemd.ssh_auto=no` to the kernel line
   (1000 is your user id). The desktop does not start, nor do your lingering user services,
   Bluetooth, SSH (its socket-activated local listener included) or Tailscale, so nothing of yours can run to watch the screen, listen, type, or answer for the
   console.
2. Log in as **root** on the console. Not as yourself followed by `su` or `sudo`: the script
   checks the login itself. Then `pgrep -u <you>` must print nothing; if it prints anything, stop
   it (`loginctl terminate-user <you>; systemctl stop user-1000.slice`) before you read on.
3. Clone the commit, check that everything that will run as root is plain printable ASCII with no
   tab, trailing space or line over 200 characters (the first command must print nothing, and the
   installer refuses anything else anyway), and read it all with `less -U -N`: it shows a
   backspace, carriage return or escape as `^H`, `^M`, `^[` instead of obeying it, and numbers each
   line, so no line can hide and a wrapped line cannot pass for two:

   ```sh
   git clone --no-tags https://github.com/antho976/Relay /root/relay-key-proxy
   cd /root/relay-key-proxy && git checkout --detach <full commit hash>
   LC_ALL=C grep -rn -e '[^ -~]' -e ' $' -e '.\{201\}' crates/relay-key-proxy scripts/key-proxy
   LC_ALL=C LESSSECURE=1 less -U -N crates/relay-key-proxy/Cargo.toml crates/relay-key-proxy/Cargo.lock crates/relay-key-proxy/src/main.rs scripts/key-proxy/*
   ```

   Those files are all the install builds, installs or runs, besides the crates `Cargo.lock` pins,
   by name, version and checksum; any other file in those folders stops it. Never run a script
   from a clone before you have read it like this, or approved it with `key-proxy-review`.
4. Run it, and type `yes` when it asks whether you read them:

   ```sh
   bash scripts/key-proxy/install.sh --user <you> --commit <full commit hash>
   ```

   It refuses unless: it runs as root from a root login (`/proc/self/loginuid` is 0) at a text
   console that is root's; nothing of yours runs (no process by effective or real user id, user
   manager stopped, user slice empty; it says how to stop them), and it keeps your user manager
   masked until it ends; the clone is root's all the way up, writable by no one else, holds no
   symbolic link, is exactly at the commit you named, clean, and on `origin/main`.

   It copies exactly the listed files out of the clone, each checked to be printable ASCII, and
   builds that copy alone, from outside it (so no `.cargo/config` applies), with a root-owned cargo
   home and `--locked`. It refuses a build script, a source file outside the folder (asked of
   `cargo metadata`, which runs nothing, and checked again in what the compiler read), a proc
   macro, and any dependency not from crates.io. It then asks for the key with no time limit and
   nothing shown as you type, checks its shape before touching anything, installs the binary, both
   units, `key-proxy-review` and `key-proxy-uninstall` to root-owned places, pipes the key straight
   into `systemd-creds`, writes a
   new pass token to your `~/.config/relay` (as you, without a login, so nothing of yours starts),
   starts the socket and the service, keeps a copy of what it installed for the next update to be
   compared with, and makes one free call (`GET /v1/models`) through the proxy to tell you whether
   Anthropic accepts the key.
5. `reboot` from that console, and turn on **Use API key** when launching a Claude session.

### Updating

Start as for the first install (steps 1 and 2), then fetch the new commit and review it with the
`key-proxy-review` the last install left. It is root-owned and was itself reviewed, so the new
commit cannot change what shows it to you; it compares with the copy installed last, not with
GitHub, refuses any file off the list or not printable ASCII, and shows every difference, the
installer's own changes included, in `less -U -N`:

```sh
cd /root/relay-key-proxy && git fetch --no-tags origin && git checkout --detach <full commit hash>
/usr/local/libexec/relay/key-proxy-review /root/relay-key-proxy
bash scripts/key-proxy/install.sh --user <you> --commit <full commit hash>
```

The installer builds only the exact tree you approved. A new token is made on every run, so
restart any API-key session that was running. An update that adds a file to the proxy cannot pass
the installed reviewer (its list refuses the new file): remove the proxy and install it afresh,
reading everything again. For a
new key, add `--replace-key` (after the review, if the commit changed).

### Removing it

Booted and logged in as for an install, run the uninstaller the install left. It is root-owned and
reviewed, so removing the proxy never runs a script from a newer clone:

```sh
/usr/local/libexec/relay/key-proxy-uninstall --user <you>
```

Status: `systemctl status relay-key-proxy.socket relay-key-proxy`, `journalctl -u relay-key-proxy`
(it never logs a request), `systemd-analyze security relay-key-proxy`.

**If you used the first version of this feature** (the key in the desktop keyring), remove it with
`secret-tool clear service relay account anthropic-api-key` and revoke that key in the console:
every agent could read it.

## What it does not stop

- **Spending.** The pass token is readable by anything running as you, so such a program can make
  API calls through the proxy, billed to the key. It can never learn the key. The spending limit
  bounds the cost; stopping the socket and the service, or `key-proxy-uninstall`, ends it.
- **A certificate a public authority issues wrongly** for `api.anthropic.com` would let someone on
  the network path read the traffic: the proxy trusts the same public roots browsers do.
- **Your Anthropic console login.** If this PC's browser is signed in to console.anthropic.com, a
  program running as you can use that session to make keys of its own. Sign in there from your
  phone only.
- **Root**, and anyone who can become root: they can read the credential.
- **The code you choose.** The service runs what the commit you picked builds, with its crates. You
  read it before it builds (first install) or approve its diff (an update); the script prints the
  binary's sha256 for your records. The crates' own build scripts run as root during the build.
- **Anthropic's side**, including Cloudflare, which ends TLS for it.
- **Anywhere you paste the key yourself.** Relay blanks `sk-ant-` keys in terminal output,
  transcript replies and audit rows (`crates/relay-core/src/redact.rs`), as a best effort.

A paired phone cannot start an API-key session (`remote.api_key`); one started at the PC can be
used from the phone like any other. That refusal limits spending, not more: anything that can
write a Claude settings file or type into a session (the phone among them) can point a session at
the proxy, as anything running as you can. Each of these spends; none shows the key.
