The two faces the vignettes set type in, so they render the same on every machine. They are the
faces Android itself draws the app's `FontFamily.SansSerif` and `FontFamily.Monospace` in, which is
what the Canvas twin (`PlanModeVignettes.kt`) and the cards around the videos use:

- `Roboto-Medium.ttf`: Roboto 3.015 (google/fonts `ofl/roboto/Roboto[wdth,wght].ttf`), instanced
  at wght 500, wdth 100 (`fonttools varLib.instancer … wght=500 wdth=100 --static`). Medium is the
  weight of the app's sans titles (`titleSmall` and up). Copyright 2011 The Roboto Project Authors,
  SIL Open Font License 1.1: see `OFL-Roboto.txt`.
- `DroidSansMono-Regular.ttf`: Droid Sans Mono, the file Android maps `monospace` to (AOSP
  `frameworks/base/data/fonts/DroidSansMono.ttf`). Regular is the weight of `labelSmall` and
  `labelMedium`. Copyright 2007 Google Corporation, Apache License 2.0: see `LICENSE-Apache-2.0.txt`.

Both are subset to printable ASCII plus `×` with `pyftsubset --unicodes="U+0020-007E,U+00D7"`. If
a vignette needs a character outside that, re-subset from the sources above, or the render falls
back to the machine's own face for that glyph.

These replaced Noto Sans Medium and Noto Sans Mono (2026-10-17), which were stand-ins: close, but
Noto Sans Mono's narrower, rounder letters set the day labels visibly differently from the app's.
