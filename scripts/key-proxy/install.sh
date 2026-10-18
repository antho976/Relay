#!/bin/bash
# Install or update Relay's API-key proxy (docs/KEY-PROXY.md, "Setting it up").
#
# It runs as root, from a root login at a text console, from a root-owned clone of a commit you
# name in full, and only while nothing at all runs as you. Your own account is where agents run:
# they can rewrite its files, its shell, its desktop and its terminals, so nothing from it may
# reach root, the build, or the moment the key is typed.
#
# It builds and installs only the files listed in STAGED, copied out of the clone, each of them
# plain printable ASCII (so nothing in them can hide from less), and on an update only the exact
# tree you approved with the key-proxy-review the last install left. Never run this script from a
# clone before you have read it (first install) or approved it (an update).
#
#   (powered on with systemd.unit=multi-user.target systemd.mask=user@<uid>.service
#   systemd.mask=bluetooth.service, logged in as root on a text console:)
#   git clone --no-tags https://github.com/antho976/Relay /root/relay-key-proxy
#   cd /root/relay-key-proxy && git checkout --detach <full 40-character commit>
#   first install: read the files docs/KEY-PROXY.md lists, with less -U
#   an update:     /usr/local/libexec/relay/key-proxy-review /root/relay-key-proxy
#   bash scripts/key-proxy/install.sh --user <login> --commit <same commit>                  # install or update
#   bash scripts/key-proxy/install.sh --user <login> --commit <same commit> --replace-key    # new key
#   removing it:   /usr/local/libexec/relay/key-proxy-uninstall --user <login>
#
# Everything is in main(), and the script exits before reading further, so bash has parsed it
# whole before running any of it.

# Everything the install builds or runs, relative to crates/ and scripts/. Any other file beside
# them stops it. (Kept identical to review.sh.)
STAGED=(relay-key-proxy/Cargo.toml relay-key-proxy/Cargo.lock relay-key-proxy/src/main.rs
    key-proxy/install.sh key-proxy/review.sh key-proxy/uninstall.sh
    key-proxy/relay-key-proxy.service key-proxy/relay-key-proxy.socket)

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
    # you reading what this builds, with less -U or key-proxy-review, before it runs.
    git -C "$repo" merge-base --is-ancestor "$commit" refs/remotes/origin/main 2>/dev/null || die "$commit is not on origin/main"

    local libexec=/usr/local/libexec/relay units=/etc/systemd/system state=/var/lib/relay-key-proxy
    local creds=/etc/credstore.encrypted cache=/var/cache/relay-key-proxy
    local key_cred=$creds/relay-anthropic-key token_cred=$creds/relay-proxy-token
    local token_dir=$home/.config/relay

    # What is built: the listed files alone, copied out of the clone.
    install -d -o root -g root -m 0700 "$cache" "$state"
    local stage=$cache/stage crate
    rm -rf -- "$stage"
    stage "$repo" "$stage"
    crate=$stage/relay-key-proxy

    # Only what you read, before cargo touches anything. An update must be the exact tree
    # key-proxy-review showed you (the copy the last install left, so this commit could not change
    # it); a first install, the files you read with less -U.
    if [[ -d $state/installed ]]; then
        [[ -f $state/approved && $(tree_hash "$stage") == $(<"$state/approved") ]] \
            || die "this tree is not the one you approved: run /usr/local/libexec/relay/key-proxy-review $repo first"
    else
        echo "key-proxy: first install of $commit. It builds and installs exactly the files you read:"
        printf '           %s\n' "${STAGED[@]}"
        local answer
        read -r -p "key-proxy: did you read every one of them with less -U? Type yes: " answer
        [[ $answer == yes ]] || die "stopped; nothing was installed"
    fi

    # Nothing in them may make cargo take code from elsewhere: a build script, a source file outside
    # the folder (asked of cargo metadata, which runs nothing), a proc macro, or a dependency not
    # from crates.io.
    local meta src
    meta=$(root_cargo "$cache" metadata --no-deps --offline --format-version 1 --manifest-path "$crate/Cargo.toml")
    ! grep -q '"custom-build"' <<<"$meta" || die "relay-key-proxy names a build script"
    # Resolved first: "relay-key-proxy/../x.rs" starts like a path inside it and is not.
    while read -r src; do
        [[ $(realpath -m -- "$src") == "$crate/"* ]] || die "relay-key-proxy builds a file outside its folder: $src"
    done < <(grep -o '"src_path":"[^"]*"' <<<"$meta" | cut -d'"' -f4)
    local tree foreign
    tree=$(root_cargo "$cache" tree --locked --manifest-path "$crate/Cargo.toml" -e normal,build --prefix none --format '{p}' \
        | sed 1d | sed -e 's/ (\*)$//' | sort -u)
    foreign=$(grep '(' <<<"$tree" || true)
    [[ -z $foreign ]] || die "dependencies that are proc macros or come from outside crates.io:
$foreign"
    echo "key-proxy: from crates.io, at the versions Cargo.lock pins:"
    tr '\n' ' ' <<<"$tree" | fold -s -w 100
    echo

    echo "key-proxy: building $commit"
    root_cargo "$cache" build --locked --release --manifest-path "$crate/Cargo.toml"
    local binary=$cache/target/release/relay-key-proxy deps=$cache/target/release/relay-key-proxy.d dep
    [[ -f $binary && ! -L $binary && -s $deps ]] || die "the build left no $binary or no record of what it read"
    # What the compiler actually read for the binary: the staged folder and pinned crates only.
    while read -r dep; do
        dep=$(realpath -m -- "$dep")
        [[ $dep == "$crate/"* || $dep == "$cache/cargo/registry/src/"* ]] || die "the build read $dep"
    done < <(cut -d: -f2- "$deps" | tr ' ' '\n' | grep -v '^$')

    local key=""
    if [[ ! -s $key_cred || $mode == replace-key ]]; then
        nothing_runs_as "$uid" "$user"
        echo "key-proxy: type the key from the screen you made it on (your phone, not this PC)."
        echo "           Nothing shows as you type, and there is no time limit."
        key=$(systemd-ask-password --timeout=0 --echo=no -n "Anthropic API key:") || die "no key was entered; nothing was installed"
        [[ $key =~ ^sk-ant-api[0-9]{2}-[A-Za-z0-9_-]{20,}$ ]] || { key=""; die "that is not an ordinary Anthropic API key (sk-ant-api...); admin keys are refused"; }
    fi

    # The socket stays up throughout: PID 1 keeps the port while the service is replaced.
    systemctl stop relay-key-proxy.service 2>/dev/null || true
    install -d -o root -g root -m 0755 "$libexec"
    install -o root -g root -m 0755 "$binary" "$libexec/relay-key-proxy"
    install -o root -g root -m 0755 "$stage/key-proxy/review.sh" "$libexec/key-proxy-review"
    install -o root -g root -m 0755 "$stage/key-proxy/uninstall.sh" "$libexec/key-proxy-uninstall"
    install -o root -g root -m 0644 "$stage/key-proxy/relay-key-proxy.service" "$units/relay-key-proxy.service"
    install -o root -g root -m 0644 "$stage/key-proxy/relay-key-proxy.socket" "$units/relay-key-proxy.socket"
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
    # What the next update is compared with, and may only replace once you approve it.
    rm -rf -- "$state/installed" "$state/approved"
    cp -R --no-dereference --preserve=mode -- "$stage" "$state/installed"
    printf '%s\n' "$commit" > "$state/commit"

    # One free call through the proxy, as a session would make it: proves the key and the path.
    local status
    status=$(printf 'x-api-key: %s\n' "$token" | curl -q -s --noproxy '*' -o /dev/null -w '%{http_code}' --max-time 20 -H @- \
        -H 'anthropic-version: 2023-06-01' http://127.0.0.1:7445/v1/models) || status=none
    token=""
    case $status in
        200) echo "key-proxy: the key works." ;;
        401) echo "key-proxy: Anthropic refused the key. Run again with --replace-key." ;;
        *) echo "key-proxy: the check through the proxy answered $status: journalctl -u relay-key-proxy" ;;
    esac
    echo "key-proxy: running on 127.0.0.1:7445, built from $commit"
    echo "           binary sha256 $(sha256sum "$libexec/relay-key-proxy" | cut -d' ' -f1)"
    echo "           Now reboot from this console (reboot), and start your desktop as usual."
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

# No process of theirs (by effective or real user id: a setuid program they started counts), their
# user manager stopped and their slice empty (a container under their sub-ids runs there).
nothing_runs_as() {
    local uid=$1 user=$2
    if pgrep -u "$uid" >/dev/null || pgrep -U "$uid" >/dev/null || systemctl is-active --quiet "user@$uid.service" \
        || systemctl is-active --quiet "user-$uid.slice"; then
        die "programs of $user are running. Stop them all, then run this again:
    loginctl terminate-user $user; systemctl stop user-$uid.slice"
    fi
}

# cargo as root, run from the root-owned cache (cargo reads .cargo/config files from where it
# runs, so none from the clone applies), with a clean environment and a root-owned cargo home.
root_cargo() {
    local cache=$1
    shift
    (cd "$cache" && env -i PATH=/usr/bin:/bin HOME=/root LC_ALL=C CARGO_HOME="$cache/cargo" CARGO_TARGET_DIR="$cache/target" \
        /usr/bin/cargo "$@")
}

# The clone and every folder above it must be root's and writable by no one else, holding no
# symbolic link. (Kept identical to review.sh.)
trusted_tree() {
    local dir=$1 owner mode
    while :; do
        read -r owner mode < <(stat -c '%u %a' -- "$dir")
        [[ $owner == 0 ]] || die "$dir is not owned by root"
        (((8#$mode & 8#022) == 0)) || die "$dir can be written by others"
        [[ $dir == / ]] && break
        dir=$(dirname -- "$dir")
    done
    [[ -z $(find "$1" -xdev \( ! -uid 0 -o -perm /022 -o -type l \) -print -quit) ]] \
        || die "something in $1 is not root's, is writable by others, or is a symbolic link"
}

# The STAGED files, copied out of the clone: all this builds and installs. The proxy's folders must
# hold exactly those files, and each must be plain printable ASCII (tab, newline, space to tilde):
# a backspace, an escape or a carriage return could hide a line from less. (Kept identical to
# review.sh.)
stage() {
    local clone=$1 dest=$2 found file
    found=$({ (cd "$clone/crates" && find relay-key-proxy -mindepth 1 ! -type d); \
        (cd "$clone/scripts" && find key-proxy -mindepth 1 ! -type d); } | sort)
    [[ $found == "$(printf '%s\n' "${STAGED[@]}" | sort)" ]] || die "the proxy's folders hold other files than these:
$(printf '  %s\n' "${STAGED[@]}")"
    for file in "${STAGED[@]}"; do
        local from=$clone/crates/$file
        [[ $file == key-proxy/* ]] && from=$clone/scripts/$file
        ! LC_ALL=C grep -q $'[^\t -~]' -- "$from" || die "$file holds bytes other than printable ASCII"
        install -D -m 0644 -- "$from" "$dest/$file"
    done
}

# One hash for a staged tree: every file's path and content. (Kept identical to review.sh.)
tree_hash() {
    (cd "$1" && find . -type f -print0 | sort -z | xargs -0 sha256sum) | sha256sum | cut -d' ' -f1
}

main "$@"
exit
