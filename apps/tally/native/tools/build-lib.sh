#!/usr/bin/env bash
# Build libtally.so for Android into android/src/main/jniLibs/<abi>/, for the Gradle shell to pack.
#
#   tools/build-lib.sh [abi ...]     abis: arm64-v8a (default) x86_64
#
# Needs a Rust toolchain with the Android standard library. The Arch rust package has none, so it
# uses the official one in $TALLY_RUST (default ~/.local/share/rust-android: rustc, cargo and
# rust-std for x86_64-linux-gnu and aarch64-linux-android from static.rust-lang.org), and the NDK
# under $ANDROID_HOME (default ~/Android/Sdk).
set -euo pipefail
cd "$(dirname "$0")/.."

RUST="${TALLY_RUST:-$HOME/.local/share/rust-android}"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
BIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
API=26
ABIS=("$@"); [ ${#ABIS[@]} -eq 0 ] && ABIS=(arm64-v8a)

for abi in "${ABIS[@]}"; do
  case "$abi" in
    arm64-v8a) triple=aarch64-linux-android ;;
    x86_64)    triple=x86_64-linux-android ;;
    *) echo "unknown abi $abi" >&2; exit 2 ;;
  esac
  var=$(echo "$triple" | tr 'a-z-' 'A-Z_')
  low=${triple//-/_}
  env PATH="$RUST/bin:$PATH" \
      "CARGO_TARGET_${var}_LINKER=$BIN/$triple$API-clang" \
      "CC_$low=$BIN/$triple$API-clang" "CXX_$low=$BIN/$triple$API-clang++" "AR_$low=$BIN/llvm-ar" \
      cargo build --release -p tally-rs --lib --target "$triple"
  mkdir -p "android/src/main/jniLibs/$abi"
  cp "target/$triple/release/libtally.so" "android/src/main/jniLibs/$abi/"
  ls -la "android/src/main/jniLibs/$abi/"
done
