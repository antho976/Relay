"""The harness's pieces that need no engine and no GPU: splits, tasks, threads to turns, the
wrapper's arguments, the stream reassembly, the GPU lock, the fixtures' date shift, statistics."""

import datetime as dt
import json
import os
import tempfile
import threading
import time
import unittest
from pathlib import Path

from relaybench import fixtures, grade, model, report, runner, truths, truths_v2  # noqa: F401 (registers the v2 truths)
from relaybench.engine import clean_env


class Splits(unittest.TestCase):
    def test_split_is_stable_and_about_thirty_percent(self):
        self.assertEqual(runner.split_of("tally.spent.this_month"), runner.split_of("tally.spent.this_month"))
        ids = [f"task.{i}" for i in range(2000)]
        share = sum(runner.split_of(i) == "holdout" for i in ids) / len(ids)
        self.assertTrue(0.26 < share < 0.34, share)


class Tasks(unittest.TestCase):
    def test_every_task_file_is_valid(self):
        tasks = runner.load_tasks("all") + runner.load_tasks("smoke")
        self.assertTrue(tasks)
        self.assertFalse(any(t["suite"] == "smoke" for t in runner.load_tasks("all")))
        for t in tasks:
            self.assertEqual(grade.validate_task(t, truths.TRUTHS), [], t["id"])

    def test_filters(self):
        smoke = runner.load_tasks("smoke")
        self.assertTrue(all(t["suite"] == "smoke" for t in smoke))
        one = runner.load_tasks("smoke", ["smoke.tally.net_*"])
        self.assertEqual([t["id"] for t in one], ["smoke.tally.net_worth"])
        dev = runner.load_tasks("smoke", split="dev")
        self.assertTrue(all(t["split"] == "dev" for t in dev))


class Turns(unittest.TestCase):
    def test_messages_become_turns(self):
        msgs = [
            {"role": "user", "body": {"text": "How much?"}},
            {"role": "assistant", "body": {"blocks": [{"type": "text", "text": "Let me look."},
                                                      {"type": "tool_use", "id": "t1", "name": "mcp__relay__money_summary", "input": {}}]}},
            {"role": "tool", "body": {"tool_use_id": "t1", "is_error": False, "text": "{\"spent\": \"10.00\"}"}},
            {"role": "assistant", "body": {"blocks": [{"type": "text", "text": "You spent $10."}]}},
            {"role": "user", "body": {"text": "And last month?"}},
            {"role": "error", "body": {"text": "The agent stopped"}},
        ]
        turns = runner.turns_of(msgs, {"money_summary": "money.summary"})
        self.assertEqual(len(turns), 2)
        self.assertEqual(turns[0]["text"], "Let me look.\n\nYou spent $10.")
        self.assertEqual(turns[0]["final"], "You spent $10.")
        self.assertEqual(turns[0]["tool_calls"][0]["op"], "money.summary")
        self.assertEqual(turns[0]["tool_calls"][0]["is_error"], False)
        self.assertEqual(turns[1]["error"], "The agent stopped")


class Gate(unittest.TestCase):
    def test_alone_waits_for_the_shared_and_holds_them_off(self):
        g = runner.Gate()
        order = []
        g.enter()

        def exclusive():
            with g.alone():
                order.append("alone")
                time.sleep(0.05)
            order.append("alone done")

        t = threading.Thread(target=exclusive)
        t.start()
        time.sleep(0.05)
        order.append("shared leaves")
        g.leave()
        t.join(2)

        def shared():
            g.enter()
            order.append("shared again")
            g.leave()

        s = threading.Thread(target=shared)
        s.start()
        s.join(2)
        self.assertEqual(order, ["shared leaves", "alone", "alone done", "shared again"])


class Wrapper(unittest.TestCase):
    ENGINE = ["-m", "/x/Qwen3.5-4B-Q8_0.gguf", "--alias", "qwen35-4b", "--host", "127.0.0.1", "--port", "4321",
              "-c", "32768", "-np", "1", "--jinja", "--reasoning", "off", "--no-webui", "--chat-template-file", "/s/t.jinja", "--no-slots"]

    def test_slots_and_context(self):
        args, port = model.rewrite_args(self.ENGINE, {"parallel": 3, "slot_context": 32768, "template": "keep"})
        self.assertEqual(port, 4321)
        self.assertEqual(args[args.index("-c") + 1], "98304")
        self.assertEqual(args[args.index("-np") + 1], "3")
        self.assertEqual(args[args.index("--chat-template-file") + 1], "/s/t.jinja")
        self.assertIn("--no-slots", args)

    def test_template_replaced_or_dropped(self):
        args, _ = model.rewrite_args(self.ENGINE, {"parallel": 1, "slot_context": 32768, "template": "/mine.jinja"})
        self.assertEqual(args[args.index("--chat-template-file") + 1], "/mine.jinja")
        args, _ = model.rewrite_args(self.ENGINE, {"parallel": 1, "slot_context": 32768, "template": "drop", "extra_args": ["--metrics"]})
        self.assertNotIn("--chat-template-file", args)
        self.assertNotIn("/s/t.jinja", args)
        self.assertEqual(args[-1], "--metrics")

    def test_parallel_fits_free_memory(self):
        gib = 1 << 30
        self.assertEqual(model.pick_parallel(3, int(4.2 * gib), 8893), 3)
        self.assertEqual(model.pick_parallel(3, int(4.2 * gib), 7000), 1)
        self.assertEqual(model.pick_parallel(3, int(4.2 * gib), 1000), 1)

    def test_server_env_has_no_fake_clock(self):
        os.environ["LD_PRELOAD"] = "/x/libfaketime.so.1"
        os.environ["FAKETIME"] = "@2026-09-18 12:00:00"
        try:
            env = model.server_env()
            self.assertNotIn("LD_PRELOAD", env)
            self.assertNotIn("FAKETIME", env)
        finally:
            del os.environ["LD_PRELOAD"], os.environ["FAKETIME"]

    def test_stream_reassembly(self):
        ev = lambda t, d: f"event: {t}\ndata: {json.dumps(dict(d, type=t))}\n\n"
        raw = (ev("message_start", {"message": {"model": "m", "usage": {"input_tokens": 1200}}})
               + ev("content_block_start", {"index": 0, "content_block": {"type": "text", "text": ""}})
               + ev("content_block_delta", {"index": 0, "delta": {"type": "text_delta", "text": "Hi "}})
               + ev("content_block_delta", {"index": 0, "delta": {"type": "text_delta", "text": "there"}})
               + ev("content_block_start", {"index": 1, "content_block": {"type": "tool_use", "id": "t", "name": "x", "input": {}}})
               + ev("content_block_delta", {"index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"a\": "}})
               + ev("content_block_delta", {"index": 1, "delta": {"type": "input_json_delta", "partial_json": "1}"}})
               + ev("message_delta", {"delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}))
        msg = model.assemble_sse(raw)
        self.assertEqual(msg["content"][0]["text"], "Hi there")
        self.assertEqual(msg["content"][1]["input"], {"a": 1})
        self.assertEqual(msg["usage"], {"input_tokens": 1200, "output_tokens": 9})
        self.assertEqual(msg["stop_reason"], "tool_use")


class Lock(unittest.TestCase):
    def test_second_holder_waits_and_says_for_whom(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "gpu.lock"
            got = []
            with model.GpuLock(path, what="first"):
                self.assertEqual(json.loads(path.read_text())["pid"], os.getpid())

                def second():
                    with model.GpuLock(path, what="second"):
                        got.append(json.loads(path.read_text())["what"])

                t = threading.Thread(target=second)
                t.start()
                time.sleep(0.2)
                self.assertEqual(got, [])
            t.join(3)
            self.assertEqual(got, ["second"])
            self.assertEqual(path.read_text(), "")


class Fixtures(unittest.TestCase):
    def test_avex_moves_back_three_weeks_keeping_weekdays(self):
        src = json.loads(fixtures.AVEX_EXPORT.read_text())
        out = fixtures.shifted_avex()
        self.assertEqual(out["exportedAt"], "2026-09-17 21:14")
        a = dt.datetime.fromtimestamp(src["sessions"][-1]["startedAt"] / 1000)
        b = dt.datetime.fromtimestamp(out["sessions"][-1]["startedAt"] / 1000)
        self.assertEqual(a - b, dt.timedelta(weeks=3))
        self.assertEqual(a.weekday(), b.weekday())
        s0, o0 = src["sessions"][0]["exercises"][0]["sets"][0], out["sessions"][0]["exercises"][0]["sets"][0]
        self.assertEqual(s0["completedAt"] - o0["completedAt"], 21 * 86400 * 1000)
        self.assertEqual(s0["weightLb"], o0["weightLb"])
        self.assertEqual(src["cardio"][0]["date"] - out["cardio"][0]["date"], 21 * 86400 * 1000)

    def test_holdings_report_is_the_shared_one(self):
        self.assertIn('"As of 2026-05-08 12:00 GMT-04:00"', fixtures.HOLDINGS_REPORT)
        self.assertEqual(fixtures.HOLDINGS_REPORT.count("DEMO0001CAD"), 3)


class Env(unittest.TestCase):
    def test_nothing_of_the_calling_session_leaks(self):
        env = clean_env({"RELAY_SESSION": "x", "RELAY_TOKEN": "t", "RELAY_BIN": "/main/relay", "CLAUDECODE": "1",
                         "CLAUDE_CODE_SESSION_ID": "s", "ANTHROPIC_API_KEY": "k", "LD_PRELOAD": "x", "PATH": "/bin",
                         "HOME": "/home/u", "XDG_RUNTIME_DIR": "/run/user/1"})
        self.assertEqual(env, {"PATH": "/bin", "HOME": "/home/u"})


class Stats(unittest.TestCase):
    def test_wilson(self):
        lo, hi = report.wilson(8, 10)
        self.assertAlmostEqual(lo, 0.4902, places=3)
        self.assertAlmostEqual(hi, 0.9433, places=3)
        self.assertEqual(report.wilson(0, 0), (0.0, 0.0))

    def recs(self, passes):
        return [{"id": tid, "run": i + 1, "pass": p, "split": runner.split_of(tid), "source": "tally", "category": "lookup",
                 "difficulty": 1, "checks": [{"type": "number", "pass": p, "why": ""}], "seconds": 10.0, "tool_calls": 1}
                for tid, ps in passes.items() for i, p in enumerate(ps)]

    def test_summary_and_compare(self):
        a = {"name": "a", "records": self.recs({"t1": [True, True], "t2": [False, False], "t3": [True, False]})}
        b = {"name": "b", "records": self.recs({"t1": [True, True], "t2": [True, True], "t3": [False, False]})}
        s = report.summarize(a["records"])
        self.assertEqual(s["overall"]["pass"], 3)
        self.assertEqual((s["always"], s["never"], s["flaky"]), (1, 1, 1))
        self.assertEqual(s["stability"]["t3"], "1/2")
        c = report.compare(a, b)
        self.assertAlmostEqual(c["delta"], 4 / 6 - 3 / 6)
        self.assertEqual([x.split()[0] for x in c["fixed"]], ["t2"])
        self.assertEqual([x.split()[0] for x in c["broken"]], ["t3"])
        lo, hi = c["delta_bootstrap95"]
        self.assertLessEqual(lo, c["delta"])
        self.assertGreaterEqual(hi, c["delta"])
        a.update(summary=s, model={}, variant={}, relay={})
        self.assertIn("t3", report.format_summary(a))
        self.assertIn("fixed", report.format_compare(a, b, c))

    def test_bootstrap_is_reproducible(self):
        vals = [1.0, 0.0, 0.5, 1.0, 1.0]
        self.assertEqual(report.bootstrap_mean(vals), report.bootstrap_mean(vals))


class V2(unittest.TestCase):
    """The second suite and its world (README, "v2")."""

    def test_v2_tasks_are_valid_and_kept_apart(self):
        tasks = runner.load_tasks("v2") + runner.load_tasks("v2/smoke")
        self.assertTrue(tasks)
        for t in tasks:
            self.assertTrue(t["id"].startswith("v2."), t["id"])
            self.assertTrue(t["suite"].startswith("v2/"), t["suite"])
            self.assertEqual(grade.validate_task(t, truths.TRUTHS), [], t["id"])
            self.assertTrue(t["truth"].startswith("v2_"), t["id"])
        self.assertEqual(runner.world_of(tasks), "v2")
        self.assertFalse(any(t["id"].startswith("v2.") for t in runner.load_tasks("all")))
        self.assertEqual(runner.world_of(runner.load_tasks("all")), "v1")
        with self.assertRaises(ValueError):
            runner.world_of(runner.load_tasks("smoke") + tasks)

    def test_the_v2_smoke_starter_joins_only_when_named(self):
        named = runner.load_tasks("v2", ["v2.smoke.*"])
        self.assertTrue(named)
        self.assertTrue(all(t["suite"] == "v2/smoke" for t in named))
        self.assertFalse(any(t["suite"] == "v2/smoke" for t in runner.load_tasks("v2")))
        self.assertEqual([t["id"] for t in runner.load_tasks("v2/smoke")], [t["id"] for t in named])

    def test_the_committed_world_is_what_the_generator_writes(self):
        import importlib.util
        path = fixtures.V2_DIR / "generate.py"
        spec = importlib.util.spec_from_file_location("v2_generate", path)
        gen = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(gen)
        for name, text in gen.render().items():
            self.assertEqual((fixtures.V2_DIR / name).read_text(), text, f"{name} is out of date: run {path}")

    def test_max_tools(self):
        task = {"id": "x", "source": "tally", "category": "lookup", "difficulty": 1, "prompt": "?", "max_tools": 1,
                "checks": [{"type": "no_error"}]}
        call = {"op": "money.summary", "input": {}, "is_error": False}
        two = {"turns": [{"text": "ok", "tool_calls": [call, call]}], "error": None, "timeout": False}
        one = {"turns": [{"text": "ok", "tool_calls": [call]}], "error": None, "timeout": False}
        self.assertEqual(grade.grade(task, {}, one)[0], True)
        ok, results = grade.grade(task, {}, two)
        self.assertFalse(ok)
        self.assertEqual(results[-1]["type"], "max_tools")
        self.assertEqual(grade.validate_task(dict(task, max_tools=-1)), ["max_tools -1: a whole number of calls, 0 or more"])
        self.assertEqual(grade.validate_task({k: v for k, v in task.items() if k != "max_tools"}), [])


if __name__ == "__main__":
    unittest.main()
