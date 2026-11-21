# Avex Independent Pre-Release Bug Scan

**Date:** 2026-08-25

**Primary revision:** `60fac9e`

**Post-scan fix revision checked:** `e5b3879` (`claude/pre-release-bug-scan-l8pnsa`)

**Scope:** `forge-android/`

**Result:** 0 additional critical, 3 additional high, 2 medium; all 5 fixed in the current worktree

## Purpose

This is a focused second-pass scan. Claude's `BUG_SCAN.md` and all eight detailed reports under
`docs/bug-scan-2026-08/` were indexed first. Their 154 findings are intentionally not repeated here.

The five findings below do not appear in that report. Their affected files were unchanged between
`60fac9e` and `e5b3879`, so both revisions contained them. The current worktree now includes the
remediation described under each finding.

## Release verdict

**Do not ship `60fac9e` or `e5b3879` as-is.** The current worktree fixes these five findings. This
does not supersede Claude's broader release verdict or the existing CI blocker.

Those two revisions were blocked because:

1. The app and gallery locks could fail open on temporary biometric errors.
2. The gallery lock did not protect the progress-photo filmstrip on Profile.
3. An interrupted JSON write could silently orphan the progress-photo library and its metadata.

## Summary

| Severity | Area | Finding | Status |
|---|---|---|---|
| HIGH | App lock | Temporary biometric unavailability is treated as successful authentication. | Fixed |
| HIGH | Photo privacy | Profile renders up to ten progress photos without passing through the gallery lock. | Fixed |
| HIGH | Photo storage | `index.json` is overwritten non-atomically and corrupt reads become a valid empty library. | Fixed |
| MEDIUM | Avatar storage | A failed replacement deletes the previous valid avatar. | Fixed |
| MEDIUM | Import boundary | Exported intents can import workout history before app-lock authentication or confirmation. | Fixed |

## HIGH 1: App lock fails open on temporary biometric errors

**Files:**

- `app/src/main/java/com/forge/app/security/BiometricAuthenticator.kt:35-36,70-75`
- `app/src/main/java/com/forge/app/ui/security/AppLockScreen.kt:63,74-91`

`BiometricAuthenticator.canAuthenticate()` reduces every Android result to a Boolean. Any result
other than `BIOMETRIC_SUCCESS` becomes `false`, and `AppLockScreen` calls `onUnlocked()` whenever
that Boolean is false.

The prompt callback has the same defect. `ERROR_HW_UNAVAILABLE` is classified as a no-credential
error, then promoted to `onUnlocked()` without a successful biometric or device credential.
Android defines hardware unavailability as temporary and instructs callers to try again later.

Reference: [AndroidX `BiometricManager`](https://developer.android.com/reference/androidx/biometric/BiometricManager#BIOMETRIC_ERROR_HW_UNAVAILABLE).

### Failing scenario

1. App lock or gallery lock is enabled.
2. Android's biometric service or sensor is temporarily unavailable.
3. `canAuthenticate()` or the prompt callback reports hardware unavailable.
4. Avex marks the session authenticated and exposes the protected content.

### Required fix

Preserve the actual `canAuthenticate` result. Fail open only after explicitly confirming that no
device credential is enrolled. Hardware unavailable, unknown, unsupported, security-update, and
prompt failures must remain locked and offer retry or cancellation.

### Resolution

Authentication capability now has three states: available, no enrolled credential, and unavailable.
Only the explicit no-credential state can fail open. Temporary, hardware, and unknown failures remain
locked with a retry action. Prompt errors re-check the system capability instead of trusting a broad
error-code list.

## HIGH 2: Gallery lock exposes the Profile filmstrip

**Files:**

- `app/src/main/java/com/forge/app/ui/settings/SettingsSecurityPage.kt:68-77`
- `app/src/main/java/com/forge/app/ui/nav/ForgeNavHost.kt:395-412`
- `app/src/main/java/com/forge/app/ui/profile/ProfileViewModel.kt:141-155`
- `app/src/main/java/com/forge/app/ui/profile/ProfileScreen.kt:400-412`
- `app/src/main/java/com/forge/app/ui/profile/ProfileExtras.kt:264-270,330-340`

Settings promises: "Require an unlock to view your progress photos." Authentication is applied only
to the full gallery route, `Routes.MIRROR_TEST`.

Profile independently loads the full photo index and renders up to ten thumbnails in `GalleryStrip`
without consulting `galleryLocked`. Tapping a thumbnail is protected because it navigates to the
gated gallery, but the thumbnail itself is already sensitive content.

### Failing scenario

1. App lock is off and Photo gallery lock is on.
2. A new foreground session has not been authenticated.
3. The user opens Profile.
4. Up to ten physique-photo thumbnails are visible without an unlock prompt.

### Required fix

Do not load or render photo bytes on Profile while `galleryLocked` is true. Render a locked
placeholder that invokes the same authentication gate before displaying the filmstrip.

### Resolution

Profile now observes the shared gallery-lock state. While locked, it passes no photo records to the
filmstrip and renders the existing three-cell strip as an `UNLOCK PHOTOS` placeholder. The gallery
lock is also seeded synchronously on cold start, preventing a first-frame thumbnail flash before the
settings flow emits.

## HIGH 3: Progress-photo index writes are not crash-safe

**File:** `app/src/main/java/com/forge/app/data/repo/ProgressPhotoRepository.kt:153-155,190-195,288-330`

`writeIndex()` overwrites `progress_photos/index.json` directly with `File.writeText`. The existing
file is truncated before the new JSON is completely persisted. There is no temporary file, sync,
atomic rename, or last-known-good copy.

`readIndex()` catches every parse or I/O failure and returns `emptyList()`. That makes a corrupt
index indistinguishable from a genuinely empty photo library.

### Failing scenario

1. The user edits photo metadata or imports a photo.
2. Storage fills, the process dies, or power is lost after truncation and before the write completes.
3. On the next launch, the malformed index is silently read as an empty library.
4. A later mutation reads that empty fallback and writes a valid index containing only the new state.

The previous JPEG files generally remain in `progress_photos/`, but Avex no longer exposes them.
Titles, notes, albums, poses, bodyweight links, muscle tags, and free tags are lost from the index.

### Required fix

Write the new JSON to a sibling temporary file, flush and sync it, then atomically rename it over the
live index. Do not rewrite the library from an empty fallback when an existing index is corrupt.
Preserve the corrupt file or a last-known-good copy for recovery.

Apply the same write pattern to `albums.json`, which currently has the same direct-overwrite shape.

### Resolution

`index.json` and `albums.json` now use Android's `AtomicFile`, including recovery of a last-known-good
backup after an interrupted write. Read failures still produce a safe empty display, but every
mutation performs a strict read first and refuses to rewrite corrupt metadata. Failed new-photo
indexing also removes only the uncommitted new image.

## MEDIUM 1: Failed avatar replacement deletes the current avatar

**File:** `app/src/main/java/com/forge/app/data/repo/AvatarRepository.kt:46-75,86-95`

If the selected URI cannot be decoded, `set()` returns `false` before opening `avatar.jpg` for
output. The shared failure branch then deletes `avatar.jpg`, destroying the previous valid avatar.
An interrupted encode can also truncate the live file because it is overwritten in place.

Write the replacement to a temporary file and rename it only after decode and JPEG compression both
succeed. A failed replacement must leave the existing avatar untouched.

### Resolution

Avatar JPEGs now use the same atomic writer. Decode and compression failures leave the previous file
untouched, bitmap cleanup runs in `finally`, and reads recover an interrupted replacement backup.

## MEDIUM 2: Exported import intents bypass app-lock authentication

**Files:**

- `app/src/main/AndroidManifest.xml:75-108`
- `app/src/main/java/com/forge/app/MainActivity.kt:96-118,243-245`
- `app/src/main/java/com/forge/app/data/importer/WorkoutImportRepository.kt:54-70,105-198`

`MainActivity` is exported for `ACTION_SEND` and `ACTION_VIEW`. A supplied content URI is read and
imported immediately, including during cold start, before the user authenticates through the app
lock. The importer commits recognized sessions directly in a Room transaction without a confirmation
step.

Another installed app can therefore issue an explicit intent with a granted URI and inject
valid-looking workout history while Avex is locked. The attack requires a local app and is additive,
which keeps this below high severity.

Stage or parse the import first, then require an authenticated session and explicit user confirmation
before committing it. Clear the consumed intent so recreation cannot repeat the operation.

### Resolution

Incoming share and view intents now stage only their URI. Avex waits until onboarding, the launch
intro, and the app lock are clear, then asks for explicit confirmation before calling the importer.
The consumed intent is neutralized, and the pending URI survives Activity recreation without being
committed twice.

## Validation

- Claude's complete report and its eight detailed area reports were searched for every class, method,
  error code, and scenario above. No matching finding was present.
- The affected files were diffed from `60fac9e` through `e5b3879`; none changed.
- The post-scan fix revision compiled and ran 1,028 app unit tests: 1,025 passed. The only three
  failures were the already-known Profile design-doctrine gate failures.
- `:shared:test` passed. `:wear:testDebugUnitTest` has no test sources.
- Biometric, process-death, storage-full, and lock-screen behavior were source-verified but not
  reproduced on a physical Android device.

### Remediation validation

- `:app:compileDebugKotlin` passes.
- Ten focused regression tests pass for biometric classification, the locked Profile strip, atomic
  rollback and backup recovery, corrupt photo-index preservation, failed avatar replacement, and the
  import lock gate.
- The complete app unit task ran 1,024 tests: 1,021 passed. The only failures are the same three
  pre-existing Profile doctrine failures in `ProfileActivityMonth.kt`, `ProfileActivityYear.kt`, and
  `ProfileSurfaceSections.kt`; none of those files changed in this remediation.
- `:app:lintDebug` reports no findings in any touched file. The repo-wide gate remains blocked by 29
  unrelated errors, with the first in `widget/ForgeWidget.kt:150`.
- Physical biometric prompts, process death, and exported-intent delivery still require device-level
  verification.

## Completed fix order

1. Biometric fail-open.
2. Profile filmstrip privacy gate.
3. Atomic progress-photo index and album writes.
4. Atomic avatar replacement.
5. Authenticate and confirm external imports.
