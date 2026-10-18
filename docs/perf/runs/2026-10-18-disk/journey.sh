#!/bin/bash
# Edit relay-core, rebuild its tests: wall time, peak RSS of compilers and linkers, bytes linked.
# Usage: journey.sh <worktree> <rounds>. Modes: full = CARGO_PROFILE_DEV_DEBUG=true in target/,
# lt = the checked-in profile (line tables only) in target-lt/. Both at 8 jobs, nice 10.
set -u
W=$1; rounds=$2; S=$(dirname "$0")
cd "$W"
for round in $(seq 1 "$rounds"); do
  for mode in full lt; do
    rm -f "$S/stop"
    bash "$S/rss-sampler.sh" "$W" "$S/stop" > "$S/rss-$mode-$round.txt" &
    sampler=$!
    touch crates/relay-core/src/lib.rs
    start=$(date +%s%N)
    if [ "$mode" = full ]; then
      CARGO_PROFILE_DEV_DEBUG=true CARGO_BUILD_JOBS=8 nice -n 10 cargo test --no-run -q -p relay-core > /dev/null 2>&1
      dir=target
    else
      CARGO_TARGET_DIR=target-lt CARGO_BUILD_JOBS=8 nice -n 10 cargo test --no-run -q -p relay-core > /dev/null 2>&1
      dir=target-lt
    fi
    status=$?
    end=$(date +%s%N)
    touch "$S/stop"; wait "$sampler"
    linked=$(find "$dir/debug/deps" -maxdepth 1 -type f -executable -newermt "@$((start / 1000000000))" -printf '%s\n' | awk '{s+=$1; n++} END {printf "%d exes %.1f GB", n, s/1e9}')
    printf '%s round%s: exit %s, %d.%01d s, peak rss sum/single MB %s, linked %s\n' "$mode" "$round" "$status" \
      $(((end - start) / 1000000000)) $((((end - start) / 100000000) % 10)) "$(cat "$S/rss-$mode-$round.txt")" "$linked"
  done
done
