#!/usr/bin/env bash
#
# What runs against a booted emulator. Two claims, neither answerable on the JVM:
#
#  1. The Room migration chain still works. MigrationTest is the only androidTest in the repo and
#     the only automated guard on the migration path DatabaseModule deliberately refuses to fall
#     back destructively from. A wrong migration means "every existing user crashes on launch while
#     a fresh install works fine" — the one class of bug that is invisible to whoever ships it.
#
#  2. The MINIFIED apk actually runs. assembleRelease proves R8 finished, not that what it produced
#     starts. Every R8-only defect (an over-eager keep rule, an enum name that survives on one side
#     of the wear wire and not the other) is invisible until the shrunk APK is launched.
#
# Everything is built before the emulator boots — see the workflow — so this script only installs,
# launches and judges.

set -euo pipefail

# The RELEASE applicationId. Note it is NOT the Kotlin namespace (com.forge.app), and not the debug
# variant (…​.avex.debug): getting this wrong is how a smoke test passes by never finding the app it
# was supposed to be watching.
APP_ID=com.quietsoftware.avex
LOGCAT=smoke-logcat.txt

echo "::group::Room migration test"
# Guarded rather than bare so a failure can say WHAT failed. Under `set -e` a bare call aborts the
# script here, and the only record of which test threw and why is the raw job log — where it sits
# above ~180 lines of Gradle cache bookkeeping, or inside an artifact you have to download and
# unzip. The Verify job already ends with a digest for exactly this reason; this is the same debt
# in the slowest job in the pipeline, where it costs the most.
if ! ./forge-android/gradlew -p forge-android :app:connectedDebugAndroidTest; then
  echo "::endgroup::"
  echo "::group::What failed"
  # The connected-test XML is the authoritative record. Grouped by exception rather than listed per
  # test: every migration case calls createDatabase, so one broken schema read fails all ~20 of them
  # with the same throwable, and twenty copies of it hide the one line that matters.
  python3 - <<'DIGEST' || echo "(no parseable test XML — see the Gradle output above)"
import glob, collections, xml.etree.ElementTree as ET

buckets = collections.defaultdict(list)
for path in glob.glob("forge-android/app/build/outputs/androidTest-results/**/*.xml", recursive=True):
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        continue
    for case in root.iter("testcase"):
        for bad in list(case.findall("failure")) + list(case.findall("error")):
            raw = bad.get("message") or bad.text or ""
            lines = [ln.strip() for ln in raw.strip().splitlines() if ln.strip()]
            head = lines[0][:300] if lines else "unknown failure"
            buckets[head].append("%s.%s" % (case.get("classname", "?"), case.get("name", "?")))

if not buckets:
    print("No failing testcase found in the XML. The run may have died before any test reported.")
for head, tests in sorted(buckets.items(), key=lambda kv: -len(kv[1])):
    print("%d test(s): %s" % (len(tests), head))
    for t in sorted(tests)[:5]:
        print("    %s" % t)
    if len(tests) > 5:
        print("    ... and %d more" % (len(tests) - 5))
    print()
DIGEST
  echo "::endgroup::"
  exit 1
fi
echo "::endgroup::"

echo "::group::Install the minified release APK"
# -not -name '*unsigned*': an earlier unsigned build leaves app-release-unsigned.apk beside the
# signed one, and installing that fails with a parse error that reads like a device fault.
apk=$(find forge-android/app/build/outputs/apk/release -name '*.apk' -not -name '*unsigned*' -print -quit)
if [ -z "$apk" ]; then
  echo "::error::No signed release APK was produced — nothing to smoke test."
  ls -la forge-android/app/build/outputs/apk/release || true
  exit 1
fi
echo "APK: $apk"

# Refuse to continue on an unsigned APK rather than letting adb fail with INSTALL_PARSE_FAILED_
# NO_CERTIFICATES, which reads like a device problem. Unsigned here means -PforgeCiSmokeSigning
# did not take effect, and the smoke test would be silently skipped for as long as that lasted.
apksigner=$(find "$ANDROID_HOME/build-tools" -maxdepth 2 -name apksigner | sort -V | tail -1)
if [ -n "$apksigner" ]; then
  "$apksigner" verify --print-certs "$apk" > /dev/null || {
    echo "::error::$apk is not signed. -PforgeCiSmokeSigning did not apply."
    exit 1
  }
  echo "Signature present."
fi

adb install -r -d "$apk"
echo "::endgroup::"

echo "::group::Cold-launch the release build"
adb logcat -c
# monkey rather than `am start -n <component>`: MainActivity carries NO MAIN/LAUNCHER filter — the
# filter lives on the .icon.* activity-aliases so the user can swap the home-screen icon. Naming the
# activity directly launches a component the launcher would never use, and hardcodes a class the
# alias indirection exists to keep loose.
adb shell monkey -p "$APP_ID" -c android.intent.category.LAUNCHER 1 || true

# Foreground detection, two ways, because the field name differs across platform versions and the
# component reported is the ALIAS (com.forge.app.icon.Default), never MainActivity — matching on the
# activity class would fail on a launch that worked perfectly.
is_foreground() {
  adb shell dumpsys window 2>/dev/null | grep -q "mCurrentFocus.*$APP_ID" ||
    adb shell dumpsys activity activities 2>/dev/null |
      grep -qE "(mResumedActivity|topResumedActivity).*$APP_ID"
}

# Give it a real chance to get past its first frame before judging. A crash-on-launch usually lands
# within a couple of seconds; the loop exits early once the app is both alive and frontmost.
resumed=0
for _ in $(seq 1 20); do
  if adb shell pidof "$APP_ID" > /dev/null 2>&1 && is_foreground; then
    resumed=1
    break
  fi
  sleep 1
done

adb logcat -d > "$LOGCAT" 2>/dev/null || true

if [ "$resumed" -ne 1 ]; then
  echo "::error::$APP_ID never reached the foreground after launch."
  echo "--- last 200 lines of logcat ---"
  tail -200 "$LOGCAT"
  exit 1
fi

# Alive is not the same as healthy: a caught-and-swallowed startup failure can leave a running
# process behind an empty screen. A FATAL EXCEPTION in the log is decisive either way.
if grep -q "FATAL EXCEPTION" "$LOGCAT"; then
  echo "::error::A fatal exception was logged during the release smoke launch."
  grep -A 30 "FATAL EXCEPTION" "$LOGCAT" | head -60
  exit 1
fi

echo "Release build launched and stayed up."
echo "::endgroup::"
