# Avex — Pre-Release Bug & Data-Corruption Scan

**Commit scanned:** `60fac9e` (branch `claude/pre-release-bug-scan-l8pnsa`, forked from `main`)
**Date:** 2026-08-25
**Scope:** `forge-android/` — `:app` (~96k lines Kotlin/Compose), `:shared`, `:wear`, plus CI config.

## Method

Eight scanners ran in parallel, each with an independent brief over one high-risk surface. Every
finding is cited to `file:line` with a concrete failing scenario. Findings were required to be
derived from reading the actual code, not from pattern-matching, and several were verified by
execution rather than inspection:

- The **migration chain was replayed in real SQLite** — schema v12 rebuilt from the exported
  JSON, every `execSQL` applied in order, and the result diffed against `13.json`…`36.json` at
  each step.
- The **exercise-name matcher was re-implemented** (tokenizer, curated map, Jaccard scorer) and
  run against ~70 real-world export names; the mismatch table is measured output.
- The **`:shared` module test suite was executed** in a standalone Maven-Central harness.
- **CI history was read from the GitHub Actions API**, not assumed.

Six findings below carry a **Verified** marker: they were re-checked against primary sources by
a second pass after the originating scanner reported them.

---

## Release verdict

**Do not ship `60fac9e` as-is.** Three independent reasons, in order of how cheaply they are
resolved:

1. **CI is red and has been for ~25 consecutive runs.** The last four runs — including this
   commit — died in 3–6 seconds at the runner level, so recent work has had *zero* automated
   verification. This is a process fault, likely Actions quota, and should be fixed before any
   code judgement is made.
2. **A confirmed silent-data-destruction path exists in restore** (§B-1) that can wipe weeks of
   training with no prompt and no user error.
3. **A unit-handling invariant is violated on three separate paths** (§U-1), causing wrong
   weights to be written to the database for every kg and stones user. This is the single
   highest-volume corruption source found, and it is the one users would notice and not forgive.

Everything else is triageable post-release.

## Findings by area

| # | Area | CRIT | HIGH | MED | LOW | Total |
|---|------|-----:|-----:|----:|----:|------:|
| 0 | CI, build & release gate | 2 | 1 | 1 | 0 | 4 |
| 1 | Room DB, migrations, DAOs | 1 | 2 | 4 | 3 | 10 |
| 2 | Backup, restore, importers | 3 | 9 | 8 | 6 | 26 |
| 3 | Units, weight math, PRs | 4 | 5 | 6 | 2 | 17 |
| 4 | Date, time, scheduling | 2 | 6 | 9 | 5 | 22 |
| 5 | Coach / adaptation engine | 2 | 5 | 7 | 4 | 18 |
| 6 | Concurrency & state | 5 | 5 | 8 | 2 | 20 |
| 7 | Wear sync, Health Connect, widget | 3 | 5 | 9 | 8 | 25 |
| 8 | Crashes, release config, security | 1 | 4 | 4 | 3 | 12 |
| | **Total** | **23** | **42** | **56** | **33** | **154** |

---

## The three cross-cutting themes

Reading 154 findings as a list understates the picture. They collapse into three patterns, and
each pattern has one fix that closes many findings at once.

### Theme 1 — The core layers are correct; every bug is at a boundary that bypasses them

This is the most important thing in this report, and it is good news. Repeatedly, the scanners
went looking for a rotten foundation and found a sound one:

- **Migrations:** chain 12→36 complete, ordered, fully registered. Zero mismatches in columns,
  affinities, NOT NULL, PKs, indices or foreign keys. No unqualified
  `fallbackToDestructiveMigration`. Enums stored by **code string, not ordinal** — so reordering
  an enum cannot corrupt data.
- **Units:** conversion constants are exact (`0.45359237`, `2.54`, `1.609344`). `setWeightUnit`
  never re-converts stored rows, so there is no toggle drift or double conversion. NaN/Infinity
  cannot reach the DB through any text parser. `E1rm` is divide-by-zero safe.
- **Wear protocol:** version skew is handled explicitly (`a newer protocol version is dropped as
  NewerVersion, not a crash`), corrupt payloads decode to `Invalid`, unknown fields are ignored,
  every DTO round-trips. All verified by running the tests.
- **Coach pass idempotency:** genuinely correct at the pass level — mutex + ISO-week primary key
  + `OnConflictStrategy.IGNORE` + a single transaction. `CoachGenBias.from()` is a true fixed
  point, so folding never compounds.
- **Rest timer:** backward clock jumps are handled and tested.

The defects are almost all *call sites that skip the correct helper*: a freestyle path that
doesn't call `toStoredWeightText`, a DAO that omits the `finished_at` filter its sibling has, an
error path that doesn't clean up what the success path does.

**Implication for triage:** these are edge fixes, not a rewrite. Most are a few lines.

### Theme 2 — The codebase has already fixed each of these bugs somewhere else

Nearly every finding has a correct sibling implementation in the same repository. The bugs are
places an existing fix was not carried across:

| Bug | The place it's already done right |
|---|---|
| Bodyweight `REPLACE` wipes the note (§D-1) | `CustomizationRepository.clearSwap` — same bug, fixed, with a comment at `:64` |
| Set-logging double-tap races (§C-1) | `DaySwapHandlers.kt:52` has a `swapsInFlight` guard for the identical bug |
| Cardio DatePicker UTC seeding (§T-1) | `BodyweightLogSheet.kt:329` converts correctly |
| Settings writes cancelled on back-out (§C-18) | `GoalsViewModel.kt:117` already uses `withContext(NonCancellable)` |
| `setRating`/`setSkipped` un-transactioned (§C-14) | `setSessionSwap` directly below at `:559` uses `withTransaction` |
| Finish totals from UI state (§C-3) | `WorkoutRepository.resolveOrphanSession:475` recomputes from Room correctly |
| Volume cap flips with no dead band (§K-9) | `InsightEngine.kt:472` has the gate; the generation path doesn't |
| Plateau reset can hit 0 lb (§K-14) | Both chip paths have a `target <= 0` guard; this one doesn't |

**Implication for triage:** each fix has a known-good template in-repo. That makes them cheap
and low-risk, and it means a reviewer can check the fix against an existing precedent rather
than reasoning from scratch.

### Theme 3 — `weightText` has two contradictory contracts, and that is the #1 corruption source

Found **independently by three scanners** working from unrelated briefs (units, Wear, database) —
the strongest signal in this report.

`WeightFormatter.kt:143` states the contract explicitly:

> *"Canonical stored weight text (**always lb**) for what the user typed in the display unit. …
> Used by BOTH the log and edit paths so unit handling can never diverge between them."*

`LoggedSet.kt:12` describes the same field differently:

> *"`weightText` is what the user typed verbatim ("BW", "2 plates", "45")"*

Those cannot both be true for a kg user, and the second reading is what three call sites
implement. Because `SetLogUseCase.kt:117-122` reads stored `weightText` back out of the DB and
re-parses it as pounds, the ambiguity becomes written-back corrupt data rather than a display
glitch.

**Implication for triage:** fix the contract first (pick one, make it explicit in both KDocs),
then the individual call sites. Fixing the call sites without settling the contract will
re-introduce the bug.

---

## Top blockers — independently verified

Six findings were re-checked against primary sources after the originating scanner reported
them. All six held.

### B-1 · CRITICAL · A *failed* restore silently destroys data at the next cold start
`data/repo/BackupRepository.kt:740-746, 783-789` + `ForgeApp.kt:67-89` · **Verified**

Staging is deliberate and well-reasoned — the code explains it avoids racing live DB readers,
and the surrounding guards are careful (version refusals, three-table validation, `deleteOrThrow`
on WAL sidecars). The defect is one missing cleanup on the error path: `pendingDb` / `pendingPrefs`
are written to `filesDir`, but the `finally` cleans only `temps` and `photoStage`. Nothing deletes
them when the restore returns `IO_ERROR`.

`ForgeApp.applyPendingRestore()` runs at every cold start and asks only whether the file exists.

> **Scenario.** A restore fails on a full disk. The UI reports failure. The user keeps training
> for two weeks. The OS evicts the process. On cold start the orphaned `pending_restore.db` is
> swapped over the live DB and the WAL sidecars are deleted. Fourteen days of sessions are gone,
> with no prompt, no undo, and no backup of the newer state. The staged prefs blob lands too, so
> program config reverts as well.

**Fix:** delete all `pending_restore.*` files in the `finally` on any non-`SUCCESS` outcome — or
better, stage under a temp name and rename to `pending_restore.*` only as the final action of a
fully successful restore, making "the file exists" and "the restore succeeded" the same fact.

### U-1 · CRITICAL · `weightText` unit divergence writes wrong weights for every kg/stones user
`ui/gym/freestyle/FreestyleLogViewModel.kt:108`, `service/wear/WatchSessionMirror.kt:113`,
`ui/gym/train/DayViewModelBuilders.kt:200` · **Verified** · *found independently by 3 scanners*

See Theme 3 above for the contract conflict. Three paths bypass `toStoredWeightText`:

- **Freestyle** stores display-unit text. A kg user logging 100 kg stores `"100"`.
- **Watch mirror** sends stored-lb text under a kg label with a display-unit step: the wrist shows
  "220.5 KG" for a 100 kg lift, and each detent adds 2.5 **lb**, not 2.5 kg. A from-zero wrist
  entry of "60 KG" stores 60 lb.
- **Day-screen prefill** seeds a raw lb value into a kg-labelled field — "220.5" re-logged as
  486.1 lb.

The corruption is *written back*, not merely displayed: `SetLogUseCase.kt:117-122` reads stored
`weightText` out of the DB and re-parses it with `WeightParser` as pounds.

**Fix:** settle the contract in both KDocs first, then route all three paths through
`toStoredWeightText`. Consider storing an explicit unit column to make the ambiguity
unrepresentable.

### U-2 · CRITICAL · Comma-decimal locales silently log weightless sets
`domain/parser/WeightParser.kt:44`, `ui/gym/train/components/SetInputRow.kt:161-175` · **Verified**

`WeightParser` accepts only `[0-9]*\.?[0-9]+` and falls through to `toDoubleOrNull()`, which
delegates to `java.lang.Double.parseDouble` — locale-independent, period-only. A German, French
or Spanish user typing `82,5` gets `weightLb = null`, which `LoggedSet`'s own KDoc says
aggregates treat as 0 lb or skip.

The row still displays `82,5`, so nothing looks wrong — while the set contributes zero volume and
can never register a PR. Silent and permanent. `SetRow.kt:83-84` compounds it by *seeding* the
plate field with locale-default `"%.1f".format(...)`, producing `"2,5"` on those same devices.

Note `toStoredWeightText` itself correctly uses `String.format(Locale.US, …)` — so the codebase's
own established pattern is right and these sites diverge from it.

### D-1 · CRITICAL · Health Connect sync destroys weigh-in notes
`data/repo/BodyweightRepository.kt:75` + `data/db/dao/BodyweightDao.kt:13` · **Verified**

`@Insert(onConflict = REPLACE)` compiles to SQLite `INSERT OR REPLACE` — **delete then insert**,
not update. `importLatestFromHealthConnect()` constructs the entry without a `note`, so it is
`null` at replace time. The guard compares only timestamps, never date keys, so it permits the
same-day collision.

> **Scenario.** User logs 07:00 with the note "fasted". At 18:00 their smart scale posts to
> Health Connect. `18:00 > 07:00` → import proceeds → same `date_key` → the 07:00 row is deleted.
> The note is gone permanently, and the row's `id` changes (fresh rowid), so any UI holding the
> old id now points at nothing.

This contradicts the method's own KDoc promise that "a typed weigh-in is never overwritten".
The guard protects the *weight* from older readings; it does not protect the *note* from a newer
same-day one.

**Fix:** carry the existing note forward, mirroring `CustomizationRepository.clearSwap`
(`:64`) — or switch to `@Upsert` / an explicit UPDATE naming only HC-owned columns.

### T-1 · CRITICAL · Cardio date picker rewrites the entry a day earlier on a no-op tap
`ui/cardio/components/CardioLogSheetSections.kt:242` · **Verified**

Material3 canonicalises `DatePickerState` to UTC midnight. `BodyweightLogSheet.kt:329` converts
explicitly for exactly this reason:

```kotlin
initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
```

The cardio sheet passes raw local millis instead. For any user west of UTC the picker highlights
the previous day; because `combineDay` converts the *result* back correctly, opening the picker
and tapping OK **without changing anything** silently moves the entry back a day.

The correct sibling implementation in the same codebase is what makes this a defect rather than
a debatable convention.

### C-1 · CRITICAL · Double-tapped LOG SET hides a logged set forever
`ui/gym/train/DayExerciseHandlers.kt:290` + `DayViewModelRefresh.kt:141`

Both coroutines read `loggedExerciseId = null` from stale UI state and each INSERTs a
`logged_exercise` row; `refreshExercises` then keeps only the one with more sets. The other set
is orphaned and invisible.

`DaySwapHandlers.kt:52` already carries a `swapsInFlight` guard for the identical race — the
set-logging path simply never got it.

This is the head of a four-CRITICAL family (§C-1 … §C-4) sharing one root cause: **the day screen
treats `_state.value.exercises` as the record of what was logged.** `DaySessionHandlers.kt:83`
then computes `totalVolumeLb` / `setCount` / `prCount` from that UI state and stamps them as
denormalised session columns driving history, Stats and Profile lifetime totals — so anything
missing from state is erased from stats permanently.

**One fix neutralises the data-loss half of all four:** recompute finish totals from Room, exactly
as `WorkoutRepository.resolveOrphanSession:475` already does correctly.

### P-1 · CRITICAL · `allowBackup` contradicts the app's own privacy copy
`AndroidManifest.xml:65-68` + `res/xml/data_extraction_rules.xml` + `ui/settings/SettingsAboutPage.kt:88-91` · **Verified**

`android:allowBackup="true"` with an explicit rules file that **includes** `forge.db`,
`datastore/`, `progress_photos/` and `avatar.jpg` in both `<cloud-backup>` and `<device-transfer>`.
Android Auto Backup uploads these to the user's Google Drive, on by default, with no in-app
prompt.

The Settings → About page states:

> "Your data moves only when you move it — Exports and backups go where you point them, through
> Android's own picker."

**Framing this fairly, because the distinction matters:** this is *not* a leak to the developer.
The "no servers, analytics or tracking" and "no Internet permission" claims are fully intact —
that was checked and holds (see Clean Bill below). Auto Backup is a standard, encrypted,
user-account-scoped OS feature, and it is what makes device-to-device transfer work. The rules
file is clearly considered, with careful comments about WAL sidecars and staging files.

The defect is narrower and real: **physique photos leave the device to Google Drive without the
user pointing anywhere**, which the fourth claim says does not happen. For a privacy-positioned
app this is an expectation gap, and progress photos are the most sensitive thing Avex holds.

**Fix — a product decision, not purely technical.** Either exclude `progress_photos/` and
`avatar.jpg` from `<cloud-backup>` (keeping them in `<device-transfer>`, which is user-initiated),
or amend the copy to say the OS backs data up to the user's own Google account. The first
preserves the promise; the second preserves the convenience. On API ≤ 30 also check
`backup_rules.xml`, which additionally sweeps in `crashes/` and plaintext export files.

---
## Complete findings

Full detail for every finding — code excerpts, the exact failing sequence, and a proposed fix —
is in `docs/bug-scan-2026-08/`, one file per area. The tables below are the complete index.

### Area 0 — CI, build & release gate

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | GitHub Actions, branch `main` | CI red for ~25 consecutive runs; last green was run #102 (2026-08-22, `603d066`). The release commit's own run failed. |
| CRIT | runs #145/#147/#149/#151 | Jobs die in 3–6s with no step records and 404 logs — runner/quota level, not code. The last four pushes to `main` got zero verification. |
| HIGH | `ui/profile/ProfileActivityMonth.kt`, `ProfileActivityYear.kt`, `ProfileSurfaceSections.kt` | 3 failing `DesignDoctrineTest` guardrails in run #143 (1014 tests, 3 failed). `maxLines=1` on user content truncates at large accessibility font scales; stray `alpha 0.4` on text where the doctrine notes 0.6 already fails AA contrast. |
| MED | `DesignDoctrine.kt:579`, `SetRow.kt:237`, `PlanModeMedia.kt:88` | Nullable-receiver warning inside the test harness itself; two "condition is always true" dead branches, one on the set-logging path. Deprecated non-auto-mirrored icons render wrong in RTL. |

### Area 1 — Room DB, migrations, DAOs

Migrations verified clean by replay (see Method). All defects are DAO/call-site.

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `data/repo/BodyweightRepository.kt:75` | §D-1 — HC upsert `REPLACE` deletes the row, wiping the weigh-in note. **Verified.** |
| HIGH | `dao/LoggedSetDao.kt:114,135,143,155,310` | Assisted sets excluded from PR detection but counted in PB display, goal progress, Bench/Squat Club trophies and profile top lift — the same lift is both not-a-PR and a trophy. |
| HIGH | `dao/LoggedSetDao.kt:46,143,155,207,297` | Untracked sessions leak into trophy maxima, goal progress and the all-time PR frontier, contradicting `Session.isUntracked` and `TrophyRepository.kt:87`'s own filter. |
| MED | `dao/LoggedExerciseDao.kt:40` | `lastLoggedBefore` orders by autoincrement id, not session time — after a history import, day-screen prefill and weight suggestion come from a stale workout. |
| MED | `dao/LoggedSetDao.kt:135,114,155,207,211` | No `finished_at` filter: a mid-workout typo (2255 lb) permanently unlocks trophies even after the set is deleted. |
| MED | `dao/TrophyNearMissDao.kt:13` | `@Insert(REPLACE)` is a no-op — no unique index on `trophy_id`. Table grows unbounded; the `LIMIT 50` window truncates the near-miss list. |
| MED | `data/repo/ResetRepository.kt:24` | "Reset session data" cascades only via FKs; `coach_decision`, `suggestion_outcome`, `mood_entry` survive and keep biasing program generation and readiness. |
| LOW | `dao/SessionDao.kt:211` | `perDayTypeStats` uses wall-clock duration, bypassing `active_seconds`; currently dead code. |
| LOW | `dao/SessionBreakDao.kt:7` | `session_break` is write-only — nothing reads it. |
| LOW | `db/Migrations.kt:159,176,200,397,419` | Columns have SQL defaults on the upgrade path but none on fresh install (entities lack `@ColumnInfo(defaultValue)`) — latent divergence between upgraded and new devices. |

### Area 2 — Backup, restore & importers

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `BackupRepository.kt:740-746,782-788` + `ForgeApp.kt:67-89` | §B-1 — failed restore leaves staged DB, applied silently at next cold start. **Verified.** |
| CRIT | `BackupRepository.kt:440-446` | `writeZipToFolder` deletes the previous good backup **before** creating its replacement, inside a silent `runCatching` — a null `createFile`/`openOutputStream` or mid-write throw leaves zero off-device backups while the app reports success. |
| CRIT | `BackupRepository.kt:421-437` | `autoBackup` truncates the zip in place (no temp+rename); ENOSPC mid-write destroys the only auto-backup while `autoBackupSavedAtMs()` still shows a fresh date. |
| HIGH | `CsvParser.kt:55` | Quote mode entered on any mid-field `"` — one unescaped quote (a note like `paused 2" off chest`) swallows the rest of the file into one cell. Every later row is dropped and the summary reports success. |
| HIGH | `ExerciseNameMatcher.kt:83-93,112,42-59` | Jaccard floor 0.66 + declaration-order tie-break silently infers equipment. **Measured:** "Bench Press"→`db-bench-press`, "Incline Bench Press"→`incline-db`, "Reverse Fly (Dumbbell)"→`db-fly` (**wrong muscle group**), "Bicep Curl"→machine seated curl. `swappedName` is nulled on match, so the original label is unrecoverable. |
| HIGH | `GymImporter.kt:80-91` | `parseWeight` treats any lone comma as a decimal point: "1,250" → 1.25 lb. A comma-formatted column imports at 1/1000 of value. |
| HIGH | `GymImporter.kt:36,59-73` | Pattern `"…'Z'"` quotes the Z as a literal, then stamps wall clock in `systemDefault()` — every imported session shifts by the device offset. *Found independently by two scanners.* |
| HIGH | `WorkoutImportRepository.kt:118-134` | `startNonce` is per-run but `countAtStart` queries the whole table — a genuinely different workout sharing a midnight instant is dropped as "already in your log". |
| HIGH | `BackupRepository.kt:200-210` + `ForgeJsonImporter.kt:46-52` | Export omits `durationSeconds`/`isAssisted`/`isAmrap`/`toFailure`/`setType`. Round-tripping turns a 90 s weighted plank into a 90-rep 45 lb set and makes assisted sets PR-eligible. |
| HIGH | `BackupRepository.kt:719-724,797-806` | Restore validation reads only `sqlite_master` — no `integrity_check` or checksum. A bit-rotted backup passes, is swapped in, and Room's default `onCorruption` deletes the DB; the original is already gone. |
| HIGH | `StrongImporter.kt:41` (+ Hevy:33, FitNotes:40, Generic:31) | `parseReps("0")` returns 0 not null, so the cardio-row guard never fires — distance rows create phantom 0×0 sets and phantom sessions inflating setCount, streaks and trophies. |
| HIGH | `ForgeJsonImporter.kt:22,26` | `canParse` builds a full `JSONObject` just to test a key; `parse` builds a second. With `MAX_IMPORT_BYTES=25MB` a large export OOMs, and `runCatching` converts it to "No new workouts found in that file." |
| MED | `WorkoutImportRepository.kt:140-144` | `activeSec` recomputed from wall clock and `coerceIn(60, 6h)`, rewriting `finishedAt` — a resumed 70-minute session spanning two days imports as 6 hours. |
| MED | `BackupRepository.kt:171-256` vs `ForgeJsonImporter.kt:25-72` | Round trip drops `dayKey` (forced FREESTYLE), tags, mood, intensity, sessionType, isUntracked, prCount, segments, difficulty, skipped, completedAt, weightText. Exported cardio/coachGoals arrays and the bodyweight CSV have **no importer at all**. `exportVersion` is written but never checked. |
| MED | `BackupRepository.kt:89` vs `:177` | Weekly export carries only `date`; full export carries `startedAt`. Both are `.json`, both listed by `scanFolder` — importing both duplicates every session from the last 7 days. |
| MED | `ForgeApp.kt:67-124` | DB/prefs/photos/avatar swap independently despite the "atomically" KDoc; a failed DB swap after a successful prefs swap irreversibly replaces live preferences while keeping live data. |
| MED | `GenericCsvImporter.kt:14,19` | `canParse` fully parses the file to read the header; `scanFolder` does that plus a full parse for up to 60 files (25 MB each) on every Import-screen visit. |
| MED | `WorkoutImportRepository.kt:253` | Charset hard-coded UTF-8 with no BOM sniff — a UTF-16LE export (Excel "Unicode Text") is rejected as unrecognised. |
| MED | `BackupRepository.kt:514,626,650,679-705` | Temps deleted only by their own `finally`; process death mid-backup leaves DB-sized copies in cacheDir forever, worsening the ENOSPC that caused it. |
| MED | `BackupRepository.kt:682-714` | ZIP entry extraction has no per-entry size cap — a high-ratio archive can fill internal storage during restore. |
| LOW | `BackupRepository.kt:104` | Weekly export writes raw `exerciseId` as `name`; seed-split ids round-trip as "Ua1"/"Lb6" and de-link from catalogue stats. |
| LOW | `BackupRepository.kt:276` vs `ForgeJsonImporter.kt:22` | Per-session export nests under `"session"` but `canParse` requires `"sessions"` — **the app cannot re-import its own** `forge_session_<id>.json`. |
| LOW | `CsvParser.kt:57` | `\r` swallowed rather than terminating a row — a CR-only file collapses to one row and imports empty. |
| LOW | `BackupRepository.kt:203` | `weightText` ("BW", "2 plates") exported but never read back; regenerated as a bare lb number, rewriting the user's notation. |
| LOW | `BackupRepository.kt:226,231-233,246` | Nullables written as `""` or `0` instead of `JSONObject.NULL`, conflating absent with zero. |
| LOW | `ImportModels.kt:22` + `HevyImporter.kt:51` | `isWarmup` parsed and asserted in tests but never mapped to `LoggedSet.setType="warmup"`. |

### Area 3 — Units, weight math & PRs

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `SetInputRow.kt:161-175`, `WeightFormatter.kt:149-154` | §U-2 — comma-decimal locale logs a weightless set while still displaying "82,5". **Verified.** |
| CRIT | `FreestyleLogScreen.kt:322,339` + `FreestyleLogViewModel.kt:108` | §U-1 — freestyle stores display-unit `weightText`, breaking the always-lb invariant. **Verified.** |
| CRIT | `WatchSessionMirror.kt:113,140-142` + wear `SessionScreen.kt` | §U-1 — mirror sends lb text with a kg label and a display-unit step; "60 KG" on the wrist stores 60 lb. |
| CRIT | `SetRow.kt:83-84` | `formatPlateCount` uses locale-default `"%.1f".format` → seeds "2,5"; PLATES bypasses `toStoredWeightText`, so the parser returns null and the stepper resets the field to "0.5". |
| HIGH | `DayViewModelBuilders.kt:200` + `SetInputRow.kt:119-121` | `prefillWeight` seeds raw lb into a kg-labelled field on any bonus set: "220.5" logged as 486.1 lb. |
| HIGH | `ExerciseCard.kt:215`, `ExerciseCardComponents.kt:121` | "Last session"/PB/preview lines print raw lb — card header reads "220.5 × 5" above rows reading "100 kg × 5". |
| HIGH | `ExerciseCard.kt:227-228` | Progression suggestion is an unconverted, unlabelled lb number above a KG field: "Suggested next → 222.5" invites a 490 lb entry. |
| HIGH | `ProgressionAdvisor.kt:517` | `trim() = "$v"` leaks full Double precision on three non-snapped call sites → "Suggested next → 220.46226218487757". |
| HIGH | `BodyweightLogSheet.kt:197-207`, `BodyFatLogSheet.kt:82` | `KeyboardType.Decimal` + a filter deleting `,` → "82,5" becomes "825" and fails range validation; BodyFat seeds with locale `%.1f` so the sheet opens already invalid. |
| MED | `WeightFormatter.kt:33-34,104` | Stones seeds at 0.1 st = 1.4 lb granularity with no untouched-field guard (`SetRow.kt:200-210` has one): repeating a 135 lb set logs 134.4 lb and paints a phantom −0.6 lb delta. |
| MED | `PrDetector.kt:26-32` + `WeightParser.kt:44` | "0" parses to 0.0 not null, so a 0 lb set is flagged a PR (gold ★ + lifetime count) via the ungated repository pass used by freestyle and the watch. |
| MED | `SetInputRow.kt:766-772` | "N for PR" hint uses a different rule from `PrDetector` and ignores `isAssisted` — says "11 for PR" where 1 rep qualifies. |
| MED | `DistanceFormatter.kt:41`, `LengthFormatter.kt:49`, `WeightFormatter.kt:137` | Unit suffixes stripped without being honoured: "5 km" typed in miles mode stores 8.05 km; "20 kg" in lb mode stores no weight at all. |
| MED | `FreestyleLogViewModel.kt:108` vs `:116` | Session `totalVolumeLb` accumulated from unclamped reps while `logSet` clamps to 999 — 100 lb × 5000 stamps 500,000 lb on a session whose sets sum to 99,900. |
| MED | `WeightFormatter.kt:67,80` + `StatsStrengthAggregations.kt:61` | Volume/relative-strength truncate instead of round after conversion: 500 lb → "226 kg" (should be 227). The truncation is locked in by `WeightFormatterTest.kt:94`. |
| LOW | `WorkoutRepository.kt:603-621` | `weightLb` unbounded (reps and holds are clamped) and the jump guard is skipped with no history — a first-ever 1e9 lb entry permanently skews charts and grants 5M XP. |
| LOW | `BodyweightLogSheet.kt:104-108` | Stones weigh-in seeds from `roundToInt()` with no untouched-field guard: re-saving an unedited 180.4 lb day rewrites it as 180.0 lb. |

### Area 4 — Date, time & scheduling

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `GymImporter.kt:36` | `'Z'` parsed as a literal → every imported session offset by the device's UTC offset, often a whole calendar day / ISO week. No ISO_INSTANT or offset pattern exists, so `+02:00` rows are dropped entirely. |
| CRIT | `CardioLogSheetSections.kt:243` | §T-1 — picker seeded with local millis against M3's UTC canonicalisation; a no-op OK rewrites the entry a day earlier. **Verified.** |
| HIGH | `CardioLogSheetSections.kt:246` | `isSelectableDate` compares UTC-midnight against local now — users east of UTC cannot select today until offset-o'clock (Tokyo 09:00, Auckland 12:00); users west can select tomorrow. |
| HIGH | `WeeklyReview.kt:59-60` | "Last week" derived as `weekStartMs − 7 × 86,400,000` from a local Monday midnight — a DST week shifts the boundary an hour, double-counting a Sunday 23:30 session or dropping a Monday 00:20 one. |
| HIGH | `WeeklyRecapWorker.kt:90-110,164` | Bare 7-day periodic anchored to first-ever launch (`KEEP`, never re-anchored) but reads the *current* ISO week — a Monday-anchored install gets the "you've been away a week" nudge every Monday despite training 3×/week. |
| HIGH | `DayCardComponents.kt:131-141` | `formatRelative` buckets by elapsed ms — a session finished Tue 22:30 reads "Today" at Wed 08:00, while `OverviewUiStateMapper.kt:136` correctly says "YESTERDAY" for the same session. |
| HIGH | `WorkoutRepository.kt:121-129` | Deload "week" is a rolling 7×24h from the apply instant and stamps the persisted `deloadMarkedHere` column — a Monday 19:00 deload tags the *next* ISO week's first session, moving the mesocycle anchor. |
| HIGH | `BlockPlanner.kt:58` | Block advance guarded only by ISO-week string equality with no elapsed-time floor — opening the app Sun 23:30 then Mon 00:20 runs **two full weekly passes 50 minutes apart**; conversely three unopened weeks advance the block by one. |
| MED | `OverviewViewModel.kt:172` | "This week" is a rolling now−7d in the coach's target gate but an ISO week in `WeeklyReview`/`StatsRepository` — the Brief can print "1 short of target" directly above a +1-set decision gated on the other count. |
| MED | `StatsRepository.kt:103-106` vs `:119` | `weekStartMs` frozen at flow construction while `todayDate` recomputes per emission — a screen left open across Sun→Mon midnight shows last week's dots with next week's "next up". |
| MED | `RestTimerController.kt:114-127` | Backward clock jumps guarded, forward ones not: a +4 min NTP correction 20 s into a 150 s rest fires the done-buzz immediately. Watch renders phone-clock `endAtMs` against its own clock with no skew correction. |
| MED | `SetInputRow.kt:138-152` | Timed-hold stopwatch uses wall clock with `rememberSaveable` — after process death it restores *running* and pre-fills the 3600 s ceiling, one tap from being logged as a real hold. |
| MED | `TrainingReminderWorker.kt:118` | 24h periodic never re-anchored, so an 18:00 reminder becomes 17:00 after every spring-forward and stays there for seven months. |
| MED | `TodayDirective.kt:140-159` | `daysSinceLast` from elapsed ms but read as "trained today" — two users who both last trained yesterday get opposite directives purely on 20:00 vs 06:30. |
| MED | `LifeEvents.kt:180` | `suppressesVerdict` returns true on `state.sick` with no window scoping — one sick check-in marks every 14-day watch window closing that pass as `not_followed`, discarding real evidence. |
| MED | `AndroidManifest.xml` + `BackupRepository.kt:46` | No `ACTION_TIMEZONE_CHANGED`/`TIME_CHANGED` receiver anywhere, and `BackupRepository` caches `ZoneId` at singleton construction — an Auckland→London flight leaves the process bucketing days in the old zone. |
| MED | `StatsRepository.kt:104,119,176` + 6 more | `Clock` injected but bypassed with `LocalDate.now()`/`System.currentTimeMillis()` — streaks (named in the `Clock` docstring) are untestable; `BodyMeasurementRepository` can write `date_key` and `recorded_at` from two different sources. |
| LOW | `res/xml/forge_widget_info.xml` | `updatePeriodMillis=1h` with no midnight/date-change trigger and one manual `updateAll` — widget shows yesterday's next-up at 06:00 Monday. |
| LOW | `StatsRepository.kt:171-193` | `computeStreak` reads only `observeRecent(120)`, so a long or twice-daily streak plateaus; the trophy path reads all sessions, so Profile can show a max streak above the current one mid-streak. |
| LOW | `TrophyEvaluator.kt:17` + 4 more | Day counts by truncating elapsed ms — the "One year" trophy unlocks at 21:00 on the anniversary; a calendar-14-day gap reads as 13 so the layoff ramp doesn't engage. |
| LOW | `OutcomeWatcher.kt:63` | 14-day window closes on elapsed ms but passes run weekly, so an evening apply slips its verdict a full extra week (21 days) while the UI shows "~0 days left". |
| LOW | `GymImporter.kt:47-49` | Locale-derived state captured at class init — switching en-US→en-GB without a process restart parses "04/05/2024" as 4 May instead of 5 April. |

### Area 5 — Coach / adaptation engine

Pass-level idempotency verified **correct** (see Clean Bill). The damage sits one layer down.

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `CoachRepository.kt:592-596` | Volume undo copies the whole shared `(day,exercise)` overlay row and flips `source` COACH→USER, stranding a coach rep-range override on the slot and **permanently user-locking it from the coach**. |
| CRIT | `CoachRepository.kt:500-505` | Volume payloads are absolute set counts minted at pass time; a regenerate/reroll/deload between the pass and the Apply tap makes them apply a stale absolute — adding sets during a deload week, or silently removing them. |
| HIGH | `AdaptationRepository.kt:336-372` | "Run a deload week" regenerates the program at 55% volume as the new baseline and nothing ever restores it — `rotationCadence` defaults to "never", so **the deload is permanent**. |
| HIGH | `AdaptationRepository.kt:155-160` → `ProgressionAdvisor.kt:385-448` | Persistent swaps aren't merged into the snapshot's program, so the plateau ladder prices a swapped slot with the original exercise's unit and name ("drop to 3 plates" on a dumbbell press). |
| HIGH | `ProgressionAdvisor.kt:316-325` | The e1RM stall series is never partitioned at a swap boundary — rotating to a lighter variation makes the stall grow forever and the coach re-proposes the identical rotation every week. |
| HIGH | `TrustLedger.kt:83-92` | Autopilot streaks count applied-but-unvalidated decisions, not weeks — two "Apply all" taps on 2-proposal weeks earn `rep_shift` autopilot with **zero watcher verdicts**. |
| HIGH | `TrustLadder.kt:41-42,120,127` | `REVERT_CAP` is documented as windowed but counts lifetime reverts (including the coach's own self-reverts), permanently freezing the coach at PROPOSE after 3 undos ever. |
| MED | `LifeEvents.kt:179-186` | `if (state.sick) return true` ignores window overlap — one sick check-in on pass day closes every in-flight decision as terminal `not_followed`. |
| MED | `CoachRepository.kt:506-510` | A revert whose undo was refused still marks itself applied, leaving the original applied+failed — the same revert proposal is re-derived every week forever. |
| MED | `CoachRepository.kt:474-479` vs `OverviewViewModel.kt:397` | Two uncoordinated deload entry points; the Overview apply doesn't retire the coach's proposed deload row, so the user can regenerate the program twice and push the deload window out. |
| MED | `PersonalProfile.kt:110-112` | Weekly volume cap flips on a bare `high.average() > low.average()` with no dead band — a 0.001 lb difference swings CHEST's ceiling 24 ↔ 12 sets. `InsightEngine.kt:472` has the gate; this path doesn't. |
| MED | `CoachRepository.kt:402` | The ±2 drift cap counts bias sets the weekly-cap trim silently removed, so the coach permanently spends volume budget the program never received. |
| MED | `CoachRepository.kt:481,495,501,507,511` | A bad/unresolvable payload returns without changing status — the decision stays `proposed` forever and Apply is a silent no-op with no way to clear it. |
| MED | `CoachRepository.kt:180-192` | Mid-week re-enable regenerate matches only `STATUS_SHADOW`; a HOLD or ERROR pass recorded while the coach was off is cached for the rest of the ISO week. |
| LOW | `ProgressionAdvisor.kt:414-426` | Plateau reset has no `target <= 0` guard (both chip paths do), so a light dumbbell lift can be told to "drop ~10%" to 0 lb. |
| LOW | `InsightEngine.kt:125` | `mostImproved` counts assisted-set weights, unlike every other strength read. |
| LOW | `InsightEngine.kt:409-417` | `amMean`/`pmMean` unguarded; NaN passes the `<` gate and renders "~0% higher". |
| LOW | `CoachPass.kt:66,73-81` | `undo_expires_at`, `scope_key`, `lesson_id` declared but never written — the documented "undo expiry / revert forward" guarantee **does not exist**. |

### Area 6 — Concurrency & state

The first four CRITICALs are one bug family: *the day screen treats `_state.value.exercises` as
the record of what was logged.*

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `DayExerciseHandlers.kt:290` | §C-1 — double-tapped LOG SET inserts two `logged_exercise` rows; `refreshExercises` keeps one and the other set is hidden forever. `DaySwapHandlers.kt:52` has the guard; this path doesn't. |
| CRIT | `DayViewModelRefresh.kt:99` | `refreshExercise` snapshots the exercise list, suspends for ~7 DB reads, then writes the stale list back at `:122` — a concurrent log on another exercise (superset alternation) is erased from state. |
| CRIT | `DaySessionHandlers.kt:83` | `finishWorkout` computes `totalVolumeLb`/`setCount`/`prCount` from UI state, not the DB, and stamps them as denormalised session columns driving history, Stats and Profile lifetime totals. |
| CRIT | `DayViewModel.kt:110` | `DayViewModel` never observes Room for the live session (`WatchSessionMirror.kt:50` does) — watch-logged sets are invisible on the phone, corrupt the next `setIndex`, and are excluded from the finished session's totals. |
| CRIT | `NoteField.kt:66` | The exercise note's only commit path is a 500 ms debounced `LaunchedEffect`; the card auto-collapses on the final set or unmounts on "move to next", cancelling the effect and silently discarding the note. |
| HIGH | `WorkoutRepository.kt:408` | `maybeRotateProgram` does read-increment-write on a DataStore key outside `edit{}` — a double-tapped FINISH loses the increment, or fires two concurrent `rerollAll()` regenerations and writes calories to Health Connect twice. |
| HIGH | `FreestyleLogViewModel.kt:99` | Freestyle `save()` writes a whole workout with no `withTransaction` and no in-flight guard — a back-press mid-loop leaves a torn session; a double-tap creates a full duplicate. |
| HIGH | `FreestyleLogScreen.kt:304` | `leave()`'s last-chance draft flush runs in `viewModelScope` then calls `onBack()` on the next line — the pop cancels the write, losing exactly the sub-600 ms edits the flush exists to protect. |
| HIGH | `CheckinViewModel.kt:89` | `save()` discards the `runCatching` Result (and swallows `CancellationException`), then unconditionally sets `answeredToday=true` — a failed check-in is reported as saved and never re-prompted. |
| HIGH | `MainActivity.kt:278` | `runBlocking` with five DataStore `.first()` reads on the main thread in `onCreate`; `ForgeApp.kt:41` additionally copies the whole DB file + photo folder synchronously in `Application.onCreate`. ANR risk on cold start after a restore. |
| MED | `RestTimerController.kt:50` | `endAtMs`/`tickJob` are non-volatile plain vars mutated from a Binder thread while the tick job reads them on Main — no happens-before; timer jumps or duplicate tick jobs. |
| MED | `SettingsRepository.kt` (whole file) | Zero `distinctUntilChanged`: every write re-emits all ~100 preference flows, re-running `OverviewViewModel`'s 14-stage combine (with DB reads) ~2×/sec during freestyle draft autosave. |
| MED | `ProfileViewModel.kt:149` | `_state.value = _state.value.copy(photos = photoRepo.photos())` evaluates the receiver before the suspending argument — concurrent edits are reverted. Same idiom in ~30 places across 9 files. |
| MED | `WorkoutRepository.kt:530` | `setRating`/`setSkipped`/`setNote` are SELECT-then-UPDATE-whole-row with no transaction — a note commit racing a SKIP tap silently un-skips the exercise. `setSessionSwap` at `:559` does it correctly. |
| MED | `WorkoutSessionService.kt:131` | The 2.5 s haptic-handoff delay sits inside `bridge.timerDone.collect`; with `extraBufferCapacity=1` and a discarded `tryEmit`, a second timer-done inside that window is dropped — no buzz. |
| MED | `ProgramChangeGuard.kt:32` | `stagedAction` is a single unsynchronised slot on a `@Singleton` — a second staged change overwrites the first, so "Discard & continue" can run the wrong action. |
| MED | `DayViewModel.kt:66` | `SavedStateHandle` is read-only; rest timer, finishedEarly, bonus sets, manual order and warmup state are memory-only, and `leaveAndResume` tears down the foreground service while the session stays active. |
| MED | `SettingsViewModel.kt:347` | ~50 settings writes are bare `viewModelScope.launch` — toggling then backing out cancels the DataStore edit. `GoalsViewModel.kt:117` already uses `NonCancellable`. |
| LOW | `NoteField.kt:50` | A `hiltViewModel()` default param constructs the 793-line Settings state graph on the live day screen to read `noteTemplates`, which the call site disables anyway. |
| LOW | `ArrivalController.kt:69` | `_queue.value = _queue.value + fresh` is non-atomic RMW on a shared singleton StateFlow; `WearFocusHolder` is read from a Binder thread. |

### Area 7 — Wear sync, Health Connect & widget

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `WatchSessionMirror.kt:113` + wear `SessionScreen.kt` | §U-1 — stored-lb text rendered under a display-unit label and stepped in the wrong unit, then re-parsed as lb by the phone. |
| CRIT | wear `SessionScreen.kt:90-94,220` | The 4 s "Not logged · reconnecting" timeout prompts a re-tap that mints a **new** `commandId`, defeating `CommandDeduper` — a slow-but-successful log becomes a duplicate set. |
| CRIT | wear `WearDataRepository.kt:184-193` | `sendBytes` has no persistence, no retry, and swallows failure; empty `connectedNodes` reads as success and a first-node throw skips the rest — a set logged out of BT range is lost forever. |
| HIGH | wear `WearDataRepository.kt:74-84` | The persisted `/cmd/ack` DataItem is replayed on every watch cold start and re-stamped with the current time, re-arming undo+rate for a set from minutes ago — RPE lands on the wrong set. |
| HIGH | `HealthConnectManager.kt:434-451` | Every HC read except `readWeightHistory` omits pagination; the default 1000-row ascending page truncates a 15-day steps window → Home shows "0 steps" and the coach sees a phantom sedentary user. |
| HIGH | `HealthConnectManager.kt:701-706` | `toWatchWorkout` aggregates DISTANCE_TOTAL/ENERGY_TOTAL with no `dataOriginFilter` — Samsung Health + Google Fit both writing one run yields 9.9 km for a 5 km run, then imported and re-written to HC. |
| HIGH | `HealthConnectManager.kt:594-610` | `writeActiveCalories` passes no `clientRecordId` while both sibling finish-mirrors upsert — any re-finish inserts a duplicate calorie record into Samsung Health/Fit with no local trace to repair. |
| HIGH | wear `TimerView.kt:89` + `SetLogUseCase.kt:184-189` | Rest-screen undo has no pending gate (SetView does) so a double-tap deletes two sets; and `UndoSetCommand` carries only `sessionId`, so it deletes `max(completedAt)` — **the phone's set, not the one the row names**. |
| MED | `WearCodec.kt:15-18` | `coerceInputValues` is off, so a future enum constant added under the "additive doesn't bump VERSION" rule makes the whole `SessionLiveDto` Invalid → wrist silently drops to idle mid-workout. No test covers unknown enums. |
| MED | `WearCommandHandler.kt:27,47,65` | The phone has no `NewerVersion` surface and returns **before** `publishAck` — a newer watch build's commands vanish with no ack, so the wrist can't distinguish "update your phone" from "BT dropped" and re-taps into the duplicate loop. |
| MED | `WearStatePublisher.kt:126-128` | All acks share one latest-wins `/cmd/ack` path; a second ack supersedes an unsynced first, stranding the earlier command's pendingId. |
| MED | `widget/ForgeWidget.kt` + `ProgramRepository.kt:307` | `updateAll()` fires only on program regeneration — never on session start/finish — so "WORKOUT IN PROGRESS", streak and week dots stay stale for an hour+. |
| MED | `widget/ForgeWidget.kt:155` | The extras Bundle is passed to Glance's third param, which is `activityOptions`, not intent extras — `EXTRA_START_DAY_KEY`/`EXTRA_RESUME_SESSION` never arrive, so **the widget deep-link is dead**. |
| MED | wear `WearHrService.kt:120-123` | `batchLoop` clears `pending` before knowing the send succeeded (BT flap = samples gone); watch-clock sample timestamps are filtered against phone-clock `startedAt`, dropping the first seconds of every trace. |
| MED | `HealthConnectManager.kt:687-693` | `recentWatchWorkouts` fetches `limit*2` rows then filters self-written **after** — an active user's own write-backs starve the import pool to empty. |
| MED | `WearStatePublisher.kt:131` + wear `TimerView.kt:59-64` | Timer expiry flips `paused=true`, unmounting `TimerView` usually before its local tick fires the buzz — the wrist stays silent and the phone buzzes in the locker. |
| MED | `widget/ForgeWidget.kt:84,136-143` | `allFinished()` loads every finished session as full entities on each widget update and walks it three times, to answer a 7-day dot row. |
| LOW | wear `WearDataRepository.kt:120` | `newerVersion` latches forever from any path and outranks every screen — one newer `/glance/today` blocks logging for the process lifetime. |
| LOW | wear `WearRoot.kt:52-65` | The replayed stale ack re-fires the PR double-tick + gold wash on every app launch. |
| LOW | `WearStatePublisher.kt:155-156` | `weekVolumeText` hardcodes "lb" while the phone uses `formatVolumeCompact(unit)`. |
| LOW | `WatchSessionMirror.kt:118-120` | `lastSetWasPr` reads per-exercise `wasPr`, so it stays true for every later set. |
| LOW | `HealthConnectManager.kt:639-643` | `Sequence.sortedBy` buffers everything before `take(HR_SERIES_MAX_SAMPLES)`, so the runaway-provider cap doesn't bound memory. |
| LOW | `HealthConnectManager.kt:283,362,604` | Auto-recorded sessions/HR/calories are all written as `Metadata.manualEntry()`. |
| LOW | `SetLogUseCase.kt:156,195` | `setIndex` = live count, so a mid-session delete yields duplicate indices and the wrist's prefill picks arbitrarily. |

### Area 8 — Crashes, release config & security

Part A (unsafe-operator crash hunt) turned up **far less than expected** — ~14 candidates were
checked and dismissed with reasons. The real risk here is concentrated in release/build config
and the lock/backup boundary.

| Sev | Location | Finding |
|-----|----------|---------|
| CRIT | `AndroidManifest.xml:65-68` + `data_extraction_rules.xml` | §P-1 — `allowBackup=true` cloud-backs `progress_photos/`, `avatar.jpg`, `forge.db` and DataStore to Google Drive, contradicting the "data moves only when you move it" claim. **Verified.** |
| HIGH | `PreferencesDataStore.kt:19` + `MainActivity.kt:278-288` | No `corruptionHandler` and zero `.catch` across 101 DataStore flows, consumed via `runBlocking` on the main thread in `onCreate` → unrecoverable crash-on-launch loop. |
| HIGH | `BackupRepository.kt:744-746` + `ForgeApp.kt:92-95` | Restore stages `pending_restore_prefs.pb` with **no validation** (the DB half gets magic-byte + schema + version checks) and swaps it into the live DataStore at boot — the reachable trigger for the finding above. |
| HIGH | `MainActivity.kt:133-136` + `AppLockManager.kt:86-98` | App lock never re-arms after screen-off: `onGenuineBackground()` is gated on `userLeaving`, set only by `onUserLeaveHint()`, which Android does **not** call for the power button, display timeout or an incoming call. |
| HIGH | `wear/proguard-rules.pro` (0 bytes) + `wear/build.gradle.kts:50` | Wear minifies with an empty rules file; the phone's "PERSISTENCE-CRITICAL" enum keep rule is not mirrored, and `ProtocolWeightUnit`/`TimerCommand.Action` cross the Data Layer as enum-name strings — **release-only**, silently dropped as `DecodeResult.Invalid`. |
| HIGH | `app/build.gradle.kts:44` + `wear/build.gradle.kts:31` | `targetSdk=35` with `compileSdk` already 36, close to Play's API-36 deadline (verify the exact date in Play Console). |
| MED | `MainActivity.kt:284,293` | `FLAG_SECURE = privacyMode \|\| appLockEnabled` omits `galleryLockEnabled` — photo-lock-only users appear in Recents and are screenshottable. |
| MED | `.github/workflows/ci.yml:52-88` | No `lintVitalRelease`, no `connectedAndroidTest` (so **`MigrationTest` — the only guard on the 12→36 chain — never runs**), and the release APK is compiled by R8 but never installed or launched. |
| MED | `AndroidManifest.xml:214-234` | `WearSyncService` `exported=true` with no `android:permission`; reachable binder writes/deletes sets via `/cmd/log-set` and `/cmd/undo-set`, and wakes the process when the app is dead. |
| MED | `MainActivity.kt:359-379` | The app-lock gate overlays a still-composed `ForgeNavHost` with no `clearAndSetSemantics` — **TalkBack reads the data behind the lock**. The gallery gate at `ForgeNavHost.kt:400` does this correctly. |
| LOW | `ui/theme/ColorExt.kt:10-17` | Throwing `toAccentColor()` (`toLong(16)` before the length check) used at `DayCard.kt:66` on a value that round-trips DataStore + Room; a safe `parseAccentHex()` already exists. |
| LOW | `MainActivity.kt:278-288` | Five sequential DataStore `first()` calls block the main thread every cold start — exactly what the app's own debug StrictMode `detectDiskReads` targets. |
| LOW | `res/xml/file_paths.xml:4` | FileProvider rooted at `filesDir` itself, so its addressable surface includes `progress_photos/`, `datastore/` and `crashes/`. |

---

## Clean bill — checked and found correct

Recorded so a later pass doesn't re-litigate these. Each was actively investigated, not skipped.

**Privacy claims hold.** No `INTERNET` or `ACCESS_NETWORK_STATE` in any of the four manifests. No
okhttp, retrofit, `HttpURLConnection`, `URL(`, `Socket`, WebView, Firebase or analytics anywhere
in source or the version catalog. **Zero** `android.util.Log` / `println` / `printStackTrace` in
`app`, `wear` or `shared` main sources — no user-data logging in any build type. Nothing sensitive
remains in git after the `.env` untrack (`025bb2f`). The §P-1 finding is an OS-backup expectation
gap, *not* first-party collection.

**Migrations.** v12 rebuilt and every `execSQL` replayed in real SQLite, diffed against
`13.json`…`36.json` at each step: zero mismatches. Chain complete, ordered, fully registered. No
entity/schema drift. Enums stored by code string, so reordering is safe. No unqualified
`fallbackToDestructiveMigration`.

**Units core.** Constants exact (`0.45359237`, `2.54`, `1.609344`). `setWeightUnit` never
re-converts stored rows — no double conversion, no toggle drift. NaN/Infinity cannot reach the DB
through any text parser. `plateWeightLb` is coerced away from zero. `E1rm` is divide-by-zero safe.
The units layer is correct *in isolation*; every finding is a boundary that bypasses it.

**Wear protocol** (verified by execution): newer protocol versions drop to `NewerVersion` rather
than crashing; corrupt and wrong-shape payloads decode to `Invalid`; unknown extra fields are
ignored; every DTO round-trips. kotlinx.serialization ships its own `META-INF/proguard` consumer
rules, so R8 renaming does not break the JSON wire format. `SessionHrSampleDao.insertAll` uses
`IGNORE` against a `(session_id, at_ms)` PK, so a re-sent HR batch is genuinely idempotent — the
gap is only that nothing re-sends.

**Rest timer core** (verified by execution): backward clock jumps re-anchor correctly, a paused
timer doesn't count down while the clock moves, remaining never goes negative.

**Coach pass idempotency.** Genuinely correct at the pass level: mutex + ISO-week primary key +
`OnConflictStrategy.IGNORE` + one transaction for pass & decisions. Single-decision double-apply
is properly guarded under `lifecycleMutex`. `CoachGenBias.from()` is a true fixed point, so
folding never compounds.

**Date/time, verified correct:** `WeekMath`, `CoachRepository.weekId` (ISO week-based-year — no
`YYYY` bug anywhere), `ProfileRepository.currentStreakWeeks`, both cardio week aggregators,
`QuietHoursSchedule.isQuietAt`, the entire vacation path, `TrophyRepository.checkComebackKid`,
`Session.durationMinutes()`, `TrainingReminderWorker.initialDelayMinutes`, `periodDaysLeft`,
Health Connect day bucketing, `BodyweightLogSheet`'s picker, `AppLockManager`, `WeeklySchedule`.

**Crash surface:** all `weightLb!!` sites are pre-filtered; LazyColumn keys are unique by
construction; nav args are validated against `Program.dayKeys`; `strings.xml` has one string and
no format args.

---

## Test coverage — where the gaps line up with the findings

The correlation is close enough to be predictive: **the untested files are where the CRITICALs
are.**

| Component | Lines | Tests | CRITICALs found |
|---|---:|---|---:|
| `CoachRepository` | 802 | **none** | 2 |
| `BackupRepository` | 915 | **none** | 3 |
| `WorkoutImportRepository.insert` | — | **none** | — |
| `ForgeJsonImporter` | — | **none** | — |
| `applyPendingRestore` | — | **none** | 1 |
| Day-screen handlers / `DayViewModel` | — | partial | 5 |
| Importers overall | — | `ImporterTest.kt`, 10 happy-path tests | 4 |
| `MigrationTest` | — | exists but **never runs in CI** (no `connectedAndroidTest`) | — |

Highest-value additions, in order: a `BackupRepository` export→import round-trip test asserting
field-by-field equality (would catch §B-1 and most of Area 2), a `CoachRepository` lifecycle test
(apply → undo → re-propose), and wiring `MigrationTest` into CI so the one guard on the migration
chain actually executes.

---

## Recommended order of work

1. **Unblock CI** — investigate the 3–6 second runner failures (likely Actions quota). Nothing
   below can be verified until this is fixed.
2. **§B-1** — one-line cleanup on the restore error path. Highest damage-to-effort ratio here.
3. **§U-1** — settle the `weightText` contract in both KDocs, then fix the three call sites.
4. **§C-1…C-4** — recompute finish totals from Room (`resolveOrphanSession:475` is the template);
   add the `swapsInFlight`-style guard to the set-logging path.
5. **§D-1, §T-1** — both are small, and both have a correct sibling implementation to copy.
6. **§P-1** — product decision on photo backup vs. copy change.
7. Wire `MigrationTest` into CI; add the backup round-trip test.
8. Triage the HIGH tier; the MEDIUM/LOW tiers are safe to defer past this release.

---

## Limitations of this scan

Stated plainly so the report isn't over-trusted:

- **No Android SDK in this environment**, so `:app` and `:wear` could not be compiled or run. Only
  `:shared` was executed. Every `:app` finding is from static reading — high-confidence given the
  `file:line` citations and the cross-confirmations, but not runtime-proven.
- **No device or emulator**, so nothing was reproduced against a real database, a real watch, or a
  real Health Connect provider.
- **Six findings were independently re-verified**; the rest carry their originating scanner's
  confidence. Three findings were reached independently by two or more scanners
  (`GymImporter.kt:36`, the `weightText` unit divergence, `LifeEvents` sick-suppression), which
  raises confidence in those specifically.
- Severity is this scan's judgement of user-visible data impact, not a triage decision. Some
  CRITICALs may be knowingly accepted; §P-1 in particular is partly a product call.
- The scan looked for defects. It is not a statement that the untouched remainder is correct.
