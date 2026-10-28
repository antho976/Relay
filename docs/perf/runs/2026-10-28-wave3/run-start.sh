#!/bin/bash
# usage: run-start.sh <out-dir> <idle|active|unfocused>
OUT=$1; MODE=$2
SOCK=relay-headless-$$
KWIN_WAYLAND_NO_PERMISSION_CHECKS=1 kwin_wayland --virtual --no-lockscreen --socket=$SOCK --width 1440 --height 900 >/dev/null 2>&1 &
KWIN=$!
for i in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/$SOCK" ] && break; sleep 0.05; done
(
  for i in $(seq 600); do pgrep -f "^$PWD/target/debug/relay-native" >/dev/null && break; sleep 0.1; done
  sleep 3
  case $MODE in
    active)
      for i in $(seq 30); do
        WAYLAND_DISPLAY=$SOCK python3 scripts/wayland-fake-input.py move $((300 + (i % 2) * 400)) 450 >/dev/null 2>&1
        sleep 2
      done;;
    unfocused)
      WAYLAND_DISPLAY=$SOCK DISPLAY= GDK_BACKEND=wayland zenity --info --width=1400 --height=860 --text="focus elsewhere" >/dev/null 2>&1 &
      echo $! > "$OUT.zenity";;
    minimized) ;;
    unfocused-active)
      WAYLAND_DISPLAY=$SOCK DISPLAY= GDK_BACKEND=wayland zenity --info --width=1400 --height=860 --text="focus elsewhere" >/dev/null 2>&1 &
      echo $! > "$OUT.zenity"
      sleep 1.5
      # A new window does not take focus by itself here: a click on the dialog gives it focus.
      WAYLAND_DISPLAY=$SOCK python3 scripts/wayland-fake-input.py click 720 450 >/dev/null 2>&1
      for i in $(seq 30); do
        WAYLAND_DISPLAY=$SOCK python3 scripts/wayland-fake-input.py move $((150 + (i % 2) * 150)) 150 >/dev/null 2>&1
        sleep 2
      done;;
  esac
) &
HELPER=$!
mkdir -p "$OUT"
[ "$MODE" = minimized ] && export RELAY_NATIVE_MINIMIZE=4
WAYLAND_DISPLAY=$SOCK DISPLAY= GDK_BACKEND=wayland python3 scripts/perf/native-baseline.py --out-dir "$OUT" --no-perf --skip-build --pages start --hidden-seconds 1 --seconds 29
RC=$?
kill $HELPER 2>/dev/null
[ -f "$OUT.zenity" ] && kill $(cat "$OUT.zenity") 2>/dev/null
kill $KWIN
exit $RC
