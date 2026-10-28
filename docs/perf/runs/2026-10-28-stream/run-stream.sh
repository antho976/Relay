#!/bin/bash
# usage: run-stream.sh <out-dir> [chars,chars]
OUT=$1; CHARS=${2:-2000,8000}
SOCK=relay-headless-$$
KWIN_WAYLAND_NO_PERMISSION_CHECKS=1 kwin_wayland --virtual --no-lockscreen --socket=$SOCK --width 1440 --height 900 >/dev/null 2>&1 &
KWIN=$!
for i in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/$SOCK" ] && break; sleep 0.05; done
mkdir -p "$OUT"
WAYLAND_DISPLAY=$SOCK DISPLAY= GDK_BACKEND=wayland python3 scripts/perf/native-baseline.py --out-dir "$OUT" --no-perf --skip-build --stream-only --stream-chars "$CHARS"
RC=$?
kill $KWIN
exit $RC
