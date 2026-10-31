#!/usr/bin/env python3
"""Tests for plugins/video/runtime/clips/streamclips.py: python3 scripts/test-streamclips.py

The parts that need ffmpeg (loudness, the whole candidates run) are skipped without it.
"""

import importlib.util
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
