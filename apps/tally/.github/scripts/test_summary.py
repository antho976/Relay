#!/usr/bin/env python3
"""Turns JUnit XML into a readable verdict for the run page, and is itself a gate.

Usage: test_summary.py <module>   (module = core | app)

Exits non-zero when any test failed OR any result file is unreadable. A test JVM that dies
mid-write leaves a truncated XML behind and can still hand Gradle a success code; a summary that
silently skips that file would report "passed" over a suite it never read.
"""
import glob
import sys
import xml.etree.ElementTree as ET


def summarize(paths):
    total = failed = skipped = 0
    failures = []
    unreadable = []
    for path in sorted(paths):
        try:
            root = ET.parse(path).getroot()
        except ET.ParseError:
            unreadable.append(path)
            continue
        suites = [root] if root.tag == "testsuite" else list(root.iter("testsuite"))
        for suite in suites:
            for case in suite.iter("testcase"):
                total += 1
                if case.find("skipped") is not None:
                    skipped += 1
                bad = case.find("failure")
                if bad is None:
                    bad = case.find("error")
                if bad is not None:
                    failed += 1
                    msg = (bad.get("message") or (bad.text or "")).strip().splitlines()
                    failures.append((case.get("classname", "?"), case.get("name", "?"), msg[0][:240] if msg else ""))
    return total, failed, skipped, failures, unreadable


def render(module, total, failed, skipped, failures, unreadable):
    lines = []
    ok = failed == 0 and not unreadable and total > 0
    lines.append(f"### {module}: " + ("passed" if ok else "FAILED"))
    lines.append(f"{total} tests, {failed} failed, {skipped} skipped")
    if total == 0:
        lines.append("")
        lines.append("No test results were found. The suite did not run.")
    for cls, name, msg in failures[:40]:
        lines.append(f"- `{cls.split('.')[-1]}` {name}: {msg}")
    for path in unreadable:
        lines.append(f"- unreadable result file: `{path}`")
    return "\n".join(lines), ok


def main(argv):
    module = argv[1] if len(argv) > 1 else "app"
    paths = glob.glob(f"{module}/build/test-results/**/*.xml", recursive=True)
    text, ok = render(module, *summarize(paths))
    print(text)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
