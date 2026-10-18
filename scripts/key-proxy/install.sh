#!/bin/bash
# Install, update or remove Relay's API-key proxy (docs/KEY-PROXY.md).
#
# It runs as root, and only from a root-owned clone at a root text console. Your own account is
# where agents run: they can rewrite its files, its shell and its terminals, so nothing from it —
# not this checkout, not its build, not a terminal on its desktop — may reach root or the key.
#
#   Ctrl+Alt+F3, log in as root, then:
#   git clone https://github.com/antho976/Relay /root/relay-key-proxy
#   cd /root/relay-key-proxy && git checkout --detach <a commit on main you trust>
#   less scripts/key-proxy/install.sh
#   bash scripts/key-proxy/install.sh --user <your login>                  # install or update
#   bash scripts/key-proxy/install.sh --user <your login> --replace-key    # put in a new key
#   bash scripts/key-proxy/install.sh --user <your login> --uninstall      # remove everything
#
# Everything is in main(), and the script exits before reading further, so bash has parsed it
# whole before running any of it.

main() {
    set -euo pipefail
    export PATH=/usr/bin:/bin LC_ALL=C
    umask 022

    local user="" mode=install
    while (($#)); do
        case $1 in
            --user) user=${2:-}; shift 2 ;;
            --replace-key) mode=replace-key; shift ;;
            --uninstall) mode=uninstall; shift ;;
            *) die "unknown option $1" ;;
        esac
    done

    [[ $EUID -eq 0 ]] || die "run it as root"
    [[ -z ${SUDO_USER:-} ]] || die "log in as root on a text console (Ctrl+Alt+F3) instead of using sudo from your desktop"
    [[ $(tty) =~ ^/dev/tty[0-9]+$ ]] || die "run it at a text console (Ctrl+Alt+F3), not in a terminal of your desktop session"
    [[ -n $user ]] || die "name your login: --user <name>"
    local entry uid home
    entry=$(getent passwd "$user") || die "no user $user"
    uid=$(cut -d: -f3 <<<"$entry")
    home=$(cut -d: -f6 <<<"$entry")
    ((uid >= 1000)) || die "$user is a system account"
    [[ -d $home ]] || die "no home directory for $user"

    local here repo
    here=$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)
    repo=$(cd -P "$here/../.." && pwd)
    trusted_tree "$repo"
    local commit
    commit=$(git -C "$repo" rev-parse HEAD)
    [[ -z $(git -C "$repo" status --porcelain) ]] || die "the clone has local changes; use a clean checkout of a commit you trust"

    local libexec=/usr/local/libexec/relay unit=/etc/systemd/system/relay-key-proxy.service
    local creds=/etc/credstore.encrypted cache=/var/cache/relay-key-proxy
    local key_cred=$creds/relay-anthropic-key token_cred=$creds/relay-proxy-token
    local token_dir=$home/.config/relay

    if [[ $mode == uninstall ]]; then
        systemctl disable --now relay-key-proxy.service 2>/dev/null || true
        rm -f -- "$unit" "$key_cred" "$token_cred" "$libexec/relay-key-proxy"
        rmdir -- "$libexec" 2>/dev/null || true
        rm -rf -- "$cache"
        systemctl daemon-reload
        as_user "$user" rm -f -- "$token_dir/key-proxy-token"
        echo "key-proxy: removed. Revoke the key in the Anthropic console if it may have been exposed."
        return
    fi

    local unit_src=$repo/scripts/key-proxy/relay-key-proxy.service
    [[ -f $unit_src && ! -L $unit_src ]] || die "$unit_src is not a regular file"
    [[ -x /usr/bin/cargo && $(stat -c %u /usr/bin/cargo) == 0 ]] || die "no root-owned /usr/bin/cargo: install Rust from your distribution (pacman -S rust)"

    echo "key-proxy: building $commit as root"
    install -d -o root -g root -m 0700 "$cache"
    # A clean environment: no RUSTC_WRAPPER, RUSTFLAGS or CARGO_* from anywhere, a root-owned cargo
    # home, and dependencies exactly as Cargo.lock pins them.
    (cd "$repo" && env -i PATH=/usr/bin:/bin HOME=/root LC_ALL=C CARGO_HOME="$cache/cargo" CARGO_TARGET_DIR="$cache/target" \
        /usr/bin/cargo build --locked --release -p relay-key-proxy)
    local binary=$cache/target/release/relay-key-proxy
    [[ -f $binary && ! -L $binary ]] || die "the build left no $binary"

    systemctl stop relay-key-proxy.service 2>/dev/null || true
    install -d -o root -g root -m 0755 "$libexec"
    install -o root -g root -m 0755 "$binary" "$libexec/relay-key-proxy"
    install -o root -g root -m 0644 "$unit_src" "$unit"
    install -d -o root -g root -m 0700 "$creds"

    if [[ ! -s $key_cred || $mode == replace-key ]]; then
        local key
        key=$(systemd-ask-password -n "Anthropic API key (input hidden):")
        if [[ ! $key =~ ^sk-ant-api[0-9]{2}-[A-Za-z0-9_-]{20,}$ ]]; then
            unset key
            die "that is not an ordinary Anthropic API key (sk-ant-api…); admin keys are refused"
        fi
        # printf is a shell builtin: the key is in no process's arguments.
        printf '%s' "$key" | systemd-creds encrypt --name=relay-anthropic-key - "$key_cred"
        unset key
        chmod 0600 "$key_cred"
    fi

    # The local pass token Claude shows the proxy. It is not the key; every run makes a new one.
    local token
    token=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
    printf '%s' "$token" | systemd-creds encrypt --name=relay-proxy-token - "$token_cred"
    chmod 0600 "$token_cred"
    # Written as the person, so a link planted in their home cannot turn it into a root write.
    printf '%s' "$token" | as_user "$user" /bin/sh -c \
        'umask 077 && mkdir -p "$1" && cat > "$1/key-proxy-token.new" && mv -f "$1/key-proxy-token.new" "$1/key-proxy-token"' sh "$token_dir"
    unset token

    systemctl daemon-reload
    systemctl enable relay-key-proxy.service >/dev/null
    systemctl restart relay-key-proxy.service
    local tries
    for tries in 1 2 3 4 5 6 7 8 9 10; do
        (exec 3<>/dev/tcp/127.0.0.1/7445) 2>/dev/null && break
        sleep 0.5
    done
    systemctl is-active --quiet relay-key-proxy.service || die "the service did not start: journalctl -u relay-key-proxy"
    echo "key-proxy: running on 127.0.0.1:7445"
    echo "           built from $commit"
    echo "           binary sha256 $(sha256sum "$libexec/relay-key-proxy" | cut -d' ' -f1)"
    echo "           Turn on 'Use API key' when launching a Claude session. Log out of this console."
}

die() {
    echo "key-proxy: $*" >&2
    exit 1
}

as_user() {
    local user=$1
    shift
    runuser -u "$user" -- "$@"
}

# The clone and every folder above it must be root's and writable by no one else, so no other
# account could have changed what is built or installed.
trusted_tree() {
    local dir=$1 owner mode
    while :; do
        read -r owner mode < <(stat -c '%u %a' -- "$dir")
        [[ $owner == 0 ]] || die "$dir is not owned by root: clone into /root as root"
        (((8#$mode & 8#022) == 0)) || [[ $dir == / ]] || die "$dir can be written by others"
        [[ $dir == / ]] && break
        dir=$(dirname -- "$dir")
    done
    [[ -z $(find "$1" -xdev \( ! -uid 0 -o \( ! -type l -perm /022 \) \) -print -quit) ]] || die "something in $1 is not root's or is writable by others"
}

main "$@"
exit
