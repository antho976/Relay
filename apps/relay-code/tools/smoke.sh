#!/usr/bin/env bash
# A smoke run of Relay Code on an Android emulator, never a phone: build the debug APK for x86_64,
# install it, open Home, a chat, Changes and Settings on sample data (the debug build's `screen`
# extra, smoke.rs) and screenshot each, and measure startup to the first frame.
#
#   tools/smoke.sh [out-dir]          default: smoke-out/ beside this script's app
#
#   AVD=Tally_API36   the emulator to boot when none is running (headless, no snapshot saved)
#   GPU=auto          its -gpu mode (swiftshader_indirect keeps it off the graphics card)
#   SKIP_BUILD=1      install the APK already built
#   KEEP_EMULATOR=1   leave an emulator this run booted running
#
# The device is leased through Relay (device.claim, released at the end), so a peer's adb install
# waits instead of racing this one. Startup is the chat screen's: `am start -W`'s TotalTime (process
# start to the activity's first frame, as Android counts it) and the app's own log line, "first
# frame N ms after start" (from android_main). Each cold start is the app force-stopped first.
set -euo pipefail
cd "$(dirname "$0")/.."
APP_DIR=$(pwd)
OUT=${1:-$APP_DIR/smoke-out}
SDK=${ANDROID_HOME:-$HOME/Android/Sdk}
export ANDROID_HOME=$SDK
ADB=$SDK/platform-tools/adb
EMU=$SDK/emulator/emulator
AVD=${AVD:-Tally_API36}
GPU=${GPU:-auto}
RELAY=${RELAY_BIN:-relay}
PKG=com.quietsoftware.relaycode
APK=$APP_DIR/native/android/build/outputs/apk/debug/app-debug.apk
mkdir -p "$OUT"

say() { printf '%s\n' "$*" >&2; }

# An emulator: one running already, else this run boots one.
serial=$("$ADB" devices | awk '$1 ~ /^emulator-[0-9]+$/ && $2 == "device" {print $1; exit}')
booted=""
if [ -z "$serial" ]; then
  say "booting $AVD (headless)…"
  "$EMU" -avd "$AVD" -no-window -no-audio -no-boot-anim -no-snapshot-save -gpu "$GPU" >"$OUT/emulator.log" 2>&1 &
  booted=$!
  for _ in $(seq 1 90); do
    serial=$("$ADB" devices | awk '$1 ~ /^emulator-[0-9]+$/ && $2 == "device" {print $1; exit}')
    [ -n "$serial" ] && break
    sleep 2
  done
  [ -n "$serial" ] || { say "the emulator did not come up (see $OUT/emulator.log)"; exit 1; }
  for _ in $(seq 1 120); do
    [ "$("$ADB" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ] && break
    sleep 2
  done
fi
case "$serial" in emulator-*) ;; *) say "refusing $serial: the smoke run uses an emulator only"; exit 1 ;; esac
A=("$ADB" -s "$serial")

# The lease: released, and a booted emulator closed, however the run ends.
finish() {
  "$RELAY" q device.release "{\"device\":\"$serial\"}" >/dev/null 2>&1 || true
  if [ -n "$booted" ] && [ -z "${KEEP_EMULATOR:-}" ]; then
    "${A[@]}" emu kill >/dev/null 2>&1 || true
    sleep 5
    kill "$booted" 2>/dev/null || true
  fi
}
trap finish EXIT
# Held by a peer (device.busy): wait for its lease to end, then ask again, up to 10 minutes.
for _ in $(seq 1 10); do
  claim=$("$RELAY" q device.claim "{\"device\":\"$serial\",\"action\":\"Relay Code smoke run\",\"minutes\":20}" 2>&1 || true)
  case "$claim" in *device.busy*) ;; *) break ;; esac
  say "the device is in use; waiting for its lease…"
  "$RELAY" q bus.wait '{"events":["device.lease.released"],"timeout_ms":60000}' >/dev/null 2>&1 || true
done
case "$claim" in *'"code"'*) say "no lease: $claim"; exit 1 ;; esac

if [ -z "${SKIP_BUILD:-}" ]; then
  say "building librelaycode.so (x86_64) and the debug APK…"
  (cd native && tools/build-lib.sh x86_64) >"$OUT/build.log" 2>&1
  ./gradlew -q :app:assembleDebug >>"$OUT/build.log" 2>&1
fi
"${A[@]}" install -r "$APK" >/dev/null

# Each screen from a cold start: its screenshot, and how long it took to its first frame.
printf 'screen\ttotal_ms\tfirst_frame_ms\n' >"$OUT/startup.tsv"
for screen in home chat changes settings; do
  "${A[@]}" shell am force-stop "$PKG"
  "${A[@]}" logcat -c
  total=$("${A[@]}" shell am start -W -n "$PKG/.CodeActivity" --es screen "$screen" | tr -d '\r' | awk -F': ' '/TotalTime/ {print $2}')
  sleep 3
  # The first line after the clear: a freshly booted emulator may make the activity again.
  first=$("${A[@]}" logcat -d -s relaycode | grep -m1 -o 'first frame [0-9]* ms' | grep -o '[0-9]*' || true)
  "${A[@]}" exec-out screencap -p >"$OUT/$screen.png"
  printf '%s\t%s\t%s\n' "$screen" "${total:-?}" "${first:-?}" >>"$OUT/startup.tsv"
  say "$screen: TotalTime ${total:-?} ms, first frame ${first:-?} ms after start"
done
"${A[@]}" shell am force-stop "$PKG"
say "shots and startup.tsv in $OUT"
