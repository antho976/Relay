# Settings, App Lock, Notifications UI

Scope: `ui/settings/**`, `ui/security/**`, `ui/notifications/**`. 31 files, 8,495 lines, all read in full.
`S/` = `forge-android/app/src/main/java/com/forge/app/ui/settings/`; other paths relative to `forge-android/app/src/main/java/com/forge/app/`.

**Counts:** P1 4 · P2 9 · P3 12 · P4 8. Ten are still open from earlier audits (D1, D2, D6, R1, U05, U07, U08, U09, U10, cleanup item 8).

## P1

### [P1] Data dialog "Back up" exports progress photos past the gallery lock
- Category: bug (security/privacy)
- Location: `S/SettingsDialogs.kt:103` → `S/SettingsScreen.kt:201-203, 328` → `S/SettingsViewModel.kt:703-712` → `data/repo/BackupRepository.kt:718-731` (`writeBackupZip` adds `progress_photos/*` and the avatar)
- Problem: commit 84ea7a9 guarded `backupNow` (`:863`) and `setBackupFolder` (`:836`) with `protectedSettings.canExportPhotos()`, and the Backup page wraps both in `authenticateSettingsAction`. The Data dialog's "Back up" calls `backupDatabase(uri)` → `backupToUri`, which has neither guard. With gallery lock on and app lock off, anyone holding the phone can do Settings → Export data → Back up → Downloads and get a ZIP of the whole gallery. The "Backup & restore" search entry opens the same dialog. *Re-checked by the coordinator.*
- Fix: return early in `backupDatabase` when `!canExportPhotos()`; route the dialog's Back up through `authenticateSettingsAction` when `galleryLockEnabled` (as `SettingsBackupPage.kt:104` does); preferably enforce inside `BackupRepository.backupToUri` so no caller can skip it.
- Confidence: high

### [P1] Leaving Settings during a restore reports failure, while the staged restore replaces all data at next launch
- Category: bug (data loss)
- Location: `S/SettingsViewModel.kt:714-719, 813-818`; `data/repo/BackupRepository.kt:804-823, 848-1025`; `ForgeApp.kt:91-121`
- Problem: the restore is a plain `viewModelScope.launch` on the Settings-scoped VM. `restoreFromIncoming` never suspends inside its `withContext`, so once started it extracts, validates, publishes the manifest (`stagedOk = true`) and returns SUCCESS. If the user pressed Back during extraction (seconds with photos, no progress UI), the outer `withContext(IO)` resumes with prompt cancellation, SUCCESS is discarded, `runCatching{}.getOrDefault(IO_ERROR)` maps the `CancellationException` to "Couldn't read that file", and the pending set isn't cleared. `applyPendingRestore` swaps it in on the next cold start, silently overwriting everything logged in between. `backupDatabase` (`:703`) and `backupNow` (`:862`) swallow the same way and report "Backup failed: …cancelled" for completed backups.
- Fix: run restores in `NonCancellable` (as `write{}` does) or an app scope; never map `CancellationException` to a failure (rethrow, or `clearPendingRestore()`); show a blocking "Restoring…" state.
- Confidence: high on mechanics, medium on frequency

### [P1] Restore can turn off the gallery lock without authenticating — still open from 2026-09-12 data.md D1
- Category: bug (security)
- Location: `S/SettingsViewModel.kt:714-719, 813-818`; `S/SettingsScreen.kt:364`; `S/SettingsDialogs.kt:132`; `data/repo/BackupRepository.kt:964-966`
- Problem: incoming prefs are copied verbatim; a backup with `gallery_lock_enabled=false` lowers the lock. A ZIP with no photos keeps the current gallery with the lock off. The confirm step is an unauthenticated dialog.
- Fix: preserve current lock/privacy keys across restore, or require `authenticateSettingsAction` before any restore while a lock is on.
- Confidence: high

### [P1] Settings dialogs stay interactive above the app-lock overlay — Settings instance of RELEASE_AUDIT R1, still open
- Category: bug (security)
- Location: `S/SettingsScreen.kt:318-447`; `S/SettingsDialogs.kt:44, 264`; `MainActivity.kt:523-556`
- Problem: the nav host stays composed under `AppLockScreen`, and each Compose `Dialog`/`AlertDialog` is its own window. App lock on ("Immediately"), open Export data, press power: the next person to open Avex sees the lock drawn *under* a live Data dialog that still offers JSON/CSV exports (share sheet), Back up and Restore. Reset, restore and import dialogs behave the same.
- Fix: the root R1 fix; minimum: `LaunchedEffect(locked) { if (locked) clearAllDialogStates() }` in SettingsScreen.
- Confidence: high (source trace; window stacking not verified on a device)

## P2

### [P2] Lock screen doesn't handle system Back, so hidden screens react while locked
- Category: bug (security)
- Location: `ui/security/AppLockScreen.kt:130-167` (no `BackHandler`); `MainActivity.kt:529-553`
- Problem: pointer input is swallowed (`:139`) but Back reaches the covered nav host. It pops Settings pages (`S/SettingsScreen.kt:226-233`) and the Settings destination itself (also cancelling an in-flight restore, P1 above). In `ProgramBuilderScreen.kt:153` Back sets `showDiscard`, opening a dialog *above* the lock with a working "Discard" button.
- Fix: `BackHandler { activity?.moveTaskToBack(true) }` in AppLockScreen.
- Confidence: high

### [P2] Destructive resets run on a cancellable scope and can leave a half-wiped app
- Category: bug (data corruption)
- Location: `S/SettingsViewModel.kt:412-419`; `data/repo/ResetRepository.kt:69-83, 100-110`
- Problem: after Confirm the dialog closes with no progress and the user presses Back. Cancelled right after `clearAllTables()`: photos, avatar, Health Connect mirrors and prefs survive; `ONBOARDING_DONE` is still true, so the user lands in an empty app with no program and no onboarding. `resetSessions` can commit and then skip `deleteSessionMirrors` (reintroducing M-02).
- Fix: wrap resets in `write{}`/`NonCancellable`; show an "Erasing…" state.
- Confidence: high

### [P2] Exports crash the app on an I/O failure; "Last session PDF" silently does nothing
- Category: bug
- Location: `S/SettingsViewModel.kt:582-644`; `data/repo/BackupRepository.kt:118, 225, 450-525`; `PdfExportRepository.kt:49-53, 201-202`
- Problem: `writeText`/`outputStream` throw `IOException` on a full disk; none of the launches catch it (`exportFullBackup` has try/finally, no catch), so the uncaught exception in `viewModelScope` kills the process. Import and backup do catch. With no finished session the PDF export returns null with no feedback (`S/SettingsDialogs.kt:162`).
- Fix: one export helper that catches `Exception` (rethrowing `CancellationException`), snackbars "Export failed…", and says "Nothing to export yet." on null.
- Confidence: high

### [P2] Date format, Time format, Compact set logging have no effect; Timezone likewise — U08 still open
- Category: bug / dead-code
- Location: `S/SettingsSubPages.kt:92, 154-157, 160, 188-201`; `ui/theme/ForgeUiSettings.kt:20, 22-23`; `MainActivity.kt:462-478`
- Problem: no reader of `.dateFormat`, `.timeFormat24h` or `.compactSetLogging` outside settings/prefs/MainActivity; `timezone` is read only by SettingsViewModel. Screens hard-code formats (`CardioWeekDetailComponents.kt:165` "h:mm a", `PdfExportRepository.kt:47`, `DayCardComponents.kt:146`, `NotesSearchScreen.kt:125`). The Format page previews the chosen format anyway; the Appearance subtitle advertises the compact toggle.
- Fix: wire a shared formatter reading `LocalForgeSettings` into every timestamp site and the set rows, or remove the controls and prefs.
- Confidence: high

### [P2] "Reset app settings" erases the deload marker, so an active deload never ends
- Category: bug (wrong training data)
- Location: `S/SettingsViewModel.kt:415` → `ResetRepository.kt:93` → `data/prefs/SettingsRepository.kt:1316-1354`; consumer `WorkoutRepository.kt:602-615`
- Problem: the reset whitelist omits `DELOAD_WEEK_START_MS` and `PROGRAM_GENERATION_SEED`. Resetting during a deload week means `restoreAfterDeload()` never fires and the reduced-volume program stays indefinitely (the bug 0ea7102 fixed, now reachable from Settings). The same reset forgets backup/import folder URIs without releasing their grants (still open, data.md P3 #2).
- Fix: preserve internal program-state keys, ideally by clearing an explicit allowlist of user-facing keys; release folder grants via `PersistedTreeGrants`.
- Confidence: high

### [P2] Factory reset keeps the auto-backup ZIP and exports while saying "Deletes ALL data" — still open from data.md D2
- Category: bug (privacy)
- Location: `S/SettingsScreen.kt:152`; `ResetRepository.kt:100-110`; `S/PrivacyPolicyPage.kt:51`
- Problem: `forge_auto_backup.zip` (DB, prefs, photos) and `exports/*` survive; "Restore last auto-backup" is offered again afterwards.
- Fix: delete app-private backups and exports in factory reset, or correct the copy and privacy text until then.
- Confidence: high

### [P2] "Back up now" says "Backed up." when the folder copy failed — still open from data.md D6
- Category: bug
- Location: `S/SettingsViewModel.kt:867-870`; `BackupRepository.kt:561, 574-597`
- Problem: the folder write sits inside an ignored `runCatching`; a revoked grant or full provider still reports success.
- Fix: return a structured local/folder result and report partial failure.
- Confidence: high

### [P2] Loading chips can't express metric plates or dumbbells — still open (U07)
- Category: bug
- Location: `S/SettingsProgramPage.kt:429-439, 170-174`
- Problem: a kg user sees 2.3/4.5/6.8/9.1/11.3/20.4 kg plates and 6.8–45.4 kg dumbbells converted from pound hardware; 10 or 20 kg plates are impossible; an onboarding-set value shows no chip selected.
- Fix: native kg denominations when `isMetric`, stored as exact lb (as `OnboardingExtras.kt` does).
- Confidence: high

### [P2] Wearable page stays stale after "Manage in Health Connect" — still open (U09)
- Category: bug
- Location: `S/SettingsRecoveryPage.kt:39-78, 301-303`; `S/HealthConnectViewModel.kt:148`
- Problem: no refresh on resume; a revoked signal keeps showing "RECEIVING".
- Fix: `LifecycleEventEffect(ON_RESUME) { viewModel.refresh() }`.
- Confidence: high

## P3

### [P3] Searching "Factory reset" opens a chooser with no factory reset in it
- Location: `S/SettingsScreen.kt:74`; `S/SettingsMainList.kt:152`; `S/SettingsDialogs.kt:277`
- Problem: maps to `SearchAction.RESET` → `onOpenResetMenu`, and `ResetMenuDialog` filters out `FACTORY`.
- Fix: a dedicated action calling `onResetTarget(ResetTarget.FACTORY)`.

### [P3] Days-per-week chips write the pref without regenerating; the next re-roll silently changes the split while saying "Same split"
- Location: `S/SettingsProgramPage.kt:205-218`; `S/SettingsViewModel.kt:439, 496-512`; `ProgramRepository.kt:316-338, 378-384`; `WorkoutRepository.kt:622-627`
- Problem: tap "5" and the plan keeps 4 days while Home and the coach use 5. Manual re-roll and auto-rotation (`rerollAll`) then call `SplitTemplates.forDays(5)`; rotation does it with no prompt, bypassing the guard at `:535`.
- Fix: re-roll with `Program.days.size` and label the chip as input for the next plan, or apply via `generateProgram(n)` behind the guard.

### [P3] Deselecting the last equipment tile switches the generator to the full gym
- Location: `S/SettingsProgramPage.kt:415-420, 158`; `ExerciseLibrary.kt:1128`
- Problem: an empty set means "all equipment". Every tile reads "0 on", yet the summary says "All equipment" and Regenerate picks barbell and machine lifts.
- Fix: forbid an empty set or store an explicit "none".

### [P3] Toggle writes decide on stale snapshots, so rapid taps are lost
- Location: `S/SettingsViewModel.kt:522-530`; `S/SettingsProgramPage.kt:416-418`
- Problem: problem-area, priority-muscle and pin toggles read `.first()` outside the DataStore edit; the equipment tile builds the new set from composed state, so two quick taps drop the first. `toggleExercisesLiked` already decides inside the edit.
- Fix: decide inside `edit{}` (like `toggleFavoriteTimezone`); add `toggleEquipment`.
- Confidence: high on mechanics, medium on frequency

### [P3] "Notifications are turned off" banner never appears below Android 13
- Location: `S/SettingsSubPages.kt:555-571`
- Problem: returns early when `SDK < 33` and checks only `POST_NOTIFICATIONS`. Android 8–12 users who blocked the app see working toggles and no recovery path. `NotificationFeed.osNotificationsEnabled` (`NotificationFeed.kt:467-473`) already fixed exactly this (M-28).
- Fix: reuse `osNotificationsEnabled(context)`; re-check on resume.

### [P3] Credential-gated Settings actions fail silently
- Location: `S/SettingsSecurityPage.kt:52-62, 160-173` (`onError = { _, _ -> }`); `S/SettingsBackupPage.kt:53, 104`
- Problem: after the phone's screen lock is removed, turning App lock off does nothing (prompt errors, error dropped) while AppLockScreen fails open, so the switch shows on and protects nothing. Lockout and cancel are silent; "Back up now" does nothing in that state.
- Fix: surface errors through `noCredentialNote` or a lockout line; allow disabling when availability is `NO_CREDENTIAL`.

### [P3] Settings re-implements the shared capsules with a ~40dp target and no disabled semantics
- Category: bad-code / a11y
- Location: `S/SettingsPrimitives.kt:318-355` vs `ui/common/Capsules.kt:31-96`
- Problem: labelMedium + 13dp padding ≈ 40dp; when disabled the click is dropped entirely so TalkBack reads no role or state. DESIGN §8 says reuse the shared components.
- Fix: delegate to `ForgePrimaryCapsule`/`ForgeOutlineCapsule`, or `heightIn(min = 48.dp)`, `bounceClick(enabled = …)`, `Role.Button`.

### [P3] Custom-hex input clips at large font, 28dp target, no accessible label — U05 still open
- Category: a11y
- Location: `S/AccentColorPicker.kt:255-259` (`.size(96.dp, 20.dp)` on a text field), `266-280` (28dp swatch); `S/SettingsPrimitives.kt:497-518` (search field unnamed once filled)
- Fix: `widthIn(min = 96.dp)` without fixed height; `minimumInteractiveComponentSize()` on the swatch; semantics `contentDescription` on both fields.

### [P3] Rotation drops open Settings dialogs and drafts — U10 still open, more instances
- Location: `S/SettingsCardioActivitiesPage.kt:46-47`; `S/SettingsVacationPage.kt:52, 111-114`; `S/SettingsScreen.kt:187-197`
- Problem: `pendingRestoreUri`, `confirmReset`, the typed "ERASE", the search query and dialog flags use `remember`; rotating mid-restore-confirm loses the picked file.
- Fix: `rememberSaveable` (Uri is Parcelable; store targets by name, dates as Long).

### [P3] "Reset this page to defaults" is a one-tap wipe with no Undo
- Location: `S/SettingsSubPages.kt:539-546`; `data/prefs/SettingsRepository.kt:51-57`
- Problem: on the Session page it erases user-authored note templates; on Format it flips a kg user back to lb. Violates DESIGN §12.
- Fix: snapshot the section first, then `showUndo` to restore.

### [P3] Changelog ships with an unresolved "starting draft" note
- Category: release
- Location: `S/Changelog.kt:9-10`
- Fix: reconcile the 0.9 entries with what shipped; delete the note.

### [P3] SettingsViewModel is a god object with a 55-stage positional combine — still open (RELEASE_AUDIT cleanup item 8)
- Category: bad-code / perf
- Location: `S/SettingsViewModel.kt:111-895, 244-360`
- Problem: 19 dependencies; every pref write re-runs ~55 `combine` stages, including the dead `computeWeeklyVolume`.
- Fix: split Backup and Program ViewModels; typed pref groups.
- Confidence: medium

## P4

- **dead-code:** `S/SettingsViewModel.kt` unused setters `setTileHidden` (405), `setCustomWarmup`, `setOverviewTileOrder` (407-410), `loadSampleData` + the `sampleDataSeeder` dependency (420, 120), `setCardioWeeklyTargetMin` (445), `togglePin` (528-530); unused state `hiddenOverviewTiles`/`overviewTileOrder` (32-33), `cardioWeeklyTargetMin` (86), `pinnedExercises` (93), `weeklyVolume` + `computeWeeklyVolume` + its revision stage (105, 356-369), `useKg` (108); `S/SettingsPrimitives.kt:423-448` `TileOrderRow`; `S/SettingsAboutPage.kt:41` nullable `viewModel`. The tile hide/reorder feature has no UI at all. Delete, along with the matching MainActivity combine stages.
- **simplify:** back-navigation logic duplicated at `S/SettingsScreen.kt:226-233` and `242-249`; one `navigateUp()`.
- **bad-code — stale comments/copy:** `S/SettingsViewModel.kt:422-423` (lock setters "persist directly"; they go through `ProtectedSettingsActions`); `S/HealthConnectViewModel.kt:101` ("cleared on refresh", but `:194` preserves it); `S/SettingsRecoveryComponents.kt:42` ("0 OF 5", rail has 9); `S/AccentColorPicker.kt:109` ("nine", 12 presets); `S/SettingsDialogs.kt:155` sends the user to Settings → Backup for photos though the dialog's own Back up includes them; `S/SettingsMainList.kt:86, 97` duplicate `rowSubtitle` strings.
- **bug (minor):** timezone labels hard-code standard-time abbreviations/offsets ("PST −8" in summer) (`S/SettingsSubPages.kt:65-78`; `S/SettingsMainList.kt:354-360`).
- **bug (minor):** backup and crash-log filenames use default-locale digits (`avex_backup_٢٠٢٦-٠٩-٢٦.zip` under Arabic/Persian) (`S/SettingsScreen.kt:198-200`). Use `LocalDate.now().toString()`.
- **bug (minor):** rapid stepper taps are lost; each tap writes the last composed value ±1 (`S/SettingsPrimitives.kt:416-418`; `S/SettingsSubPages.kt:450-461`; `S/SettingsQuietHours.kt:50-51`).
- **bad-code:** duplicate glyphs `ui/notifications/NoticeIcons.kt:122-137` (Watch) vs `S/SettingsIcons.kt:128-143` (Wearable).
- **bad-code:** `ui/notifications/NotificationsViewModel.kt:39` `runCatching` swallows `CancellationException`, unlike `durable{}` in the same file.

**Checked and not reported:** holiday start/end order (normalized in the repo); stale reminder after section reset (worker re-checks the pref); Settings deload marker on `generate` (fixed); chip selected-state semantics (fixed; U04's settings scope closed); FileProvider scope (limited to `exports/`); About-page gesture claims (all exist); INTERNET permission (absent).

## Files reviewed (31)
- `ui/settings/`: AccentColorPicker, AppIconPicker, Changelog, HealthConnectViewModel, ImportDialog, PrivacyPolicyPage, SettingsAboutPage, SettingsBackupPage, SettingsCardioActivitiesPage, SettingsCoachPage, SettingsDialogs, SettingsFormat, SettingsIcons, SettingsMainList, SettingsPrimitives, SettingsProgramPage, SettingsQuietHours, SettingsRecoveryComponents, SettingsRecoveryPage, SettingsScreen, SettingsSecurityPage, SettingsStoragePage, SettingsSubPages, SettingsVacationPage, SettingsViewModel, SettingsWhatsNewPage
- `ui/security/`: AppLockScreen
- `ui/notifications/`: NoticeIcons, NotificationsOptionsSheet, NotificationsScreen, NotificationsViewModel
