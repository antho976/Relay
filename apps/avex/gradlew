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
cd "$APP_HOME/forge-android" || exit 1
exec ./gradlew "$@"
