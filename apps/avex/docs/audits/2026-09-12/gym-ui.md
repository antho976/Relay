# Gym rendering and state audit

Read-only review of HEAD 9637809, across whole source files. Complements [domain.md](domain.md). The separate data reviewer owns all ui/gym/stats rendering; this report covers the other 41 rendering/state files (11,140 lines). No production edits, no Gradle, no device or rendered screenshot claims.

Skills applied: Impeccable native audit and Forge design doctrine. The repository audit skill contains only “Not pinned.” Existing production audit/remediation records were checked to distinguish residual defects from previously fixed claims.

## Confirmed findings

### G1 / P2: Advancing from the last exercise can finish a workout with earlier exercises still incomplete

- Primary: app/src/main/java/com/forge/app/ui/gym/train/DaySessionContent.kt:242-265.
- Trigger: start a day with several exercises, use the live Up Next picker to jump to the last one, then log its target sets or choose Done with this exercise while earlier exercises remain incomplete.
- The Up Next list correctly wraps around and excludes completed exercises at214-218. The primary action computes a different successor at242-245: only a later-index, non-skipped exercise. On the last slot this is null, so it labels itself FINISH WORKOUT and directly dispatches FinishWorkout, even while Up Next displays unfinished earlier slots. It can also advance through already completed later slots.
- The live caller is reachable: UpNextBubble.onSelectExercise writes shownExerciseId at324. DaySessionHandlers.kt:80-101 closes the session, stops the service and timer without checking remaining exercises. It does not show an incomplete-workout warning.
- Impact: users following the supported reorder/jump flow get an incorrect finish action and can close the session before completing the remaining plan.
- Fix: derive both the Up Next panel and advance/finish actions from the same ordered list of incomplete exercises, wrapping once around the list. Keep explicit finish-anyway behavior separately available through the existing workout finish control. Verify last-slot jump, revisit-completed slot, skipped middle slot and genuinely all-complete states.
- Evidence: complete source-to-write-path trace, not a Compose runtime reproduction.

### G2 / P2: Current exercise history and table heading use pounds while the input/table rows honor kilograms or stones

- Primary: app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCard.kt:215-216 and302.
- Trigger: choose kg or st, log a weighted exercise and return to it in a later session.
- The “last” sentence prints persisted weightText directly. That text is canonical pounds, as enforced by DaySessionContent.kt:276-277 and FreestyleLogScreen.kt:419-425. The table header is hardcoded WEIGHT · LB. Meanwhile the same card's SetRow.kt:123-129 formats the actual row values and edit seed with the selected WeightUnit, and SetInputRow.kt:362-375 labels and accepts that unit correctly.
- Example: a prior 50 kg set is stored near 110.231 lb. The header repeats that raw 110.231 value as the previous weight while the visible input/rows use 50 kg. Timed holds also print the irrelevant raw-weight × 0-reps sentence instead of held duration.
- Impact: the same active logging surface contradicts its unit setting, so the last-performance cue can be misread when choosing the next weight.
- Fix: format prior performance with the canonical weight/duration formatters and effective exercise type, including a unit suffix; derive the header label from WeightUnit. The duplicate collapsed-card formatter at ExerciseCardComponents.kt:119-125 should use the same helper if retained, but that collapsed branch is not claimed as live because the sole caller currently forces expansion.
- Evidence: source-confirmed rendering expressions and canonical storage boundary; no screenshot claim.

### G3 / P2: Configuration recreation loses the latest freestyle draft edits

- Primary: app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogScreen.kt:338-381, especially338 and375-381.
- Trigger: edit a freestyle set and rotate/recreate the activity within the 600 ms autosave debounce. The in-memory exercise list, draft identity, start time, pending draft and overlay state all use plain remember. The only durable write waits600ms in LaunchedEffect; recreation disposes the composition and cancels that delay.
- The flush at389-394 runs only through explicit toolbar/system Back, not activity destruction. FreestyleLogViewModel.kt:54-58 explicitly leaves editable state in the screen and has no retained copy. MainActivity has no configChanges override in AndroidManifest.xml.
- On recreation the latest list is empty again and the one-shot load at371-374 offers the older on-disk draft, losing the newest weight/reps edit. Even after the debounce a simple rotation unnecessarily returns an active logger to the Resume prompt. ExerciseBrowserScreen.kt:166-169 also loses pending selections because both its selection state and the parent showBrowser flag are plain remember.
- Fix: keep the authoritative editing model in a retained ViewModel and save process-restorable draft state with SavedStateHandle, or provide an explicit Saver for the full draft/UI state. Continue durable autosave for app-kill recovery. Restoring a configuration change should resume the current screen directly and preserve the latest edit and picker selections.
- Evidence: source-confirmed ownership/cancellation path; physical rotation/activity-recreation test remains needed.

### G4 / P2 accessibility: The main set-entry text fields have no persistent semantic label

- Primary: app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt:785-802 and app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogScreen.kt:1094-1109.
- The hot-path BasicTextFields expose the editable value but receive no semantic purpose/unit label. Their visible WEIGHT/REPS/HOLD labels are separate sibling Text nodes (SetInputRow.kt:288,362-369 and FreestyleLogScreen.kt:825-858). BigNumberField/FsUnderlineField do not accept a label parameter. Once filled, even the numeric placeholder is gone; navigating directly between editable controls leaves only ambiguous numbers.
- This affects every ordinary live weighted set, bodyweight reps field, and freestyle weight/reps/hold field. Repeated freestyle rows additionally need exercise and set-number context to disambiguate otherwise identical controls.
- Fix: provide a persistent accessible label on each field using the appropriate text-field/semantics API, including unit and set context where needed. Preserve the native editable-text semantics. Add a semantics assertion for field identity and do a focused TalkBack editing pass.
- Evidence: source-confirmed missing label association. Actual TalkBack wording was not exercised, so no exact screen-reader announcement is claimed.

## Cleanup with safe scope

1. Remove obsolete Train-tab wrapper branches and unused callbacks after checking public previews/tests. DayListScreen.kt:97-114 still routes to a private TrainTab, but its sole production caller HubScreen.kt:133-149 always passes initialTab=1. The old day-card/reroll/edit layout is unreachable through current navigation. Do not spend release time fixing its visual debt as if it were active.
2. Remove unused ExerciseCard callbacks onOpenGoalSetter, onSetExerciseUnit, onMoveUp and onMoveDown (ExerciseCard.kt:80-96) and dead plumbing where no other caller exists. The unit-setting callback is passed but never called; the live input explicitly says units are changed in Settings. Other undispatched DayUiEvent handlers should be reconciled with settled removals before being revived.
3. SessionSummaryComponents.kt still holds MoodPrompt/MoodChip, JournalField, TagPicker and SESSION_TAGS without an external caller. They are detached UI left behind by the simplified summary, suitable for deletion or moving only if another intentional caller exists. They are not presented as missing features.
4. Use an immutable java.time formatter for HistoryRows.kt:222-227 instead of global SimpleDateFormat instances. historyDayLabel runs on Dispatchers.Default through SessionHistoryViewModel.kt:94-102, while formatHistoryDate is also called by the freestyle template composition. The shared mutable formatters are unsafe under overlapping use and freeze the locale/time zone at class initialization. No deterministic live failure was reproduced, so this is a robustness cleanup rather than a ranked release defect.

## Unverified visual follow-up

- RestTimerControlsDialog uses a non-scrollable body and a horizontal row of four duration presets; SessionDetailCharts.SegmentRow uses a non-wrapping metric row. Check those live dialogs/controls at200% font size, narrow width and landscape. Source alone does not establish a specific observed clipped control.
- No contrast, hit-target or complete visual score is assigned without rendered/device evidence. Source assessment: accessibility needs the G4 repair; runtime-state correctness needs G1/G3; unit/theming conformance needs G2. Performance showed no additional reproducible blocker in this pass.
- The earlier top-lift cache concern for equal-volume history edits was rejected for current UI reachability: SessionDetail only exports, re-logs and changes session type; it has no set editor/reopen path. Parent/data reviewer may retain it as cleanup if another producer is not found.

## File coverage

All files below were fully inspected across this phase or the preceding business/state phase. Supporting navigation, ViewModel and write-path calls were rechecked for findings. All ui/gym/stats files are covered by the separate data reviewer, not claimed here.

- app/src/main/java/com/forge/app/ui/gym/freestyle/ExerciseBrowserScreen.kt
- app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogScreen.kt
- app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleTemplatePicker.kt
- app/src/main/java/com/forge/app/ui/gym/history/HistoryRows.kt
- app/src/main/java/com/forge/app/ui/gym/history/SessionHistoryScreen.kt
- app/src/main/java/com/forge/app/ui/gym/notes/NotesSearchScreen.kt
- app/src/main/java/com/forge/app/ui/gym/session/SessionDetailCharts.kt
- app/src/main/java/com/forge/app/ui/gym/session/SessionDetailComponents.kt
- app/src/main/java/com/forge/app/ui/gym/session/SessionDetailMetricCards.kt
- app/src/main/java/com/forge/app/ui/gym/session/SessionDetailScreen.kt
- app/src/main/java/com/forge/app/ui/gym/session/SessionTypeDialog.kt
- app/src/main/java/com/forge/app/ui/gym/session/state/SessionDetailUiState.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayDialogs.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayListScreen.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayScreen.kt
- app/src/main/java/com/forge/app/ui/gym/train/DaySessionContent.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayWarmupBuilder.kt
- app/src/main/java/com/forge/app/ui/gym/train/RestTimerHapticCues.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/AddExerciseSheet.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/DayCard.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/DayCardComponents.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/DifficultyRater.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/DislikeSwapPromptDialog.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCard.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseCardComponents.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/ExerciseChartSheet.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/NoteField.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/RestTimerBubble.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/SessionSummaryComponents.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/SessionSummarySheet.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/SetInputRow.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/SetRow.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/SetTable.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/SwapPickerSheet.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/TrainIcons.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/TrainingHelpers.kt
- app/src/main/java/com/forge/app/ui/gym/train/components/WarmupFlow.kt
- app/src/main/java/com/forge/app/ui/gym/train/state/DayListUiState.kt
- app/src/main/java/com/forge/app/ui/gym/train/state/DayUiEvent.kt
- app/src/main/java/com/forge/app/ui/gym/train/state/DayUiState.kt
- app/src/main/java/com/forge/app/ui/gym/train/state/SessionSummary.kt
