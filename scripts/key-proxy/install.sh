#!/usr/bin/env bash
# Install, update or remove Relay's API-key proxy (docs/KEY-PROXY.md). Read it before running it:
# it runs as root. From your own account, in a terminal outside Relay:
#
#   cargo build --release -p relay-cli
#   sudo scripts/key-proxy/install.sh               # install, or update the binary; asks for the key once
#   sudo scripts/key-proxy/install.sh --replace-key # put in a new key
#   sudo scripts/key-proxy/install.sh --uninstall   # remove the service, the key and the pass token
#
# The key is typed at a hidden prompt and goes straight into a systemd credential encrypted with
# the host's root-only key. It is never written in the clear, never in an argument or variable a
# process of yours can see, and never in this repository.
set -euo pipefail

die() { echo "key-proxy: $*" >&2; exit 1; }
[[ $EUID -eq 0 ]] || die "run it with sudo"
user=${SUDO_USER:-}
[[ -n $user && $user != root ]] || die "run it with sudo from your own account, not as root"
home=$(getent passwd "$user" | cut -d: -f6)
[[ -d $home ]] || die "no home directory for $user"

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../.." && pwd)
libexec=/usr/local/libexec/relay
unit=/etc/systemd/system/relay-key-proxy.service
creds=/etc/credstore.encrypted
key_cred=$creds/relay-anthropic-key
token_cred=$creds/relay-proxy-token
token_dir=$home/.config/relay

# Everything written in the person's home is written as them, so a link planted there cannot
# turn a root write into a write somewhere else.
as_user() { runuser -u "$user" -- "$@"; }

if [[ ${1:-} == --uninstall ]]; then
    systemctl disable --now relay-key-proxy.service 2>/dev/null || true
    rm -f "$unit" "$key_cred" "$token_cred" "$libexec/relay-key-proxy" "$libexec/KEY-PROXY.md"
    rmdir "$libexec" 2>/dev/null || true
    systemctl daemon-reload
    as_user rm -f "$token_dir/key-proxy-token"
    echo "key-proxy: removed. Revoke the key in the Anthropic console if it may have been exposed."
    exit 0
fi
[[ -z ${1:-} || $1 == --replace-key ]] || die "unknown option $1"

binary=$repo/target/release/relay
[[ -f $binary ]] || die "no $binary: build it first with cargo build --release -p relay-cli"
echo "key-proxy: installing $binary"
echo "           built from: $(as_user git -C "$repo" log -1 --format='%h %s' 2>/dev/null || echo unknown)"
if [[ -n $(as_user git -C "$repo" status --porcelain --untracked-files=no 2>/dev/null) ]]; then
    echo "           warning: the checkout has uncommitted changes; the proxy is only as trustworthy as the code it was built from"
fi

# Root-owned copies: the service must not run anything the person (or an agent) can rewrite.
install -d -o root -g root -m 0755 "$libexec"
install -o root -g root -m 0755 "$binary" "$libexec/relay-key-proxy"
install -o root -g root -m 0644 "$repo/docs/KEY-PROXY.md" "$libexec/KEY-PROXY.md"
install -o root -g root -m 0644 "$here/relay-key-proxy.service" "$unit"
echo "           sha256: $(sha256sum "$libexec/relay-key-proxy" | cut -d' ' -f1)"
install -d -o root -g root -m 0700 "$creds"

if [[ ! -s $key_cred || ${1:-} == --replace-key ]]; then
    key=$(systemd-ask-password -n "Anthropic API key (input hidden):")
    [[ $key =~ ^sk-ant-[A-Za-z0-9_-]{14,}$ ]] || { unset key; die "that is not an Anthropic API key (sk-ant-…)"; }
    # printf is a shell builtin: the key is in no process's arguments.
    printf '%s' "$key" | systemd-creds encrypt --name=relay-anthropic-key - "$key_cred"
    unset key
    chmod 0600 "$key_cred"
fi

# The local pass token: Claude shows it to the proxy. It is not the key, and a new one is made
# on every run, so running this again also rotates it.
token=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
printf '%s' "$token" | systemd-creds encrypt --name=relay-proxy-token - "$token_cred"
chmod 0600 "$token_cred"
printf '%s' "$token" | as_user sh -c 'umask 077 && mkdir -p "$1" && cat > "$1/key-proxy-token.new" && mv -f "$1/key-proxy-token.new" "$1/key-proxy-token"' sh "$token_dir"
unset token

systemctl daemon-reload
systemctl enable relay-key-proxy.service >/dev/null
systemctl restart relay-key-proxy.service
sleep 1
systemctl is-active --quiet relay-key-proxy.service || die "the service did not start: journalctl -u relay-key-proxy"
echo "key-proxy: running on 127.0.0.1:7445. Turn on 'Use API key' when launching a Claude session."
