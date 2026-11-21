# Avex phone app: whole-codebase review and release readiness (2026-09-26)

Audited revision: **54c81b2** (`main` after PR #195). Read-only review: this change adds documentation only.

## Method

Twelve reviewers ran in parallel. Eleven each read every Kotlin file in one slice of the phone app (`forge-android/app/src/main`, ~111k lines, plus `:shared`) in full and followed callers and callees outside their slice as needed. The twelfth covered the build, manifest, CI and release layer, plus a scripted dead-code sweep across the whole module. Each finding cites `file:line` with a concrete failing scenario. Reviewers checked prior audits (`docs/audits/2026-09-12/`, `docs/RELEASE_AUDIT_2026-09-12.md`, `BUG_SCAN.md`) and marked findings that are **still open**; findings already fixed were not re-reported.

Evidence beyond reading:
- The migration chain was replayed in sqlite3 against all 32 exported schemas.
- `:shared` tests were built and run standalone (30/30 pass).
- The pure `program` package was compiled with three harnesses to measure generator output across seeds.
- kg/lb grid math, week-id locale formatting and date parsing were reproduced on the JVM.

Every P1 and a sample of P2s were then re-checked against the source by the coordinator; those are marked *Re-checked by the coordinator* in the area files.

**Not verified:** no Android build, lint, R8 or instrumented test could run in this environment (Google Maven is blocked). CI run 36219235878 on this commit passed all three jobs. Nothing was checked on a device.

## Verdict: not ready to release

The build and config layer is in good shape: targetSdk 36 meets the Play requirement in force since 2026-08-31, R8 and resource shrinking are on, signing is guarded, there's no `INTERNET` permission, and CI is green on this commit. What blocks release is the ten P1s below. Four of them are still open from the 2026-09-12 audit, and the files involved haven't changed since.

## P1: fix before release

| # | Finding | Status | Where |
|---|---|---|---|
| 1 | **App lock doesn't cover window-owning UI.** Dialogs, bottom sheets, the photo viewer and compare, and the Undo snackbar stay usable above the lock overlay. "Discard & continue" can delete an in-progress workout without unlocking. The lock screen also doesn't handle Back (P2), so hidden screens react while locked. | Still open (R1), wider than reported | [03](03-profile.md), [04](04-settings-security-notifications.md), [06](06-common-nav-theme-overview-goals.md), [12](12-release-readiness-and-dead-code.md) |
| 2 | **Restore can switch off the gallery lock without authentication.** Prefs are copied verbatim. | Still open (D1) | [04](04-settings-security-notifications.md), [08](08-data-repositories.md) |
| 3 | **"Back up" in the Data dialog exports every progress photo past the gallery lock.** The guard added in 84ea7a9 covers the other two backup entry points but not this one. | New | [04](04-settings-security-notifications.md) |
| 4 | **Leaving Settings mid-restore reports "Couldn't read that file", but the staged restore still replaces all data at the next launch.** Anything logged in between is lost. | New | [04](04-settings-security-notifications.md), [08](08-data-repositories.md) |
| 5 | **Background program regeneration silently deletes the in-progress workout.** Triggers are the block's scheduled deload (on every app resume, even with the coach off) and freestyle-save rotation. Neither goes through `ProgramChangeGuard`. | New | [08](08-data-repositories.md) |
| 6 | **Turning the coach off and back on mid-week makes every later resume that week delete and re-propose the week's decisions,** including applied ones with undo data. In auto mode, sets ratchet up once per resume. | New | [08](08-data-repositories.md) |
| 7 | **kg users' progression targets move on a 2.5 lb grid.** 60 kg at the top of its rep range suggests "60.1 kg" next, so double progression never adds loadable weight. | New | [10](10-coach-adapt-engine-program.md) |
| 8 | **The "keep this weight" and "consolidate" cues ignore readiness, light-day and deload scaling.** This is the most common branch, so a deload week keeps full load. | Still open (Domain D1). The release reviewer re-grades it to P2. | [10](10-coach-adapt-engine-program.md), [12](12-release-readiness-and-dead-code.md) |
| 9 | **Hevy exports from pound-based accounts import with no weights at all:** only `weight_kg` is read, so years of history arrive as bodyweight sets. | New (medium confidence: depends on Hevy's `weight_lbs` header) | [09](09-db-importers-prefs-health.md) |
| 10 | **Watch "Log set" identity is held only in Compose `remember`,** so a set can be logged twice or lost across recreation. This is in the Wear module. | Still open (W01) | [12](12-release-readiness-and-dead-code.md) |

## Most important P2s

**Data and training correctness**
- Repeating a past freestyle workout relabels imported and seed-program moves as custom **Chest** moves and writes that into the registry ([02](02-gym-freestyle-stats-history.md)).
- A coach swap makes the user's pinned note and rest override coach-owned, so the next regenerate deletes them ([08](08-data-repositories.md)).
- Generation params are built in three places and the Settings path ignores `personalCaps` ([08](08-data-repositories.md)).
- The full JSON export drops `wasPr`, `hitFullTarget`, `supersetGroup` and three cardio fields. After an import, PR lists and trophies read zero ([08](08-data-repositories.md)).
- Strong and Hevy cardio rows import as phantom lifting sessions again. This is a regression ([09](09-db-importers-prefs-health.md)).
- Generic CSV silently drops `5/4/2024`-style dates ([09](09-db-importers-prefs-health.md)).
- Re-imports duplicate history ([09](09-db-importers-prefs-health.md)).
- "Reset app settings" wipes the deload marker (so the deload never ends), milestones, dismissals and the schedule ([04](04-settings-security-notifications.md), [09](09-db-importers-prefs-health.md)).
- A pinned isolation lands twice in one day in ~37% of dumbbell-only weeks ([10](10-coach-adapt-engine-program.md)).
- A rep-range shift is computed on one day and applied to another ([10](10-coach-adapt-engine-program.md)).
- Training blocks never advance in ar/fa/bn/mr locales ([10](10-coach-adapt-engine-program.md)).
- Imported weighted timed sets crash the plateau ladder (D5, still open) ([10](10-coach-adapt-engine-program.md)).
- Cardio edits silently re-round the stored distance ([05](05-cardio-academy-trophies-recap.md)).
- Backdating a weigh-in erases that day's note ([03](03-profile.md)).
- Stones users see weight rounded to whole stones ([03](03-profile.md)).
- Goal targets lose the comma decimal ("82,5" becomes 825 kg) and drift on re-save ([06](06-common-nav-theme-overview-goals.md)).

**Live session**
- Double-tapping any leave or finish action pops the hub and leaves a blank screen ([01](01-gym-train.md)).
- Pressing Back while "Loading session…" can leave an orphan session and notification ([01](01-gym-train.md)).
- Reopening a session replays the PR celebration ([01](01-gym-train.md)).
- The UP NEXT pill shows raw float pounds to kg users ("+2.299999999999997") ([01](01-gym-train.md)).
- The last-slot advance finishes with earlier exercises still incomplete (G1, still open) ([01](01-gym-train.md)).

**Reliability**
- `TimeChangeReceiver` never calls `super.onReceive`, so Hilt never injects it and zone changes aren't handled ([11](11-domain-services-core.md)).
- The watch and phone double-alert at every rest ([11](11-domain-services-core.md)).
- WorkManager REPLACE policies cancel or shift the reminder, the weekly recap and the midnight widget run (W08, W09, D3, still open) ([11](11-domain-services-core.md)).
- Resets run on a cancellable scope and can half-wipe the app ([04](04-settings-security-notifications.md)).
- Exports crash the app on a full disk ([04](04-settings-security-notifications.md)).

**Settings that do nothing:** Date format, Time format, Compact set logging and Timezone are saved but never read. Home ignores the week-start setting ([04](04-settings-security-notifications.md), [06](06-common-nav-theme-overview-goals.md), [09](09-db-importers-prefs-health.md)).

**Privacy and policy:** factory reset keeps the auto-backup ZIP (with photos), `exports/` and `crashes/`, while the privacy policy says Settings can "erase all Avex data" (D2, still open) ([04](04-settings-security-notifications.md), [08](08-data-repositories.md), [12](12-release-readiness-and-dead-code.md)).

**Process:** `main` was red for five days (09-21 to 09-26) while seven PRs merged, because no checks are required ([12](12-release-readiness-and-dead-code.md)).

## Recurring patterns (fix once, fix many)

1. **User-committed writes on a cancellable scope, and `runCatching` swallowing `CancellationException`.** Restores, resets, imports, measurements, profile saves, cardio saves, backups and workers all do this. `SettingsViewModel.write{}` and `setUserName` already show the fix. Add one shared `durableLaunch` / `runCatchingNonCancel` helper and use it everywhere.
2. **Window-owning surfaces under the app lock.** One root rule (don't compose dialogs, sheets or the snackbar host while `appLocked`, and handle Back on the lock screen) closes P1 #1 across every screen.
3. **Re-saving a formatted seed quantises stored data.** This affects cardio distance, bodyweight kg/lb, body fat, goal targets and elevation. The rule should be: if the text equals the seeded display string, save the stored value.
4. **Unit handling outside `WeightFormatter`.** Examples: the lb grid for kg users, "100.0 kg" for whole kilos, whole-stone rounding, the UP NEXT pill, the raw-lb "last" text, and the hardcoded "WEIGHT · LB" header. Round in the display unit and format through one helper.
5. **Decimal inputs not using `filterDecimalInput`.** Affected: `GoalEditorScreen.kt:389, 542`; `DayDialogs.kt:110`; `MirrorTestViewer.kt:384`.
6. **No re-entrancy or double-tap guards, and no `launchSingleTop` anywhere.** This causes the blank-screen pops, duplicate goals and duplicate screens.
7. **Read-modify-write on a stale snapshot.** Examples: `refreshExercises`, `CoachViewModel.runAndRefresh`, the settings toggles, `resolveOrphanSession`.
8. **Duplicated logic that has drifted.** Examples: the two cardio save paths, three generation-param builders, the stall loop copied into `cutSuppressedStalls`, next-up inputs built three ways, `KG_PER_LB` defined three times, and three quad/ham balance definitions.
9. **Rotation and process-death state loss from plain `remember`.** Affected: drafts, open sheets, the focused exercise, gallery navigation, the exercise browser.
10. **Locale.** `Locale.getDefault()` and class-init `SimpleDateFormat`s ignore per-app language changes, and week ids are formatted with locale digits.
11. **Accessibility.** Many tap targets are under 48dp (`clickableLabeled` adds no minimum size), chips don't announce selected state, filled fields have no accessible name, charts have no value descriptions, and the Undo timeout is a fixed 4–5 s.

## Dead code

- **Scripted sweep:** 206 verified dead declarations (165 with no references, including 10 documented as parked, and 41 used only by tests). There are 6 wholly unused files (~770 lines), 4 files used only by tests, and **0 unused resources**. Full list: [`dead-code-verified.tsv`](dead-code-verified.tsv).
- **Unreachable features the script can't see:**
  - ~20 `DayUiEvent`s that are never dispatched, which leaves the GoalSetter, PlateCalculator, WarmupSuggester and intensity pick unreachable.
  - The Train tab, which still runs a DB loop on the Stats page.
  - The NotesSearch, Nutrition, Coach Lab, Coach Timeline and Academy routes.
  - Most Coach v3 output: SessionAdaptor, SignalRegistry, PreSessionBrief (still computed on every Overview refresh), GoalPortfolio, ProjectScanner, most WeeklyReview fields, most of TrustLadder.
  - The warmup ramp, which is generated but never shown.
  - The tile hide/reorder settings.
  - The injury-restriction, day-name, custom-warmup and custom-exercise writers.
- **Large deletable blocks:**
  - ~1,100 lines in `ui/profile`.
  - ~400 lines of Home components.
  - ~330 lines each in SurfaceKit and in the coach goal/project surface.
  - 17 stats aggregation helpers.
  - ~30 DAO methods.
  - The retired `article_event` DAO and its provider.

## Findings by area

| File | Area | P1 | P2 | P3 | P4 |
|---|---|--:|--:|--:|--:|
| [01](01-gym-train.md) | Live session (Train) UI | 0 | 7 | 20 | 6 |
| [02](02-gym-freestyle-stats-history.md) | Freestyle, stats, session detail, history, notes | 0 | 5 | 15 | 13 |
| [03](03-profile.md) | Profile, gallery, camera, body data | 1 | 11 | 15 | 16 |
| [04](04-settings-security-notifications.md) | Settings, app lock, notifications | 4 | 9 | 12 | 8 |
| [05](05-cardio-academy-trophies-recap.md) | Cardio, Academy, trophies, recap | 0 | 9 | 17 | 20 |
| [06](06-common-nav-theme-overview-goals.md) | Shared components, nav, theme, Home, check-in, goals | 1 | 8 | 12 | 16 |
| [07](07-coach-onboarding-builder-experiment.md) | Coach UI, onboarding, program builder, experiment | 0 | 2 | 12 | 17 |
| [08](08-data-repositories.md) | Repositories, backup/export/restore | 3 | 8 | 10 | 13 |
| [09](09-db-importers-prefs-health.md) | Room DB, importers, prefs, Health Connect | 1 | 5 | 9 | 10 |
| [10](10-coach-adapt-engine-program.md) | Coach/adaptation engine, program generator | 2 | 7 | 18 | 7 |
| [11](11-domain-services-core.md) | Domain, services, widget, security, startup, shared | 0 | 7 | 12 | 6 groups |
| [12](12-release-readiness-and-dead-code.md) | Release config, CI, checklist, dead-code sweep | 3 | 5 | 9 | 12 |

Row counts are per area and overlap: the lock, restore, factory-reset and dead-settings findings each appear in several areas. After de-duplication there are **10 distinct P1s** (table above).

Severity key: **P1** = crash, data loss or corruption, wrong training data or prescription, or a security/privacy hole. **P2** = user-visible incorrect behaviour or a notable performance cost. **P3** = minor bug or meaningful code-quality issue. **P4** = nit or cleanup.

## Suggested order

1. **The lock boundary:** P1 #1, #2 and #3 together, plus the lock-screen Back handler.
2. **Data loss:** P1 #4, #5 and #6, the reset and restore cancellation fixes (pattern 1), and the "Reset app settings" allow-list.
3. **Prescription math:** P1 #7 and #8, the pinned-duplicate generator fix, the rep-shift day mismatch, and the D5 crash. Add the regression tests listed in [10](10-coach-adapt-engine-program.md).
4. **Import correctness:** Hevy lb, cardio rows, CSV dates, re-import dedupe, and the export field round-trip.
5. **Process:** turn on required checks for `main`, then the remaining P2s by area.
6. **Dead code:** a deletion pass after the fixes above, using [`dead-code-verified.tsv`](dead-code-verified.tsv) and the unreachable-feature lists.
