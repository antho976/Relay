# Avex 0.9 — release film

90 seconds, 1920×1080, 30fps. `npm run studio` to work on it, `npm run render` to output
`out/avex-0.9.mp4`.

## What the edit is built on

**Pace.** Nineteen beats in ninety seconds cannot all be the same length; the 0.8.9 cut ran 3:36
because every beat was ~11 seconds and entered the same way. `theme.ts` carries three speeds, and a
beat's speed decides how much copy it is allowed to hold — a paragraph at 6–7s, one line at 4s, a
headline alone at 3s, cut hard on both sides. `XFADE.cut` is a genuine zero-frame cut: an edit where
all eighteen joins are crossfades has no punctuation, only commas.

**One device.** Before and after are the same phone, never two. A side-by-side halves both screens
and turns a release into spot-the-difference. `Seam` (in `Device.tsx`) stacks the two captures in one
body and sweeps an accent edge between them — AccentRed replacing AccentNavy being, literally, what
0.9 shipped. The same gesture does the section breaks, as the `sweep` transition.

**Two kinds of camera.** `Device.focus` flies a window *inside* a capture, and `Camera`/`Shot` moves
the whole frame. They are not interchangeable: `focus` at z > 1 crops horizontally as well as
vertically, so it eats the edges of words. In-screen travel therefore belongs to `Detail`, which
fills the frame edge to edge; a `Solo` shows the whole phone and moves the camera on it instead.

**Drawn and filmed, in one body.** `Device` takes `children`, so a natively rebuilt Compose screen
(`ui/`) and a real capture sit in the same phone. Some beats can only be drawn — the Home morph needs
both eras on one clock, the watch is a 454px round display that no recording makes legible at 1080p,
and the notifications beat turns on a banner flying into a bell that a screen recording renders as
four frames of something small moving.

## Sound

`public/sfx/`, synthesised by `tools/make-sfx.sh` — nothing licensed. Sound only where the picture
shows an interaction that would make one: the watch (stepper, log, PR, rest), the onboarding pages
landing, the banner reaching the bell. No music bed, no ambience, and deliberately nothing under the
transitions. Silence is the default state, not a gap.

`make-sfx.sh` normalises all seven files to the same **−3 dBFS** peak, so `Sound.tsx`'s `LEVEL` map
is the only thing shaping the hierarchy. Setting it in both places attenuated every cue twice and put
the film's loudest moment at −14.5 dBFS — which survives headphones and nothing else.

The transients sit high in that map on purpose: they carry a 22–24 dB peak-to-loudness crest against
the tonal cues' 6–9, so a tap at 0.8 and a chime at 0.62 are nowhere near equally loud to a listener.
Matching them by peak is what makes them read as one palette. Integrated loudness for the film is
low by design — most of it is silence — so judge the mix by cue peaks, not by LUFS.

## Footage

`public/cfr/` — every clip transcoded to **constant** 30fps. `adb screenrecord` writes variable frame
rate (these takes averaged ~56fps), so a frame index measured with ffmpeg meant a different instant
to Remotion, which counts composition frames at 30. The offsets in `Release.tsx` only land where they
were measured because the two now agree.

Unused but available: `before/after-settings`, `before/after-stats`, `before-home`, `after-checkin`.

## Claims

Every figure on screen is either visible in the footage or asserted by the source:

| On screen | Source |
|---|---|
| 147 commits · 632 files | `git log/diff --stat 2fe2379..main` |
| 15 pages down to 9 | 0.8.9 `PAGE_WELCOME`(0)…`PAGE_PREVIEW`(14); 0.9 `GENERATED_PATH` has 9 |
| Thirty-five pieces, five tracks | `docs/ACADEMY_ART.md`; the app's own masthead reads `35 PIECES · 40 MIN` |
| volume no longer truncates to 52… | `before-coach.mp4` shows the header reading `52....` |
| the twelve-row year grid | `before-lasttab.mp4` shows `THIS YEAR` as JAN–DEC dot rows |

An earlier cut of this film claimed **145 commits · 460 files**, **28 Academy lessons** and
**seventeen pages down to ten**. None of the three survives contact with the code — the lesson count
comes from a commit message that its own commit contradicts (31 lessons + 4 articles = 35 pieces),
and no tree has ever produced 17 or 10 onboarding pages.

**Open:** `versionName` is still `0.8.8.3` in `forge-android/app/build.gradle.kts` and `versionCode`
is 89. Nothing in `2fe2379..main` bumps either. The film says 0.9 throughout; the build should be
bumped before it ships, or the film is announcing a version the app does not report.
