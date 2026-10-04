#!/usr/bin/env bash
# Device captures of Home for the design finish review. What the review asks about is the shell
# around the screen: the real status bar, the raised navigation bar and its indicator pill, the
# add FAB, the system insets. Robolectric goldens draw none of that, so an emulator takes them.
#
#   bash .github/scripts/capture.sh phone    phone.png, then phone-200.png at the 200% font scale
#   bash .github/scripts/capture.sh tablet   tablet.png
#
# Each capture is one run of CaptureTest, which reaches Home and screenshots the whole display.
# The PNGs land in ./captures (gitignored); device-captures.yml commits them to .impeccable/captures.
set -euo pipefail

device="${1:-}"
case "$device" in
  phone | tablet) ;;
  *) echo "::error::usage: capture.sh phone|tablet"; exit 2 ;;
esac

APP_ID=com.quietsoftware.tally.debug
RUNNER="$APP_ID.test/androidx.test.runner.AndroidJUnitRunner"
APP_APK=app/build/outputs/apk/debug/app-debug.apk
TEST_APK=app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
# CaptureTest writes to the app's external files dir; the shell user may read it, the app needs no
# storage permission to write it.
REMOTE="/sdcard/Android/data/$APP_ID/files/captures"
OUT=captures

# sys.boot_completed flips before the package manager answers; installing then fails at random.
wait_for_framework() {
  local deadline=$((SECONDS + 180))
  while ((SECONDS < deadline)); do
    if [[ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]] && adb shell pm path android > /dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "::error::Android framework did not become ready."
  return 1
}

taken=()

# Demo mode pins the clock, battery and signal and hides the emulator's own notification icons,
# so two runs differ only where the app does. The bar's height and icon tint stay the system's.
demo() { adb shell am broadcast -a com.android.systemui.demo -e command "$@" > /dev/null 2>&1 || true; }
steady_bar() {
  adb shell settings put global sysui_demo_allowed 1 > /dev/null 2>&1 || true
  demo enter
  demo clock -e hhmm 0941
  demo battery -e level 100 -e plugged false
  demo network -e wifi show -e level 4 -e mobile show -e datatype none -e level 4
  demo notifications -e visible false
}

capture() {
  local name="$1" out
  # A font-scale change or a SystemUI restart drops demo mode, so pin the bar before every shot.
  steady_bar
  echo "::group::Capture $name"
  # am instrument exits 0 whether or not the test passed, so the verdict is read from its output.
  out=$(adb shell am instrument -w -e class com.tally.app.CaptureTest -e captureName "$name" "$RUNNER" 2>&1) || true
  echo "$out"
  echo "::endgroup::"
  if ! grep -qF "OK (1 test)" <<< "$out"; then
    echo "::error::CaptureTest did not pass for $name."
    exit 1
  fi
  taken+=("$name")
}

wait_for_framework
mkdir -p "$OUT"

echo "::group::Display ($device)"
# For the record: what size and density these pixels came from.
{
  echo "$device"
  adb shell wm size | tr -d '\r'
  adb shell wm density | tr -d '\r'
} | tee "$OUT/display-$device.txt"
echo "::endgroup::"

echo "::group::Install the debug app and its test APK"
for apk in "$APP_APK" "$TEST_APK"; do
  if [ ! -f "$apk" ]; then echo "::error::$apk is missing; assemble it before the emulator boots."; exit 1; fi
  adb install -r -t -g "$apk"
done
# Read into a variable first: under pipefail, grep -q leaving a pipe early can fail it by SIGPIPE.
instrumentations=$(adb shell pm list instrumentation | tr -d '\r')
if ! grep -qF "$RUNNER" <<< "$instrumentations"; then
  echo "::error::$RUNNER is not installed. The device lists:"
  echo "$instrumentations"
  exit 1
fi
# No stale capture from an earlier run on this AVD can stand in for a fresh one.
adb shell rm -rf "$REMOTE" > /dev/null 2>&1 || true
echo "::endgroup::"

echo "::group::Quiet system"
# A software-rendered emulator is still busy for a while after boot; SystemUI that is starved then
# raises "System UI isn't responding" over whatever is on screen. Let it settle, and keep error
# dialogs off the screen (CaptureTest also closes any that appear before it shoots).
sleep 45
adb shell settings put global hide_error_dialogs 1 || true
adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS > /dev/null 2>&1 || true
# Nothing here counts as user activity, and a screen that sleeps mid-run stops the Activity.
adb shell svc power stayon true > /dev/null 2>&1 || true
adb shell input keyevent KEYCODE_WAKEUP > /dev/null 2>&1 || true
echo "::endgroup::"

if [ "$device" = phone ]; then
  capture phone

  # 200% is the largest font step Android 14 offers, and where labels beside values wrap or clip
  # first. The emulator is thrown away after this, but leave the setting as found regardless.
  trap 'adb shell settings put system font_scale 1.0 > /dev/null 2>&1 || true' EXIT
  adb shell settings put system font_scale 2.0
  # The configuration change reaches the system asynchronously; each capture starts a fresh app
  # process, which then reads the new scale.
  sleep 2
  echo "font_scale is $(adb shell settings get system font_scale | tr -d '\r')"
  capture phone-200
  adb shell settings put system font_scale 1.0
  trap - EXIT
else
  capture tablet
fi

echo "::group::Pull the captures"
for name in "${taken[@]}"; do
  adb pull "$REMOTE/$name.png" "$OUT/$name.png"
  if [ ! -s "$OUT/$name.png" ]; then echo "::error::$name.png is empty."; exit 1; fi
done
ls -l "$OUT"
echo "::endgroup::"
