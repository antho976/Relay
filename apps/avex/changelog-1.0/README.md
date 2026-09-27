# Avex 1.0 — changelog video

46 seconds, 1920×1080, 60 fps, 120 BPM (a beat is 30 frames, a bar is 120). Same pipeline as
`trailer-1.0/`: one HTML page where every frame is a function of time, rendered frame by frame.

```
cat part1.html rig.js scenes.js > index.html                  # the page is assembled from three parts
python3 tools/music_cl.py out/score.wav                       # the score (+ out/kicks.json)
PAGE=$PWD/index.html node ../trailer-1.0/tools/render.mjs cues out/cues.json
python3 ../trailer-1.0/tools/mixdown.py out/score.wav out/cues.json out/mix.wav
ffmpeg -i out/mix.wav -c:a libmp3lame -b:a 192k audio/changelog-mix.mp3
PAGE=$PWD/index.html node ../trailer-1.0/tools/render.mjs video out/picture.mp4 60
ffmpeg -i out/picture.mp4 -i out/mix.wav -c:v copy -c:a aac -b:a 256k -shortest out/avex-1.0-changelog.mp4
python3 ../trailer-1.0/tools/synccheck.py out/avex-1.0-changelog.mp4 out/cues.json out/kicks.json
```

## What it covers

Everything from 2026-08-26 to HEAD: 177 non-merge commits, `git diff --shortstat 8daeb27 HEAD` = 766
files, +46,995 / −30,569. Each item is the **final** state at HEAD against 8daeb27 (the last commit
before the window): anything reworked several times appears once, in its newest form, and anything
added then removed is gone. Ranks, XP and trophies are behind `Features.SHOW_GAMIFICATION = false`
and are not listed. Every item was confirmed in the code, not taken from a commit message.

| Bars | Section |
|---|---|
| 1–2 | Title: Avex, 0.9 → 1.0, 177 commits · 766 files changed |
| 3–14 | Six big changes, each a before/after on one phone: the seam sweeps from 0.9 to 1.0 and lands on the downbeat, then the new screen does its thing |
| 15–18 | Thirteen more, one per beat |
| 19–21 | Fifteen small details, ticked off on eighths, held for a bar |
| 22–23 | End card: Thanks for testing |

## The big six (before → after)

| On screen | Evidence |
|---|---|
| Set-by-set logging | f491f89, 9b283b1, 1b2cd74, 1877a2d · `FreestyleLogParts.kt`, `FreestyleStartParts.kt`; before: `FreestyleLogScreen.kt`@8daeb27 |
| Academy, redrawn (35 pieces → 12 drawn lessons) | 4e9e5bc · `AcademyScreen.kt`, `Lesson.kt`, `AcademyFigures.kt` |
| A simpler Coach page (advanced tracking opt-in) | bfc2433, 3906fae, af22a0b · `CoachScreen.kt`, `CoachAdvancedPrompt.kt:104-132` |
| Your program as a week (bars, tap to read, hold to reorder) | ddffa3f · `ProgramBuilderScreen.kt`, `WeekBarRail.kt` |
| Tap a day you trained (months, "That day" sheet) | 1500ea0 · `ProfileActivityMonth.kt`, `DayLog.kt` |
| Cleaner settings (titles, switches, steppers) | 85f6590 · `SettingsPrimitives.kt`, `SettingsSubPages.kt`, `SettingsProgramPage.kt` |

## Also changed

Check-in in the bell (345b6d0) · quieter Home card (345b6d0) · real rest days (681bb17, 97126b2,
e0ad3b2) · smarter rest timer (e0ad3b2, ea87ebe) · one-line exercise plan (c33d2dd) · better
generated plans (4a6ab70, ea87ebe) · weights in your unit (74d703b, 9f79c12) · tidier stats (d72f020) ·
onboarding refresh (f94a3c1, f7e27a8) · export progress (216487f, ba2ce3b) · import fixes (e81aa3e,
9f79c12, 00c7fa0) · stronger app locks (84ea7a9, 2c9acca, 7f9f2cb, f19eeb1) · watch fixes (f42d031,
ce6338c, 509c6af, 1515479, ce7e3c8).

## Small details

ed8ba08 warm-up gate · 1515479 check-in kg · d72f020 7.5 kg records and VS LAST arrows · 51f2ec8
last-exercise hint · e8e66b4 custom cardio names, "That day" scroll, single rest buzz, custom muscle ·
f8bcbe0 notes-only exit prompt, Cardio back, dead quick action removed · 6c67095 onboarding
kg/lb conversion · f19eeb1 "Discard & continue" · 078a592 Health Connect body fat.

The phone screens are simplified drawings of those screens; names and numbers in them (Bench Press,
60 × 8, September 12, …) are example data.
