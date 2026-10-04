#!/usr/bin/env python3
"""The reporter's own test: it must never call a broken suite a pass."""
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(__file__))
import test_summary as ts  # noqa: E402

GOOD = '<testsuite><testcase classname="a.B" name="x"/><testcase classname="a.B" name="y"/></testsuite>'
BAD = '<testsuite><testcase classname="a.B" name="x"><failure message="boom">trace</failure></testcase></testsuite>'
TRUNCATED = '<testsuite><testcase classname="a.B" name="x">'


def run(*docs):
    with tempfile.TemporaryDirectory() as d:
        paths = []
        for i, doc in enumerate(docs):
            p = os.path.join(d, f"TEST-{i}.xml")
            with open(p, "w") as f:
                f.write(doc)
            paths.append(p)
        return ts.render("app", *ts.summarize(paths))


def test_passes_good():
    text, ok = run(GOOD)
    assert ok, text


def test_fails_on_failure():
    text, ok = run(GOOD, BAD)
    assert not ok and "boom" in text, text


def test_fails_on_truncated_file():
    text, ok = run(GOOD, TRUNCATED)
    assert not ok and "unreadable" in text, text


def test_fails_when_nothing_ran():
    text, ok = run()
    assert not ok and "did not run" in text, text


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
    print("test_summary self check: ok")
