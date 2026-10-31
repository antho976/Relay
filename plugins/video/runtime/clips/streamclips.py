#!/usr/bin/env python3
"""Stream clips: from a stream's VOD to the moments worth a Short, and the files a clip is made from.

Written by Relay (plugins/video); edits here are overwritten. The stream-clips skill says how an
agent uses it. Standard library only. It calls ffmpeg, and yt-dlp and whisper.cpp when they are
installed (`tools` says what is missing and how to get it).

    streamclips.py tools
    streamclips.py fetch URL --out DIR                      audio, chat and info for a Twitch VOD
    streamclips.py fetch URL --out DIR --section 1:02:10-1:03:05 [--section ...]   the picture, for clips
    streamclips.py transcribe AUDIO --out DIR               words with timings (whisper.cpp)
    streamclips.py signals AUDIO --out FILE [--chat FILE]   loudness and chat by the second
    streamclips.py candidates --transcript F --signals F --out FILE [--top 25]
    streamclips.py words --transcript F --start S --end E --out FILE   a clip's captions, from 0
    streamclips.py cut VIDEO --start S --end E --out FILE   a clip's footage at a constant rate
    streamclips.py style VIDEO... --out DIR                 how a reference channel's Shorts are cut

Times are seconds, or h:mm:ss / m:ss. Every result is JSON on stdout as well as in its file.
"""

import argparse
import json
import math
import os
import re
import shutil
import statistics
import subprocess
import sys
from pathlib import Path

# Words and emotes chat types when something lands. Chat reacts a few seconds after the moment.
HYPE = re.compile(
    r"\b(kekw|lul|lulw|omegalul|lmao+|lmfao|ahah\w*|haha\w*|xd+|pog\w*|clip(\s*it|\s*that)?|w+|l+|no\s*way|wtf|holy|bro+|"
    r"icant|ica?nt|kappa|monkas|pepelaugh|sadge|aware|copium|ez+|gg+)\b|\?{3,}|!{3,}",
    re.IGNORECASE,
)
CHAT_LAG_S = 6
FFMPEG = os.environ.get("RELAY_FFMPEG") or shutil.which("ffmpeg") or "ffmpeg"
FFPROBE = os.environ.get("RELAY_FFPROBE") or shutil.which("ffprobe") or "ffprobe"
HOME = Path.home()


def secs(value):
    """Seconds from `75`, `75.5`, `1:15` or `0:01:15`."""
    if isinstance(value, (int, float)):
        return float(value)
    parts = [float(p) for p in str(value).strip().split(":")]
    total = 0.0
    for p in parts:
        total = total * 60 + p
    return total


def clock(s):
    s = max(0, int(round(s)))
    h, rest = divmod(s, 3600)
    m, sec = divmod(rest, 60)
    return f"{h}:{m:02d}:{sec:02d}" if h else f"{m}:{sec:02d}"


def emit(result, out=None):
    text = json.dumps(result, indent=1, ensure_ascii=False)
    if out:
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        Path(out).write_text(text + "\n")
    print(text)


def run(cmd, timeout=None):
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    if proc.returncode != 0:
        tail = "\n".join((proc.stderr or proc.stdout).strip().splitlines()[-12:])
        sys.exit(f"{Path(cmd[0]).name} failed ({proc.returncode}):\n{tail}")
    return proc


# ---- Tools ----

def find_tools():
    whisper = os.environ.get("RELAY_WHISPER") or shutil.which("whisper-cli")
    for guess in (HOME / ".local/share/whisper.cpp/bin/whisper-cli", HOME / ".local/share/whisper.cpp/build/bin/whisper-cli"):
        if not whisper and guess.exists():
            whisper = str(guess)
    model = os.environ.get("RELAY_WHISPER_MODEL")
    if not model:
        for guess in (HOME / "models/ggml-large-v3-turbo.bin", HOME / "models/ggml-large-v3-turbo-q8_0.bin"):
            if guess.exists():
                model = str(guess)
                break
    return {
        "ffmpeg": shutil.which(FFMPEG) or (FFMPEG if Path(FFMPEG).exists() else None),
        "yt_dlp": os.environ.get("RELAY_YTDLP") or shutil.which("yt-dlp"),
        "whisper": whisper,
        "whisper_model": model,
    }


INSTALL = {
    "yt_dlp": "curl -L -o ~/.local/bin/yt-dlp https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp && chmod +x ~/.local/bin/yt-dlp",
    "whisper": "git clone --depth 1 https://github.com/ggml-org/whisper.cpp ~/.local/share/whisper.cpp && cd ~/.local/share/whisper.cpp && "
               # A bounded -j: a bare -j starts every CUDA kernel at once and runs a 32 GB machine out of memory.
               "cmake -B build -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=native -DCMAKE_BUILD_TYPE=Release && "
               "cmake --build build -j 4 --config Release --target whisper-cli",
    "whisper_model": "mkdir -p ~/models && curl -L -o ~/models/ggml-large-v3-turbo.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
}


def cmd_tools(_args):
    found = find_tools()
    missing = {k: INSTALL[k] for k, v in found.items() if not v and k in INSTALL}
    emit({"found": found, "missing": missing})


def need(name):
    tool = find_tools()[name]
    if not tool:
        sys.exit(f"{name} is not installed. The person installs it: {INSTALL.get(name, name)}")
    return tool


# ---- Fetch ----

def cmd_fetch(args):
    ytdlp = need("yt_dlp")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    common = [ytdlp, "--no-progress", "--no-playlist", "-o", str(out / "%(id)s.%(ext)s")]
    if args.cookies:
        common += ["--cookies", args.cookies]
    if args.section:
        files = []
        for section in args.section:
            a, b = (secs(x) for x in section.split("-", 1))
            name = f"%(id)s-{int(a)}-{int(b)}.%(ext)s"
            run([ytdlp, "--no-progress", "--no-playlist", "-f", args.format, "--download-sections", f"*{a}-{b}", "--force-keyframes-at-cuts",
                 "-o", str(out / name), *(["--cookies", args.cookies] if args.cookies else []), args.url])
            files.append(section)
        emit({"sections": files, "files": sorted(p.name for p in out.iterdir())})
        return
    # The audio alone (a few hundred MB for hours) and the chat replay, never the whole picture.
    run(common + ["-f", "Audio_Only/bestaudio/worst", "--write-info-json", "--write-subs", "--sub-langs", "rechat,live_chat", args.url])
    files = sorted(p.name for p in out.iterdir())
    info = next((out / f for f in files if f.endswith(".info.json")), None)
    meta = {}
    if info:
        data = json.loads(info.read_text())
        meta = {k: data.get(k) for k in ("id", "title", "uploader", "duration", "upload_date", "width", "height", "fps")}
    emit({"files": files, "info": meta, "chat": next((f for f in files if "rechat" in f or "live_chat" in f), None)})


# ---- Transcribe ----

def whisper_words(data):
    """Words with timings from whisper.cpp's full JSON (-ojf): tokens that start with a space
    start a word, the rest join the word before; bracketed tokens are whisper's own markers."""
    words = []
    for seg in data.get("transcription", []):
        first = len(words)
        for tok in seg.get("tokens", []):
            text = tok.get("text", "")
            if not text or text.startswith("[_") or (text.startswith("[") and text.endswith("]")):
                continue
            off = tok.get("offsets", {})
            start, end = off.get("from", 0), off.get("to", 0)
            dtw = tok.get("t_dtw", -1)
            p = tok.get("p")
            if len(words) > first and not text.startswith(" "):
                w = words[-1]
                w["text"] += text
                w["endMs"] = max(w["endMs"], end)
                if p is not None:
                    w["_p"].append(p)
                continue
            words.append({"text": text, "startMs": start, "endMs": end, "timestampMs": dtw * 10 if dtw is not None and dtw >= 0 else None,
                          "_p": [p] if p is not None else []})
    out = []
    for w in words:
        if not w["text"].strip():
            continue
        conf = round(min(w["_p"]), 3) if w["_p"] else None
        start = w["startMs"]
        # whisper.cpp's offsets run late and its DTW time is where the word is heard: start the word
        # no later than that.
        if w["timestampMs"] is not None:
            start = min(start, w["timestampMs"])
        out.append({"text": w["text"], "startMs": start, "endMs": max(w["endMs"], start + 40), "timestampMs": w["timestampMs"], "confidence": conf})
    for prev, cur in zip(out, out[1:]):
        if prev["endMs"] > cur["startMs"]:
            prev["endMs"] = max(prev["startMs"] + 40, cur["startMs"])
    return out


def sentences(words, max_gap_ms=900, max_len_ms=14000):
    """Lines of the transcript for reading: broken at a sentence's end, a pause or 14 s."""
    lines = []
    cur = []
    for w in words:
        if cur and (w["startMs"] - cur[-1]["endMs"] > max_gap_ms or w["endMs"] - cur[0]["startMs"] > max_len_ms):
            lines.append(cur)
            cur = []
        cur.append(w)
        if re.search(r"[.!?]$", w["text"].strip()):
            lines.append(cur)
            cur = []
    if cur:
        lines.append(cur)
    return [{"startMs": l[0]["startMs"], "endMs": l[-1]["endMs"], "text": "".join(w["text"] for w in l).strip()} for l in lines]


def cmd_transcribe(args):
    whisper = need("whisper")
    model = need("whisper_model")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    wav = out / "audio-16k.wav"
    run([FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-i", args.audio, "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", str(wav)])
    base = out / "whisper"
    cmd = [whisper, "-m", model, "-f", str(wav), "-ojf", "-of", str(base), "-l", args.language, "-t", str(args.threads), "-np"]
    if "turbo" in Path(model).name:
        cmd += ["--dtw", "large.v3.turbo"]
    if args.prompt:
        cmd += ["--prompt", args.prompt]
    run(cmd)
    words = whisper_words(json.loads(Path(f"{base}.json").read_text()))
    (out / "words.json").write_text(json.dumps(words, ensure_ascii=False) + "\n")
    lines = sentences(words)
    (out / "transcript.txt").write_text("".join(f"[{clock(l['startMs'] / 1000)}] {l['text']}\n" for l in lines))
    wav.unlink(missing_ok=True)
    emit({"words": len(words), "lines": len(lines), "files": ["words.json", "transcript.txt", "whisper.json"]})


# ---- Signals ----

def loudness_by_second(audio):
    """The loudest momentary loudness (LUFS, 400 ms windows) in each second, from ffmpeg's ebur128."""
    proc = run([FFMPEG, "-hide_banner", "-nostats", "-i", audio, "-map", "0:a:0", "-af", "ebur128=framelog=info", "-f", "null", "-"])
    per = {}
    for m in re.finditer(r"t:\s*([\d.]+)\s+TARGET:.*?M:\s*(-?[\d.]+|-inf)", proc.stderr):
        t, v = float(m.group(1)), m.group(2)
        val = -70.0 if v == "-inf" else max(-70.0, float(v))
        s = int(t)
        per[s] = max(per.get(s, -70.0), val)
    n = max(per) + 1 if per else 0
    return [per.get(i, -70.0) for i in range(n)]


def chat_messages(path):
    """(seconds, text) for each message of a chat replay: TwitchDownloader's JSON, yt-dlp's rechat or
    live_chat, or lines of `[h:mm:ss] name: text`."""
    raw = Path(path).read_text(errors="replace")
    msgs = []
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        data = None
        if raw.lstrip().startswith("{"):  # yt-dlp's live_chat: one JSON object per line
            for line in raw.splitlines():
                try:
                    item = json.loads(line)
                except json.JSONDecodeError:
                    continue
                act = item.get("replayChatItemAction", {})
                ms = act.get("videoOffsetTimeMsec")
                for a in act.get("actions", []):
                    r = a.get("addChatItemAction", {}).get("item", {}).get("liveChatTextMessageRenderer", {})
                    text = "".join(run_.get("text", "") for run_ in r.get("message", {}).get("runs", []))
                    if ms is not None:
                        msgs.append((int(ms) / 1000, text))
            return msgs
        for m in re.finditer(r"^\[(\d+:\d{2}(?::\d{2})?)\]\s*[^:]*:\s*(.*)$", raw, re.MULTILINE):
            msgs.append((secs(m.group(1)), m.group(2)))
        return msgs
    comments = data.get("comments", data) if isinstance(data, dict) else data
    for c in comments or []:
        if not isinstance(c, dict):
            continue
        t = c.get("content_offset_seconds", c.get("contentOffsetSeconds", c.get("offset")))
        msg = c.get("message", {})
        if isinstance(msg, dict):
            text = msg.get("body") or "".join(f.get("text", "") for f in msg.get("fragments", []) if isinstance(f, dict))
        else:
            text = str(msg or c.get("body", ""))
        if t is not None:
            msgs.append((float(t), text))
    return msgs


def cmd_signals(args):
    loud = loudness_by_second(args.audio)
    n = len(loud)
    chat = [0] * n
    hype = [0] * n
    count = 0
    if args.chat:
        for t, text in chat_messages(args.chat):
            s = int(t)
            if 0 <= s < n:
                chat[s] += 1
                count += 1
                if HYPE.search(text or ""):
                    hype[s] += 1
    emit({"seconds": n, "chat_messages": count, "loudness": [round(v, 1) for v in loud], "chat": chat, "hype": hype}, args.out)


# ---- Candidates ----

def smooth(xs, radius):
    out = []
    acc = [0.0]
    for x in xs:
        acc.append(acc[-1] + x)
    for i in range(len(xs)):
        a, b = max(0, i - radius), min(len(xs), i + radius + 1)
        out.append((acc[b] - acc[a]) / (b - a))
    return out


def zscores(xs):
    if len(xs) < 2:
        return [0.0] * len(xs)
    # Robust: the median and its absolute deviation, with the spread floored at half the standard
    # deviation so a mostly flat signal (no hype for minutes) does not blow up, and capped at 8.
    med = statistics.median(xs)
    mad = statistics.median(abs(x - med) for x in xs)
    scale = max(1.4826 * mad, 0.5 * statistics.pstdev(xs), 1e-6)
    return [max(-8.0, min(8.0, (x - med) / scale)) for x in xs]


def cmd_candidates(args):
    sig = json.loads(Path(args.signals).read_text())
    words = json.loads(Path(args.transcript).read_text())
    n = sig["seconds"]
    # Loudness above the stream's own level a minute around it: shouting over a quiet stretch counts.
    loud = sig["loudness"]
    base = smooth(loud, 60)
    above = [max(0.0, l - b) for l, b in zip(loud, base)]
    lz = zscores(smooth(above, 2))
    has_chat = sig.get("chat_messages", 0) > 0
    # Chat counts against its own pace, and is moved back by the lag.
    cz = [0.0] * n
    hz = [0.0] * n
    if has_chat:
        rate = smooth(sig["chat"], 5)
        pace = smooth(sig["chat"], 120)
        rel = [r / (p + 0.5) for r, p in zip(rate, pace)]
        hype = smooth(sig["hype"], 5)
        crz = zscores(rel)
        hhz = zscores(hype)
        for i in range(n):
            j = min(n - 1, i + CHAT_LAG_S)
            cz[i] = crz[j]
            hz[i] = hhz[j]
    # Speech density: a moment with nobody talking rarely makes a Short.
    talk = [0.0] * n
    for w in words:
        for s in range(int(w["startMs"] / 1000), min(n, int(w["endMs"] / 1000) + 1)):
            talk[s] = 1.0
    talk = smooth(talk, 8)
    weights = (0.45, 0.35, 0.2) if has_chat else (1.0, 0.0, 0.0)
    score = [max(0.0, weights[0] * cz[i] + weights[1] * lz[i] + weights[2] * hz[i]) * (0.4 + 0.6 * talk[i]) for i in range(n)]
    score = smooth(score, 3)
    # Peaks, at least `spacing` seconds apart, best first.
    order = sorted(range(n), key=lambda i: score[i], reverse=True)
    taken = []
    for i in order:
        if score[i] <= 0 or len(taken) >= args.top:
            break
        if all(abs(i - j) >= args.spacing for j in taken):
            taken.append(i)
    lines = sentences(words)
    out = []
    for rank, peak in enumerate(taken, 1):
        a = max(0, peak - args.before)
        b = min(n, peak + args.after)
        ctx = [l for l in lines if l["endMs"] / 1000 >= a - 20 and l["startMs"] / 1000 <= b + 10]
        out.append({
            "rank": rank, "peak": clock(peak), "peak_s": peak, "window": [clock(a), clock(b)], "window_s": [a, b],
            "score": round(score[peak], 2),
            "why": {"chat": round(cz[peak], 1), "loud": round(lz[peak], 1), "hype": round(hz[peak], 1), "talk": round(talk[peak], 2)},
            "chat_per_10s": sum(sig["chat"][max(0, peak - 5 + CHAT_LAG_S):peak + 5 + CHAT_LAG_S]) if has_chat else None,
            "transcript": [f"[{clock(l['startMs'] / 1000)}] {l['text']}" for l in ctx],
        })
    emit({"seconds": n, "with_chat": has_chat, "candidates": out}, args.out)


# ---- A clip's files ----

def cmd_words(args):
    words = json.loads(Path(args.transcript).read_text())
    a, b = secs(args.start) * 1000, secs(args.end) * 1000
    clip = []
    for w in words:
        if w["endMs"] <= a or w["startMs"] >= b:
            continue
        shift = lambda v: None if v is None else round(min(max(v, a), b) - a)
        clip.append({"text": w["text"], "startMs": shift(w["startMs"]), "endMs": shift(w["endMs"]), "timestampMs": shift(w.get("timestampMs")),
                     "confidence": w.get("confidence")})
    if clip:
        clip[0]["text"] = clip[0]["text"].lstrip()
    if not args.out:
        print(json.dumps(clip, ensure_ascii=False, indent=1))
        return
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(clip, ensure_ascii=False, indent=1) + "\n")
    emit({"out": args.out, "words": len(clip), "text": "".join(w["text"] for w in clip).strip()})


def probe(path):
    proc = run([FFPROBE, "-v", "error", "-print_format", "json", "-show_streams", "-show_format", path])
    data = json.loads(proc.stdout)
    video = next((s for s in data["streams"] if s.get("codec_type") == "video"), {})
    num, _, den = video.get("avg_frame_rate", "0/1").partition("/")
    fps = float(num) / float(den or 1) if float(den or 1) else 0
    return {"width": video.get("width"), "height": video.get("height"), "fps": round(fps, 3), "duration": float(data["format"].get("duration", 0))}


def cmd_cut(args):
    a, b = secs(args.start), secs(args.end)
    src = probe(args.video)
    fps = args.fps or (60 if src["fps"] > 45 else 30)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    # Seek before the input (fast), then re-encode from that exact frame at a constant rate: a copy
    # would start at the keyframe before.
    run([FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-ss", f"{a:.3f}", "-i", args.video, "-t", f"{b - a:.3f}",
         "-map", "0:v:0", "-map", "0:a:0?", "-vf", f"fps={fps}", "-c:v", "libx264", "-preset", "veryfast", "-crf", str(args.crf),
         "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-movflags", "+faststart", args.out])
    emit({"out": args.out, **probe(args.out), "source": src})


def cmd_style(args):
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    report = []
    for i, video in enumerate(args.videos, 1):
        info = probe(video)
        proc = run([FFMPEG, "-hide_banner", "-nostats", "-i", video, "-map", "0:v:0", "-vf", "select='gt(scene,0.32)',showinfo", "-f", "null", "-"])
        cuts = [round(float(m.group(1)), 2) for m in re.finditer(r"pts_time:([\d.]+)", proc.stderr)]
        sheet = out / f"{i:02d}-sheet.png"
        step = max(0.5, info["duration"] / 24)
        rows = math.ceil(min(24, info["duration"] / step) / 6) or 1
        run([FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-i", video, "-vf", f"fps=1/{step:.3f},scale=270:-2,tile=6x{rows}:padding=6:margin=6", "-frames:v", "1", str(sheet)])
        loud = run([FFMPEG, "-hide_banner", "-nostats", "-i", video, "-map", "0:a:0?", "-af", "ebur128", "-f", "null", "-"]).stderr
        lufs = re.findall(r"I:\s*(-?[\d.]+) LUFS", loud)
        dur = info["duration"] or 1
        report.append({"video": video, **info, "cuts": len(cuts), "cuts_per_minute": round(len(cuts) / dur * 60, 1),
                       "median_shot_s": round(statistics.median([b - a for a, b in zip([0] + cuts, cuts + [dur])]), 2),
                       "loudness_lufs": float(lufs[-1]) if lufs else None, "sheet": str(sheet), "sheet_every_s": round(step, 2)})
    emit({"videos": report}, out / "style.json")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("tools")
    f = sub.add_parser("fetch")
    f.add_argument("url")
    f.add_argument("--out", required=True)
    f.add_argument("--section", action="append", help="start-end; fetches that part's picture and sound")
    f.add_argument("--format", default="bestvideo[height<=1080]+bestaudio/best[height<=1080]/best")
    f.add_argument("--cookies", help="a cookies.txt for a VOD only subscribers can watch")
    t = sub.add_parser("transcribe")
    t.add_argument("audio")
    t.add_argument("--out", required=True)
    t.add_argument("--language", default="en")
    t.add_argument("--threads", type=int, default=8)
    t.add_argument("--prompt", help="names and words to spell right: the streamer, the game, its terms")
    s = sub.add_parser("signals")
    s.add_argument("audio")
    s.add_argument("--out", required=True)
    s.add_argument("--chat")
    c = sub.add_parser("candidates")
    c.add_argument("--transcript", required=True, help="words.json from transcribe")
    c.add_argument("--signals", required=True)
    c.add_argument("--out", required=True)
    c.add_argument("--top", type=int, default=25)
    c.add_argument("--spacing", type=int, default=75, help="seconds between two candidates' peaks")
    c.add_argument("--before", type=int, default=40)
    c.add_argument("--after", type=int, default=15)
    w = sub.add_parser("words")
    w.add_argument("--transcript", required=True)
    w.add_argument("--start", required=True)
    w.add_argument("--end", required=True)
    w.add_argument("--out")
    k = sub.add_parser("cut")
    k.add_argument("video")
    k.add_argument("--start", required=True)
    k.add_argument("--end", required=True)
    k.add_argument("--out", required=True)
    k.add_argument("--fps", type=int)
    k.add_argument("--crf", type=int, default=18)
    y = sub.add_parser("style")
    y.add_argument("videos", nargs="+")
    y.add_argument("--out", required=True)
    args = p.parse_args()
    {"tools": cmd_tools, "fetch": cmd_fetch, "transcribe": cmd_transcribe, "signals": cmd_signals, "candidates": cmd_candidates,
     "words": cmd_words, "cut": cmd_cut, "style": cmd_style}[args.cmd](args)


if __name__ == "__main__":
    main()
