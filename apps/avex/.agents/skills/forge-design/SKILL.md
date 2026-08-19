---
name: forge-design
description: MANDATORY before any UI work in forge-android — creating or changing Compose screens, components, layouts, colors, fonts/typography, spacing, buttons/controls, motion/animation, charts, empty states, inputs, or any user-facing text/copy generation. Loads the binding Forge design doctrine (.Codex/DESIGN.md) that all UI must follow. Trigger on words like: UI, screen, redesign, restyle, layout, design, look, theme, color, font, animation, button, page, component, copy, text.
---

# Forge design doctrine — router

This file routes; it does not restate. An earlier version summarised the doctrine and drifted from
it — it named the wrong wordmark and taught a verdict the doctrine bans by name. Rules now live in
ONE place so they cannot disagree with themselves. Read the files below rather than trusting a
summary of them.

## 1. Read the core, now

**`.Codex/DESIGN.md`** — in full, before writing or planning any UI code. It is binding, ~400 lines,
and §0 tells you how to use it. Do not skip it because the change looks small.

## 2. Pick the archetype, then open its recipe

The archetype (§3) determines which toolkit is legal. Each has a compiling reference under
`forge-android/app/src/debug/java/com/forge/app/ui/recipes/`:

| Building | Recipe |
|---|---|
| Home, Stats, Cardio, Coach, Profile | `OverviewRecipe.kt` |
| a session / lift / entry page | `DetailRecipe.kt` |
| History, Goals, Trophies, a picker | `ListRecipe.kt` |
| settings, an editor, onboarding | `SettingsRecipe.kt` |
| live session, freestyle log | `LiveRecipe.kt` |
| a sheet or dialog | `ModalRecipe.kt` |

**Copy the recipe's scaffold rather than composing a screen from prose.** They are debug-only so they
never ship, but they compile against the real primitives — if one drifts, the build breaks. Each
shows the section rhythm, where the mark goes, the zero-state branch inline, and a 200% font preview.

## 3. Pull satellites only when the question arises

| File | Read it when |
|---|---|
| `.Codex/design/MAP.md` | "what already exists?" / "why is this here?" |
| `.Codex/design/SETTLED.md` | **before re-adding anything that feels missing** — most obvious improvements were already tried and cut. Also holds the open contrast decisions. |
| `.Codex/design/FAILURES.md` | a layout feels off and you want the named diagnosis |
| `.Codex/design/AUDIT.md` | picking up doctrine debt |
| `.Codex/design/DECISIONS.md` | why a rule exists, or why it changed |
| `.Codex/design/WEAR.md` | touching `:wear` only |

## 4. Finish

Run the §15 checklist against every touched screen, then:

```
gradle -p forge-android :app:testDebugUnitTest
```

Three suites run: `DesignDoctrineTest` (the mechanical rules), `DoctrineParityTest` +
`DoctrineSelfCheckTest` (the doc's stated values must equal the code's), and `RecipeScreenshotTest`
(golden images of the six archetypes at 100% and 200% font scale). If a golden changed, look at the
diff before re-recording with `:app:recordRoborazziDebug`.

`DesignDoctrineTest` enforces the mechanical rules (the alpha ladder, the type scale, dividers,
banned characters in rendered strings, clamped content, literal durations, screen names in top bars).
Existing debt is frozen in `app/src/test/resources/design-allowlist.txt`, so a failure means **your**
change. The message names the rule, the offending line and the exact fix.

## 5. During a redesign

Rewriting screens usually *removes* old violations. The gate treats that as a change too, so it
fails with "debt was PAID DOWN here, lower these numbers". That is deliberate, but hand-editing the
allowlist across many files is a slog, so bank the wins in one go:

```
gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' \
  -Dforge.paydown=true --rerun-tasks
```

This **lowers counts and refuses to raise any**. If the same pass introduced a new violation it
fails and writes nothing, naming the offender. Safe to run whenever the gate complains about
paydown.

`-Dforge.regen=true` is the other one, and it is NOT the same: it accepts everything, including new
violations. Use it only after a deliberate doctrine change (a new rule, a changed ladder), never to
turn a red build green.

**`--rerun-tasks` matters.** A system property alone does not invalidate the Gradle test task, so
without it the command silently does nothing and reports success.

Screenshot goldens follow the same shape: `:app:verifyRoborazziDebug` fails on any visual change,
and `:app:recordRoborazziDebug` accepts the new pixels. Look at the diff before you re-record; the
goldens cover 100% and 200% font scale, AMOLED and monochrome, and a change in any of those is a
question worth answering.

## 6. If you change a decision

Write it down the same turn. New *rules* → `DESIGN.md`. New *screens/features* → `MAP.md`.
*Removals* → `SETTLED.md`. A newly named mistake → `FAILURES.md`. If the rule is mechanically
checkable, add it to `DesignDoctrineTest` instead of relying on it being read.

`DESIGN.md` is capped at 400 lines with no headroom by design: adding to it means finding something
that can leave, move to a satellite, or become a test.
