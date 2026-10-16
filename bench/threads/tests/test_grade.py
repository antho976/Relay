"""The grader, without an engine or a GPU: `python3 -m unittest discover bench/threads`."""

import unittest

from relaybench import grade
from relaybench.grade import numbers


def thread(*texts, calls=(), error=None, timeout=False):
    turns = [{"prompt": f"q{i}", "text": t, "tool_calls": [], "error": None} for i, t in enumerate(texts)]
    if turns:
        turns[-1]["tool_calls"] = list(calls)
    return {"turns": turns, "error": error, "timeout": timeout}


def call(op, inp=None, is_error=False):
    return {"op": op, "name": "mcp__relay__" + op.replace(".", "_"), "input": inp or {}, "is_error": is_error}


class Numbers(unittest.TestCase):
    def test_money_forms(self):
        self.assertEqual(numbers("You spent $1,234.56 this month"), [1234.56])
        self.assertEqual(numbers("1234.56"), [1234.56])
        self.assertEqual(numbers("1,234.56 CAD"), [1234.56])
        self.assertEqual(numbers("**$157.99**"), [157.99])
        self.assertEqual(numbers("CA$99.50"), [99.5])
        self.assertEqual(numbers("$1,234,567.89"), [1234567.89])

    def test_negatives(self):
        self.assertEqual(numbers("-$12.30"), [-12.3])
        self.assertEqual(numbers("$-12.30"), [-12.3])
        self.assertEqual(numbers("−12.30"), [-12.3])
        self.assertEqual(numbers("($12.30)"), [-12.3])

    def test_a_list_item_or_a_range_is_not_a_minus(self):
        self.assertEqual(numbers("- $45.00 groceries"), [45.0])
        self.assertEqual(numbers("10-20 sets"), [10.0, 20.0])
        self.assertEqual(numbers("(see 3 entries)"), [3.0])

    def test_suffixes_and_percent(self):
        self.assertEqual(numbers("$1.2k"), [1200.0])
        self.assertEqual(numbers("about $2.5M"), [2500000.0])
        self.assertEqual(numbers("up 12.50%"), [12.5])
        self.assertEqual(numbers("5 km"), [5.0])

    def test_dates_times_and_chart_json_are_not_numbers(self):
        self.assertEqual(numbers("On 2026-09-18 at 12:00 you had 3 entries"), [3.0])
        self.assertEqual(numbers('```chart\n{"periods": 2}\n```\nSpent $5'), [5.0])

    def test_words_with_digits_are_not_numbers(self):
        self.assertEqual(numbers("v1.2 abc123 Q3"), [])
        self.assertEqual(numbers("42.18."), [42.18])


class NumberCheck(unittest.TestCase):
    def test_tolerance_and_magnitude(self):
        c = {"type": "number", "of": "spent", "tol": 0.01}
        self.assertTrue(grade.check_number(c, {"spent": 1234.56}, thread("It was $1,234.56."))[0])
        self.assertFalse(grade.check_number(c, {"spent": 1234.56}, thread("About $1,235."))[0])
        self.assertTrue(grade.check_number(dict(c, tol=1), {"spent": 1234.56}, thread("About $1,235."))[0])
        # Over budget by 50: "$50 over" holds it unless the sign is asked for.
        self.assertTrue(grade.check_number({"type": "number", "of": "left"}, {"left": -50.0}, thread("$50.00 over"))[0])
        self.assertFalse(grade.check_number({"type": "number", "of": "left", "signed": True}, {"left": -50.0}, thread("$50.00 over"))[0])
        self.assertTrue(grade.check_number({"type": "number", "of": "left", "signed": True}, {"left": -50.0}, thread("-$50.00"))[0])

    def test_any_of_and_lists(self):
        c = {"type": "number", "of": "spent"}
        self.assertTrue(grade.check_number(c, {"spent": [10.0, 12.5]}, thread("12.50"))[0])
        c = {"type": "number", "any_of": ["a", "b", 7]}
        self.assertTrue(grade.check_number(c, {"a": 1.0, "b": 2.0}, thread("7 sessions"))[0])
        self.assertFalse(grade.check_number(c, {"a": 1.0, "b": 2.0}, thread("3 sessions"))[0])

    def test_missing_value_fails_with_a_reason(self):
        ok, why = grade.check_number({"type": "number", "of": "nope"}, {}, thread("1"))
        self.assertFalse(ok)
        self.assertIn("no expected", why)

    def test_reads_the_last_turn_unless_told(self):
        t = thread("You spent $10", "And $20 on food")
        self.assertFalse(grade.check_number({"type": "number", "value": 10}, {}, t)[0])
        self.assertTrue(grade.check_number({"type": "number", "value": 10, "turn": 0}, {}, t)[0])
        self.assertTrue(grade.check_number({"type": "number", "value": 10, "turn": "all"}, {}, t)[0])


class TextChecks(unittest.TestCase):
    def test_any_all_none(self):
        t = thread("Your biggest expense was Rent, $1,500.")
        self.assertTrue(grade.run_check({"type": "text_any", "values": ["rent", "mortgage"]}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "text_all", "values": ["rent", "mortgage"]}, {}, t)[0])
        self.assertTrue(grade.run_check({"type": "text_none", "values": ["mortgage"]}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "text_none", "values": ["RENT"]}, {}, t)[0])

    def test_regex_and_truth_values(self):
        t = thread("Values are as of May 8, 2026.")
        self.assertTrue(grade.run_check({"type": "text_any", "regex": [r"\bmay 8\b"]}, {}, t)[0])
        self.assertTrue(grade.run_check({"type": "text_any", "of": "names"}, {"names": ["Groceries", "May"]}, t)[0])
        self.assertFalse(grade.run_check({"type": "text_any", "of": "names"}, {"names": ["Rent"]}, t)[0])


class ToolChecks(unittest.TestCase):
    def test_tool_with_args_subset(self):
        t = thread("ok", calls=[call("money.tx.list", {"period_offset": -1, "category_id": 4}), call("money.summary")])
        self.assertTrue(grade.run_check({"type": "tool", "ops": ["money.tx.list"], "args": {"period_offset": -1}}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "tool", "ops": ["money.tx.list"], "args": {"period_offset": 0}}, {}, t)[0])
        self.assertTrue(grade.run_check({"type": "tool", "op": "money.summary"}, {}, t)[0])
        self.assertTrue(grade.run_check({"type": "tool", "ops": ["money.tx.list"], "args": {"category_id": {"$truth": "cat"}}}, {"cat": 4}, t)[0])

    def test_decimal_strings_match_numbers(self):
        t = thread("added", calls=[call("money.tx.add", {"amount": "4.75", "type": "EXPENSE"})])
        self.assertTrue(grade.run_check({"type": "tool", "ops": ["money.tx.add"], "args": {"amount": 4.75, "type": "expense"}}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "tool", "ops": ["money.tx.add"], "args": {"amount": 4.5}}, {}, t)[0])

    def test_no_tool(self):
        t = thread("ok", calls=[call("money.summary")])
        self.assertTrue(grade.run_check({"type": "no_tool", "ops": ["money.tx.add"]}, {}, t)[0])
        t = thread("ok", calls=[call("money.tx.add", {"amount": "1"})])
        self.assertFalse(grade.run_check({"type": "no_tool", "ops": ["money.tx.add", "money.tx.update"]}, {}, t)[0])

    def test_names_as_claude_code_reports_them(self):
        self.assertEqual(grade.op_of("mcp__relay__money_tx_add"), grade.op_of("money.tx.add"))

    def test_min_count_and_ok_only(self):
        t = thread("ok", calls=[call("money.tx.add", is_error=True)])
        self.assertTrue(grade.run_check({"type": "tool", "ops": ["money.tx.add"]}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "tool", "ops": ["money.tx.add"], "ok_only": True}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "tool", "ops": ["money.tx.add"], "min": 2}, {}, t)[0])


class ChartAndErrors(unittest.TestCase):
    def test_chart_subset(self):
        text = 'Here:\n```chart\n{"type": "donut", "title": "Where it went", "query": {"by": "category", "periods": 1}}\n```'
        t = thread(text)
        self.assertTrue(grade.run_check({"type": "chart", "match": {"query": {"by": "category"}}}, {}, t)[0])
        self.assertTrue(grade.run_check({"type": "chart", "any_of": [{"type": "bar"}, {"type": "DONUT"}]}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "chart", "match": {"type": "line"}}, {}, t)[0])
        self.assertFalse(grade.run_check({"type": "chart", "match": {}}, {}, thread("no chart"))[0])
        bad = thread("```chart\n{not json}\n```")
        ok, why = grade.run_check({"type": "chart", "match": {}}, {}, bad)
        self.assertFalse(ok)
        self.assertIn("invalid", why)

    def test_no_error(self):
        self.assertTrue(grade.run_check({"type": "no_error"}, {}, thread("fine"))[0])
        self.assertFalse(grade.run_check({"type": "no_error"}, {}, thread("x", error="The agent stopped"))[0])
        self.assertFalse(grade.run_check({"type": "no_error"}, {}, thread("x", timeout=True))[0])

    def test_effect(self):
        self.assertTrue(grade.run_check({"type": "effect", "truth": "x"}, {}, thread("x"), {"ok": True, "why": "added"})[0])
        self.assertFalse(grade.run_check({"type": "effect", "truth": "x"}, {}, thread("x"), {"ok": False, "why": "none"})[0])
        self.assertFalse(grade.run_check({"type": "effect", "truth": "x"}, {}, thread("x"), None)[0])
        self.assertFalse(grade.run_check({"type": "effect", "truth": "x"}, {}, thread("x"), {"error": "boom"})[0])


class Grading(unittest.TestCase):
    def test_all_checks_must_pass(self):
        task = {"checks": [{"type": "number", "of": "n"}, {"type": "no_error"}]}
        ok, checks = grade.grade(task, {"n": 3.0}, thread("3"))
        self.assertTrue(ok)
        ok, checks = grade.grade(task, {"n": 3.0}, thread("3", error="boom"))
        self.assertFalse(ok)
        self.assertEqual([c["pass"] for c in checks], [True, False])

    def test_no_checks_is_not_a_pass_and_unknown_types_fail(self):
        self.assertFalse(grade.grade({"checks": []}, {}, thread("x"))[0])
        self.assertFalse(grade.grade({"checks": [{"type": "vibes"}]}, {}, thread("x"))[0])

    def test_validate(self):
        good = {"id": "a", "source": "tally", "category": "lookup", "difficulty": 1, "prompt": "p", "truth": "t",
                "checks": [{"type": "number", "of": "x"}]}
        self.assertEqual(grade.validate_task(good, {"t": None}), [])
        self.assertTrue(grade.validate_task(dict(good, truth="u"), {"t": None}))
        self.assertTrue(grade.validate_task(dict(good, checks=[{"type": "tool"}]), {"t": None}))


if __name__ == "__main__":
    unittest.main()
