#!/bin/bash
# Show what an update of Relay's API-key proxy would install, before any of it runs
# (docs/KEY-PROXY.md, "Updating"). The install puts it, root-owned, at
# /usr/local/libexec/relay/key-proxy-review, so a new commit cannot change what shows you that
# commit: the copy that runs is the one you approved last time.
#
#   /usr/local/libexec/relay/key-proxy-review /root/relay-key-proxy
#
# It compares the clone's proxy folder and scripts with the copy installed last
# (/var/lib/relay-key-proxy/installed), shows every difference in less, with control bytes and
# anything outside ASCII shown as codes (^[, <E2>) so nothing in the diff can redraw the screen or
# pass for something else, and on `yes` records that exact tree as approved. The installer builds
# only an approved tree.

main() {
    set -euo pipefail
    export PATH=/usr/bin:/bin LC_ALL=C
    umask 077
    [[ $EUID -eq 0 ]] || die "run it as root"
    [[ $(</proc/self/loginuid) == 0 ]] || die "log in as root itself on a text console"
    local console
    console=$(tty) || die "no terminal"
    [[ $console =~ ^/dev/tty[0-9]+$ && $(stat -c %u -- "$console") == 0 ]] || die "run it at a root text console"

    local clone=${1:-}
    [[ -d $clone ]] || die "usage: key-proxy-review <root-owned clone>"
    clone=$(cd -P "$clone" && pwd)
    trusted_tree "$clone"
    local state=/var/lib/relay-key-proxy cache=/var/cache/relay-key-proxy
    [[ -d $state/installed ]] || die "nothing is installed to compare with: for a first install, read the files as docs/KEY-PROXY.md says"

    install -d -o root -g root -m 0700 "$cache"
    local work
    work=$(mktemp -d "$cache/review.XXXXXX")
    trap 'rm -rf -- "$work"' EXIT
    stage "$clone" "$work/new"
    echo "key-proxy: clone at $(git -C "$clone" rev-parse HEAD), installed $(cat "$state/commit" 2>/dev/null || echo unknown)"
    local rc=0
    (cd "$state" && diff -ruN --text installed "$work/new") > "$work/diff" || rc=$?
    case $rc in
        0) echo "key-proxy: no differences from what is installed." ;;
        1) env -i PATH=/usr/bin:/bin TERM="${TERM:-linux}" LC_ALL=C LESSSECURE=1 less -- "$work/diff" ;;
        *) die "diff failed" ;;
    esac
    local answer
    read -r -p "key-proxy: approve exactly this for the installer? Type yes: " answer
    [[ $answer == yes ]] || die "not approved"
    tree_hash "$work/new" > "$state/approved"
    echo "key-proxy: approved. Now run the installer from the clone (bash scripts/key-proxy/install.sh …)."
}

die() {
    echo "key-proxy: $*" >&2
    exit 1
}

# The clone and every folder above it must be root's and writable by no one else, holding no
# symbolic link. (Kept identical to install.sh.)
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

# The proxy's folder and its scripts, copied out of the clone: all the installer builds and
# installs. (Kept identical to install.sh.)
stage() {
    local clone=$1 dest=$2
    [[ ! -e $clone/crates/relay-key-proxy/target ]] || die "the clone has a build folder in it; use a fresh clone"
    mkdir -p -- "$dest"
    cp -R --no-dereference --preserve=mode -- "$clone/crates/relay-key-proxy" "$dest/relay-key-proxy"
    cp -R --no-dereference --preserve=mode -- "$clone/scripts/key-proxy" "$dest/key-proxy"
}

# One hash for a staged tree: every file's path and content. (Kept identical to install.sh.)
tree_hash() {
    (cd "$1" && find . -type f -print0 | sort -z | xargs -0 sha256sum) | sha256sum | cut -d' ' -f1
}

main "$@"
exit
