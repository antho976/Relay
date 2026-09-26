# Avex independent UI source audit: Academy and cardio

Baseline: `9637809`, worktree `gentle-zebra`, 2026-09-12. Read-only review. No production files changed.

## Scope and evidence

Read every line of all 26 non-ViewModel Kotlin files under `ui/academy` and `ui/cardio`, 6,182 lines. Followed relevant callbacks into CardioViewModel, CardioSessionDetailViewModel, ForgeNavHost, HealthConnectManager, and BounceClick. Root owns the ViewModel audit and complete Gradle gate; no additional Gradle invocation was run here. Source review applies the local Impeccable native audit, Android guidance and Forge design doctrine. It does not claim rendered fidelity, screen-reader behavior observed on a device, visual scores, or frame-time measurements. Prior production audit and remediation notes were checked to avoid reporting already-fixed cover decoding, custom activity resolution and animation observability issues.

Paths below are relative to `forge-android/app/src/main/java/com/forge/app/` unless stated otherwise. Line ranges refer to current source. Six retained defects below are source-confirmed reachable behaviors; the accessibility findings establish missing semantics in code, with actual TalkBack wording still requiring device verification.

## Retained findings

### U01, P2: Disabling Academy rotation can remove a recommendation from every location

**Locations:** `ui/academy/AcademyScreen.kt:122-127`; `ui/academy/AcademyGallery.kt:520-529`.

**Trigger:** Have two or more unread coach recommendations. Let the seven-second hero advance to the second or later item. Enable TalkBack touch exploration or turn animations off while this saved selection remains active.

**Evidence:** With `turns == false`, the screen removes `pointers.take(1)` from its chapters. The rotator still selects the saved `shownId`, then exits its effect without changing that ID when rotation is disabled. For `[A, B, C]` with `shownId = B`, the hero stays on B while chapters retain B and C and remove A. There is no manual carousel navigation. A is therefore absent from this Academy page; B is duplicated. This differs from the earlier fixed issue in which the animation-off setting failed to recompose.

**Fix:** Derive the displayed ID and the excluded chapter ID from one state owner. A small fix can force index zero synchronously whenever `turns` is false, so the removal matches the displayed item. A composition regression should exercise stopping rotation on index one and assert every unread ID remains reachable exactly once.

### U02, P2: Opening a cardio entry from History or Weeks drops its heart-rate and watch-stat features

**Locations:** `ui/cardio/CardioSessionDetailScreen.kt:68-80`; `ui/cardio/CardioSessionDetailViewModel.kt:124-143`; routes in `ui/nav/ForgeNavHost.kt:261-289`. Working sibling: `ui/cardio/CardioScreen.kt:152-166`, `ui/cardio/CardioViewModel.kt:337-352`.

**Trigger:** View a cardio session with a matched watch workout and at least two HR points from the Cardio tab. Then view that same entry through History, or Weeks > a week > a session.

**Evidence:** The in-tab screen passes `sessionHr`, `sessionWatch` and `adoptWatchStats`. The routed screen passes none of `hr`, `watchStats`, or `onAdoptWatchStats`, leaving their defaults null. Its loader reads steps and routes only. Both History and Weeks use this routed destination. The same saved entry therefore loses its HR graph, measured calories/duration/distance and explicit apply action based solely on the route used to reach it.

**Fix:** Share one session-detail state/load/action owner between the routed and tab variants, or complete the routed loader and pass the same fields/actions. Verify navigation parity using the same sample session.

### U03, P2: Watch duration/distance corrections are hidden unless heart-rate data is also available

**Locations:** `ui/cardio/components/CardioSessionDetailSheet.kt:233-246, 392-475`; independent data loading in `ui/cardio/CardioViewModel.kt:337-352`; `data/health/HealthConnectManager.kt:879-881`.

**Trigger:** Grant ExerciseSession and relevant distance permissions but deny heart-rate permission, or use a watch workout that has duration/distance and zero or one HR point. Open the matched manually logged cardio entry in the Cardio tab.

**Evidence:** `matchWatchSession` produces `watchStats` independently from `readHrSeries`. Missing HR permission returns an empty HR series. The only UI for measured duration/distance and `use watch stats` is inside `HeartRateSection`, and the parent instantiates that section only for `hr != null && hr.size >= 2`. Valid watch data is available in state, but the user cannot see or apply it without unrelated HR data. This remains even after U02 is repaired.

**Fix:** Render measured watch stats as an independent section or sibling of the optional HR chart. HR permission should control only HR content. Cover a matched duration/distance workout with empty HR in a composition test.

### U04, P2: Cardio selection chips expose no selected state to accessibility

**Locations:** `ui/cardio/components/CardioLogComponents.kt:342-365`; effort and HR callers in `CardioLogSheetSections.kt:86-113`, conditions at `119-132`; same issue in custom glyph selection at `CustomActivityDialog.kt:115-132`.

**Trigger:** Use TalkBack to select or review effort, HR zone, weather conditions or a rest reason in the cardio log form, or choose a custom activity icon.

**Evidence:** `PillChip(selected)` uses the selected flag exclusively for fill, border and text color, and adds only `bounceClick`. `BounceClick.kt:24-51` delegates to plain `clickable`; it adds no selected/checked/stateDescription semantics. A previously selected option and a deselected option have identical text/action semantics. Conditions permit several selections and repeated taps deselect them, so the user cannot inspect the saved selection without seeing color. The custom glyph chooser repeats this pattern.

**Fix:** Expose selection semantics on the actual clickable node, with selectable groups for single selection and checked/toggleable semantics for multi-selection. Keep one click handler, not nested click and toggle handlers. Announce selected state for glyph choices as well. The form's semantic regression should assert state changes in both directions.

### U05, P2: Filled cardio numeric inputs lose their accessible field names

**Locations:** `ui/cardio/components/CardioLogComponents.kt:236-266, 313-334`; parent group labels at `177-186`.

**Trigger:** With a screen reader, move focus directly between existing duration/distance or optional interval/lap/incline/elevation values while editing a populated cardio session.

**Evidence:** `CompactNumberField` renders its caption as a separate sibling Text outside the BasicTextField, and the BasicTextField has only a weighted modifier. `NumberInputRow` also supplies neither a semantic name nor a Material field label; its section title and unit are separate siblings. Once populated, the placeholder is absent. Thus the editable node provides its value and edit actions but no semantic association with Duration, Distance, Incline, etc. Visually adjacent labels are not a programmatic label relationship. A user navigating editable controls directly cannot reliably tell which value is being changed.

**Fix:** Provide each editable node an explicit accessible label while preserving its native editable semantics and unit, or use a field wrapper whose label is included in the field's semantics. Do not clear the edit actions/value to add a description. Assert label/value availability on filled fields; verify TalkBack navigation on device.

### U06, P2: Returning from an older cardio week always resets the chart to the latest page

**Locations:** `ui/cardio/CardioWeeksScreen.kt:79-106, 179-185`.

**Trigger:** With more than one page of cardio history, page back, open an older week's bar, then use either the back arrow or system Back to return to the chart.

**Evidence:** `pagesBack` is a plain `remember` declared after the `openWeek != null` early return. Opening any week removes that remember call from composition; returning creates `mutableIntStateOf(0)` again. The chart resets even without activity recreation. Rotation while browsing also resets it. This makes sequential review of older weeks require repeatedly paging back through the same windows.

**Fix:** Hoist `pagesBack` before the detail branch and store it with `rememberSaveable`, or place it in the retained screen state. Keep its existing range clamp. Check page > detail > Back retains the prior window.

## Lower-priority follow-ups and source risks requiring rendered/device evidence

- Several fixed Rows could crowd or clip controls at 200% font scale or compact width: hero figures in `CardioComponents.kt:108-143` are a Row despite the preceding comment claiming wrapping, HR zone/effort chips in `CardioLogSheetSections.kt:86-113`, and chart date navigation in `CardioWeeksScreen.kt:236-254`. Do not assign a visual failure score from source alone. Render compact portrait/landscape with large type to confirm the reachable result.
- `CustomActivityDialog.kt:72-154` places title, name field, glyph grid and footer in one non-scrollable Column, with no explicit IME adaptation. Its footer can be pressured by landscape/large text/keyboard. Verify actual dialog-window measurements before promoting to a release defect.
- Duration accepts `H:MM` (`CardioLogComponents.kt:374-396`) but requests a Number IME from its caller. Many numeric keyboards do not offer a colon. Minutes remain a valid fallback; verify intended phone keyboards before treating the clock-entry feature as reliably accessible.
- `RouteThumbnail.kt:30-49` allocates latitude/longitude lists and rebuilds a Path on every Canvas draw. It also scales latitude and longitude independently, changing route proportions. Bound/remember geometry outside drawing if profiling confirms cost; keep any shape-preservation change explicit rather than accidentally treating a thumbnail as a map.
- Time labels omit timezone in remember keys: `CardioWeekDetailComponents.kt:53-59`, `CardioSessionDetailSheet.kt:108-110` and `WatchImportsSection.kt:68-70`. A timezone switch while retaining the same item can leave old formatted timestamps until that component leaves composition. Root's day/time audit owns broader anchoring concerns.

## Cleanup, without behavior changes

- Remove stale narrative comments claiming the route thumbnail is not populated (`RouteThumbnail.kt:21-22`); the current screens and VM actively supply routes.
- Several large Academy comments recount abandoned visual implementations rather than current contracts. Keep the actual invariants (promotion ID, track order, reading completion, picture sizing) and remove stale inventory counts and historical UI instructions. `AcademyGallery.kt` still refers to 29 of 35 pieces awaiting art despite the active cover map.
- `CardioSessionDetailScreen.kt:18` still describes confirmed Delete although the current flow uses Undo. `CardioUiState.kt` comments describing the old five-entry default and absent zero-goal bar no longer match the week lens and WHO baseline.
- Keep the two detail presentations on one data/action contract; U02 illustrates the existing maintenance cost of duplicated wiring.

## Current behavior verified in source

- Academy covers decode off Main into a bounded bitmap cache and use measured display size; the earlier synchronous full-raster issue is fixed.
- Academy reader completion comes from the tail becoming visible rather than dismissing the screen; lesson headings use the shared heading component.
- Cardio draft primitives and custom activity draft names/glyph IDs use rememberSaveable, and system Back has explicit edit/detail handling.
- Session deletion navigation has a guard against two pops when both the deleted flag and database disappearance arrive.
- Charts and meters generally supply value-based semantics; the missing state/names above are localized gaps rather than an absence of accessibility work.

## Full non-ViewModel source coverage

All files listed below were read in full. Root owns the seven ViewModels, with relevant excerpts read here only to establish UI reachability and data-flow evidence.

- `forge-android/app/src/main/java/com/forge/app/ui/academy/AcademyCover.kt` (57 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/academy/AcademyGallery.kt` (703 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/academy/AcademyScreen.kt` (415 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/academy/ArticleScreen.kt` (57 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/academy/LessonBlocks.kt` (185 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/academy/LessonScreen.kt` (78 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/academy/Reader.kt` (263 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/CardioComponents.kt` (362 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/CardioScreen.kt` (570 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/CardioSessionDetailScreen.kt` (82 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/CardioWeeksScreen.kt` (279 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/LocalCardioTypes.kt` (30 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioBars.kt` (192 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioEntryRow.kt` (163 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioLogComponents.kt` (397 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioLogSheet.kt` (406 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioLogSheetSections.kt` (335 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioPaceTrendSection.kt` (117 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioSessionDetailSheet.kt` (476 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioWeekBars.kt` (180 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioWeekDetail.kt` (252 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CardioWeekDetailComponents.kt` (181 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/CustomActivityDialog.kt` (157 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/RouteThumbnail.kt` (57 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/components/WatchImportsSection.kt` (96 lines)
- `forge-android/app/src/main/java/com/forge/app/ui/cardio/state/CardioUiState.kt` (92 lines)

## Final bounded pass: common controls, onboarding and settings

Same baseline and source-only method. No Gradle, visual scoring, device execution, or production edits. Read control/behavior code in 59 additional files; comment/import-only lines were suppressed in some batch reads. Root retains ownership of settings mutations, protected restores, app lock and snackbar behavior. Additional findings below are distinct from those root-owned defects.

### U07, P2: Settings cannot represent common metric plate weights or dumbbell limits

**Code:** `ui/settings/SettingsProgramPage.kt:311-324`, conversion helper `143-147`; correct metric choices in `ui/onboarding/OnboardingExtras.kt:244-261`.
**Trigger/evidence:** A kg user opens Equipment to set a 10 kg/20 kg plate or a 30 kg dumbbell ceiling. Settings offers only pound denominations, converted for display: plates 5/10/15/20/25/45 lb, dumbbells 15/20/25/30/40/50/75/100 lb. There is no free input. None represents the requested metric hardware. A 10 kg onboarding choice is stored correctly but has no matching settings chip. Choosing the nearest setting changes the physical calibration, not just display units; `ui/gym/train/DayExerciseHandlers.kt:50` feeds this plate setting into WeightParser, and DayViewModelRefresh reads the dumbbell ceiling for progression.
**Fix:** Offer native kg hardware denominations and convert them to the canonical stored lb, as onboarding does. Include custom numeric entry where users need arbitrary equipment limits. Test exact unit round trips for 10/20 kg plates and 30 kg dumbbells.

### U08, P2: The timezone selector changes only Settings, not application timestamps

**Code:** `ui/settings/SettingsSubPages.kt:181-200, 319-417`; `data/prefs/SettingsRepository.kt:596-598`; `ui/settings/SettingsViewModel.kt:312-313`.
**Trigger/evidence:** Choose a timezone several hours from the device timezone, then inspect a known cardio/history timestamp. Repository-wide reference search finds the stored timezone consumed only by SettingsViewModel and Settings text/picker state. Timestamp screens keep using system timezone, e.g. `ui/cardio/CardioScreen.kt:103-117` and `CardioSessionDetailSheet.kt:108-110`. The preference offers apparent control over formatting without affecting that formatting. There is also no explicit return-to-device action in the picker after a fixed zone is selected; the normal default is resolved by the repository to the device ID, not a blank value.
**Fix:** Either remove this unused preference/control, or define and implement its formatting scope consistently. Avoid silently changing workout-day grouping while repairing a display preference; use one explicit zone source and test it. Do not count the missing device option as a separate defect.

### U09, P2: Wearable Settings remains stale after changing grants in Health Connect

**Code:** `ui/settings/SettingsRecoveryPage.kt:39-78, 317-318`; `ui/settings/HealthConnectViewModel.kt:148-168`; `ui/settings/SettingsRecoveryComponents.kt:86-118`.
**Trigger/evidence:** Open Wearable Settings, choose Manage in Health Connect, revoke a previously granted signal and return. The Manage action uses startActivity, not an activity-result callback. The screen has no lifecycle refresh observer. The retained ViewModel refreshes only on init or explicit permission-launcher callbacks, so the row continues to say connected and its Connect action remains hidden. Granting externally produces the reverse stale state. Returning to the page inside the same Settings destination retains the same Hilt VM.
**Fix:** Refresh permission/availability state on resume, preferably separating that cheap refresh from optional history imports. Verify grant and revoke round trips through the actual Manage action.

### U10, P2: Settings dismisses active custom-activity and holiday drafts on rotation

**Code:** `ui/settings/SettingsCardioActivitiesPage.kt:46-60`; `ui/settings/SettingsVacationPage.kt:52-58, 110-114`; child saveables in `ui/cardio/components/CustomActivityDialog.kt:61-62`.
**Trigger/evidence:** Start adding/editing a custom activity in Settings, type a name and choose its icon, then rotate. The parent uses plain remember for showCreate/editing, so the dialog disappears even though its child draft is saveable. The restored custom-activity fields have no active parent to show them. Holiday creation similarly loses showAdd and all start/end/label fields, with no saved state. Reopening starts from blank/default values.
**Fix:** Save the parent dialog identity as primitives (creation flag or existing activity code), and save holiday draft primitives. Keep the active modal and its content under one restoration owner; cover recreation while the dialog and date picker are active.

### Expanded scope for existing accessibility findings

- U04 also applies to `ui/settings/SettingsPrimitives.kt:160-188`: PillChip changes colors only and adds no selected/checked semantics. It affects units, training mode, schedule days, equipment, problem areas, coach mode, quiet preferences and wearable brand. `ui/common/ExerciseLibraryPicker.kt:112-140` likewise omits selected state for singleSelect, where no Checkbox child exists to carry state.
- U05 also applies to populated SettingsSearchField (`SettingsPrimitives.kt:397-417`) and CustomHexInput (`AccentColorPicker.kt:229-259`): captions/placeholders do not name the editable node once filled. Keep these in the same remediation family, not separate counts.

### Shortening and follow-up notes

- Share the settings/onboarding/cardio choice-control implementation. The current copies already disagree on selected semantics and disabled handling. Settings PillChip also replaces the original alpha with `.copy(alpha = alpha)` at `SettingsPrimitives.kt:176-185`; when enabled alpha is 1, this converts transparent backgrounds and 0.15 tints into opaque colors. This is a source color-math issue; no rendered visual severity assigned here.
- `UnitSegment` in `OnboardingPrimitives.kt:194` and the deprecated `EmptyState` in `common/EmptyState.kt:40` have no production call sites. `FirstTouchTip` is still used and should not be deleted as if equally dead.
- `common/LaunchScenes.kt` has no live IconLaunchScene caller, but AvexIntro explicitly documents retaining it for re-wiring. Removing that dormant feature is a product choice, not an automatic cleanup.
- Common drag state captures its initial onMove callback at `DragReorder.kt:54-62`; current inspected callers delegate to stable VMs or Compose state, so no reachable stale-callback defect is claimed. Updating it through rememberUpdatedState would make its contract safer if callbacks later capture changing plain values.
- Onboarding option/choice/equipment controls already expose selected semantics; the draft keeper synchronously records answers for recreation. No new retained onboarding defect was confirmed in this bounded pass.

### Coverage for this pass

**Common, 28 files:** AccentHex.kt, ArrivalBannerHost.kt, ArrivalController.kt, AvexIntro.kt, BounceClick.kt, Capsules.kt, ConfettiOverlay.kt, CurrentLocale.kt, DayLog.kt, DragReorder.kt, DurableWrite.kt, Editorial.kt, EmptyState.kt, ExerciseLibraryPicker.kt, ForgeModifiers.kt, ForgeMotionKit.kt, ForgeShimmer.kt, ForgeSwitch.kt, Format.kt, HapticExt.kt, ListMotion.kt, NotificationBell.kt, ProgramChangeGuard.kt, ProgramChangeGuardHost.kt, RelativeTime.kt, SegmentPill.kt, SparklineSeries.kt, TouchExploration.kt.
**Onboarding, 11 files:** OnboardingDraft.kt, OnboardingDraftKeeper.kt, OnboardingExtras.kt, OnboardingGymSteps.kt, OnboardingPrimitives.kt, OnboardingScaffold.kt, OnboardingScreen.kt, OnboardingSoreSpots.kt, OnboardingSteps.kt, OnboardingWeekMeter.kt, PlanModeMedia.kt.
**Settings, 20 files:** AccentColorPicker.kt, AppIconPicker.kt, ImportDialog.kt, SettingsAboutPage.kt, SettingsBackupPage.kt, SettingsCardioActivitiesPage.kt, SettingsCoachPage.kt, SettingsDialogs.kt, SettingsFormat.kt, SettingsMainList.kt, SettingsPrimitives.kt, SettingsProgramPage.kt, SettingsQuietHours.kt, SettingsRecoveryComponents.kt, SettingsRecoveryPage.kt, SettingsScreen.kt, SettingsStoragePage.kt, SettingsSubPages.kt, SettingsVacationPage.kt, SettingsWhatsNewPage.kt.
**Not read in full in this bounded pass:** common AvexWordmark.kt, ExerciseIcons.kt, LaunchScenes.kt, VectorBuilders.kt (art/geometry); onboarding OnboardingIcons.kt, PlanModeVignettes.kt; settings SettingsIcons.kt, Changelog.kt, PrivacyPolicyPage.kt. Root-owned SnackbarController.kt, SnackbarControllerHost.kt, SettingsSecurityPage.kt and ViewModels were not re-audited; only cited ViewModel boundaries were followed. This is explicit scoped coverage, not a claim that these omitted files were reviewed by this agent.
