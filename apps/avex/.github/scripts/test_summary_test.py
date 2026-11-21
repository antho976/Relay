#!/usr/bin/env python3
"""Regression tests for test_summary.py.

The summary is the only readable record of what a CI run actually found, which makes a wrong
summary worse than none: it is believed. This suite exists because the script used to report

    ### Tests — passed
    **412** tests · **0** failed · **0** skipped
    ### Failures (1)
    app — TEST-com.forge.app.SomeTest.xml: Unreadable result file …

for a run whose test JVM crashed mid-write — the headline and the detail contradicting each other
in the same document, with the reassuring half on top.

No pytest, no dependencies: the script has none, and a summary generator that needed a package
installed would be one more thing to keep working. Run it directly:

    python3 .github/scripts/test_summary_test.py
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import pathlib
import sys
import tempfile

SCRIPT = pathlib.Path(__file__).with_name("test_summary.py")

_spec = importlib.util.spec_from_file_location("test_summary", SCRIPT)
assert _spec and _spec.loader
test_summary = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(test_summary)


PASSING_XML = """<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="com.forge.app.GoodTest" tests="3" skipped="0" failures="0" errors="0">
  <testcase name="a" classname="com.forge.app.GoodTest"/>
  <testcase name="b" classname="com.forge.app.GoodTest"/>
  <testcase name="c" classname="com.forge.app.GoodTest"/>
</testsuite>
"""

FAILING_XML = """<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="com.forge.app.BadTest" tests="2" skipped="0" failures="1" errors="0">
  <testcase name="a" classname="com.forge.app.BadTest"/>
  <testcase name="b" classname="com.forge.app.BadTest">
    <failure message="expected:&lt;1&gt; but was:&lt;2&gt;">stack</failure>
  </testcase>
</testsuite>
"""

# What a JVM that died mid-write leaves behind: a well-formed prologue and no closing tag.
TRUNCATED_XML = """<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="com.forge.app.CrashedTest" tests="9" skipped="0" failures="0" errors="0">
  <testcase name="a" classname="com.forge.app.CrashedTest"/>
"""


@contextlib.contextmanager
def results(*files: tuple[str, str]):
    """Run the script against a throwaway tree holding exactly `files`."""
    previous = os.getcwd()
    with tempfile.TemporaryDirectory() as tmp:
        directory = pathlib.Path(tmp) / "forge-android" / "app" / "build" / "test-results" / "testDebugUnitTest"
        directory.mkdir(parents=True)
        for name, body in files:
            (directory / name).write_text(body, encoding="utf-8")
        os.chdir(tmp)
        try:
            yield
        finally:
            os.chdir(previous)


def run() -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = test_summary.main()
    return code, out.getvalue()


def check(condition: bool, what: str) -> None:
    if not condition:
        raise AssertionError(what)
    print(f"  ok  {what}")


def test_clean_run_passes() -> None:
    with results(("TEST-com.forge.app.GoodTest.xml", PASSING_XML)):
        code, text = run()
    check(code == 0, "a clean run exits 0")
    check("Tests — passed" in text, "a clean run is headed 'passed'")
    check("**3** tests · **0** failed" in text, "a clean run counts its tests")


def test_ordinary_failure_is_counted() -> None:
    with results(("TEST-com.forge.app.BadTest.xml", FAILING_XML)):
        code, text = run()
    check(code == 1, "a failing run exits 1")
    check("Tests — FAILED" in text, "a failing run is headed 'FAILED'")
    check("**1** failed" in text, "a failing run counts the failure")


def test_unreadable_result_is_a_failure() -> None:
    """The regression this file exists for."""
    with results(("TEST-com.forge.app.CrashedTest.xml", TRUNCATED_XML)):
        code, text = run()
    check(code == 1, "a truncated result file exits 1")
    check("Tests — FAILED" in text, "a truncated result file is headed 'FAILED', not 'passed'")
    check("**1** failed" in text, "a truncated result file counts toward the failed total")
    check("Unreadable result file" in text, "a truncated result file still names itself")


def test_unreadable_result_does_not_hide_behind_passing_ones() -> None:
    with results(
        ("TEST-com.forge.app.GoodTest.xml", PASSING_XML),
        ("TEST-com.forge.app.CrashedTest.xml", TRUNCATED_XML),
    ):
        code, text = run()
    check(code == 1, "one broken file among passing ones still exits 1")
    check("Tests — FAILED" in text, "one broken file among passing ones still reads FAILED")
    check("**3** tests · **1** failed" in text,
          "the broken file is counted without inventing tests it never reported")


def test_no_results_at_all() -> None:
    with results():
        code, text = run()
    check(code == 0, "no results at all exits 0 — the Gradle step already failed the job")
    check("No test results were produced" in text, "no results at all says so")


def main() -> int:
    # `startswith("test_")` alone would also pick up the imported `test_summary` module.
    tests = [
        value for name, value in sorted(globals().items())
        if name.startswith("test_") and callable(value) and getattr(value, "__module__", None) == __name__
    ]
    for test in tests:
        print(f"{test.__name__}:")
        test()
    print(f"\n{len(tests)} tests passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
