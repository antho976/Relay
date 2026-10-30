---
name: house-style-review
description: Review a screen, component, mockup or design-canvas artboard against the owner's house style and design fundamentals before calling UI work done, or when asked to critique, audit, polish or say what's wrong with a design - look at the real render, run the checklist (the one answer, hierarchy, crowding, size, roundness, boxes and lines, consistency with the rest of the app, colour and measured contrast, the seven states, copy and voice, icons, motion, platform physics), name each problem with its failure shape, rank by what the owner would notice first, fix in one batch and re-check once. Use with house-style; for a whole-app sweep run it screen by screen.
---

# Reviewing against the house style

A review looks at what the owner will see, not at the code. Every finding names its failure shape
(from `house-style/reference/failures.md`), where it is, and the fix. Load `house-style` first.

## 1. Look at the real thing

- Get a render at real size: `design.render` for a canvas artboard; a screenshot or golden for an app
  (a virtual compositor on the desktop, Roborazzi or the emulator on Android, a browser at desktop and
  360 px for a page). Code alone is not a review; say so if no render was possible.
- Put the screen beside its neighbours (the page next door, the same component elsewhere). Half of what
  the owner calls "off" is inconsistency between adjacent screens.
- Have the before handy if this is a change. Compare like with like.

## 2. Checklist

Go through every line. A screen passes a line or gets a finding.

**The answer**
- [ ] The screen's one question is obvious, and its answer is the most prominent thing, above the fold.
- [ ] Nothing (notice, tip, strip, banner) sits between the answer and the main action.
- [ ] One hero; everything else steps down. The squint test still shows the groups.

**Crowding and size**
- [ ] No section exists "because it needs to be there": each one changes a decision or is cut.
- [ ] One caption per section at most; no explainer for the obvious; no fact or chart repeated.
- [ ] No more than about four uniform status rows in a run (the AI look).
- [ ] Controls, popups, figures and panels are trim, not oversized. Long text is cut to one line where
  one line works.
- [ ] The page is not empty either: it carries real readings for its subject.
- [ ] Scrolling: the useful part is not buried under low-value sections.

**Shape and surfaces**
- [ ] Pills only for small toggles, badges and counts; one radius scale; adjacent elements share a shape.
- [ ] Passive content has no box or grey backdrop (phone apps); desktop cards follow the project's
  glass scale exactly.
- [ ] No hairlines as decoration; lines are data.
- [ ] Translucency and surface steps match the rest of the app.
- [ ] No glow, neon, gradient chrome or decorative shadow.

**Consistency**
- [ ] Same shell, tokens, type ramp, radius, press response and ground as the rest of the app (and
  across spaces: Dev, Threads and the phone apps look like one family).
- [ ] Settings and sub-pages match the main pages' quality and look.
- [ ] Nothing outside the asked scope changed (Relay: terminal panes untouched).
- [ ] Fonts unchanged unless asked.

**Colour and contrast** (measure on the rendered pixels)
- [ ] Neutral chrome; the accent only for what asks for you, in few places; state colours keep one
  meaning; data may be colourful.
- [ ] Text ≥ 4.5:1, meaningful marks and focus rings ≥ 3:1. Sample with
  `magick shot.png -format "%[pixel:p{x,y}]" info:` and compute the ratio; a thumbnail flatters.
- [ ] Filled vs empty, on vs off, differ by shape as well as tone.

**Actions**
- [ ] One filled primary per surface; secondary outlined; row actions are drawn pills with the row as the tap.
- [ ] Nothing looks tappable that isn't; no nested taps; one back affordance, at the top left.
- [ ] Destructive acts undo; confirms name the act.

**States**: draw and check each: zero, one, many, overflow, loading, error, stale/offline.
- [ ] Zero is the section's own shape at zero, not "No data" rows and not hidden.
- [ ] No spinner or skeleton over local instant data; no dash or n/a for a real state.

**Copy**
- [ ] Specific, dry, in "you"; no exclamation marks, em dashes, hype or unearned praise.
- [ ] Buttons are verbs naming the act; errors say consequence and fix; machine ids translated.
- [ ] Verdicts travel with their reading.

**Icons**
- [ ] One family, one grid and stroke; nothing stock beside custom; optically centred in its key.
- [ ] Toolbar labels under icons where labels are shown; no emoji or unicode glyphs as icons.

**Motion**
- [ ] Motion explains a change; no idle loops on working screens; related items stagger.
- [ ] Popups and sheets ease in smoothly; reduce-motion settles everything.

**Physics** (per `house-style-native`)
- [ ] Text hugs its container; longest realistic string fits; 200% font scale on phones, minimum
  window width on desktop.
- [ ] Touch 44–48 dp / desktop keys ≥ 28 px; keyboard path and visible focus for every action.
- [ ] Screen-reader labels; charts describe their value.

## 3. Rank and report

Rank by what the owner would notice first: something that looks broken or wrong, then crowding and
size, then inconsistency with neighbours, then states nobody drew, then polish. Report as a numbered
list, one line each:

```
1. Button wall: Coach brief: Apply and Skip filled on every row. Make Apply/Skip drawn outlined pills, one filled "Apply all" at the end.
2. Too big: guardrail popup: 520 px wide, three-line body. 380 px, one line, raw reason in the tooltip.
3. Status-word empty: Stats › Cardio at zero: "No data" ×3. Draw the empty week bars with "0 sessions".
```

Then a line for what is good and should stay. The owner keeps what works ("rest is fine"), and
saying it stops the next agent from redesigning it.

## 4. Fix and re-check, bounded

Fix everything in one batch, re-render, run the checklist once more on what changed, and stop. Do
not loop on polish. If a finding is a real taste call (a removal, a moved section, a new pattern),
list it as a question for the owner instead of deciding it, unless they said not to ask.
