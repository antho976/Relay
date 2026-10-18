#!/bin/bash
# Remove Relay's API-key proxy: the service and socket, the key and the token, the installed
# programs and their state (docs/KEY-PROXY.md, "Removing it"). The install puts it, root-owned, at
# /usr/local/libexec/relay/key-proxy-uninstall, so removing the proxy never runs a script from a
# newer clone you have not reviewed.
#
#   /usr/local/libexec/relay/key-proxy-uninstall --user <login>

main() {
    set -euo pipefail
    export PATH=/usr/bin:/bin LC_ALL=C
    [[ $EUID -eq 0 ]] || die "run it as root"
    [[ $(</proc/self/loginuid) == 0 ]] || die "log in as root itself on a text console"
    local console
    console=$(tty) || die "no terminal"
    [[ $console =~ ^/dev/tty[0-9]+$ && $(stat -c %u -- "$console") == 0 ]] || die "run it at a root text console"
    [[ ${1:-} == --user && -n ${2:-} ]] || die "usage: key-proxy-uninstall --user <login>"
    local entry uid gid home
    entry=$(getent passwd "$2") || die "no user $2"
    uid=$(cut -d: -f3 <<<"$entry")
    gid=$(cut -d: -f4 <<<"$entry")
    home=$(cut -d: -f6 <<<"$entry")

    local libexec=/usr/local/libexec/relay units=/etc/systemd/system creds=/etc/credstore.encrypted
    systemctl disable --now relay-key-proxy.socket relay-key-proxy.service 2>/dev/null || true
    rm -f -- "$units/relay-key-proxy.service" "$units/relay-key-proxy.socket" \
        "$creds/relay-anthropic-key" "$creds/relay-proxy-token" \
        "$libexec/relay-key-proxy" "$libexec/key-proxy-review" "$libexec/key-proxy-uninstall"
    rmdir -- "$libexec" 2>/dev/null || true
    rm -rf -- /var/cache/relay-key-proxy /var/lib/relay-key-proxy
    systemctl daemon-reload
    # As the person, without a login (setpriv opens no PAM session), detached and time-limited.
    timeout 10 setsid setpriv --reuid="$uid" --regid="$gid" --init-groups --reset-env -- \
        rm -f -- "$home/.config/relay/key-proxy-token" >/dev/null 2>&1 || true
    echo "key-proxy: removed. Revoke the key in the Anthropic console if it may have been exposed."
}

die() {
    echo "key-proxy: $*" >&2
    exit 1
}

main "$@"
exit
