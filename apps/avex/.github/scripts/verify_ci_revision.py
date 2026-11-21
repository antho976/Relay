#!/usr/bin/env python3
"""Fail closed when a PR run checks out a tree from another revision, including shallow clones."""
import subprocess
import sys


def contains_pr_head(expected, head, commit):
    # Read raw commit headers: rev-list/show may hide parents at a shallow checkout boundary.
    headers = commit.split("\n\n", 1)[0].splitlines()
    parents = [line.removeprefix("parent ") for line in headers if line.startswith("parent ")]
    return bool(expected) and (expected == head or expected in parents)


def main():
    expected = sys.argv[1]
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    commit = subprocess.check_output(["git", "cat-file", "-p", "HEAD"], text=True)
    if not contains_pr_head(expected, head, commit):
        print(f"::error::Checked out {head}, which does not contain the requested PR head {expected}.")
        return 1
    print(f"Checkout {head} contains PR head {expected}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
