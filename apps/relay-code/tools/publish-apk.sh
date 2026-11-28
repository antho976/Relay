#!/usr/bin/env bash
# Puts a Relay Code APK where the PC serves it to the paired phone (phone.app.latest and
# phone.app.apk, BUS.md §10.21): the instance's data folder, beside its store.
#
#   apps/relay-code/tools/publish-apk.sh <apk> [instance]
#   apps/relay-code/tools/publish-apk.sh android/build/outputs/apk/release/app-release.apk dev
#
# The instance is `stable` unless named (`dev`, `stable`); its folder is
# ${XDG_DATA_HOME:-~/.local/share}/relay-v4/<instance>/phone/. The build number is the APK's own
# versionCode when aapt can read it, else RELAY_CODE_BUILD, else the commit count (what
# tools/build-lib.sh and Gradle build with). Writes latest.json ({build, version, size, sha256,
# made_at}) beside the APK. The APK goes first, and each file lands under a temporary name before
# it is renamed, so the phone never reads a latest.json naming a half-copied APK. Nothing to
# restart: the phone asks on its next check (on open, on return to the front, every 15 minutes).
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "usage: $0 <apk> [instance]" >&2
    exit 2
fi
apk=$1
instance=${2:-stable}
[[ -f $apk ]] || { echo "$apk: no such file" >&2; exit 2; }
[[ $instance =~ ^[a-z]+$ ]] || { echo "instance: lowercase letters only: $instance" >&2; exit 2; }

build=""
aapt=$(ls -d "${ANDROID_HOME:-$HOME/Android/Sdk}"/build-tools/*/aapt 2>/dev/null | sort -V | tail -1 || true)
if [[ -n $aapt ]]; then
    build=$("$aapt" dump badging "$apk" 2>/dev/null | sed -n "s/.*versionCode='\([0-9]*\)'.*/\1/p" | head -1 || true)
fi
build=${build:-${RELAY_CODE_BUILD:-$(git -C "$(dirname "$0")" rev-list --count HEAD)}}
[[ $build =~ ^[0-9]+$ ]] || { echo "build must be a whole number: $build" >&2; exit 2; }
version="0.$build"

dir="${XDG_DATA_HOME:-$HOME/.local/share}/relay-v4/$instance/phone"
size=$(stat -c %s "$apk")
sha=$(sha256sum "$apk" | cut -d' ' -f1)
made_at=$(( $(date +%s) * 1000 ))

mkdir -p "$dir"
cp "$apk" "$dir/relay-code.apk.part"
printf '{"build":%s,"version":"%s","size":%s,"sha256":"%s","made_at":%s}\n' "$build" "$version" "$size" "$sha" "$made_at" > "$dir/latest.json.part"
mv "$dir/relay-code.apk.part" "$dir/relay-code.apk"
mv "$dir/latest.json.part" "$dir/latest.json"
echo "Relay Code $version (build $build): $size bytes, sha256 $sha"
echo "published to $dir"
