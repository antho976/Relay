#!/usr/bin/env python3
"""Tests for plugins/video/runtime/clips/streamclips.py: python3 scripts/test-streamclips.py

The parts that need ffmpeg (loudness, the whole candidates run) are skipped without it.
"""

import contextlib
import importlib.util
import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "plugins/video/runtime/clips/streamclips.py"
spec = importlib.util.spec_from_file_location("streamclips", SCRIPT)
sc = importlib.util.module_from_spec(spec)
sys.dont_write_bytecode = True
spec.loader.exec_module(sc)


def tok(text, a, b, p=0.9, dtw=-1):
    return {"text": text, "offsets": {"from": a, "to": b}, "p": p, "t_dtw": dtw}


class Words(unittest.TestCase):
    def test_tokens_join_into_words_and_markers_are_dropped(self):
        data = {"transcription": [
            {"tokens": [tok("[_BEG_]", 0, 0), tok(" No", 0, 300, dtw=12), tok(" one", 300, 500), tok("-tapped", 500, 800, p=0.4),
                        tok("!", 800, 860), tok("[_TT_40]", 860, 860)]},
            {"tokens": [tok(" KEKW", 3000, 3400)]},
        ]}
        words = sc.whisper_words(data)
        self.assertEqual([w["text"] for w in words], [" No", " one-tapped!", " KEKW"])
        self.assertEqual(words[1]["confidence"], 0.4)
        self.assertEqual(words[0]["timestampMs"], 120)
        self.assertEqual((words[1]["startMs"], words[1]["endMs"]), (300, 860))

    def test_a_segment_never_joins_the_word_before_it(self):
        data = {"transcription": [{"tokens": [tok(" go", 0, 200)]}, {"tokens": [tok("od", 1000, 1200)]}]}
        self.assertEqual([w["text"] for w in sc.whisper_words(data)], [" go", "od"])

    def test_words_follow_dtw_when_the_decoder_drifts(self):
        # As on a real stream: the decoder put "Chat," 16 s before it was said.
        data = {"transcription": [{"tokens": [tok(" Chat,", 12_700, 18_300, dtw=2920), tok(" I", 18_300, 19_300, dtw=2940),
                                              tok(" go", 29_500, 29_800, dtw=2970)]}]}
        words = sc.whisper_words(data)
        self.assertEqual([w["startMs"] for w in words], [29_050, 29_250, 29_500])
        self.assertTrue(all(w["endMs"] <= 29_700 + 1200 for w in words), words)

    def test_a_line_whisper_repeats_is_kept_once(self):
        seg = lambda text, a: {"text": text, "tokens": [tok(" " + text.split()[0], a, a + 300)]}
        data = {"transcription": [seg("Don't add oil to cold pan.", 0), seg("Don't add oil to cold pan.", 1000),
                                  seg("don't add oil to cold pan", 2000), seg("Okay.", 3000), seg("Don't add oil to cold pan.", 4000),
                                  seg("Okay.", 9000)]}
        self.assertEqual([w["startMs"] for w in sc.whisper_words(data)], [0, 3000])

    def test_lines_break_at_sentences_and_pauses(self):
        words = [{"text": t, "startMs": a, "endMs": a + 200} for t, a in [(" Hi.", 0), (" so", 300), (" anyway", 600), (" later", 3000)]]
        self.assertEqual([l["text"] for l in sc.sentences(words)], ["Hi.", "so anyway", "later"])


class Scores(unittest.TestCase):
    def test_a_flat_signal_with_one_spike_stays_bounded(self):
        z = sc.zscores([0] * 500 + [3] + [0] * 500)
        self.assertLessEqual(max(z), 8.0)
        self.assertGreater(max(z), 4.0)
        self.assertEqual(z[0], 0.0)

    def test_times_read_both_ways(self):
        self.assertEqual(sc.secs("1:02:03"), 3723)
        self.assertEqual(sc.secs("2:05"), 125)
        self.assertEqual(sc.clock(3723), "1:02:03")


class Chat(unittest.TestCase):
    def write(self, text):
        f = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
        f.write(text)
        f.close()
        return f.name

    def test_twitchdownloader_json(self):
        path = self.write(json.dumps({"comments": [{"content_offset_seconds": 12.5, "message": {"body": "KEKW"}},
                                                   {"content_offset_seconds": 13, "message": {"fragments": [{"text": "clip it"}]}}]}))
        self.assertEqual(sc.chat_messages(path), [(12.5, "KEKW"), (13.0, "clip it")])

    def test_lines_with_times(self):
        path = self.write("[0:01:02] someone: LUL\n[1:00:00] other: hi\n")
        self.assertEqual(sc.chat_messages(path), [(62.0, "LUL"), (3600.0, "hi")])

    def test_hype_words(self):
        for text in ["KEKW", "LMAOOO", "clip it", "?????", "OMEGALUL"]:
            self.assertTrue(sc.HYPE.search(text), text)
        self.assertFalse(sc.HYPE.search("hello chat"))


class CutEdges(unittest.TestCase):
    # 10 ms frames: quiet, a word from 1.0 s to 1.5 s, quiet, a word from 2.0 s on.
    levels = [-90.0] * 100 + [-15.0] * 50 + [-90.0] * 50 + [-15.0] * 100
    words = [{"startMs": 1000, "endMs": 1500}, {"startMs": 2000, "endMs": 3000}]

    def test_an_end_inside_a_word_moves_to_the_quiet_after_it(self):
        e = sc.check_edge(self.levels, self.words, 1.3, "end")
        self.assertTrue(e["harsh"] and e["in_word"] and e["fixable"])
        self.assertTrue(1.53 <= e["move_to"] < 1.97, e)

    def test_a_start_inside_a_word_moves_before_it(self):
        e = sc.check_edge(self.levels, self.words, 2.1, "start")
        self.assertTrue(e["harsh"])
        self.assertTrue(1.53 <= e["move_to"] < 1.97, e)

    def test_a_quiet_edge_between_words_is_fine(self):
        self.assertFalse(sc.check_edge(self.levels, self.words, 1.75, "end")["harsh"])

    def test_a_word_estimate_over_silence_is_not_a_cut(self):
        words = [{"startMs": 1000, "endMs": 1800}]
        self.assertFalse(sc.check_edge(self.levels, words, 1.7, "end")["harsh"])

    def test_continuous_speech_is_stuck(self):
        levels = [-15.0] * 300
        e = sc.check_edge(levels, [{"startMs": 0, "endMs": 3000}], 1.5, "end")
        self.assertTrue(e["harsh"] and not e["fixable"])

    def cuts(self, keep):
        """`cuts` over these levels, as the skill runs it, with what it printed."""
        args = sc.argparse.Namespace(keep=json.dumps(keep), words=None, media="section.mp4", runs_on=False, strict=False)
        saved, sc.speech_levels = sc.speech_levels, lambda _media: self.levels
        try:
            with tempfile.TemporaryDirectory() as d:
                args.words = str(Path(d) / "words.json")
                Path(args.words).write_text(json.dumps(self.words))
                out = io.StringIO()
                with contextlib.redirect_stdout(out):
                    sc.cmd_cuts(args)
                return json.loads(out.getvalue())
        finally:
            sc.speech_levels = saved

    def test_a_keep_in_the_kits_milliseconds_is_checked_not_passed_unheard(self):
        # keepSpeech's spans are milliseconds: read as seconds, every edge lay past the sound's end,
        # where it is silent, and the clip passed with a word cut off.
        result = self.cuts([[0, 1300]])
        self.assertEqual(result["keep_unit"], "ms")
        self.assertFalse(result["ok"], result)
        end = result["keep"][0][1]
        self.assertTrue(1530 <= end < 1970 and isinstance(end, int), result["keep"])
        self.assertEqual(self.cuts([[0, 1.3]])["keep_unit"], "s")
        self.assertTrue(self.cuts([[0, 1.75]])["ok"])

    def test_a_keep_past_the_end_is_refused(self):
        with self.assertRaises(SystemExit):
            self.cuts([[0, 9000.0 * 1000]])


class Fetch(unittest.TestCase):
    def test_the_url_is_never_read_as_an_option(self):
        calls = []
        saved = sc.need, sc.run
        sc.need, sc.run = (lambda _name: "yt-dlp"), (lambda cmd, timeout=None: calls.append(cmd))
        try:
            with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
                for section in (None, ["0:10-0:20"]):
                    sc.cmd_fetch(sc.argparse.Namespace(url="--exec=touch pwned", out=d, cookies=None, section=section, format="best"))
        finally:
            sc.need, sc.run = saved
        self.assertEqual(len(calls), 2)
        for cmd in calls:
            self.assertEqual(cmd[-2:], ["--", "--exec=touch pwned"], cmd)


@unittest.skipUnless(shutil.which("ffmpeg"), "needs ffmpeg")
class Candidates(unittest.TestCase):
    def test_the_planted_moments_are_found(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            # Ten minutes of quiet noise, loud at 120–124 s and 400–403 s; chat bursts at 126 s and 300 s.
            subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i",
                            "aevalsrc=exprs='(random(0)-0.5)*(0.04+0.7*(between(t\\,120\\,124)+between(t\\,400\\,403)))':s=16000:d=600",
                            str(d / "a.wav")], check=True)
            comments = []
            t = 0.0
            while t < 600:
                burst = 126 <= t <= 136 or 300 <= t <= 308
                t += 0.125 if burst else 2.0
                comments.append({"content_offset_seconds": t, "message": {"body": "KEKW" if burst else "hi"}})
            (d / "chat.json").write_text(json.dumps({"comments": comments}))
            words = [{"text": f" w{i}", "startMs": i, "endMs": i + 280} for i in range(0, 600_000, 330)]
            (d / "words.json").write_text(json.dumps(words))
            run = lambda *a: subprocess.run([sys.executable, "-B", str(SCRIPT), *a], check=True, capture_output=True, text=True)
            run("signals", str(d / "a.wav"), "--chat", str(d / "chat.json"), "--out", str(d / "s.json"))
            out = json.loads(run("candidates", "--transcript", str(d / "words.json"), "--signals", str(d / "s.json"),
                                 "--out", str(d / "c.json"), "--top", "3").stdout)
            peaks = sorted(c["peak_s"] for c in out["candidates"])
            self.assertEqual(len(peaks), 3)
            for want, got in zip([122, 300, 401], peaks):
                self.assertLess(abs(want - got), 8, peaks)
            # No chat replay (a VOD from YouTube): the loud moments alone are still found.
            run("signals", str(d / "a.wav"), "--out", str(d / "s2.json"))
            out = json.loads(run("candidates", "--transcript", str(d / "words.json"), "--signals", str(d / "s2.json"),
                                 "--out", str(d / "c2.json"), "--top", "2").stdout)
            self.assertFalse(out["with_chat"])
            peaks = sorted(c["peak_s"] for c in out["candidates"])
            self.assertEqual(len(peaks), 2, out)
            for want, got in zip([122, 401], peaks):
                self.assertLess(abs(want - got), 8, peaks)


if __name__ == "__main__":
    unittest.main()
