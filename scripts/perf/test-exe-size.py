#!/usr/bin/env python3
"""A ratchet on what one `cargo test` links (docs/perf/FIXES-2026-10-18.md).

Every integration-test file is an executable of its own, linked against the whole engine: with
full debug info each was ~450 MB, and one `cargo test` wrote 22 GB and held 12 GB of RAM in
its linkers. Line tables only brought them to ~170 MB. This fails when the largest test
executable, or all of them together, grows past its budget: debug info come back, a heavy
dependency, or many new test files (each one is another link of the engine; add tests to an
existing file where one fits). Lower a budget when the number drops.

    python3 scripts/perf/test-exe-size.py                 # the budgets below
    python3 scripts/perf/test-exe-size.py --max-mb 200 --total-gb 10

Builds nothing new after `cargo test`: it asks cargo, which finds everything fresh, for the
executables of the default members.
"""

import argparse
import json
import os
import subprocess
import sys

# 2026-10-18, rust 1.99, dev profile with line tables only: 62 executables, largest 184 MB, all
# 9.5 GB. With full debug info the largest was ~450 MB.
MAX_MB = 210
TOTAL_GB = 11


def executables(extra):
    out = subprocess.run(
        ["cargo", "test", "--no-run", "--message-format=json", *extra],
        check=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
    ).stdout
    for line in out.splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("executable") and message["profile"]["test"]:
            yield message["executable"]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--max-mb", type=float, default=MAX_MB)
    parser.add_argument("--total-gb", type=float, default=TOTAL_GB)
    args, extra = parser.parse_known_args()
    sizes = {path: os.path.getsize(path) for path in set(executables(extra))}
    if not sizes:
        sys.exit("no test executables: run cargo test first")
    largest = max(sizes, key=sizes.get)
    total = sum(sizes.values())
    print(f"{len(sizes)} test executables, {total / 1e9:.1f} GB; largest {os.path.basename(largest)} at {sizes[largest] / 1e6:.0f} MB")
    failed = False
    if sizes[largest] > args.max_mb * 1e6:
        print(f"the largest is over its {args.max_mb:.0f} MB budget", file=sys.stderr)
        failed = True
    if total > args.total_gb * 1e9:
        print(f"together they are over their {args.total_gb:.1f} GB budget", file=sys.stderr)
        failed = True
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
