# Release readiness and cross-module dead-code sweep

Scope: Gradle/R8/signing config, manifests, resources, CI workflows, the `.claude/RELEASING.md` checklist, privacy docs, tests, status of the 2026-09-12 P1s, and a scripted dead-code sweep over `app/src/main`.
Paths are relative to `forge-android/` unless noted.

**Counts:** P1 3 · P2 5 · P3 9 · P4 12. 206 verified dead declarations (165 zero-reference incl. 10 documented as parked, 41 test-only), 6 wholly unused files (~770 lines), 4 test-only files (~370 lines), **0 unused resources**.

## 1. Verdict: not ready

**Blocking**
- Three P1s from the 2026-09-12 audit are still open (R1, Data D1, W01; section 3). The fourth (Domain D1) is re-graded here to P2 but is also open. None of the files they name has changed since the audit.
- New P1s found by this audit's code review (see `README.md`): background regeneration deleting the in-progress workout, the coach off→on decision wipe, the kg progression grid, the Hevy lb import, and the Data-dialog gallery-lock bypass.
- The privacy policy's deletion claim is false (P2-2).
- Unresolved Play Console gates: Data Safety and Health Apps declarations; specialUse foreground-service declaration; whether versionCode 91 has already been uploaded; the destructive-fallback question for schemas 1–11.

**In good shape**
- targetSdk 36 on phone and Wear, which meets the Play requirement in force since 2026-08-31; compileSdk 37; minSdk 26 (phone) / 30 (Wear).
- versionName 0.9 in both modules; versionCode 91 / 100091, and `release_version_check.py` enforces the Wear +100,000 offset.
- Minify and resource shrinking on for both release builds; signing guarded; CI blocks committed keystores; secret grep found nothing.
- FileProvider exposes only `exports/`. No `INTERNET` in either source manifest.
- Predictive back, RTL, themed (monochrome) launcher icon in place.
- No exact alarms; POST_NOTIFICATIONS requested at runtime; the specialUse FGS uses a typed `startForeground` on API 34+.
- Health Connect privacy-policy routing works.
- CI on HEAD 54c81b2 (run 36219235878) passed all three jobs, including the Room migration test (12→38) and a launch of the minified release APK.

## 2. RELEASING.md checklist

| Item | Status | Evidence |
|---|---|---|
| versionName matches | Met | `app/build.gradle.kts:49`, `wear/build.gradle.kts:36` |
| versionCode increased; Wear = phone + 100,000 | Offset met; vs Play unknown | Bumped to 91 in `e435a11` (08-26) |
| Changelog.kt, CHANGELOG.md, Play notes | Met, content stale | Written before the 09-21→09-26 work; `Changelog.kt:9-10` still says "starting draft" |
| targetSdk meets Play deadline | Met | 36 on both |
| Tests, Roborazzi, lint zero errors, CI green on the release commit | Met (CI 36219235878) | No lint.xml, baseline or lint block, so defaults apply |
| Signing configured, AABs verified | Unknown | No `v*` tag exists, so `release.yml` has never run |
| Privacy policy published at the Play URL | Unknown | |
| Both Health Connect privacy actions open the policy | Met | Manifest `:115`/`:537`; `MainActivity.kt:175` |
| Data Safety / Health Apps declarations | Unknown | |
| Policy, listing and app agree | Not met | P2-2, P3-1, P3-4 |
| Upgrade path, device matrix, Wear smoke | Unknown | No device available |
| Migration tests incl. oldest supported schema | Met (CI) | `MigrationTest.kt:358` full chain |
| Destructive fallback 1–11 | Open | `ForgeDatabaseFactory.kt:27`; restore guarded by `MIN_RESTORABLE_VERSION = 12` |
| Baseline profile | Not met (P4) | |
| 16 KB page size | Unknown | CameraX ships native libs; check with APK Analyzer |
| INTERNET absent from merged manifest | Source met; merged unverified | CI artifact host blocked from this environment |

## 3. Status of the 2026-09-12 P1s

- **R1 (app lock doesn't cover dialogs / Undo): OPEN.** `MainActivity.kt:523-558` still draws the lock as a sibling overlay, `SnackbarControllerHost` sits after it (`:558`), and the photo viewer/compare still use a platform `Dialog`. This audit found many more window-owning surfaces with the same problem (see `README.md`).
- **Data D1 (restore can lower the gallery lock): OPEN.** `data/repo/BackupRepository.kt:966-968` copies incoming prefs unchanged.
- **Domain D1 (keep-weight/consolidate ignore readiness/deload scaling): OPEN**, re-graded to P2 here; the coach-engine review keeps it at P1 (`10-…`). `weightChange(prevMax, prevMax)` at `domain/adapt/ProgressionAdvisor.kt:161-166, 175-179` ignores the `scale` computed at `:138`.
- **W01 (watch Log-set identity not durable): OPEN.** `pendingId`, `timedOutId`, `timedOutPayload` held in `remember` (`wear/.../SessionScreen.kt:72-80`); `sendLogSet` never persists them; `WearEditRecovery.Kind` (`:23`) covers only RPE and UNDO.
- **Prior P2 D2 (reset keeps backups): OPEN.**

## 4. Findings

### P1

1. **[P1] App lock doesn't cover dialogs or Undo (prior R1).** Location: `app/src/main/java/com/forge/app/MainActivity.kt:523-558`; `ui/profile/MirrorTestViewer.kt:205`; `ui/profile/MirrorTestCompare.kt:142`. Fix: hide/dismiss private dialogs and gate snackbar actions on `appLocked`. Confidence: high.
2. **[P1] Restore can lower protection without authentication (prior D1).** Fix: keep current app-lock, gallery-lock and timeout values when staging `pendingPrefs`, or require biometrics before a restore that would lower them; apply to manual and auto-backup restore. Confidence: high.
3. **[P1] Watch LOG_SET isn't durable (prior W01).** Location: `wear/src/main/java/com/forge/wear/ui/SessionScreen.kt:72-80`; `WearEditRecovery.kt:23`. A set can be logged twice or lost across Activity recreation. Fix: persist the command id and payload before sending, retry with the same id, clear only on a matching ack. Confidence: high. *(Wear module; outside the phone-app source review but part of release readiness.)*

### P2

1. **[P2] Progression cues skip deload/light-day scaling (prior Domain D1, re-graded).** `ProgressionAdvisor.kt:161-166, 175-179`. Fix: apply `scale` once with load rounding on both keep-weight branches.
2. **[P2] Factory reset keeps data the privacy policy says is erased.** `forge_auto_backup.zip` (+ tmp/json), `exports/` and `crashes/` survive, while `PrivacyPolicyPage.kt:51` says Settings can "erase all Avex data". Location: `data/repo/ResetRepository.kt:100-109`. Fix: delete them after cancelling `AutoBackupWorker`; add a `ResetRepository` test.
3. **[P2] No required checks on main.** main was red from 09-21 to 09-26 (CI runs 308–325, `DesignDoctrineTest.noEmDashesInRenderedStrings`) while PRs #187–#194 merged, so the migration + smoke-launch job was skipped for five days. Fix: branch protection requiring Guard, Verify and Instrumented.
4. **[P2] Wear health-permission rationale activity missing.** `wear/src/main/AndroidManifest.xml:22` requests `health.READ_HEART_RATE` on API 36 without a `VIEW_PERMISSION_USAGE`/`HEALTH_PERMISSIONS` rationale activity. Fix: add a `ViewPermissionUsageActivity` alias. Confidence: medium.
5. **[P2] Destructive Room fallback for schemas 1–11 is an unresolved RELEASING.md gate.** `ForgeDatabaseFactory.kt:27`. Fix: check Play's version distribution; if old installs remain, add a salvage migration. Confidence: medium.

### P3

1. Wear's `ACTIVITY_RECOGNITION` permission isn't disclosed in `.claude/PRIVACY.md` or `PrivacyPolicyPage`.
2. No CI check fails on `INTERNET` in the merged manifest. Fix: `aapt2 dump permissions` on both APKs against an allowlist.
3. The CI emulator is API 34 only (below targetSdk 36) and the Wear APK is never launched.
4. Device-to-device transfer still runs with `allowBackup=false` on API 31+ and copies `forge.db` without `forge.db-wal`, so recent sessions can be lost; the comment at `app/src/main/AndroidManifest.xml:87-91` is wrong. Fix: checkpoint in a BackupAgent, or exclude device transfer; correct the comment.
5. The SEND filter accepts `text/plain` and `octet-stream` but the handler reads only `EXTRA_STREAM` (`AndroidManifest.xml:120-128`, `MainActivity.kt:145`), so Avex appears for every text share and does nothing; the `content:` VIEW filter carries an unneeded BROWSABLE category (`:132`).
6. `wear/src/main/AndroidManifest.xml:70` exports `WearDataListenerService` without `BIND_WEARABLE_LISTENER`.
7. No tests at all for PdfExportRepository, ResetRepository, WidgetMidnightWorker, WorkoutSessionService, TimeChangeReceiver, ForgeWidget. Coverage elsewhere is healthy: 1,702 app, 50 Wear and 30 shared `@Test`s, 27 androidTest, 47 goldens.
8. The specialUse foreground service needs a Play Console declaration and video.
9. The engine reads injury restrictions but nothing writes them (`data/repo/CheckinRepository.kt:89-114`).

### P4

1. No `baseline-prof.txt`, although the build wires profileinstaller (`app/build.gradle.kts:322`).
2. R8 rules are asymmetric: Wear keeps `com.forge.shared.**`, the app doesn't, despite a comment saying they mirror each other; the global `enum *` rule and the whole Health Connect records keep are broader than needed.
3. AGP 9 opt-out flags in `gradle.properties:17-28` (`builtInKotlin=false`, `newDsl=false`, strict full-mode keep rules off, etc.) will need migrating before AGP 10.
4. Glance, documentfile and fragment versions are inline, outside the version catalog. Aging pins: datastore 1.1.1, coroutines 1.9.0, wear-compose 1.4.1, biometric 1.1.0.
5. Stale comments: `.github/workflows/release.yml:125-127` (`file()` vs `rootProject.file`), the build file's baseline-profile note, the Wear rules' "mirrors" claim.
6. `DATE_CHANGED` in a manifest receiver is never delivered on API 26+ (`AndroidManifest.xml:622`).
7. `strings.xml` holds only `app_name`; all UI copy is hardcoded; Wear tile labels are hardcoded in the manifest. The Play notes ship fr-CA but the app is English only.
8. The widget info XML has no preview, resize mode or target cells.
9. The 0.9 release notes don't mention the freestyle rebuild, the Academy rebuild (12 animated lessons) or the settings redesign.
10. Enum states never produced: `CoachPassStatus.ERROR` (`AutoCoachPlanner.kt:25`), `Confidence.LOW` (`Recommendation.kt:8`), `ProjectScanner.Kind.LOW_VOLUME` (`:34`).
11. `DirectiveRepository.kt:32` injects `workoutRepository` and never uses it.
12. The bulk dead code in section 5.

## 5. Verified dead code

**Method.** A script lexes each Kotlin file (blanking comments and strings, tracking scopes) and found 9,054 declarations in `app/src/main`. It counts references across app, wear, shared and baselineprofile (.kt and .xml), minus declaration sites. Excluded: `override`/`operator` members, Hilt modules/providers, TypeConverters, `@Serializable` classes, manifest-declared components, enum entries. DAO `@Query` methods with no callers are included. Every row was re-checked with a raw `grep -rnw`; there is no reflection in `app/src/main`.

**Full list:** [`dead-code-verified.tsv`](dead-code-verified.tsv) (206 rows: `zero` = no references, `testonly` = referenced only from tests; `parked` = the code documents it as intentionally kept).

**Resources:** 0 unused. Every drawable, mipmap, raw, layout, xml, color, style and string resource is referenced.

**Wholly unused files**
- `ui/coach/GoalPickerDialog.kt` (179 lines, parked)
- `ui/profile/ProfileActivityYear.kt` (358, parked)
- `ui/profile/ProfileYearHeatmap.kt` (108)
- `ui/overview/components/NavTile.kt` (98)
- `ui/common/RelativeTime.kt` (12)
- `data/repo/RestDayRepository.kt` (14)

**Used only by tests:** `domain/cardio/CardioCalorieEstimator.kt`, `domain/coach/CoachSignal.kt` (SignalRegistry), `domain/coach/SessionAdaptor.kt`, `domain/engine/ZoneCoach.kt`.

The per-area reports add dead code the script can't see: unreachable routes and never-dispatched events (whose declarations are referenced but never executed), for example ~20 never-dispatched `DayUiEvent`s (`01-…`), the unreachable Train tab and NotesSearch route (`01-…`, `02-…`), and the Nutrition / Coach Lab / Coach Timeline / Academy routes (`06-…`).

## 6. What could and couldn't be verified

**Could:** every build and manifest file, proguard rules, resources, CI workflows and scripts, release/privacy/audit docs; git history for each prior P1; CI status and failing-test logs through the GitHub API; the Play target-API rule via web search; `:shared` tests (30/30 passed) as a standalone build in a scratch copy through Google's Maven Central mirror.

**Couldn't:** any Android build or test in this environment (dl.google.com and the CI artifact host are blocked; Maven Central returned 429s), so there is no independent merged-manifest, R8, lint or 16 KB check; Play Console state; device behaviour; the Wear OS 6 rationale requirement (help pages blocked).

**Sources:** [Google Play target API 2026](https://median.co/blog/google-plays-target-api-level-requirement-for-android-apps) · [Play target API deadline, API 36](https://dev.to/dainyjose/google-play-requires-android-16-api-level-36-by-august-31-2026-react-native-migration-guide-1d51) · [Android 12 backup behavior changes](https://developer.android.com/about/versions/12/behavior-changes-12) · [Health Connect get started (rationale alias)](https://developer.android.com/health-and-fitness/health-connect/get-started) · [Health Services permissions](https://developer.android.com/health-and-fitness/health-services/permissions)
