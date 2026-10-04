#!/usr/bin/env bash
# What runs against a booted emulator. Two claims the JVM cannot answer:
#  1. The real UI flow works on a device with LeakCanary watching: after every test, a retained
#     Activity, ViewModel, Fragment or View fails that test (DetectLeaksAfterTestSuccess).
#  2. The MINIFIED release APK launches and stays up. "R8 finished" is not "R8 produced an app".
set -euo pipefail

APP_ID=com.quietsoftware.margin
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
adb logcat -c
adb shell monkey -p "$APP_ID" -c android.intent.category.LAUNCHER 1 || true
up=0
for _ in $(seq 1 25); do
  if adb shell pidof "$APP_ID" > /dev/null 2>&1 && adb shell dumpsys window 2>/dev/null | grep "mCurrentFocus.*$APP_ID" > /dev/null; then
    up=1; break
  fi
  sleep 1
done
sleep 3
adb logcat -d > "$LOGCAT" 2>/dev/null || true
if [ "$up" -ne 1 ]; then
  echo "::error::$APP_ID never reached the foreground."; tail -200 "$LOGCAT"; exit 1
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
