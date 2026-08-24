# Avex Release Readiness Audit

Date: August 23, 2026  
Audit target: Avex Android phone and Wear apps  
Local commit: `c81eac7`  
Current GitHub `main`: `238fe54024f874e19771788fad88a565cf30b516`  
Audit mode: Static inspection plus local release gates. No device or emulator was connected.

## Verdict

**Do not release the current build yet.**

The release is blocked by failing CI, release-lint errors with real minimum-SDK risks, an incomplete Health Connect privacy path, stale version metadata, and missing release signing configuration. The R8 phone and Wear bundles compile, but the generated bundles are unsigned and are not ready for Play upload.

The local and canonical checkouts were also four commits behind GitHub `main` at audit time. The latest remote changes do not appear to remove the blockers below, but final fixes and validation must run from an updated branch.

## P0 release blockers

| Area | Blocker | Required result |
| --- | --- | --- |
| CI | Latest GitHub `main` workflow failed. The latest eight observed runs were failed or cancelled. | Current `main` must have a clean CI run. |
| Unit and doctrine tests | 954 tests ran and 3 failed. | All phone, shared, and Wear tests must pass. |
| Phone lint | 30 errors, 122 warnings, 11 hints. | Zero release-lint errors. Review high-risk warnings. |
| Wear lint | 2 errors, 13 warnings, 4 hints. | Zero release-lint errors and API-safe behavior on Wear OS 3. |
| Health Connect | Privacy-policy intent filters route to `MainActivity`, which does not display a dedicated privacy policy. | Both Health Connect privacy actions must open the same complete policy declared in Play Console. |
| Versioning | Phone and Wear still report `0.8.8.3`; a commit named Version 0.8.9 did not update Gradle metadata. | Set the intended public version and increment both version codes. |
| Signing | No `forge-android/keystore.properties` was available. Both AABs are unsigned. | Produce and verify signed upload bundles with the registered upload key. |
| Target API | Phone targets API 35. Play requires API 36 for phone updates starting August 31, 2026. | Prefer targeting API 36 now, or ship before the deadline with an explicit follow-up release. |

## 1. CI and test failures

Latest observed GitHub Actions run:

- Commit: `238fe54024f874e19771788fad88a565cf30b516`
- Run: [CI run 32658540112](https://github.com/antho976/Avex/actions/runs/32658540112)
- Result: failed
- Test count: 954 total, 3 failed

### Large-font wrapping failures

`DesignDoctrineTest.userContentWrapsAtLargeFontScales` found `maxLines = 1` on user-facing content:

- `ui/profile/ProfileActivityMonth.kt`: lines 175, 250, 261, 268
- `ui/profile/ProfileActivityYear.kt`: lines 195, 261, 328, 335

These need flexible wrapping and visual verification at 200% font scale.

### Typography scale failures

`DesignDoctrineTest.typeComesFromTheScale` reported direct `fontSize` values outside the shared type scale:

- `ProfileActivityMonth.kt`: lines 249, 267, 282, 290
- `ProfileActivityYear.kt`: lines 259, 334, 349, 357
- `ProfileSurfaceSections.kt`: lines 156, 236, 277, 441

Replace direct sizes with the appropriate doctrine typography tokens.

### Alpha ladder failures

`DesignDoctrineTest.onlyLadderAlphas` reported:

- `ProfileSurfaceSections.kt:257`: alpha `0.4`
- `ProfileSurfaceSections.kt:286`: alpha `0.18`
- Stale allowlist entry: `ProfileScreen` alpha `0.5` expected once but is now absent

Use approved alpha values and remove the stale allowlist entry.

## 2. Release lint failures

### Wear blockers

1. `wear/src/main/java/com/forge/wear/data/WristHaptics.kt:15`
   - Uses `VibratorManager` and `getDefaultVibrator`, which require API 31.
   - Wear `minSdk` is 30.
   - This is a runtime compatibility risk on Wear OS 3 and needs an API guard or API-30 fallback.

2. `WearHrService.kt:73`
   - Uses `FOREGROUND_SERVICE_TYPE_HEALTH`, introduced in API 34.
   - Guard and test this path for API 30 through 33.

Lower-priority Wear warnings include disabled resource shrinking, no monochrome launcher icon, and missing square/round tile preview assets.

### Phone blockers

- `ForgeWidget.kt`, lines 150 through 230:
  - 14 `ResourceType` errors and 12 `RestrictedApi` errors.
  - `ColorProvider(bgArgb)` is being resolved as a resource-ID overload and uses a restricted API.
  - The Glance padding calls also trigger resource-type errors.
- `SettingsVacationPage.kt:173,181`:
  - `LocalDate.ofInstant` requires API 34 without the correct desugaring or fallback.
  - Phone `minSdk` is 26, so this can fail on API 26 through 33.
- `AppLockScreen.kt:59`:
  - Casts `LocalContext` to `Activity`. Use the Compose activity-local API instead.
- `CsvParser.kt:15`:
  - Contains a literal byte-order mark. Represent it as `\uFEFF`.

CI currently runs unit tests, Roborazzi, `assembleDebug`, and `assembleRelease`, but does not explicitly run `:app:lintRelease` or `:wear:lintRelease`. Add both tasks so these failures block pull requests before the release stage.

## 3. Health Connect privacy and Play declarations

The manifest registers both Health Connect privacy actions, but they target `MainActivity`:

- `androidx.health.connect.action.SHOW_PERMISSIONS_RATIONALE`
- Android 14 and newer permission-usage alias

`MainActivity` has no dedicated handling that displays a privacy policy, so the current path opens the normal app instead of a complete privacy-policy screen.

The existing `.claude/PRIVACY.md` is not publishable. It still:

- Uses the Forge name
- Is dated June 16, 2026
- Contains `<your-contact-email>`
- Documents only sleep, resting heart rate, and weight access

The manifest currently requests 14 Health Connect permissions:

- Read sleep, resting heart rate, weight, steps, exercise, heart rate, distance, total calories, heart-rate variability, and lean body mass
- Write weight, active calories, exercise, and heart rate

The final policy and Play Console declarations should also account for camera, biometrics, notifications, foreground services, Wear body sensors, retention, deletion, and sharing behavior. The in-app policy must match the public policy and the Play Data Safety and Health Apps declarations.

Official references:

- [Health Connect setup and privacy requirements](https://developer.android.com/health-and-fitness/health-connect/get-started)
- [Publish a Health Connect app](https://developer.android.com/health-and-fitness/health-connect/publish)
- [Google Play target API requirements](https://support.google.com/googleplay/android-developer/answer/11926878?hl=en)

Positive privacy finding: the merged phone manifest has no `INTERNET` permission. It does include `ACCESS_NETWORK_STATE` through a dependency.

## 4. Version, signing, and package metadata

Current phone metadata in `app/build.gradle.kts`:

- Application ID: `com.quietsoftware.avex`
- Minimum SDK: 26
- Target SDK: 35
- Version code: 89
- Version name: `0.8.8.3`

Current Wear metadata in `wear/build.gradle.kts`:

- Application ID: `com.quietsoftware.avex`
- Minimum SDK: 30
- Target SDK: 35
- Version code: 100089
- Version name: `0.8.8.3`

The version names must match and both version codes must increase. Preserve the established Wear version-code relationship unless Play Console history requires a different next value.

The release build produced:

- Phone AAB: 20,742,348 bytes, unsigned
- Wear AAB: 3,754,934 bytes, unsigned

Play App Signing does not make an unsigned local upload acceptable. The upload AAB must be signed with the registered upload key.

## 5. User-visible Forge branding to change

These are visible to users and should be updated to Avex:

- Widget placeholder and dynamic title: `FORGE`
- PDF header: `FORGE - Session Report`
- Trophy names: `Seven-Day Forge` and `Million Pound Forge`
- Review `Forged Strength` separately. It may be intentional language rather than old branding.

Rename newly generated export and support files to Avex equivalents:

- `forge_weekly_export.json`
- `forge_export.json`
- `forge_session_<id>.json`
- `forge_sessions.csv`
- `forge_prs.csv`
- `forge_bodyweight.csv`
- `forge_cardio.csv`
- `forge_session.pdf`
- `forge_rank_card.png`
- `forge_backup_<date>.zip`
- `forge_crash_logs_<date>.zip`

Backward compatibility is required. Existing Forge-named imports, backups, and files must continue to be recognized.

### Internal Forge identifiers to keep

Do not mass-rename these for branding:

- Kotlin namespace and packages under `com.forge.*`
- Module folder `forge-android`
- Room database name `forge.db`
- DataStore name `forge_settings`
- Notification channel IDs
- WorkManager unique-work names
- Resource IDs, class names, theme names, and filenames that users never see
- Legacy import support

Renaming persistent internal identifiers can wipe data, reset settings, duplicate notifications or scheduled jobs, and break upgrades. They are compatibility identifiers now, not public branding.

## 6. Documentation, changelog, and store listing

Update these before release:

- `.claude/PRIVACY.md`
- `.claude/RELEASING.md`
- `.claude/CHANGELOG.md`
- `.claude/FEATURES.md`
- `forge-android/README.md`

`RELEASING.md` still describes a sideload-only app with no Play Store or wearable, which is no longer accurate.

The in-app `ui/settings/Changelog.kt` stops at version 0.8.8 from July 2026. Add the actual release version and cover the major shipped changes, including the Wear companion, Academy/library work, Profile activity grid, onboarding redesign, and recent settings, notification, and icon fixes.

Prepare matching Play release notes and refresh screenshots or the feature graphic if the visible product has materially changed. No current Play listing asset manifest was found in the repository.

## 7. Upgrade safety and required device validation

Room schema history exists from version 12 through 36, with explicit migrations. The database uses destructive fallback for versions 1 through 11. Confirm that no supported tester or production installation can still have schema 11 or earlier, because upgrading such a build will erase local data.

Migration tests exist under `androidTest`, but CI does not run them and no connected device was available during this audit.

Before upload, test a signed release build with:

1. Upgrade from the exact current Play build without uninstalling.
2. Cold start and database migration with real retained data.
3. Workout logging, PRs, rest timer, notifications, and widget updates.
4. Backup export, restore, and import of legacy Forge-named backups.
5. Health Connect permission rationale, policy route, read/write behavior, and revoked permissions.
6. Phone and watch pairing, heart-rate capture, tile, complications, and disconnected behavior.
7. Phone API 26 and a current Android version.
8. Wear API 30 and a current Wear version.
9. Normal and 200% phone font scale, plus enlarged Wear font scale.
10. Dark and AMOLED themes.

## 8. Repository hygiene after blockers

These do not enter the AAB and should not delay the release-blocking fixes, but should be cleaned up:

- Remove tracked `forge-android/app/release/output-metadata.json`, which still identifies `com.forge.app`, version code 85, version `0.8.5`.
- Remove other tracked generated release output, including dependency metadata.
- Review tracked `.env`, `.kotlin/errors/*.log`, `.idea/**`, `.design-backups/**`, `backups/**`, `.impeccable/review/**`, `forge-before-after.mp4`, and old root branding exports.
- Extend `.gitignore` for local environment files, IDE state, Kotlin error logs, and generated release output after removing the tracked copies.

## 9. Positive findings

- Phone and Wear application ID is correctly `com.quietsoftware.avex`.
- Phone and Wear app name is Avex.
- The internal file `ic_stat_forge` visually contains the Avex mark. Its filename does not need migration.
- `compileSdk` is 36.
- R8 minification and phone resource shrinking are configured, and both R8 bundles compile.
- Room schemas 12 through 36 and migrations are present.
- No `TODO` or `FIXME` markers were found in the main source scan.
- The repository was clean before this report was added.

## 10. Compact native quality score

| Lens | Score | Main reason |
| --- | ---: | --- |
| Accessibility | 2/4 | Large-font wrapping fails and RTL is disabled. |
| Performance | 3/4 | R8 and baseline profiles exist, but no hardware validation was completed. |
| Appearance and theming | 2/4 | Typography, alpha, widget, and PDF branding violations remain. |
| Platform conformance | 2/4 | Native Compose foundation is solid, but minimum-SDK and Health Connect routing issues remain. |
| Adaptivity | 2/4 | Large-font checks fail and no device, tablet, or watch run was completed. |
| **Total** | **11/20** | **Acceptable foundation, failed release gate.** |

## Ordered release checklist

1. Update the worktree to current `main` and rerun the audit gates.
2. Fix all phone and Wear lint errors, especially minimum-SDK violations.
3. Fix the three doctrine tests and verify Profile at 200% font scale.
4. Implement and publish the complete Health Connect privacy-policy route.
5. Update Play Data Safety and Health Apps declarations to the final permission set.
6. Set the real release version, increment both version codes, and target API 36 on phone.
7. Configure the upload signing key and produce signed phone and Wear AABs.
8. Replace user-visible Forge branding while preserving internal compatibility IDs and legacy imports.
9. Update the in-app changelog, release docs, Play notes, and listing assets.
10. Run migration tests and the signed on-device phone/watch smoke matrix.
11. Require green CI with explicit phone and Wear `lintRelease` tasks.

## Commands used for the local release gate

The combined gate was run with the Android SDK configured and no Gradle daemon:

```fish
set -lx ANDROID_HOME /home/anthony/Android/Sdk
./gradlew :app:testDebugUnitTest :shared:test :wear:testDebugUnitTest :app:verifyRoborazziDebug :app:lintRelease :wear:lintRelease :app:bundleRelease :wear:bundleRelease --no-daemon
```

A second run used Gradle's `--continue` behavior to establish that both release bundles compile despite the independent test and lint failures.

## Validation boundary

This audit proves the current static, unit-test, lint, Roborazzi, and bundle-compilation state described above. It does not prove runtime behavior, migration safety on a real install, release signing, Play upload acceptance, phone-watch interoperability, Health Connect behavior, or accessibility on physical hardware. Those remain required release gates.
