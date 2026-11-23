#!/usr/bin/env bash
# Build libleaf.so for Android into android/src/main/jniLibs/<abi>/, for the Gradle shell to pack.
#
#   tools/build-lib.sh [abi ...]     abis: arm64-v8a (default) x86_64
#
# Needs a Rust toolchain with the Android standard library. The Arch rust package has none, so it
# uses the official one in $LEAF_RUST (default ~/.local/share/rust-android: rustc, cargo and
# rust-std for x86_64-linux-gnu and aarch64-linux-android from static.rust-lang.org), and the NDK
# under $ANDROID_HOME (default ~/Android/Sdk).
set -euo pipefail
cd "$(dirname "$0")/.."

RUST="${LEAF_RUST:-$HOME/.local/share/rust-android}"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
BIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
API=26
ABIS=("$@"); [ ${#ABIS[@]} -eq 0 ] && ABIS=(arm64-v8a)
# The build number the app knows itself by (update.rs reads it with option_env!), the commit count;
# Gradle's versionCode takes the same number. Set LEAF_BUILD to choose another.
export LEAF_BUILD="${LEAF_BUILD:-$(git rev-list --count HEAD)}"
echo "LEAF_BUILD=$LEAF_BUILD"


for abi in "${ABIS[@]}"; do
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android ;;
    x86_64)    triple=x86_64-linux-android ;;
    *) echo "unknown abi $abi" >&2; exit 2 ;;
  esac
  var=$(echo "$triple" | tr 'a-z-' 'A-Z_')
  low=${triple//-/_}
  # 16 KB pages (Android 15+ devices): NDKs before r28 link with 4 KB alignment by default.
  env PATH="$RUST/bin:$PATH" \
      "CARGO_TARGET_${var}_LINKER=$BIN/$triple$API-clang" \
      "CC_$low=$BIN/$triple$API-clang" "CXX_$low=$BIN/$triple$API-clang++" "AR_$low=$BIN/llvm-ar" \
      "CARGO_TARGET_${var}_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384" \
      cargo build --release -p leaf-app --lib --target "$triple"
  mkdir -p "android/src/main/jniLibs/$abi"
  cp "target/$triple/release/libleaf.so" "android/src/main/jniLibs/$abi/"
  ls -la "android/src/main/jniLibs/$abi/"
done

# Gradle packs every ABI under jniLibs: a libleaf.so left from an earlier run for an ABI not
# built now would ship stale. Remove those (only the library this script makes), and the folder
# when that leaves it empty.
for dir in android/src/main/jniLibs/*/; do
  [ -d "$dir" ] || continue
  dir=${dir%/}
  abi=$(basename "$dir")
  keep=false
  for built in "${ABIS[@]}"; do [ "$built" = "$abi" ] && keep=true; done
  if ! $keep && [ -f "$dir/libleaf.so" ]; then
    echo "removing stale $dir/libleaf.so (not built in this run)"
    rm -f "$dir/libleaf.so"
    rmdir "$dir" 2>/dev/null || true
  fi
done
