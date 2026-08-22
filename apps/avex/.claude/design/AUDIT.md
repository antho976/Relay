# Forge — font-scale & doctrine audit (2026-07-24)

Satellite of `.claude/DESIGN.md`. A snapshot of where the codebase stands against §14 (physics) and
the mechanical rules now enforced by `DesignDoctrineTest`. Not doctrine — a work list.

Regenerate the raw counts with:

```
gradle -p forge-android :app:testDebugUnitTest --tests '*RegenerateAllowlist*' -Dforge.regen=true
```

---

## Correction to the headline number

An initial grep counted **570 fixed `.height()` calls** and that number framed this work as a large
migration. Classifying them says otherwise:

| Bucket | Count | Verdict |
|---|---|---|
| `Spacer`, `Canvas`, `Image`, `Icon`, drawn bars/marks, heights ≤8dp | 517 | **Correct.** A drawn mark has no text to reflow; a fixed height is right. |
| Fixed height, no text nearby — needs a glance | 25 | Charts, sheets and rails. Most are drawn content. |
| Fixed height on something that renders text | 7 | Of these, 5 are false positives (anatomy glyphs and week-dot marks with a *sibling* label, not a child). |

**So the real fixed-height exposure is ~2 sites, not 570.** The scroll of `.height()` calls is mostly
this codebase using `Spacer(Modifier.height(n.dp))` for its spacing rhythm, which is exactly right.

The genuine 200% risk lives somewhere else entirely — in clamped content and the type-scale bypass.

## 1 — Fixed height on a text-bearing container (HIGH, ~2 sites)

- `ui/nav/ForgeBottomBar.kt:60` — `.height(58.dp)` on the `Row` holding all five tab labels.
  At 200% the labels clip to "Cardi" / "Profi". **Confirmed on device 2026-07-25.**
  **The `heightIn(min = 58.dp)` fix recorded here was WRONG — tried and reverted.** The binding
  constraint is horizontal, not vertical: at 200% the label cannot fit a fifth of the screen width,
  so freeing the height just lets it wrap to two lines, which grows the bar into the middle of the
  page and pushes the content off. Height was never what clipped it.
  Real options, none of them one-liners: drop to icon-only above a scale threshold; abbreviate the
  labels; let the bar scroll horizontally; or cap the label's effective scale. **This is a design
  decision, not a mechanical fix** — leave it until it can be made deliberately.
- `ui/gym/freestyle/ExerciseBrowserScreen.kt:351` — `.height(190.dp)` on a section that includes
  rows of text. Worth a look.

Cleared on inspection (drawn marks with an adjacent, not enclosed, label — leave them alone):
`ExerciseBrowserScreen.kt:278/304/330` and `FreestyleLogScreen.kt:608` are `BodyAnatomy` figure
glyphs; `OverviewTiles.kt:72` is a week-dot `Box`; `CardioPaceTrendSection.kt:91`,
`ExerciseChartSheet.kt:117` and `BodyMeasurementsScreen.kt:191` are charts and ghost lines.

## 2 — Clamped user content (HIGH, 39 sites)

`maxLines = 1` on a value the user typed silently truncates instead of wrapping (§14). Confirmed on
`exercise.name`, `day.name`, `folder.displayName`, `file.name` and a lift label. Chrome and mono
labels may clamp; **names and notes may not.**

Most affected: `settings/SettingsSubPages.kt`, `profile/ProfileHeader.kt`,
`gym/train/DaySessionContent.kt`, `gym/session/SessionDetailMetricCards.kt` (3 each).

## 3 — Type-scale bypass (MEDIUM, 293 sites — but ~170 are sanctioned)

§6 allows exactly one off-scale use: 8–9sp figure captions.

| Size | Count | Verdict |
|---|---|---|
| 9sp · 8sp | 152 · 18 | **Sanctioned** (§6 figure captions) — 170 of the 293 |
| 10sp · 11sp · 13sp | 86 · 27 · 6 | Redundant — these ARE `labelSmall` / `labelMedium` / `labelLarge`. Delete the `fontSize`, keep the style. Zero visual change. |
| 52 · 36 · 28 · 22sp | 1 each | Redundant — these ARE `displayLarge` / `headline{Large,Medium,Small}`. |
| 7 · 12 · 14 · 16 · 18sp | 1 · 5 · 2 · 2 · 1 | **Genuinely off-scale.** Decide: promote to a real style, or snap to the nearest rung. |

The ~123 redundant ones are the cheapest debt in this list: a mechanical delete with no visual
consequence, which would take the largest rule count down by ~40%.

## 4 — Everything else the gate froze

| Rule | Count | Note |
|---|---|---|
| `alpha` | 227 | Off-ladder alphas across 42 distinct values against a 7-rung ladder (§5). |
| `divider` | 35 | 20 sit in `gym/train` (untouchable, `SETTLED.md`); the rest migrate to air rhythm (§7). |
| `em-dash` | 32 | Em dashes in **rendered strings** (§11). Each is a one-line copy fix. |
| `screen-name-title` | 31 | `title = { Text(...) }` in a `TopAppBar` — the screen naming itself in the chrome (§4.6). |
| `literal-duration` | 22 | Durations not from `ForgeMotion`, so the system reduce-motion preference is silently ignored (§9). |
| `raw-color` | 10 | 8 are the confetti palette (deliberate); `ExerciseCardComponents.kt:299/318` introduce a second green next to the reserved `△ LAST` `#5BC873`. |
| `bang` | 3 | Exclamation marks in rendered strings (§11). |

**Total frozen debt: 692** across 240 (rule, file) buckets.

## 5 — Contrast

Measured and recorded in `SETTLED.md` under *Open decisions*. Two failures the doctrine itself
mandates — accent-coloured `action →` links (2.35–3.40:1) and the inline error line (3.69:1) — need a
product call, not a silent fix. Everything else measures clean.

## Deferred: real-screen first-run goldens

The screenshot suite covers the six archetype recipes, not real screens. That gap let Profile's BODY
section ship with no marks at all on a new account (see *Empty by omission* in `FAILURES.md`) — the
static gate could not see it, because nothing about it is mechanically wrong, and no golden rendered
it.

The fix is a first-run golden of the real Profile, which needs the Roborazzi setup (this branch) and
the fixed Profile code (`relay/term-3`) in the same tree. Worth doing once they meet: first-run is
the one state every new user sees and the one nobody re-checks. It needs Hilt/ViewModel plumbing in
the test, which the recipes deliberately avoid, so it is real work rather than another `@Test` line.

## Arrived with the Coach v3 merge (2026-07-24)

Sixteen violations came in when `origin/main` merged. They belong to that feature, not to the
doctrine work, and were frozen rather than rewritten: a merge resolution is the wrong place to edit
another branch's UI. Worth a pass when Coach v3 is next touched.

| Rule | Where |
|---|---|
| `font-size` ×2 | `ui/academy/AcademyScreen.kt` |
| `font-size` ×2 | `ui/checkin/CheckinSheet.kt` |
| `font-size` | `ui/academy/LessonBlocks.kt` · `ui/coach/CoachBlockSection.kt` · `ui/coach/CoachProjectSection.kt` · `ui/coach/GoalPickerDialog.kt` · `ui/gym/session/SessionTypeDialog.kt` |
| `screen-name-title` | `ui/coach/GoalPickerDialog.kt` · `ui/gym/session/SessionTypeDialog.kt` |
| `em-dash` | `domain/coach/GoalPortfolio.kt` · `domain/engine/ZoneCoach.kt` |
| `bang` | `ui/overview/OverviewScreen.kt` |
| `max-lines` | `ui/coach/CoachGoalsSection.kt` |
| `alpha` 0.4 | `ui/coach/GoalPickerDialog.kt` |

The four copy ones (`em-dash` ×2, `bang`) are one-line fixes with no visual risk and would be the
cheapest thing to clear.

## CLEARED 2026-08-20 — settings pass

`ui/settings/` was rebuilt against §3 (`design/DECISIONS.md`, 2026-08-20). **62 violations paid down,
none added; baseline 933 → 871.** Settings now carries zero debt on six of the nine rules: `divider`,
`em-dash`, `font-size`, `max-lines`, `screen-name-title` and `unlabelled-clickable`. What remains
there is 1 alpha (the §8-specified `StatusDot` 0.55 ring), 2 `max-lines` on chrome, 3 `font-size` at
the sanctioned 9sp, and 2 `unlabelled-clickable` in the accent colour wheel.

Two items below are fully resolved by that pass and are struck rather than deleted, so the trail holds:

- **Unthemed M3 container tones** — fixed at the ROOT. `pearlColorScheme` now sets the whole
  `surfaceContainer*` ladder, `outlineVariant` and `surfaceTint`, so the per-call-site debt below no
  longer exists for anyone. The settings sheets/pickers named in it are done; the remaining call
  sites (`AvatarPickerSheet`, `BodyMeasurementLogSheet`, `RankSection`, `MirrorTestScreen`,
  `ProgramBuilderDayDetail`, `CardioLogSheetSections`, `MirrorTestViewer`) inherit the fix and only
  owe the explicit `containerColor = surface` that §5 asks for as documentation.
- **§3's redundant `fontSize`** — 31 of the ~123 were in settings and are gone.

Still open and untouched by this pass: `ForgeBottomBar`'s 200% label clipping (§1 below), the
`literal-duration` and `alpha` backlogs outside settings, and the deferred first-run goldens.

## Unthemed M3 container tones (2026-07-25)

`ForgeTheme` sets `background`/`surface`/`surfaceVariant`/`outline`/`primary`, but never the
`surfaceContainer*` family, `outlineVariant` or `surfaceTint`, so anything reaching for one silently
falls through to **Material's stock dark palette** — a lighter, purple-leaning grey that belongs to no
theme here. `ModalBottomSheet` defaults to `surfaceContainerLow`, `DatePickerDialog` to
`surfaceContainerHigh`, its header rule to `outlineVariant`, and a `TextButton` to navy-on-that
(~1.6:1). Found on device: the body-log sheets read as a pale slab on near-black.

§5 states the call-site rule (a modal is `containerColor = surface`). These still owe it:

| Kind | Where |
|---|---|
| sheets | `AvatarPickerSheet` · `BodyMeasurementLogSheet` · `RankSection` · `MirrorTestScreen` · `ProgramBuilderDayDetail` · ~~`AppIconPicker`~~ |
| pickers | `CardioLogSheetSections` · `MirrorTestViewer` · ~~`SettingsVacationPage`~~ |

**The one-line root fix — give `pearlColorScheme` real Pearl container tones — landed 2026-08-20.**
No call site falls through to Material stock any more; the ones above owe only the explicit
`containerColor = surface` §5 asks for as a statement of intent.

## Suggested order

1. **`em-dash` (32) + `bang` (3)** — pure copy, no visual risk, closes §11 completely.
2. **Redundant `fontSize` (~123)** — mechanical delete, zero visual change.
3. **`maxLines = 1` on names (39)** — the real 200% exposure.
4. **`ForgeBottomBar` height** — one line.
5. **`literal-duration` (22)** — each one currently ignores reduce-motion.
6. **`alpha` (227)** — largest, do it per-file as screens are touched.
7. **`screen-name-title` (31)** — needs a serif hero added where the top-bar title is removed, so it
   is design work per screen, not a sweep.
