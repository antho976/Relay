#!/bin/sh
#
# Delegating Gradle wrapper.
#
# The Gradle project for this repo lives in the forge-android/ subdirectory,
# but tools (e.g. the ADE "Run on device") look for ./gradlew at the tree root
# — the git worktree root. This shim forwards every invocation to the real
# wrapper inside forge-android/ so those tools work unchanged.
#
APP_HOME=$(cd "$(dirname "$0")" && pwd)

# local.properties is gitignored, so a fresh worktree has no
# forge-android/local.properties and gradle cannot find the SDK. The ADE copies
# the primary checkout's file to the *tree root*, which gradle never reads — so
# take sdk.dir from whichever copy this tree has and export it instead. Only
# when neither SDK env var is already set; those win by design.
if [ -z "$ANDROID_HOME" ] && [ -z "$ANDROID_SDK_ROOT" ]; then
    for f in "$APP_HOME/forge-android/local.properties" "$APP_HOME/local.properties"; do
        [ -f "$f" ] || continue
        sdk=$(sed -n 's/^[ 	]*sdk\.dir[ 	]*=[ 	]*//p' "$f" | tr -d '\r' | tail -n 1)
        if [ -n "$sdk" ]; then
            ANDROID_HOME=$sdk
            export ANDROID_HOME
            break
        fi
    done
fi

cd "$APP_HOME/forge-android" || exit 1
exec ./gradlew "$@"
