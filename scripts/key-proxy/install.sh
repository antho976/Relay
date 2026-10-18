#!/bin/bash
# Install, update or remove Relay's API-key proxy (docs/KEY-PROXY.md, "Setting it up").
#
# It runs as root, from a root login at a text console, from a root-owned clone of a commit you
# name in full, and only while nothing at all runs as you. Your own account is where agents run:
# they can rewrite its files, its shell, its desktop and its terminals, so nothing from it may
# reach root, the build, or the moment the key is typed.
#
#   (booted without the desktop, logged in as root on a text console:)
#   git clone --no-tags https://github.com/antho976/Relay /root/relay-key-proxy
#   cd /root/relay-key-proxy && git checkout --detach <full 40-character commit>
#   less scripts/key-proxy/install.sh
#   bash scripts/key-proxy/install.sh --user <login> --commit <same commit>                  # install or update
#   bash scripts/key-proxy/install.sh --user <login> --commit <same commit> --replace-key    # new key
#   bash scripts/key-proxy/install.sh --user <login> --commit <same commit> --uninstall      # remove everything
#
# Everything is in main(), and the script exits before reading further, so bash has parsed it
# whole before running any of it.

main() {
    set -euo pipefail
    export PATH=/usr/bin:/bin LC_ALL=C
    umask 022

    local user="" commit="" mode=install
    while (($#)); do
        case $1 in
            --user) user=${2:-}; shift 2 ;;
            --commit) commit=${2:-}; shift 2 ;;
            --replace-key) mode=replace-key; shift ;;
            --uninstall) mode=uninstall; shift ;;
            *) die "unknown option $1" ;;
        esac
    done

    # Root, logged in as root: su and sudo change the user id but never the login's, and a
    # console reached through them belongs to the person, where their programs can read it.
    [[ $EUID -eq 0 ]] || die "run it as root"
    [[ $(</proc/self/loginuid) == 0 ]] || die "log in as root itself on a text console; su and sudo from your own login do not count"
    local console
    console=$(tty) || die "no terminal"
    [[ $console =~ ^/dev/tty[0-9]+$ ]] || die "run it at a text console, not in a terminal or over ssh"
    [[ $(stat -c %u -- "$console") == 0 ]] || die "$console is not root's"

    [[ -n $user ]] || die "name your login: --user <name>"
    local entry uid home
    entry=$(getent passwd "$user") || die "no user $user"
    uid=$(cut -d: -f3 <<<"$entry")
    home=$(cut -d: -f6 <<<"$entry")
    ((uid >= 1000)) || die "$user is a system account"
    [[ -d $home ]] || die "no home directory for $user"
    # Nothing of theirs may run: not a desktop, a lingering service, an agent or a process left
    # behind. Then nothing of theirs can watch, steer or draw over what happens here.
    if pgrep -u "$uid" >/dev/null; then
        die "programs of $user are running. Stop them all first, then run this again:
    systemctl stop user@$uid.service; loginctl terminate-user $user; pkill -KILL -u $uid"
    fi

    # Exactly the commit named, in full: a tag or branch named like a short hash cannot stand in.
    [[ $commit =~ ^[0-9a-f]{40}$ ]] || die "name the commit in full: --commit <40 hex characters>"
    local here repo
    here=$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)
    repo=$(cd -P "$here/../.." && pwd)
    trusted_tree "$repo"
    [[ $(git -C "$repo" rev-parse HEAD) == "$commit" ]] || die "the clone is not at $commit"
    [[ -z $(git -C "$repo" status --porcelain --ignored) ]] || die "the clone has changes or extra files; use a fresh clone"
    git -C "$repo" merge-base --is-ancestor "$commit" refs/remotes/origin/main 2>/dev/null || die "$commit is not on origin/main"

    local libexec=/usr/local/libexec/relay units=/etc/systemd/system state=/var/lib/relay-key-proxy
    local creds=/etc/credstore.encrypted cache=/var/cache/relay-key-proxy
    local key_cred=$creds/relay-anthropic-key token_cred=$creds/relay-proxy-token
    local token_dir=$home/.config/relay

    if [[ $mode == uninstall ]]; then
        systemctl disable --now relay-key-proxy.socket relay-key-proxy.service 2>/dev/null || true
        rm -f -- "$units/relay-key-proxy.service" "$units/relay-key-proxy.socket" "$key_cred" "$token_cred" "$libexec/relay-key-proxy"
        rmdir -- "$libexec" 2>/dev/null || true
        rm -rf -- "$cache" "$state"
        systemctl daemon-reload
        as_user "$user" rm -f -- "$token_dir/key-proxy-token"
        echo "key-proxy: removed. Revoke the key in the Anthropic console if it may have been exposed."
        return
    fi

    # What the build reads that could change how it builds, beyond the proxy's own sources and
    # the dependencies Cargo.lock pins: none of it is expected, so any of it stops the install.
    grep -qE '^\[(patch|replace)' "$repo/Cargo.toml" && die "Cargo.toml has a [patch] or [replace] section"
    [[ ! -e $repo/crates/relay-key-proxy/build.rs ]] || die "relay-key-proxy has a build script"
    [[ -x /usr/bin/cargo && $(stat -c %u /usr/bin/cargo) == 0 ]] || die "no root-owned /usr/bin/cargo: install Rust from your distribution (pacman -S rust)"
    local previous=""
    [[ -f $state/commit ]] && previous=$(<"$state/commit")
    if [[ -n $previous && $previous != "$commit" ]]; then
        echo "key-proxy: changes since the installed $previous:"
        git -C "$repo" diff --stat "$previous" "$commit" -- crates/relay-key-proxy scripts/key-proxy Cargo.toml Cargo.lock || true
    fi

    echo "key-proxy: building $commit as root"
    install -d -o root -g root -m 0700 "$cache" "$state"
    # Run from the root-owned cache, not the clone: cargo reads .cargo/config files from where it
    # runs, so the clone's (if a commit ever adds one) never applies. A clean environment, a
    # root-owned cargo home, and dependencies exactly as Cargo.lock pins them.
    (cd "$cache" && env -i PATH=/usr/bin:/bin HOME=/root LC_ALL=C CARGO_HOME="$cache/cargo" CARGO_TARGET_DIR="$cache/target" \
        /usr/bin/cargo build --locked --release --manifest-path "$repo/Cargo.toml" -p relay-key-proxy)
    local binary=$cache/target/release/relay-key-proxy
    [[ -f $binary && ! -L $binary ]] || die "the build left no $binary"

    local key=""
    if [[ ! -s $key_cred || $mode == replace-key ]]; then
        echo "key-proxy: type the key from the screen you made it on (your phone, not this PC). Nothing echoes."
        key=$(systemd-ask-password -n "Anthropic API key:")
        [[ $key =~ ^sk-ant-api[0-9]{2}-[A-Za-z0-9_-]{20,}$ ]] || { key=""; die "that is not an ordinary Anthropic API key (sk-ant-api…); admin keys are refused"; }
    fi

    # The socket stays up throughout: PID 1 keeps the port while the service is replaced.
    systemctl stop relay-key-proxy.service 2>/dev/null || true
    install -d -o root -g root -m 0755 "$libexec"
    install -o root -g root -m 0755 "$binary" "$libexec/relay-key-proxy"
    install -o root -g root -m 0644 "$here/relay-key-proxy.service" "$units/relay-key-proxy.service"
    install -o root -g root -m 0644 "$here/relay-key-proxy.socket" "$units/relay-key-proxy.socket"
    install -d -o root -g root -m 0700 "$creds"
    if [[ -n $key ]]; then
        # printf is a shell builtin: the key is in no process's arguments.
        printf '%s' "$key" | systemd-creds encrypt --name=relay-anthropic-key - "$key_cred"
        key=""
        chmod 0600 "$key_cred"
    fi

    # The local pass token Claude shows the proxy. It is not the key; every run makes a new one.
    local token
    token=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
    printf '%s' "$token" | systemd-creds encrypt --name=relay-proxy-token - "$token_cred"
    chmod 0600 "$token_cred"
    # Written as the person, so a link planted in their home cannot turn it into a root write;
    # detached from this console, and given ten seconds.
    printf '%s' "$token" | as_user "$user" /bin/sh -c \
        'umask 077 && mkdir -p "$1" && cat > "$1/key-proxy-token.new" && mv -f "$1/key-proxy-token.new" "$1/key-proxy-token"' sh "$token_dir" \
        || die "could not write $token_dir/key-proxy-token"

    systemctl daemon-reload
    systemctl enable --now relay-key-proxy.socket >/dev/null
    systemctl enable relay-key-proxy.service >/dev/null
    systemctl restart relay-key-proxy.service
    systemctl is-active --quiet relay-key-proxy.service || die "the service did not start: journalctl -u relay-key-proxy"
    printf '%s\n' "$commit" > "$state/commit"

    # One free call through the proxy, as a session would make it: proves the key and the path.
    local status
    status=$(printf 'x-api-key: %s\n' "$token" | curl -s -o /dev/null -w '%{http_code}' --max-time 20 -H @- \
        -H 'anthropic-version: 2023-06-01' http://127.0.0.1:7445/v1/models) || status=none
    token=""
    case $status in
        200) echo "key-proxy: the key works." ;;
        401) echo "key-proxy: Anthropic refused the key. Run again with --replace-key." ;;
        *) echo "key-proxy: the check through the proxy answered $status: journalctl -u relay-key-proxy" ;;
    esac
    echo "key-proxy: running on 127.0.0.1:7445, built from $commit"
    echo "           binary sha256 $(sha256sum "$libexec/relay-key-proxy" | cut -d' ' -f1)"
    echo "           Log out of this console, then start your desktop again."
}

die() {
    echo "key-proxy: $*" >&2
    exit 1
}

# As the person: detached from root's console (no terminal to hold, nothing written to it), and
# given ten seconds, so a mount of theirs that never answers cannot stall the install.
as_user() {
    local user=$1
    shift
    timeout 10 setsid runuser -u "$user" -- "$@" >/dev/null 2>&1
}

# The clone and every folder above it must be root's and writable by no one else, and it may hold
# no symbolic link: a committed link would be followed by the build to a file another account
# writes.
trusted_tree() {
    local dir=$1 owner mode
    while :; do
        read -r owner mode < <(stat -c '%u %a' -- "$dir")
        [[ $owner == 0 ]] || die "$dir is not owned by root: clone into /root as root"
        (((8#$mode & 8#022) == 0)) || die "$dir can be written by others"
        [[ $dir == / ]] && break
        dir=$(dirname -- "$dir")
    done
    [[ -z $(find "$1" -xdev \( ! -uid 0 -o -perm /022 -o -type l \) -print -quit) ]] \
        || die "something in $1 is not root's, is writable by others, or is a symbolic link"
}

main "$@"
exit
