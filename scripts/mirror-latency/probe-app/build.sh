#!/usr/bin/env bash
# Build the latency probe APK with the Android SDK's command-line tools: no Gradle, no project.
# Usage: build.sh [out-dir]   (default: target/mirror-latency/probe in the checkout, ignored by git)
# Environment: ANDROID_HOME (default ~/Android/Sdk), PROBE_KEYSTORE (default the debug keystore).
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../../.." && pwd)
sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}
platform=$(ls -d "$sdk"/platforms/android-* | sort -V | tail -1)
tools=$(ls -d "$sdk"/build-tools/* | sort -V | tail -1)
out=${1:-$root/target/mirror-latency/probe}
keystore=${PROBE_KEYSTORE:-$HOME/.android/debug.keystore}
[[ -f $keystore ]] || keytool -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -dname "CN=Android Debug,O=Android,C=US" -keyalg RSA -validity 10000 >/dev/null
mkdir -p "$out/classes" "$out/dex"
javac --release 17 -classpath "$platform/android.jar" -d "$out/classes" $(find "$here/src" -name '*.java')
"$tools/d8" --min-api 26 --lib "$platform/android.jar" --output "$out/dex" $(find "$out/classes" -name '*.class')
"$tools/aapt2" link -o "$out/unsigned.apk" --manifest "$here/AndroidManifest.xml" -I "$platform/android.jar"
python3 -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1], "a").write(sys.argv[2], "classes.dex")' "$out/unsigned.apk" "$out/dex/classes.dex"
"$tools/zipalign" -f -p 4 "$out/unsigned.apk" "$out/aligned.apk"
"$tools/apksigner" sign --ks "$keystore" --ks-pass pass:android --key-pass pass:android --out "$out/latprobe.apk" "$out/aligned.apk"
echo "$out/latprobe.apk"
