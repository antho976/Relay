#!/usr/bin/env python3
"""Turn the JUnit XML Gradle leaves behind into a Markdown job summary.

Without this, the only record of WHICH of a thousand-odd tests failed is a zip artifact you have to
download and open in a browser. The run page says "Verify (JVM) failed" and nothing else, so the
first thing anyone does on a red build is re-run it locally to find out what broke — which is the
one thing CI was supposed to save them.

Reads nothing but files on disk and writes Markdown to stdout, so it is equally usable locally:

    python3 .github/scripts/test_summary.py
"""

from __future__ import annotations

import glob
import os
import sys
import xml.etree.ElementTree as ET

# Gradle writes one directory per test task; the module name is the second path segment.
RESULT_GLOBS = (
    "forge-android/*/build/test-results/*/TEST-*.xml",
    "forge-android/*/build/outputs/androidTest-results/**/TEST-*.xml",
)

# A stack trace in a job summary is unreadable and blows the 1 MB summary budget. The first lines
# carry the assertion message, which is the part that names what actually broke — the design
# doctrine suite in particular prints the exact allowlist edit to make.
MESSAGE_LINES = 12
MAX_FAILURES_SHOWN = 25


def module_of(path: str) -> str:
    parts = path.split(os.sep)
    return parts[1] if len(parts) > 2 else "?"


def collect() -> tuple[dict[str, list[int]], list[tuple[str, str, str]]]:
    """Returns per-module [tests, failures, skipped] totals, and every failure's detail."""
    totals: dict[str, list[int]] = {}
    failures: list[tuple[str, str, str]] = []

    seen: set[str] = set()
    for pattern in RESULT_GLOBS:
        for path in glob.glob(pattern, recursive=True):
            if path in seen:
                continue
            seen.add(path)
            try:
                root = ET.parse(path).getroot()
            except ET.ParseError:
                # A truncated XML means the test JVM died mid-write — worth saying so rather than
                # reporting a smaller, cheerier number.
                #
                # It also has to COUNT. This used to append the detail line without touching the
                # totals, so a run whose test JVM crashed printed the headline "Tests — passed" with
                # the broken suite listed underneath it: the two halves of the same summary
                # contradicting each other, and the reassuring half on top. An unreadable result is
                # the strongest possible statement that the suite's verdict is unknown, so it is
                # counted as a failure of the module it belongs to.
                totals.setdefault(module_of(path), [0, 0, 0])[1] += 1
                failures.append((module_of(path), os.path.basename(path),
                                 "Unreadable result file — the test JVM likely crashed."))
                continue

            suites = [root] if root.tag == "testsuite" else root.iter("testsuite")
            for suite in suites:
                module = module_of(path)
                row = totals.setdefault(module, [0, 0, 0])
                row[0] += int(suite.get("tests", 0))
                row[1] += int(suite.get("failures", 0)) + int(suite.get("errors", 0))
                row[2] += int(suite.get("skipped", 0))

                for case in suite.iter("testcase"):
                    for bad in list(case.findall("failure")) + list(case.findall("error")):
                        name = f'{case.get("classname", "?")} > {case.get("name", "?")}'
                        message = (bad.get("message") or bad.text or "").strip()
                        trimmed = "\n".join(message.splitlines()[:MESSAGE_LINES])
                        failures.append((module, name, trimmed))
    return totals, failures


COVERAGE_XML = "forge-android/app/build/reports/jacoco/coverageReport/coverageReport.xml"

# Reported per area rather than as one number, because one number for this repo is meaningless: the
# domain layer sits near 90% while the Compose UI sits near zero — and the UI is not untested, it is
# covered by 41 screenshot goldens and a design-doctrine scan, neither of which executes lines. A
# single blended figure hides both facts and invites someone to "improve" it by testing screens the
# wrong way.
COVERAGE_AREAS = ("domain", "data", "program", "service", "core", "ui")


def coverage_section() -> list[str]:
    if not os.path.exists(COVERAGE_XML):
        return []
    try:
        root = ET.parse(COVERAGE_XML).getroot()
    except ET.ParseError:
        return []

    def lines(element) -> tuple[int, int]:
        for counter in element.findall("counter"):
            if counter.get("type") == "LINE":
                return int(counter.get("covered", 0)), int(counter.get("missed", 0))
        return 0, 0

    per_area: dict[str, list[int]] = {area: [0, 0] for area in COVERAGE_AREAS}
    for package in root.findall("package"):
        name = package.get("name", "").replace("com/forge/app/", "")
        area = name.split("/")[0]
        if area not in per_area:
            continue
        covered, missed = lines(package)
        per_area[area][0] += covered
        per_area[area][1] += missed

    covered, missed = lines(root)
    total = covered + missed
    if total == 0:
        return []

    out = ["", "### Coverage", "",
           f"**{100 * covered / total:.1f}%** of {total:,} lines "
           f"(JVM unit tests only — the screenshot goldens execute no lines)", "",
           "| Area | Lines | Covered |", "| --- | ---: | ---: |"]
    for area in COVERAGE_AREAS:
        area_covered, area_missed = per_area[area]
        area_total = area_covered + area_missed
        if area_total:
            out.append(f"| `{area}` | {area_total:,} | {100 * area_covered / area_total:.1f}% |")
    return out


def main() -> int:
    totals, failures = collect()

    if not totals:
        print("### Tests\n")
        print("No test results were produced — the run failed before any test task started.")
        return 0

    tests = sum(v[0] for v in totals.values())
    failed = sum(v[1] for v in totals.values())
    skipped = sum(v[2] for v in totals.values())

    print(f'### Tests — {"FAILED" if failed else "passed"}\n')
    print(f"**{tests}** tests · **{failed}** failed · **{skipped}** skipped\n")
    print("| Module | Tests | Failed | Skipped |")
    print("| --- | ---: | ---: | ---: |")
    for module in sorted(totals):
        t, f, s = totals[module]
        print(f"| `{module}` | {t} | {f} | {s} |")

    if failures:
        print(f"\n### Failures ({len(failures)})\n")
        for module, name, message in failures[:MAX_FAILURES_SHOWN]:
            print(f"<details><summary><code>{module}</code> — {name}</summary>\n")
            print("```")
            print(message or "(no message)")
            print("```")
            print("</details>\n")
        if len(failures) > MAX_FAILURES_SHOWN:
            print(f"_...and {len(failures) - MAX_FAILURES_SHOWN} more — see the `test-reports` artifact._")

    for line in coverage_section():
        print(line)

    # Non-zero when anything failed. The Gradle step above this one already fails the job on a
    # normal test failure, so in practice this fires for the case Gradle CANNOT see: a result file
    # it wrote and then could not finish, from a JVM that died with an exit code Gradle read as
    # success. The summary step runs `if: always()`, so this is the last chance for a crashed test
    # JVM to be something other than a green run.
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
