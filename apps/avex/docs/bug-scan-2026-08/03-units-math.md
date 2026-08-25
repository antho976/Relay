# Units, weight math & numeric correctness — pre-release audit

Scope: `domain/units/`, `shared/weight/`, `domain/pr/`, `domain/volume/`, `domain/measurement/`,
`domain/parser/WeightParser.kt`, `domain/adapt/E1rm.kt` + `ProgressionAdvisor.kt`,
`ui/gym/train/components/SetInputRow.kt` / `SetRow.kt`, `ui/gym/freestyle/`, the wear mirror /
`SetLogUseCase`, and the profile log sheets.

## What is already correct (so it isn't re-litigated)

- Conversion constants are exact, not the sloppy 2.2: `KG_PER_LB = 0.45359237`
  (`WeightFormatter.kt:7`), `CM_PER_INCH = 2.54` (`LengthFormatter.kt:5`), `KM_PER_MILE = 1.609344`
  (`DistanceFormatter.kt:5`). Only `FEET_PER_METER = 3.28084` (`ElevationFormatter.kt:5`) is
  truncated, and its 3e-7 relative error is irrelevant at whole-foot display.
- The canonical stored unit is pounds everywhere, and `SettingsRepository.setWeightUnit`
  (`SettingsRepository.kt:277-299`) only writes preference keys — it never re-converts stored rows.
  **There is no double-conversion or drift-on-repeated-toggle bug at the settings layer.**
- Reps are clamped at the repository boundary (`WorkoutRepository.kt:58`).
- `NaN` / `Infinity` cannot reach the DB through the text parsers: every parser lowercases before
  `toDoubleOrNull()`, and Java's `parseDouble` only accepts the exact tokens `NaN` / `Infinity`
  (`WeightParser.kt:26`, `WeightFormatter.kt:135`, `LengthFormatter.kt:49`, `DistanceFormatter.kt:41`).
  `WeightParser.kt:44` additionally rejects `NaN` via `takeIf { it >= 0.0 }`.
- `plateWeightLb` is coerced to `1.0..200.0` (`SettingsRepository.kt:772`), so no divide-by-zero in
  the many `lb / plateLb` expressions.
- `E1rm.epley` guards `reps <= 1`, and `epleyInverse` can never divide by zero (`E1rm.kt:16-25`).
- `SetRow`'s inline edit already defends against display→lb round-trip drift by handing back the
  full-precision display value when the field is untouched (`SetRow.kt:200-210`). That mitigation is
  good and should be copied to the other seeding paths (see HIGH #5).

---

## [CRITICAL] A comma-decimal locale silently logs a weightless set

**File:** `app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:161-175` (`onWeightChange`, no character filter) and `:328-333` (`KeyboardType.Text`); `app/src/main/java/com/forge/app/ui/gym/train/components/SetRow.kt:180-186` (edit field, `KeyboardType.Decimal`, `onValueChange = { editWeight = it }` — no filter); `app/src/main/java/com/forge/app/domain/units/WeightFormatter.kt:134-154`; `app/src/main/java/com/forge/app/domain/parser/WeightParser.kt:44`; `app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogScreen.kt:322`

**What:** The weight field accepts arbitrary text with no normalisation of the decimal separator. On
a comma-decimal locale (de/fr/es/it/pt/nl/ru/tr/pl…) the IME's decimal key emits `,`. Every parser in
the chain is `'.'`-only:

- `parseToLb("82,5", KG)` → `"82,5".toDoubleOrNull()` → `null`
- `toStoredWeightText` (`WeightFormatter.kt:152`) then does `?: return trimmed` → passes `"82,5"`
  through **verbatim** as the "canonical lb text"
- `WeightParser.parse("82,5", …)` (`WeightParser.kt:44`) → `null`
- `WorkoutRepository.logSet` writes `LoggedSet(weightText = "82,5", weightLb = null)`

`weightLb = null` is the app's encoding for *bodyweight*. The set is therefore permanently excluded
from volume (`VolumeCalculator.kt:14`), PR detection (`PrDetector.kt:26`), e1RM
(`E1rm.kt:34`), the strength curves, Hall of Fame and the rep-max frontier. Nothing in the UI
reports it: `canSubmit` (`SetInputRow.kt:214-217`) only checks `weight.isNotBlank()`, and
`SetRow.kt:117` renders `set.weightLb?.let{…} ?: set.weightText`, so the row displays "82,5" and
looks correctly logged.

**Scenario:** Device locale `de-DE`, unit kg. User logs Bench Press `82,5` × 8.
Expected: `weightLb = 181.88`, session volume `+1455.1 lb`, PR evaluated.
Actual: `weightLb = null`, session volume `+0`, never a PR, absent from every chart — while the set
row reads "82,5" and the workout summary reads "0 lb volume". A whole training history is silently
hollowed out for every comma-locale user. The same input through the freestyle logger takes the
identical path (`FreestyleLogScreen.kt:322`).

**Fix:** Normalise at the input boundary before any parse. Add a shared
`fun normalizeDecimal(s: String) = s.replace(',', '.')` in `WeightFormatter.kt` and call it first in
`parseToLb`, `parseToCm`, `parseToKm`, `parseToMeters` and `WeightParser.parse`. Additionally, make
`toStoredWeightText` and the submit paths refuse to store a non-`"BW"`/non-plate weight string that
`WeightParser` cannot parse, rather than writing `weightLb = null` silently — surface an inline
"couldn't read that weight" error on the field.

---

## [CRITICAL] Freestyle stores `weightText` in the DISPLAY unit, breaking the "weightText is always lb" invariant

**File:** `app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogScreen.kt:322`, `:327`, `:339`; `app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogViewModel.kt:108`

**What:** The invariant is documented in two places —
`app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:215`
("Canonical lb weight string (weightText is always stored in lb)") and
`app/src/main/java/com/forge/app/data/db/entities/LoggedSet.kt:12-17` — and the structured Train
flow honours it by converting on submit
(`app/src/main/java/com/forge/app/ui/gym/train/DaySessionContent.kt:271`, `:277`:
`toStoredWeightText(weight, weightUnit)`).

Freestyle does not. It converts only the numeric column and stores the raw field text:

```kotlin
val weightLb = if (ex.bodyweight) null else parseToLb(s.weight, weightUnit)   // :322  correct lb
...
weightText = if (ex.bodyweight) "" else s.weight.trim(),                      // :339  DISPLAY unit
weightLb   = weightLb,
```

So one `logged_set` row carries `weight_text = "100"` and `weight_lb = 220.462…` — the two columns
disagree by the conversion factor, and the DB now contains rows in two different conventions with no
discriminator.

**Scenario:** Unit = kg. User freestyle-logs Bench `100` × 5 → row `{weightText:"100", weightLb:220.46}`.
Two days later on the structured Train day, the wrist prefill path reads the *text* column:
`SetLogUseCase.kt:117` (`lastPerformanceSets(...).weightText` → `"100"`) → `SetLogUseCase.kt:122`
`WeightParser.parse("100", WEIGHT)` → **100.0 lb (45.4 kg)**. The watch logs 45 kg for a lift the app
knows was 100 kg. The same text column is what `WatchSessionMirror.kt:113`/`:150` sends to the wrist
as the target weight, what `ExerciseCard.kt:215` and `ExerciseCardComponents.kt:121` print as
"last session 100 × 5", what `DayViewModelBuilders.kt:89`/`:132` print as the last-session preview
and the all-time PB, and what `BackupRepository.kt:203`/`:305` writes into the export JSON. Because
the structured rows in the same list hold lb text, the two conventions are interleaved and no
consumer can tell them apart.

**Fix:** In `FreestyleLogScreen.save()` set `weightText = toStoredWeightText(s.weight.trim(), weightUnit)`
(and skip conversion for plate-count exercises exactly as `DaySessionContent.kt:271` does). Add a
unit test asserting `WeightParser.parse(row.weightText) ≈ row.weightLb` for every write path. A
one-off migration cannot repair existing rows (the unit at write time isn't recorded) — ship the fix
before release.

---

## [CRITICAL] The watch shows stored LB numbers under the user's kg/st label and steps in lb

**File:** `app/src/main/java/com/forge/app/service/wear/WatchSessionMirror.kt:112-115`, `:140-142`, `:147-151`; `wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:58-60`, `:99-111`, `:147`, `:169`, `:223`, `:334-339`; `app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt:115-122`

**What:** The mirror publishes `targetWeightText` = the raw stored **lb** text
(`WatchSessionMirror.kt:113` and `:150`), alongside `unit = protoUnit` (the user's display unit) and
`weightStep = WeightSteps.weightStep(protoUnit, isPlates)` — a step expressed in the **display**
unit (2.5 for KG, 0.5 for ST, `shared/.../WeightSteps.kt:23-28`). The watch never converts:

- `SessionScreen.kt:60` seeds `weightValue = session.targetWeightText?.toDoubleOrNull()` — an lb number
- `SessionScreen.kt:169`/`:179` renders it next to `unitLabel(session)` → "KG"
- `SessionScreen.kt:110` steps it: `weightValue + detents * weightStep` — adds a *kg-sized* step to an *lb* number
- `SessionScreen.kt:223` sends the result back as `weightText`
- `SetLogUseCase.kt:122` parses that back as **lb**

The phone gets this right by contrast — `SetInputRow.stepWeight` (`SetInputRow.kt:202-208`) steps a
field that already holds a display-unit value, and `DaySessionContent.kt:271` converts on submit.
Only the wrist skips both halves.

**Scenario A (display is flatly false).** Unit = kg, last set 100 kg (stored `"220.5"`).
Wrist shows the figure **220.5** with the caption **KG**. The user is told they are about to lift
220.5 kg (486 lb).

**Scenario B (steps are 2.2× too small).** From that screen the user spins one detent to add 2.5 kg.
The watch computes `220.5 + 2.5 = 223.0`, displays "223 KG", and logs `WeightParser.parse("223")` =
**223.0 lb = 101.15 kg**. Intended +2.5 kg, got +1.13 kg, and the wrist claimed 223 kg.

**Scenario C (worst — a slot with no target).** Bodyweight/first-time slot: `targetWeightText` is
null → `weightValue` starts null → `step()` builds from `0.0`. The user spins to what reads
"60 KG" (24 detents × 2.5) and taps Log → **60.0 lb (27.2 kg)** is written. A 60 kg set is recorded
as 27 kg, permanently, with no on-device signal.

**Scenario D (stones).** Unit = st, stored `"220.5"` → wrist reads "220.5 ST" ≈ 1.4 tonnes.

**Fix:** Convert at the publisher/consumer boundary, the same way the phone does. In
`WatchSessionMirror.buildDto` send `targetWeightText = weightInputValue(lastLb, unit)` (display
unit); in `SetLogUseCase.logFromWatch` convert the incoming `cmd.weightText` back with
`toStoredWeightText(cmd.weightText, settingsRepo.weightUnit.first())` before
`WeightParser.parse` — but keep plate-count exercises passing through unconverted, matching
`DaySessionContent.kt:271`. Also carry an explicit `unitOfWeightText` flag in `LogSetCommand` so a
stale command from an older watch build can be rejected rather than mis-parsed.

---

## [CRITICAL] `formatPlateCount` is locale-formatted and seeds the plate-count input field

**File:** `app/src/main/java/com/forge/app/ui/gym/train/components/SetRow.kt:83-84`; consumed at `SetRow.kt:119` (edit seed), `app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:120` (log seed) and `:284` (tap-to-autofill)

```kotlin
internal fun formatPlateCount(plates: Double): String =
    if (plates % 1.0 == 0.0) plates.toInt().toString() else "%.1f".format(plates)
```

**What:** `"%.1f".format(x)` is Kotlin's `String.format` **extension**, which uses
`Locale.getDefault()` — unlike every other formatter in `domain/units/`, which correctly pins
`Locale.US`. On a comma-decimal locale this emits `"2,5"`, and that string is written straight into
the editable weight field. PLATES exercises deliberately bypass `toStoredWeightText`
(`DaySessionContent.kt:271`: `if (ex.plan.unit == ExerciseUnit.PLATES) weight else …`), so `"2,5"`
goes untouched to `WeightParser.parse`, whose plate regex `^([0-9]*\.?[0-9]+)\s*(plates?|p)$` and
`toDoubleOrNull` both reject it.

**Scenario:** Locale `fr-FR`, plate weight 15 lb, prior set 37.5 lb (2.5 plates).
Opening the input row seeds the field with `"2,5"` (`SetInputRow.kt:120`). Tapping LOG SET stores
`{weightText:"2,5", weightLb:null}` — a 37.5 lb set recorded as bodyweight, contributing 0 to volume
and never a PR.
Secondary effect: pressing the `+` stepper on that field runs `stepWeight` (`SetInputRow.kt:203`),
where `"2,5".toDoubleOrNull()` is `null` → `base = 0.0` → the field jumps to **"0.5"**, silently
throwing away the 2.5-plate seed.

**Fix:** `String.format(Locale.US, "%.1f", plates)`. Then sweep every remaining bare
`"%.<n>f".format(...)` / `"%,d".format(...)` in the codebase (30 sites; only the 27 explicit
`String.format(Locale.US, …)` calls are safe) and pin any that feeds an input field, an ID, or a
value that is later re-parsed. Consider a lint rule banning the bare extension.

---

## [HIGH] `prefillWeight` puts raw stored LB text into a kg/st-labelled input field

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DayViewModelBuilders.kt:200` (`prefillWeight = prevFirstSet?.weightText`); `app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:119-121`; `app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCard.kt:372`, `:378`

**What:** `seedWeight` prefers `priorSetForActiveRow` (converted correctly via `weightInputValue`)
but falls back to `prefillWeight.orEmpty()` — the **unconverted** `weightText` column. That fallback
is live whenever `state.priorSets.getOrNull(state.loggedSets.size)` is null
(`ExerciseCard.kt:378`), i.e. on every bonus set beyond last session's set count. The field is
labelled "WEIGHT · KG" (`SetInputRow.kt:322`), and submit runs
`toStoredWeightText(weight, weightUnit)` (`DaySessionContent.kt:271`), so whatever sits there is
interpreted as kg.

**Scenario:** Unit = kg. Last session: 2 sets of Bench at 100 kg (stored `"220.5"`). This session the
user does 3 sets. On set 3 the field pre-fills **"220.5"** under the KG label. They tap LOG SET
unchanged → `toStoredWeightText("220.5", KG)` = `220.5 / 0.45359237` = **486.1 lb**. A 100 kg set is
recorded as 220.5 kg — a 2.2× inflation that also poisons the PR frontier, the jump-warning baseline
and every future progression suggestion for that lift.

**Fix:** Make `prefillWeight` carry lb, not text: change `DayUiState.prefillWeight` to
`prefillWeightLb: Double?` (from `prevFirstSet?.weightLb`) and seed with
`weightInputValue(lb, weightUnit)` / `formatPlateCount(lb / plateLb)`, matching
`SetInputRow.kt:120`. Same change for the "Use last: …" hint at `SetInputRow.kt:575-577`.

---

## [HIGH] "last session" / all-time-PB / preview lines print raw lb text under a kg or st setting

**File:** `app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCard.kt:215`; `app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCardComponents.kt:121`; `app/src/main/java/com/forge/app/ui/gym/train/DayViewModelBuilders.kt:89` (`"Last: ${it.weightText} × ${it.reps}"`) and `:132` (`allTimePbText`)

**What:** Four read surfaces interpolate `weightText` directly instead of formatting `weightLb`
through `formatWeight(lb, weightUnit)` — which is exactly what the set rows immediately below them
do (`SetRow.kt:114-117`).

**Scenario:** Unit = kg, a 100 kg × 5 bench (stored `"220.5"` / `220.5`).
The card header reads **"Target 3 × 8 · last session 220.5 × 5"** while the ledger rows underneath
read **"100 kg × 5"**. Same set, two numbers 2.2× apart, on one screen. In stones it reads
"220.5 × 5" against "15 st 11 lb". The all-time-PB line has the same defect.

**Fix:** Replace each with `priorLastSet.weightLb?.let { formatWeight(it, weightUnit) } ?: priorLastSet.weightText`
(and the plate-count variant where `isPlates`), i.e. reuse `SetRow.kt:114-117`'s `displayWeight`
expression. `preview` and `allTimePbText` should move out of the ViewModel builder into the
composable so they can see `LocalForgeSettings.current.weightUnit`.

---

## [HIGH] The progression suggestion is an unconverted lb number shown to kg/st users

**File:** `app/src/main/java/com/forge/app/ui/gym/train/DayViewModelBuilders.kt:166` (`displaySuggested = suggestion?.inputText`) and `:212`; rendered at `app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCard.kt:227-228`; produced in lb by `app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt:209-238`

**What:** `Recommendation.WeightChange.inputText` is documented as "text for the weight field …
plain lb" (`ProgressionAdvisor.kt:499-501`). `ExerciseCard.kt:228` renders it with no conversion and
**no unit label**: `"Suggested next → ${state.suggestedWeight}"`. The input field two rows below is
labelled KG. (The legacy layout at `SetInputRow.kt:543` at least appends a literal `" lb"`, so the
two surfaces also contradict each other.)

**Scenario:** Unit = kg. Last session's top set 100 kg (stored 220.5 lb). `dumbbellStepLb = 2.5`
(`AdaptThresholds.kt:14`), so `floorToGrid(220.5 + 2.5, 2.5)` = **222.5**. The card says
"Suggested next → 222.5" directly above a field marked "WEIGHT · KG". The user types 222.5 →
`222.5 / 0.45359237` = **490.5 lb (222.5 kg)**, more than double what the coach meant (101 kg).

**Fix:** Convert at render: for non-PLATES units show
`weightInputValue(suggestion.targetWeightLb, weightUnit)` plus `unitLabel(weightUnit)`. Keep the
plate-count text (`"3 plates"`) as-is. `suggestedTargetLb` is already carried on the state
(`DayUiState.kt:236`), so no plumbing is needed.

---

## [HIGH] `ProgressionAdvisor.trim` leaks full Double precision into the suggestion chip

**File:** `app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt:517`, consumed at `:121`, `:224` and `:508`

```kotlin
private fun trim(v: Double): String = if (v % 1.0 == 0.0) "${v.toInt()}" else "$v"
```

**What:** The `progressSuggestion`/`backOffSuggestion` paths snap through `floorToGrid` first, so
`"$v"` is usually clean. Three call sites do **not** snap: `sameWeightInput` (`:121`, used by the
hold-the-weight recommendations at `:129` and `:143`), the "at your heaviest dumbbell" branch
(`:224`), and the off-grid plate fallback (`:508`). All three pass `prevMax` — a raw
`logged_set.weight_lb` — straight to `trim`.

**Scenario:** A set logged through the freestyle logger stores full-precision lb
(`FreestyleLogScreen.kt:322`: `parseToLb("100", KG)` = `220.46226218487757`; the structured path
rounds to `"220.5"` first, which is why this only bites freestyle-sourced history). `prevMax` is
then `220.46226218487757` and the card renders
**"Suggested next → 220.46226218487757"**. `"$v"` also produces scientific notation
(`"1.0E9"`) for an absurd entry, which `WeightParser` cannot parse back.

**Fix:** `private fun trim(v: Double): String = if (v % 1.0 == 0.0) "${v.toInt()}" else String.format(Locale.US, "%.1f", v)`
— identical to `WeightFormatter.trimDecimal` (`WeightFormatter.kt:33-34`); better still, delete it and
call `weightInputValue`.

---

## [HIGH] Comma-decimal locales cannot enter a decimal in the bodyweight / body-fat / measurement sheets

**File:** `app/src/main/java/com/forge/app/ui/profile/BodyweightLogSheet.kt:197-207` and `:82` seeding; `app/src/main/java/com/forge/app/ui/profile/BodyFatLogSheet.kt:80-83` and `:129-139`; `app/src/main/java/com/forge/app/ui/profile/BodyMeasurementLogSheet.kt:98-107`

**What:** All three fields declare `KeyboardType.Decimal` — which on a comma locale renders `,` as
the decimal key — and then filter it out:

```kotlin
val f = v.filter { ch -> ch.isDigit() || ch == '.' }
```

The separator is silently deleted rather than translated, so the typed value is multiplied by 10^n.
Separately, `BodyFatLogSheet.kt:82` seeds the field with the locale-formatted
`"%.1f".format(seed)`, which on the same device produces a string its own parser
(`parseSaneBodyFat`, `:41-42`) rejects.

**Scenario A:** Locale `de-DE`, unit kg. User types their weight `82,5`. The filter yields `"825"` →
`parseSaneBodyweightLb("825", useKg=true)` = 1819 lb, outside `60.0..1000.0`
(`OnboardingScreen.kt:109-117`) → `invalid = true`, Save stays disabled, supporting line reads
"Enter 27–454 kg" while the field visibly shows 825. The user cannot record a non-integer weight at
all.

**Scenario B:** Same device, body fat. A stored 18.5 % re-opens as `"18,5"` (`BodyFatLogSheet.kt:82`)
→ `parseSaneBodyFat` returns null → the sheet opens **already in an error state** with Save locked,
for a value the app itself wrote.

**Scenario C:** Measurements. Waist `82,5` cm → `"825"` → outside `MIN_CM..MAX_CM` (5.0..300.0,
`BodyMeasurementType.kt:22-23`) → rejected.

**Fix:** In all three `onValueChange` filters, map `,` → `.` before filtering
(`v.replace(',', '.').filter { it.isDigit() || it == '.' }`), and pin the seeds to
`String.format(Locale.US, "%.1f", …)` — `BodyweightLogSheet` and `BodyMeasurementLogSheet` already
seed correctly via `weightInputValue` / `lengthInputValue`; only `BodyFatLogSheet.kt:82` needs it.

---

## [MEDIUM] Seeding a set in stones re-quantises the weight to the nearest 1.4 lb

**File:** `app/src/main/java/com/forge/app/domain/units/WeightFormatter.kt:33-34` (`trimDecimal`, one decimal) and `:104` (`weightInputValue`); consumed at `app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:120`

**What:** Stones renders as a single decimal, so the seeded input has a granularity of
0.1 st = **1.4 lb**. `SetRow`'s edit path already guards against the resulting drift
(`SetRow.kt:200-210` returns the full-precision display value when the field is untouched), but the
*log* path in `SetInputRow` has no equivalent guard: whatever the seed says is what gets converted
back on submit.

**Scenario:** Unit = st. Prior set 135 lb. Seed = `trimDecimal(135/14)` = `trimDecimal(9.642857…)` =
**"9.6"**. User taps LOG SET to repeat the same weight → `parseToLb("9.6", ST)` = `9.6 × 14` =
**134.4 lb**. The set they intended as "same as last" is logged 0.6 lb lighter, and because
`WEIGHT_DELTA_EPS_LB = 0.5` (`SetRow.kt:74`) that exceeds the tolerance, so the row renders a
phantom **"−0.6 lb"** delta and a downward trend arrow. In kg the same mechanism moves 225 lb →
"102.1" → 225.09 lb, which the 0.5 lb epsilon does absorb.

**Fix:** Port `SetRow.kt:200-210`'s mitigation into `SetInputRow.submitSet` — if the field text is
still byte-identical to `seedWeight` and the prior set has a `weightLb`, submit
`toDisplayWeight(priorLb, unit).toString()` instead of the rounded seed. Alternatively seed stones
with two decimals (0.01 st = 0.14 lb, inside the epsilon).

---

## [MEDIUM] A 0 lb set is treated as a personal record

**File:** `app/src/main/java/com/forge/app/domain/pr/PrDetector.kt:26-32`; `app/src/main/java/com/forge/app/domain/parser/WeightParser.kt:44`; `app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:238-251`

**What:** `PrDetector`'s doc says "Sets without a numeric weight … never count as PRs", but
`WeightParser.kt:44` accepts `0.0` (`takeIf { it >= 0.0 }`), so a typed `"0"` yields
`weightLb = 0.0`, not `null`, and sails past the `newWeightLb == null` guard. When no prior set
competes at the same-or-higher rep count, `PrDetector.kt:31` returns `true` unconditionally.

The day screen partially hides this behind `computePrFlags`' `hasPriorHistory` gate
(`DayViewModelBuilders.kt:274`), but the repository-level pass used by **freestyle**
(`FreestyleLogViewModel.kt:121`) and the **watch** (`SetLogUseCase.kt:165`) has no such gate.

**Scenario A:** Freestyle-log a brand-new exercise as `0` × 5. `flagPrForLoggedExercise` →
`PrDetector.isPr(emptyList(), 0.0, 5)` → `true` → `wasPr = true` persisted, the lifetime PR counter
increments and the gold ★ appears for a 0 lb set.
**Scenario B (with history):** History = `[100 lb × 5]`. Log `0` × 20. The filter `reps >= 20`
returns nothing → `?: return true` → a 0 lb set is a PR over a 100 lb one.

**Fix:** Treat 0 as bodyweight at the parser (`takeIf { it > 0.0 }` in `WeightParser.kt:44`) or add
`if (newWeightLb <= 0.0) return false` to `PrDetector.isPr`. Add regression tests —
`PrDetectorTest.kt` currently covers `null` weight but never `0.0`.

---

## [MEDIUM] The "N for PR" hint uses a different rule from `PrDetector` and ignores assisted sets

**File:** `app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:766-772` (`repsNeededForPr`) vs `app/src/main/java/com/forge/app/domain/pr/PrDetector.kt:27-32`

**What:** `PrDetector` compares against `history.filter { weightLb != null && !isAssisted && reps >= newReps }`.
The hint compares against `history.filter { weightLb != null && weightLb >= weightLb }` — it drops
the `!isAssisted` filter entirely and inverts which dimension is thresholded.

**Scenario:** History for Pull-up = one **assisted** set at 100 lb × 10. User types 100 in the
weight field. Hint renders **"11 for PR"** (`maxRepsAtOrAbove = 10`, +1). But `PrDetector.isPr`
excludes assisted sets, so its competing population is empty and **any** rep count — including 1 —
already qualifies. The app tells the user they need 11 reps for a record they would get at 1.

**Fix:** Replace `repsNeededForPr` with a search that calls `PrDetector.isPr(priorSets, weightLb, n)`
for ascending `n` and returns the first `n` that answers true (bounded to the plan's rep range), so
the hint and the flag can never diverge.

---

## [MEDIUM] Unit suffixes are stripped without checking they match the active unit

**File:** `app/src/main/java/com/forge/app/domain/units/DistanceFormatter.kt:41`; `app/src/main/java/com/forge/app/domain/units/LengthFormatter.kt:49-50`; `app/src/main/java/com/forge/app/domain/units/ElevationFormatter.kt:34`; `app/src/main/java/com/forge/app/domain/units/WeightFormatter.kt:137-139`

**What:** Each parser strips *both* unit suffixes and then applies the *setting's* conversion,
so a suffix that contradicts the current unit is silently ignored rather than honoured or rejected:

```kotlin
val cleaned = input.trim().lowercase().removeSuffix("mi").removeSuffix("km").trim()
return fromDisplayDistance(numeric, useMiles)   // DistanceFormatter.kt:41-43
```

**Scenario A (wrong stored number):** Distance unit = miles. User types `"5 km"` in the cardio
distance field. `parseToKm` strips `"km"`, then multiplies by `KM_PER_MILE`: **8.05 km stored** for
a 5 km run — a 61 % overstatement of distance, pace and the calorie estimate derived from it.
The same shape applies to `parseToCm("32 in", useCm = true)` → 32 cm (should be 81.3), and
`parseToMeters("120 m", useMiles = true)` → 36.6 m.

**Scenario B (silent weight loss):** Weight unit = lb. User types `"20 kg"`. `parseToLb`'s LB branch
strips only `"lb"`, so `"20 kg".toDoubleOrNull()` is null → `toStoredWeightText` passes the text
through → `WeightParser.parse("20 kg")` matches no pattern → `weightLb = null` → another invisible
bodyweight set (same end state as the CRITICAL locale finding, different trigger).

**Fix:** Parse the suffix, don't discard it: extract `(number, suffix?)`, and when a suffix is
present convert *from that unit* (`"5 km"` → 5 km regardless of the toggle; `"20 kg"` → 44.1 lb).
Reject an unrecognised trailing token rather than falling through to `null`.

---

## [MEDIUM] Freestyle writes a session volume computed from unclamped reps

**File:** `app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogViewModel.kt:108` vs `:116` and `:123`; `app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:58`, `:615`

**What:** `workoutRepo.logSet(...)` clamps reps to `0..999` (`sanitizeReps`, `WorkoutRepository.kt:58`),
but the volume accumulator two lines later uses the **raw** input value:

```kotlin
val setId = workoutRepo.logSet(loggedExerciseId, setIdx, s.weightText, s.weightLb, s.reps, …) // clamped
totalVolumeLb += (s.weightLb ?: 0.0) * s.reps                                                // unclamped
```

`finishSession` then denormalises that number onto the session row (`:123`). The structured Train
flow is immune because it re-reads sets from the DB first
(`DaySessionHandlers.kt:85`, `VolumeCalculator.sessionVolumeLb(allSets)`).

**Scenario:** Freestyle-log 100 lb × `5000` reps (the reps field has no digit cap —
`FreestyleLogScreen.kt:760` is just `new.filter { it.isDigit() }`). The set persists with
`reps = 999` (99,900 lb of volume) but `session.total_volume_lb` is stamped **500,000 lb**. The
history card, the profile ledger and `XpEngine.kt:38` all read the stamped number and permanently
disagree with the set list by 5×.
Related: 11+ digits (`"99999999999"`) overflow `Int`, so `s.reps.toIntOrNull()` at
`FreestyleLogScreen.kt:337` returns null and the whole set is **silently dropped** at save with no
error.

**Fix:** Accumulate from the clamped value (`totalVolumeLb += (s.weightLb ?: 0.0) * sanitizeReps(s.reps)`),
or better, re-read the persisted sets and call `VolumeCalculator.sessionVolumeLb` as the day flow
does. Cap the reps field to 3 digits (`.take(3)`) in both `FreestyleLogScreen.kt:760` and
`SetInputRow.kt:556`.

---

## [MEDIUM] Volume totals truncate instead of rounding

**File:** `app/src/main/java/com/forge/app/domain/units/WeightFormatter.kt:67` and `:80`; `app/src/main/java/com/forge/app/data/repo/StatsStrengthAggregations.kt:61`

**What:** `formatVolume` / `formatVolumeCompact` finish with `"${v.toInt()} $u"` — truncation toward
zero, applied *after* the kg/st conversion, so the error is largest in the unit that needs it least.
`buildHallOfFame` does the same with `(x * 10).toInt() / 10.0`.

**Scenario:** A 500 lb session shown in kg: `500 × 0.45359237` = 226.796 → **"226 kg"** (0.8 kg lost;
the correct rounding is 227). `WeightFormatterTest.kt:94` currently *asserts* the truncated value, so
the behaviour is locked in by a test. In lb, a 999.9 lb session reads "999 lb".
Relative strength 1.99× bodyweight renders as **"1.9×"** (`StatsStrengthAggregations.kt:61`).

**Fix:** `v.roundToInt()` in both `WeightFormatter` sites and `((x * 10).roundToInt() / 10.0)` in
`buildHallOfFame`; update `WeightFormatterTest.formatVolumeKgConvertsBelowThousand` to expect
`"227 kg"`.

---

## [LOW] Weights are unbounded — an absurd entry permanently skews every aggregate

**File:** `app/src/main/java/com/forge/app/data/repo/WorkoutRepository.kt:603-621`; `app/src/main/java/com/forge/app/domain/parser/WeightParser.kt:44`; guard at `app/src/main/java/com/forge/app/ui/gym/train/DayExerciseHandlers.kt:245-260`

**What:** Reps are clamped (`sanitizeReps`) and holds are clamped
(`durationSeconds?.coerceIn(0, MAX_HOLD_SECONDS)`), but `weightLb` has no bound anywhere in the
write path. The jump-confirm dialog is the only guard, and it is skipped entirely when
`lastWeightLb == null` — i.e. on the first-ever set of an exercise
(`DayExerciseHandlers.kt:245`: `newWeightLb != null && lastWeightLb != null && lastWeightLb > 0`).

**Scenario:** First time on an exercise, fat-finger `1000000000` × 1. No warning fires. The row
stores 1e9 lb; the session's volume becomes 1e9, `XpEngine.kt:38` grants 5,000,000 XP, and every
volume chart's y-axis is flattened forever. Deleting the set fixes the charts but the awarded XP and
any unlocked trophies persist.

**Fix:** Add `MAX_LOGGED_WEIGHT_LB` (e.g. 2000.0) beside `MAX_LOGGED_REPS`
(`WorkoutRepository.kt:55`) and coerce in `logSet`/`updateSet`, or reject at the input boundary with
an inline error. Also run the jump check against an absolute ceiling when there is no history.

---

## [LOW] Bodyweight weigh-ins in stones round-trip to whole pounds

**File:** `app/src/main/java/com/forge/app/ui/profile/BodyweightLogSheet.kt:104-108`, `:116-118`

**What:** The stones branch seeds two integer fields from `seedLb.roundToInt() / 14` and
`seedLb.roundToInt() % 14`, and re-derives lb as `st * 14.0 + lb`. Unlike `SetRow.kt:200-210`, there
is no "field untouched → keep full precision" guard.

**Scenario:** A weigh-in imported from Health Connect as 180.4 lb seeds "12 st" / "12 lb". Re-opening
the sheet for that day and tapping Save without editing rewrites the entry as **180.0 lb**. Repeated
over an import history this quantises the whole bodyweight trend to whole pounds and can flip a
small week-over-week delta's sign.

**Fix:** Track whether either stones field was edited; if not, pass `seedLb` through unchanged.

---

## Verification note (independently re-checked)

Two of the four CRITICALs were re-verified from primary sources.

### The `weightText` unit invariant is explicitly documented — and freestyle violates it

`WeightFormatter.kt:143-148`, the KDoc on `toStoredWeightText`, states the contract verbatim:

> "Canonical stored weight text (**always lb**) for what the user typed in the display unit. A
> numeric kg/stones entry is converted to its lb value; anything non-numeric ("BW", "2 plates")
> passes through unchanged for WeightParser to interpret. **Used by BOTH the log and edit paths
> so unit handling can never diverge between them.**"

So this is not an inferred invariant — the codebase asserts it, and names divergence as the
exact thing the function exists to prevent. `FreestyleLogViewModel.kt:108` calls
`workoutRepo.logSet(..., s.weightText, s.weightLb, ...)` with a `weightText` that never passed
through `toStoredWeightText`, so a kg-mode user's freestyle set is stored with display-unit text.

Note `LoggedSet.kt`'s own KDoc describes `weightText` more loosely as "what the user typed
verbatim" — the two comments disagree, which is likely how the divergence got in.

**The corruption path is live, not theoretical.** `SetLogUseCase.kt:117-122` (the wrist entry
point) does:

```kotlin
val weightText = cmd.weightText
    ?: sessionPrefill(row?.id, setsByLogged)
    ?: workoutRepo.lastPerformanceSets(effectiveId).lastOrNull()?.weightText
    ?: if (isBodyweight) "BW" else return Result(false, "no target yet, pick a weight")
...
val weightLb = WeightParser.parse(weightText, effectivePlan.unit, plateLb)
```

It reads a **stored** `weightText` straight back out of the DB and re-parses it as lb. A kg user
who logs 100 kg in freestyle stores the text "100"; the watch later prefills from it and parses
100 **lb** — a 55% understatement written back as a new logged set.

### Comma-decimal locales do produce a null weight

`WeightParser.kt` accepts only `[0-9]*\.?[0-9]+` in its regexes and falls through to
`text.toDoubleOrNull()`. Kotlin's `toDoubleOrNull` delegates to `java.lang.Double.parseDouble`,
which is **locale-independent** and accepts only `.` as the decimal separator. So "82,5" matches
no regex and returns null from `toDoubleOrNull()` → `WeightParser.parse` returns null →
`weightLb = null`, which `LoggedSet`'s own KDoc says "aggregates should treat as 0 lb or skip".

The set still displays "82,5" to the user because `weightText` is stored verbatim, so nothing
looks wrong — while the set contributes zero volume and can never register a PR. Silent and
permanent.

Worth noting `toStoredWeightText` itself is locale-safe (`String.format(Locale.US, "%.1f", lb)`),
which makes the locale-default `"%.1f".format(...)` at `SetRow.kt:83-84` a genuine inconsistency
against the codebase's own established pattern rather than an oversight of the whole layer.
