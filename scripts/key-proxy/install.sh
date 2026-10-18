#!/bin/bash
# Install, update or remove Relay's API-key proxy (docs/KEY-PROXY.md, "Setting it up").
#
# It runs as root, from a root login at a text console, from a root-owned clone of a commit you
# name in full, and only while nothing at all runs as you. Your own account is where agents run:
# they can rewrite its files, its shell, its desktop and its terminals, so nothing from it may
# reach root, the build, or the moment the key is typed.
#
#   (booted with systemd.unit=multi-user.target systemd.mask=user@<uid>.service, logged in as
#   root on a text console:)
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
    local entry uid gid home
    entry=$(getent passwd "$user") || die "no user $user"
    uid=$(cut -d: -f3 <<<"$entry")
    gid=$(cut -d: -f4 <<<"$entry")
    home=$(cut -d: -f6 <<<"$entry")
    ((uid >= 1000)) || die "$user is a system account"
    [[ -d $home ]] || die "no home directory for $user"
    # Nothing of theirs may run: not a desktop, a lingering service, an agent, a container under
    # their sub-ids or a process left behind. Then nothing of theirs can watch, steer, hear or draw
    # over what happens here. And nothing of theirs may start until this ends: logind starts their
    # user manager for any session opened as them, and lingering keeps it.
    nothing_runs_as "$uid" "$user"
    systemctl mask --runtime "user@$uid.service" >/dev/null
    trap "systemctl unmask --runtime user@$uid.service >/dev/null 2>&1" EXIT

    # Exactly the commit named, in full: a tag or branch named like a short hash cannot stand in.
    [[ $commit =~ ^[0-9a-f]{40}$ ]] || die "name the commit in full: --commit <40 hex characters>"
    local here repo
    here=$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)
    repo=$(cd -P "$here/../.." && pwd)
    trusted_tree "$repo"
    [[ $(git -C "$repo" rev-parse HEAD) == "$commit" ]] || die "the clone is not at $commit"
    [[ -z $(git -C "$repo" status --porcelain --ignored) ]] || die "the clone has changes or extra files; use a fresh clone"
    # A slip check, not a defence: whoever can push to main decides what is on it. The defence is
    # you reading what this builds, below.
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
        as_user "$uid" "$gid" rm -f -- "$token_dir/key-proxy-token"
        echo "key-proxy: removed. Revoke the key in the Anthropic console if it may have been exposed."
        return
    fi

    # What would make the build take code from anywhere but the proxy's own sources and the
    # crates.io versions Cargo.lock pins: a patch or replacement, a build script of its own, or a
    # dependency from a path, a git repository or another registry. None is expected; any stops it.
    grep -qE '^[[:space:]]*(\[[[:space:]]*(patch|replace)|(patch|replace)[[:space:]]*[.=])' "$repo/Cargo.toml" \
        && die "Cargo.toml patches or replaces dependencies"
    [[ ! -e $repo/crates/relay-key-proxy/build.rs ]] || die "relay-key-proxy has a build script"
    grep -qE '^[[:space:]]*build[[:space:]]*=' "$repo/crates/relay-key-proxy/Cargo.toml" && die "relay-key-proxy names a build script"
    [[ -x /usr/bin/cargo && $(stat -c %u /usr/bin/cargo) == 0 ]] || die "no root-owned /usr/bin/cargo: install Rust from your distribution (pacman -S rust)"
    install -d -o root -g root -m 0700 "$cache" "$state"
    local tree foreign
    tree=$(root_cargo "$cache" "$repo" tree --locked -p relay-key-proxy -e normal,build --prefix none --format '{p}' \
        | sed -e 's/ (\*)$//' -e 's/ (proc-macro)$//' | sort -u)
    foreign=$(grep -v '^relay-key-proxy ' <<<"$tree" | grep '(' || true)
    [[ -z $foreign ]] || die "dependencies from outside crates.io:
$foreign"

    # What is about to run as root, for you to read: on an update everything that changed under
    # the proxy since the commit installed last, and always every crate it builds from.
    local previous=""
    [[ -f $state/commit ]] && previous=$(<"$state/commit")
    if [[ -n $previous && $previous != "$commit" ]]; then
        echo "key-proxy: what changed since the installed $previous:"
        git -C "$repo" --no-pager diff "$previous" "$commit" -- crates/relay-key-proxy scripts/key-proxy Cargo.toml Cargo.lock || true
    else
        echo "key-proxy: $(git -C "$repo" --no-pager log -1 --format='%H %an, %ad: %s')"
    fi
    echo "key-proxy: the crates it builds from, each from crates.io at the version Cargo.lock pins:"
    grep -v '^relay-key-proxy ' <<<"$tree" | tr '\n' ' ' | fold -s -w 100
    echo
    local answer
    read -r -p "key-proxy: build this and install it as root? Type yes: " answer
    [[ $answer == yes ]] || die "stopped; nothing was installed"

    echo "key-proxy: building $commit as root"
    root_cargo "$cache" "$repo" build --locked --release -p relay-key-proxy
    local binary=$cache/target/release/relay-key-proxy
    [[ -f $binary && ! -L $binary ]] || die "the build left no $binary"

    local key=""
    if [[ ! -s $key_cred || $mode == replace-key ]]; then
        nothing_runs_as "$uid" "$user"
        echo "key-proxy: type the key from the screen you made it on (your phone, not this PC)."
        echo "           Nothing shows as you type, and there is no time limit."
        key=$(systemd-ask-password --timeout=0 --echo=no -n "Anthropic API key:") || die "no key was entered; nothing was installed"
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
    # Written as the person, so a link planted in their home cannot turn it into a root write.
    printf '%s' "$token" | as_user "$uid" "$gid" /bin/sh -c \
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

# As the person, without a login: setpriv opens no PAM session, so logind starts nothing of
# theirs. Detached from root's console (no terminal to hold, nothing written to it), and given ten
# seconds, so a mount of theirs that never answers cannot stall the install.
as_user() {
    local uid=$1 gid=$2
    shift 2
    timeout 10 setsid setpriv --reuid="$uid" --regid="$gid" --init-groups --reset-env -- "$@" >/dev/null 2>&1
}

# No process of theirs, their user manager stopped and their slice empty (a container under their
# sub-ids runs there with another uid).
nothing_runs_as() {
    local uid=$1 user=$2
    if pgrep -u "$uid" >/dev/null || systemctl is-active --quiet "user@$uid.service" || systemctl is-active --quiet "user-$uid.slice"; then
        die "programs of $user are running. Stop them all, then run this again:
    loginctl terminate-user $user; systemctl stop user-$uid.slice"
    fi
}

# cargo as root, run from the root-owned cache rather than the clone (cargo reads .cargo/config
# files from where it runs, so the clone's, should a commit add one, never applies), with a clean
# environment and a root-owned cargo home.
root_cargo() {
    local cache=$1 repo=$2
    shift 2
    (cd "$cache" && env -i PATH=/usr/bin:/bin HOME=/root LC_ALL=C CARGO_HOME="$cache/cargo" CARGO_TARGET_DIR="$cache/target" \
        /usr/bin/cargo "$@" --manifest-path "$repo/Cargo.toml")
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
