#!/usr/bin/env python3
"""Decide whether a PR needs the build jobs, or touches only files CI has nothing to say about.

The build jobs are required checks on main. A workflow that `paths-ignore` skips never reports
them, so a docs-only PR would wait forever for a check that will never start. Instead the workflow
always runs on a PR, and this decides whether Verify and Instrumented do any work. A job skipped by
its own `if:` reports as passed, which is exactly right for a PR that changes no build input.

Fails closed: anything unexpected means "build".
"""
import fnmatch
import subprocess
import sys

# Mirrors the push trigger's paths-ignore in ci.yml. Keep the two lists in step.
IGNORED = (
    "docs/**",
    "backups/**",
    ".design-backups/**",
    ".impeccable/**",
    "remotion-*/**",
    # Repository-root media only: a pattern without "/" matches the top level and nothing else, so
    # the Roborazzi goldens under forge-android/ still build.
    "*.png",
    "*.jpg",
    "*.mp4",
    "PRODUCT.md",
    "README.md",
)


def is_ignored(path):
    for pattern in IGNORED:
        if pattern.endswith("/**"):
            prefix = pattern[: -len("**")]
            top = path.split("/", 1)[0] + "/"
            if "/" in path and fnmatch.fnmatchcase(top, prefix):
                return True
        elif "/" not in path and fnmatch.fnmatchcase(path, pattern):
            return True
    return False


def needs_build(paths):
    # An empty diff is not evidence of anything: build.
    return not paths or any(not is_ignored(p) for p in paths)


def changed_paths():
    # HEAD is the PR's synthetic merge commit; its first parent is the base it merges into, so this
    # diff is exactly what the PR changes on main. Renames count on both sides.
    out = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "HEAD^1", "HEAD"], text=True
    )
    return [line for line in out.splitlines() if line]


def main():
    try:
        paths = changed_paths()
    except (subprocess.CalledProcessError, OSError) as error:
        print(f"Could not list changed files ({error}); building.")
        return "true"
    build = needs_build(paths)
    print(f"{len(paths)} changed file(s); build: {build}.")
    return "true" if build else "false"


if __name__ == "__main__":
    result = main()
    if len(sys.argv) > 1:
        with open(sys.argv[1], "a") as out:
            out.write(f"build={result}\n")
