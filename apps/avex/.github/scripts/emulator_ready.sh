#!/usr/bin/env bash
# Sourced by instrumented.sh. An ADB connection is not proof that Android's framework is ready:
# https://developer.android.com/tools/adb#devicestatus

wait_for_android_framework() {
  local budget=${1:-120} deadline=$((SECONDS + ${1:-120})) remaining boot package activity
  local stable=0
  echo "Waiting up to ${budget}s for Android boot, package manager and activity service."
  while ((SECONDS < deadline)); do
    remaining=$((deadline - SECONDS))
    boot=$(timeout "${remaining}s" adb shell getprop sys.boot_completed 2>/dev/null) || boot=""
    remaining=$((deadline - SECONDS))
    ((remaining > 0)) || break
    package=$(timeout "${remaining}s" adb shell pm path android 2>/dev/null) || package=""
    remaining=$((deadline - SECONDS))
    ((remaining > 0)) || break
    activity=$(timeout "${remaining}s" adb shell service check activity 2>/dev/null) || activity=""
    if [[ ${boot//$'\r'/} == 1 && $package == package:* && $activity == *"Service activity: found"* ]]; then
      stable=$((stable + 1))
      if ((stable >= 2)); then
        echo "Android framework is ready."
        return 0
      fi
    else
      stable=0
    fi
    sleep 1
  done
  echo "::error::Android framework did not become ready within ${budget}s."
  return 1
}

install_release_apk() {
  local apk=$1 output status
  wait_for_android_framework || return 1
  if output=$(timeout 120s adb install -r -d "$apk" 2>&1); then
    printf '%s\n' "$output"
    return 0
  else
    status=$?
  fi
  printf '%s\n' "$output"
  # The framework can restart between the readiness probe and install. Retry exactly once for
  # that observed infrastructure failure. Signature, version, parse and other APK errors stay red.
  if [[ $output != *"Can't find service: package"* ]]; then
    return "$status"
  fi
  echo "Package service disappeared during install; waiting before one retry."
  wait_for_android_framework || return 1
  timeout 120s adb install -r -d "$apk"
}

capture_emulator_failure() {
  echo "::group::Emulator state at failure"
  timeout 10s adb devices -l || true
  timeout 10s adb shell getprop sys.boot_completed || true
  timeout 10s adb shell service check package || true
  timeout 10s adb shell service check activity || true
  # Capture even when failure precedes the smoke launch, as the original package-service failure did.
  timeout 10s adb logcat -d -t 400 >> "${LOGCAT:-smoke-logcat.txt}" 2>&1 || true
  echo "::endgroup::"
}
