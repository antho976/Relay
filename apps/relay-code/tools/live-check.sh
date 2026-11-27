#!/usr/bin/env bash
# Relay Code's link against a real door on this PC, with a throwaway `test` instance of Relay:
# nothing touches the dev or stable instance, or a phone.
#
#   apps/relay-code/tools/live-check.sh
#
# For the owner only, in a terminal of their own: it acts as the person at that terminal on the
# test instance. Agents never run it.
#
# It builds relay from this checkout, starts the test engine and a door on loopback, and gives the
# engine a project (a scratch git repository) and a fake published APK. Then it waits: pairing is
# the owner's to do, so it prints the one command to run in a second terminal, which writes the
# pairing link to a file. Once the link is there it runs rc-link's live test (tests/live_door.rs):
# pair, connect by itself, chat.send typed once under one id, session.transcript with `before`,
# phone.app.latest and phone.app.apk. What it started is stopped at the end; the test instance's
# store stays in ~/.local/share/relay-v4/test.
#
# RELAY_LIVE_PORT (7431) is the door's port, RELAY_LIVE_WAIT (600) how many seconds to wait for
# the pairing.
set -euo pipefail

# An agent's session is how Relay tells an agent from the owner: inside one, this is not run.
if [[ -n ${RELAY_SESSION:-} ]]; then
    echo "Run this in your own terminal, outside Claude Code."
    exit 2
fi

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
port=${RELAY_LIVE_PORT:-7431}
wait_s=${RELAY_LIVE_WAIT:-600}
work=$(mktemp -d)
relay="$root/target/debug/relay"
build=7
pids=()

test_relay() { "$relay" --instance test "$@"; }
# The same in the background, its pid kept to stop it at the end.
start_bg() {
    local log=$1
    shift
    "$relay" --instance test "$@" > "$log" 2>&1 &
    pids+=($!)
}

cleanup() {
    test_relay q app.quit '{"force":true}' > /dev/null 2>&1 || true
    for p in "${pids[@]}"; do kill "$p" 2> /dev/null || true; done
    wait 2> /dev/null || true
    rm -rf -- "$work"
}
trap cleanup EXIT

field() { python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }

echo "Building relay..."
cargo build -q --manifest-path "$root/Cargo.toml" -p relay-cli

echo "Starting the test engine..."
start_bg "$work/engine.log" serve
for _ in $(seq 1 100); do
    test_relay q bus.ping > /dev/null 2>&1 && break
    sleep 0.1
done
test_relay q bus.ping > /dev/null || { echo "The test engine did not start:"; cat "$work/engine.log"; exit 1; }

echo "A project for it, and a fake APK..."
repo="$work/ws/app"
mkdir -p "$repo"
git -C "$repo" init -q -b main
git -C "$repo" -c user.email=live@check -c user.name=live commit -q --allow-empty -m init
ws=$(test_relay q workspace.create "{\"path\":\"$work/ws\"}" | field id)
project=$(test_relay q project.add "{\"workspace_id\":$ws,\"path\":\"$repo\"}" | field id)
head -c 200000 /dev/urandom > "$work/fake.apk"
RELAY_CODE_BUILD=$build "$here/publish-apk.sh" "$work/fake.apk" test > /dev/null

echo "Opening the test door on 127.0.0.1:$port..."
start_bg "$work/door.log" remote serve --bind "127.0.0.1:$port" --no-rendezvous
for _ in $(seq 1 100); do
    grep -q "listening on" "$work/door.log" && break
    sleep 0.1
done
grep -q "listening on" "$work/door.log" || { echo "The test door did not open:"; cat "$work/door.log"; exit 1; }

link_file="$work/pair-link.txt"
cat <<EOF

Pairing is yours to do, and Relay refuses it inside an agent's session: run this in a terminal of
your own (not in Claude Code), as it is:

  $relay --instance test remote pair --no-confirm > $link_file

Waiting up to $wait_s s for the link...
EOF
link=""
for _ in $(seq 1 "$wait_s"); do
    link=$(grep -o 'relay://pair[^[:space:]]*' "$link_file" 2> /dev/null | head -1 || true)
    [[ -n $link ]] && break
    sleep 1
done
[[ -n $link ]] || { echo "No pairing link came: nothing was tested."; exit 1; }

echo "Running the live test..."
cd "$root/apps/relay-code/native"
RELAY_E2E_LINK="$link" RELAY_E2E_URL="ws://127.0.0.1:$port" RELAY_E2E_PROJECT="$project" RELAY_E2E_BUILD="$build" \
    cargo test --locked -p rc-link --test live_door -- --nocapture
echo "Live check passed."
