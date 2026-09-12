# Avex whole-app release audit

Audited application revision: **96378095238dcc8b4ae7cab150098e2f26a1f36c**. Review date: 2026-09-12. Branch: `relay/gentle-zebra`.

The branch was fetched and fast-forwarded to `origin/main` before review. This covers the whole application across phone, Wear, shared code, storage, coaching, screens, background work and release configuration. It is not a review of only the new commits. This deliverable changes documentation and evidence files only.

**Release recommendation: address the P1 findings before release.** The existing automated gate passes, but the audit found privacy boundaries, unreliable watch logging and progression behavior that those tests do not cover. P2 findings include reproducible incorrect results, data transfer loss, lifecycle errors and accessibility gaps. P3 denotes lower-impact defects. Speculative or device-dependent follow-ups are separated from retained findings.

## Priority index

Each specialist report contains precise source locations, triggers, evidence limits, proposed repairs and coverage. Prefixes below identify the owning report, so Data D1 and Domain D1 are different findings. Root R1 combines the dialog and snackbar manifestations of one app-lock boundary problem.

| Priority | Finding | Detail |
| --- | --- | --- |
| P1 | Existing photo dialogs remain above the app-lock overlay; Undo also remains available while locked | Root R1 below |
| P1 | Restore can turn gallery protection off without authenticating | [Data D1](audits/2026-09-12/data.md) |
| P1 | Below-range and consolidation load cues ignore readiness/light-day/deload scaling | [Domain D1](audits/2026-09-12/domain.md) |
| P1 | Main watch set commands have no durable pending payload/identity, allowing lost or duplicate logs across recreation | [Wear W01](audits/2026-09-12/wear.md) |
| P2 | Factory reset retains private backups and exports | [Data D2](audits/2026-09-12/data.md) |
| P2 | Single/weekly JSON round trips lose untracked-session status | [Data D3](audits/2026-09-12/data.md) |
| P2 | Weekly JSON truncates cardio timestamps and deduplication drops distinct workouts | [Data D4](audits/2026-09-12/data.md) |
| P2 | PDF exports turn timed sets into zero-repetition sets | [Data D5](audits/2026-09-12/data.md) |
| P2 | Failure to write the configured external backup destination is reported as success | [Data D6](audits/2026-09-12/data.md) |
| P2 | Empty or wrong-movement bouts count as successful coach swaps and earn trust | [Domain D2](audits/2026-09-12/domain.md) |
| P2 | Midnight widget worker replaces and cancels itself before refreshing | [Domain D3](audits/2026-09-12/domain.md) |
| P2 | Accepted imported weighted timed-only history crashes progression evaluation | [Domain D5](audits/2026-09-12/domain.md) |
| P2 | Delayed watch commands are interpreted using the phone's new weight unit | [Wear W02](audits/2026-09-12/wear.md) |
| P2 | Reopening Wear rest timer delays its haptic relative to its displayed countdown | [Wear W03](audits/2026-09-12/wear.md) |
| P2 | Background health-service startup violates while-in-use permission restrictions | [Wear W04](audits/2026-09-12/wear.md) |
| P2 | A direct session A-to-B transition can reuse heart-rate/calorie state | [Wear W05](audits/2026-09-12/wear.md) |
| P2 | Wear tile callback blocks Main on unbounded Data Layer reads | [Wear W06](audits/2026-09-12/wear.md) |
| P2 | Widget opens a retained plan while the app is in freestyle mode | [Wear W07](audits/2026-09-12/wear.md) |
| P2 | Weekly recap scheduling delays and shifts the intended weekday | [Wear W08](audits/2026-09-12/wear.md) |
| P2 | Cold startup replaces due training-reminder work | [Wear W09](audits/2026-09-12/wear.md) |
| P2 | Disabling Academy rotation can hide one recommendation and duplicate another | [Academy/Cardio U01](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Cardio detail opened through History/Weeks lacks HR and watch-stat features | [Academy/Cardio U02](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Valid watch duration/distance corrections are hidden when HR is unavailable | [Academy/Cardio U03](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Cardio/settings option chips do not expose selected state | [Academy/Cardio U04](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Populated cardio/settings inputs lack accessible names | [Academy/Cardio U05](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Returning from an older cardio week resets the chart to the latest page | [Academy/Cardio U06](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Undo's fixed expiry ignores the user's accessibility timeout | Root R2 below |
| P2 | An unchanged body measurement cannot be logged for today | Root R3 below |
| P2 | Rotation discards bodyweight, body-fat and measurement drafts | Root R4 below |
| P2 | Camera shutter has no accessible name; camera toggles omit state | Root R5 below |
| P2 | Camera permission denial leaves a nonfunctional retry action | Root R6 below |
| P2 | Last-exercise advance offers Finish while earlier exercises remain incomplete | [Gym G1](audits/2026-09-12/gym-ui.md) |
| P2 | Live prior-set cue and table heading use pounds despite kg/st settings | [Gym G2](audits/2026-09-12/gym-ui.md) |
| P2 | Freestyle recreation loses edits made within the autosave debounce | [Gym G3](audits/2026-09-12/gym-ui.md) |
| P2 | Main set-entry fields have no persistent accessible purpose/unit labels | [Gym G4](audits/2026-09-12/gym-ui.md) |
| P2 | This-week muscle bars include previous-week training | [Stats S1](audits/2026-09-12/stats-ui.md) |
| P2 | Untracked recent sessions receive incorrect comparison and BEST badges | [Stats S2](audits/2026-09-12/stats-ui.md) |
| P2 | Calendar training-day actions expose no date or set count | [Stats S3](audits/2026-09-12/stats-ui.md) |
| P2 | Equipment Settings cannot represent common metric plate weights and dumbbell limits | [Controls U07](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Timezone selector changes Settings but no application timestamp formatting | [Controls U08](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Health Connect grant changes made through Manage leave Settings stale | [Controls U09](audits/2026-09-12/academy-cardio-ui.md) |
| P2 | Rotation dismisses custom-activity and holiday drafts in Settings | [Controls U10](audits/2026-09-12/academy-cardio-ui.md) |
| P3 | A final “not followed” coaching outcome displays “still watching” indefinitely | [Domain D4](audits/2026-09-12/domain.md) |
| P3 | Calendar current date remains frozen through midnight/date changes | [Stats S4](audits/2026-09-12/stats-ui.md) |
| P3 | Several statistical charts omit accessible series/count information | [Stats S5](audits/2026-09-12/stats-ui.md) |

## Root findings: lock, profile and shared interaction

Source paths here are relative to `forge-android/app/src/main/java/com/forge/app/`.

### R1, P1: The app lock does not own all visible windows and actions

**Locations:** `MainActivity.kt:523-558`; `ui/nav/ForgeNavHost.kt:454-469`; `ui/profile/MirrorTestViewer.kt:205`; `ui/profile/MirrorTestCompare.kt:142`; `ui/common/SnackbarControllerHost.kt:69-71`.

Enable app lock with immediate relocking and leave the independent gallery lock off. Open a photo viewer/compare dialog, background the app and return, then cancel authentication. MainActivity keeps the nav host composed and draws an opaque sibling lock screen in the Activity window. The gallery viewer uses Compose `Dialog`, which owns a separate Android window. Its route checks only `galleryLocked`, so the existing photo window remains above the Activity lock. Clearing semantics on the Activity's nav-host box does not remove another window.

The root Undo host is also drawn after the lock overlay and outside the cleared semantics subtree. Delete a goal/cardio entry, background and reopen within four seconds: Undo's action has no authentication gate and remains operable while locked. This is the same incomplete boundary, rather than a second independent high-priority finding.

**Repair:** make app authentication control private dialog/sheet composition and all global actions, preserving navigation state separately. Hiding only the Activity content is insufficient. Exercise gallery lock off/on, each dialog, background timeout, canceled biometric prompt and TalkBack. Two reviewers independently traced the source; separate-window behavior has not been reproduced on a device in this audit.

### R2, P2: Undo bypasses accessibility timeout preferences

**Locations:** `ui/common/SnackbarController.kt:63-68,94`; `ui/common/SnackbarControllerHost.kt:57-67`.

Every event expires four seconds after posting. The host displays an indefinite Material snackbar but wraps it in its own remaining-time timeout. That bypasses the accessibility-adjusted timeout Material would normally apply. Users who need longer to find and activate Undo lose their only reversal opportunity at the same four seconds.

**Repair:** compute and retain the accessibility-recommended action timeout when posting, then preserve that deadline across recreation. Keep exactly-once behavior. Verify extended “time to take action” settings and TalkBack. Missing API usage and hard expiry are source-confirmed; device wording/timing was not observed. The relevant supported API is [AccessibilityManager.calculateRecommendedTimeoutMillis](https://developer.android.com/reference/kotlin/androidx/compose/ui/platform/AccessibilityManager).

### R3, P2: Unchanged body measurements cannot be recorded as a new observation

**Location:** `ui/profile/BodyMeasurementLogSheet.kt:53-78`.

Seed waist at 85 cm from yesterday. Open “Log measurements” today after measuring 85 cm again. Inputs start from the latest value, but Save includes only text unequal to that seed, leaving the button disabled. Even typing the same value again cannot express a fresh observation. If another field changes, the unchanged measurement is silently omitted.

**Repair:** track explicit fields chosen/measured today separately from whether their number changed. Do not solve this by silently copying every historical measurement to today; the existing comment correctly warns that doing so fabricates observations. Add an explicit unchanged-value path and a daily observation test. Confirmed from form conditions and the per-day repository model.

### R4, P2: Rotation loses profile logging drafts

**Locations:** `ui/profile/ProfileScreen.kt:203-204`; `BodyMeasurementsScreen.kt:86`; `BodyMeasurementLogSheet.kt:58`; `BodyweightLogSheet.kt:90-113`; `BodyFatLogSheet.kt:72-82`.

Open a bodyweight/body-fat/measurement sheet and enter unsaved values or a note, then rotate. Visibility and editable values use plain `remember`; Activity configuration changes are not intercepted. The sheet closes and its unsaved values disappear. The cardio form already uses saveable draft state, so this finding is limited to these profile flows.

**Repair:** retain sheet visibility and an explicit draft with date and unit, using saveable state or a retained ViewModel. Avoid reseeding a dirty draft when database flows refresh. Source/lifecycle trace only; no device rotation run.

### R5, P2: The camera's primary action has no accessible name

**Locations:** `ui/profile/ProgressCameraScreen.kt:224-229,237-246`.

The shutter is an empty styled Box with `bounceClick`, without text, content description, button role or action label. A screen reader cannot identify it as the capture action. Ghost/Grid/Timer use visual color changes without exposing their on/off state.

**Repair:** give the actual shutter node a stable capture/retry label, button role and enabled/busy semantics. Expose camera toggles as stateful controls. Preserve native click actions when adding semantics. Source-confirmed absence; TalkBack and Switch Access need device validation.

### R6, P2: Permanently denied camera permission has no working recovery action

**Locations:** `ui/profile/ProgressCameraScreen.kt:99-106,178-179,264-279`.

After repeated denial, the fallback's “Allow camera” only repeats `RequestPermission`. There is no permanently-denied state or app-settings action. On Android versions that suppress further permission dialogs, repeated taps cannot recover camera access.

**Repair:** distinguish first request, rationale and permanently denied states; direct the latter to app permission settings and refresh permission on resume. Confirm on the supported API range. This follows the documented [Android permission-denial behavior](https://developer.android.com/training/permissions/requesting); no permission device test was performed.

## Bad/outdated code and worthwhile simplification

These changes should be tied to behavior and validated in small steps. A release audit is not a reason to upgrade every dependency or rewrite stable subsystems.

1. **Share JSON session/cardio serialization.** The existing shared set writer fixed set-level drift, but full/weekly/single session writers still disagree on flags and timestamps. Data D3/D4 are concrete reasons to remove that duplication.
2. **Unify cardio-detail state and actions.** Tab and routed detail loaders already diverge. A shared detail owner removes the failure in U02. Both cardio ViewModels also duplicate entry normalization and construction.
3. **Use one strength-set eligibility rule.** Progression's coarse filter diverges from E1rm eligibility and causes D5. Filter/map valid results once before minimum-sample checks instead of force-unwrapping nullable results.
4. **Remove verified unreachable UI and state.** The gym report identifies the unused DayList Train tab and dead event wiring. `OverviewUiState.cardioWeekDays` has no rendering consumer; it should not be reported as a live wrong-count bug. `StatsEffortAggregations.buildEffortDistribution` and its DAO query also have no production callers. Preserve intentionally parked gamification/conditioning unless product scope explicitly changes.
5. **Migrate deprecated Hilt Compose imports deliberately.** 37 production files import the deprecated `androidx.hilt.navigation.compose.hiltViewModel`; [AndroidX moved the API and artifact](https://developer.android.com/jetpack/androidx/releases/hilt). Update dependency and imports together with navigation/retained-ViewModel checks.
6. **Modernize build configuration separately from the release repairs.** Kotlin `kotlinOptions`, disabled built-in Kotlin/new DSL flags, and dependency-version warnings deserve a compatibility-tested toolchain task. Current pins have documented compatibility reasons. Lint's “new version available” messages are not proof of vulnerabilities or an instruction to take every latest version.
7. **Replace stale historical comments with current contracts.** Examples include the Wear tile callback's incorrect thread assumption, JSON export comments saying no reader exists, route-thumbnail comments saying data is absent, and Academy artwork counts. Keep reasons and invariants, remove superseded implementation narratives.
8. **Prefer typed settings groups over positional `Array<Any>` casts.** MainActivity/SettingsViewModel have large flow aggregations whose position/type coupling is difficult to review. Extract cohesive preference groups when changing those flows, rather than doing a release-wide rewrite.

Additional P3 data issues are documented separately: Health Connect weight history above 5,000 records is marked complete after truncation, settings reset retains forgotten SAF grants, and the PDF singleton retains the old timezone. Race candidates without a demonstrated concurrent producer remain follow-ups, including whole-row session-tag writes, reroll/generation overlap, and shared timer publication races. The equal-volume top-lift cache concern was not promoted because no supported finished-set editor was found.

## Verification and coverage

| Check | Result at the audited revision |
| --- | --- |
| Phone unit tests | 1,694 passed, 205 suites |
| Wear unit tests | 50 passed, 7 suites |
| Shared unit tests | 30 passed, 4 suites |
| Total | **1,774 tests, zero failures/errors/skips** |
| Roborazzi verification | Passed against 46 committed screenshot PNGs |
| Release lint | Zero errors/fatal; phone 103 warnings/11 hints, Wear 6 warnings/4 hints |
| Phone and Wear release bundles | Both built successfully; unsigned local verification outputs |
| Android instrumentation sources | Compiled successfully; not run |
| CI support scripts | 10 discovered tests and 5 separate summary-script tests passed |
| Production probes | Confirmed incorrect load targets, swap outcomes, terminal label, imported-data NPE and recap scheduling calculations |
| Attached devices/emulators | None |

The Gradle gate completed successfully in 2m 38s, with 182 tasks (100 executed, 82 from cache). Test counts come from generated JUnit XML and lint counts from release XML. The 109 warnings include dependency/toolchain suggestions, 13 missing recommended Modifier parameters, obsolete/deprecated APIs and modernization suggestions. They are not 109 confirmed product bugs. Build output also contained 58 unique Kotlin warning lines.

Reproduction command, from `forge-android`, in fish:

```fish
set -lx ANDROID_HOME /home/anthony/Android/Sdk
set -lx ANDROID_SDK_ROOT /home/anthony/Android/Sdk
./gradlew :app:testDebugUnitTest :wear:testDebugUnitTest :shared:test :app:verifyRoborazziDebug :app:lintRelease :wear:lintRelease :app:bundleRelease :wear:bundleRelease :app:compileDebugAndroidTestKotlin --no-daemon
```

From the repository root, run `python docs/audits/2026-09-12/run_probes.py` after the gate to reproduce the targeted findings using compiled production classes and the pinned Kotlin 2.2.10/WorkManager 2.10.1 libraries. These are demonstrations of defects, not regression tests that should stay broken. Outputs and machine-readable validation are in [validation.json](audits/2026-09-12/validation.json).

The production inventory is 625 main-source Kotlin files: phone 594/105,414 lines, Wear 22/2,708, shared 6/681, baseline profile 3/87. Automated checks cover the configured modules. The source review used parallel directory reading for the specialist scopes, with explicit exclusions for some static art/copy, plus root tracing of startup, navigation, locks, settings, profile, shared helpers, build configuration and CI. The specialist appendices list their files; overlapping review counts must not be added as unique coverage. This is a whole-app audit with risk-driven deep traces, not a claim that every line of every resource/configuration file was manually examined.

No attached phone/watch meant no biometric/dialog-window, reconnect/process-death, background health service, camera/permission, TalkBack, large-font/RTL, or performance measurements on hardware. Screenshot verification establishes baseline agreement for existing cases, not complete visual approval. Instrumentation compilation does not establish migration/device behavior. No signed release, Play upload, CI run on a remote branch, vulnerability scan, or independent medical/scientific fact-check of Academy content was performed.

## Repair sequence

1. Close the lock/restore privacy boundaries, then make watch command identity/payload durable and fix reduced-load progression.
2. Repair data round trips/reset ownership/backup destination reporting and the accepted-import crash. Add tests for those exact triggers.
3. Fix workout navigation/draft retention, background scheduling, watch timer/session ownership, and route parity.
4. Finish accessibility and small correctness repairs, then run targeted device scenarios and the existing release gate on the changed application.

The report records findings and proposed changes. Application repairs are still outstanding.
