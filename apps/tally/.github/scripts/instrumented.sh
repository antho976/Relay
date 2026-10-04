#!/usr/bin/env bash
# What runs against a booted emulator. Two claims the JVM cannot answer:
#  1. The real UI flow works on a device with LeakCanary watching: after every test, a retained
#     Activity, ViewModel, Fragment or View fails that test (DetectLeaksAfterTestSuccess).
#  2. The MINIFIED release APK launches and stays up. "R8 finished" is not "R8 produced an app".
set -euo pipefail

APP_ID=com.quietsoftware.tally
LOGCAT=smoke-logcat.txt

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

wait_for_framework

echo "::group::UI flow + leak detection (connectedDebugAndroidTest)"
if ! ./gradlew :app:connectedDebugAndroidTest; then
  echo "::endgroup::"
  echo "::group::What failed"
  python3 - <<'DIGEST' || true
import glob, xml.etree.ElementTree as ET
for path in glob.glob("app/build/outputs/androidTest-results/**/*.xml", recursive=True):
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        print("unreadable:", path); continue
    for case in root.iter("testcase"):
        for bad in list(case.findall("failure")) + list(case.findall("error")):
            head = ((bad.get("message") or bad.text or "").strip().splitlines() or ["?"])[0][:400]
            print(f"{case.get('classname')}.{case.get('name')}: {head}")
DIGEST
  echo "::endgroup::"
  exit 1
fi
echo "::endgroup::"

echo "::group::Install the minified release APK"
apk=$(find app/build/outputs/apk/release -name '*.apk' -not -name '*unsigned*' -print -quit)
if [ -z "$apk" ]; then echo "::error::No signed release APK."; exit 1; fi
adb install -r -d "$apk"
echo "::endgroup::"

echo "::group::Cold-launch the release build"
# A busy software-rendered emulator can raise "System UI isn't responding" over the app. That
# dialog takes window focus from a healthy app, so keep error dialogs off the screen and ask the
# activity manager which activity is resumed instead of which window has focus.
adb shell settings put global hide_error_dialogs 1 > /dev/null 2>&1 || true
adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS > /dev/null 2>&1 || true
adb logcat -c
adb shell monkey -p "$APP_ID" -c android.intent.category.LAUNCHER 1 || true
resumed() {
  # Read first, then match: under pipefail an early-exiting grep -q can fail the pipe by SIGPIPE.
  local acts
  acts=$(adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' || true)
  grep -qE "(topResumedActivity|ResumedActivity).*$APP_ID/" <<< "$acts"
}
up=0
for _ in $(seq 1 45); do
  if adb shell pidof "$APP_ID" > /dev/null 2>&1 && resumed; then
    up=1; break
  fi
  sleep 1
done
sleep 3
adb logcat -d > "$LOGCAT" 2>/dev/null || true
if [ "$up" -ne 1 ]; then
  echo "::error::$APP_ID never reached the foreground."
  # The cause, wherever it sits in the log: a crash, an ANR, or the app's own lines. A tail would
  # show only the system noise logged after it.
  echo "-- process: $(adb shell pidof "$APP_ID" 2>/dev/null | tr -d '\r' || true)"
  adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' | grep -E "ResumedActivity|mFocusedApp" | head -5 || true
  adb shell dumpsys window 2>/dev/null | tr -d '\r' | grep -E "mCurrentFocus|mFocusedApp" | head -5 || true
  grep -nE "FATAL EXCEPTION|AndroidRuntime|ANR in|$APP_ID|Process: " "$LOGCAT" | head -150 || true
  exit 1
fi
if grep -q "FATAL EXCEPTION" "$LOGCAT"; then
  echo "::error::A fatal exception was logged during the release launch."
  grep -A 30 "FATAL EXCEPTION" "$LOGCAT" | head -60
  exit 1
fi
if ! adb shell pidof "$APP_ID" > /dev/null 2>&1; then
  echo "::error::The release build died after launching."; tail -120 "$LOGCAT"; exit 1
fi
echo "Release build launched and stayed up."
echo "::endgroup::"
