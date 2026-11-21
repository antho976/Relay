#!/usr/bin/env python3
"""Refuse to cut a release whose version numbers disagree with each other or with the tag.

Three facts have to line up and nothing in the build enforces any of them:

  * The git tag and :app's versionName. A tag that says v0.9 while the APK inside says 0.8.8 is a
    release nobody can later identify from a crash report.
  * :app and :wear versionName. The two APKs go out under ONE Play listing; a user seeing "0.9" on
    the phone and "0.8.8" on the watch has no way to tell which half is stale.
  * :wear versionCode == :app versionCode + 100_000. That offset is the scheme wear/build.gradle.kts
    documents in a comment, and Play requires the codes on one listing to be distinct. Get it wrong
    and the upload is rejected AFTER the whole pipeline has run, or — worse — accepted with the
    watch APK ordered ahead of a phone release it predates.

Usage:  release_version_check.py v0.9
"""

from __future__ import annotations

import re
import sys

APP = "forge-android/app/build.gradle.kts"
WEAR = "forge-android/wear/build.gradle.kts"

WEAR_VERSION_CODE_OFFSET = 100_000


def read(path: str, key: str) -> str:
    text = open(path, encoding="utf-8").read()
    # Kotlin numeric literals may carry underscores (100_089); strip them for the comparison.
    match = re.search(rf'^\s*{key}\s*=\s*"?([0-9A-Za-z._-]+)"?\s*$', text, re.MULTILINE)
    if not match:
        sys.exit(f"Could not find {key} in {path}")
    return match.group(1).replace("_", "")


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        sys.exit(f"usage: {argv[0]} <tag>")
    tag = argv[1]
    expected = tag[1:] if tag.startswith("v") else tag

    app_name = read(APP, "versionName")
    app_code = int(read(APP, "versionCode"))
    wear_name = read(WEAR, "versionName")
    wear_code = int(read(WEAR, "versionCode"))

    problems: list[str] = []
    if app_name != expected:
        problems.append(
            f"tag {tag} implies versionName {expected!r}, but {APP} says {app_name!r}"
        )
    if wear_name != app_name:
        problems.append(
            f"versionName differs across modules: app {app_name!r} vs wear {wear_name!r}"
        )
    if wear_code != app_code + WEAR_VERSION_CODE_OFFSET:
        problems.append(
            f"wear versionCode should be app + {WEAR_VERSION_CODE_OFFSET} "
            f"({app_code + WEAR_VERSION_CODE_OFFSET}), but it is {wear_code}"
        )

    if problems:
        for problem in problems:
            print(f"::error::{problem}")
        return 1

    print(f"Version check passed: {tag} -> {app_name} (app {app_code}, wear {wear_code})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
