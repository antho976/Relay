#!/usr/bin/env bash
# Puts a Leaf APK on the server, where the phone finds it (GET /v1/app/latest, /v1/app/apk).
#
#   apps/leaf/tools/publish-apk.sh <apk> <build> <version>
#   apps/leaf/tools/publish-apk.sh native/android/build/outputs/apk/release/app-release.apk 42 0.4.2
#
# Writes latest.json ({build, version, size, sha256, made_at}) beside the APK and copies both to
# server1's leaf data folder (mounted at /data in the leaf-server container). The APK goes first
# and each file lands under a temporary name before it is renamed, so the phone never reads a
# latest.json that points at a half-copied APK. LEAF_HOST and LEAF_APP_DIR change where.
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <apk> <build> <version>" >&2
    exit 2
fi
apk=$1 build=$2 version=$3
host=${LEAF_HOST:-server1}
dir=${LEAF_APP_DIR:-apps/leaf/server/data/leaf/app}

[[ -f $apk ]] || { echo "$apk: no such file" >&2; exit 2; }
[[ $build =~ ^[0-9]+$ ]] || { echo "build must be a whole number: $build" >&2; exit 2; }
[[ $version =~ ^[A-Za-z0-9._+-]+$ ]] || { echo "version: letters, digits and . _ + - only: $version" >&2; exit 2; }

size=$(stat -c %s "$apk")
sha=$(sha256sum "$apk" | cut -d' ' -f1)
made_at=$(( $(date +%s) * 1000 ))
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
printf '{"build":%s,"version":"%s","size":%s,"sha256":"%s","made_at":%s}\n' "$build" "$version" "$size" "$sha" "$made_at" > "$tmp/latest.json"

echo "Leaf $version (build $build): $size bytes, sha256 $sha"
ssh "$host" "mkdir -p '$dir'"
scp -q "$apk" "$host:$dir/leaf.apk.part"
scp -q "$tmp/latest.json" "$host:$dir/latest.json.part"
ssh "$host" "cd '$dir' && mv leaf.apk.part leaf.apk && mv latest.json.part latest.json && ls -l leaf.apk latest.json"
echo "published to $host:$dir"
