# Domain, program, training business audit

Read-only review of HEAD 9637809. Reviewed the full scoped source, not commit diffs. No production edits and no Gradle invocations by this reviewer. Prior audit/remediation records checked: docs/AVEX_PRODUCTION_SOURCE_AUDIT_2026-09-01.md, docs/AUDIT_DEFERRED.md, docs/audit-fixes-2026-09-07.md. The audit skill contains only “Not pinned.”

## Confirmed findings

### D1 / P1: Ordinary progression cues ignore a light day and deload phase

- Primary: app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt:175-179. Same defect in consolidation branch at161-166.
- The method computes readiness/intensity/phase load scale at119-143, but these branches return prevMax as target unchanged and omit the scale reason. Any normal weighted set that has not reached the top of its rep range takes the keep-weight branch. This is the common path, not an unusual malformed input.
- Reproduced against compiled app classes: previous100lb ×8, range8-12, JUST_RIGHT, readiness-5 yields target100lb. Add DELOAD phase and target remains100lb. The branch should apply the composed scale (.95 or .8075), with normal load-grid rounding, while preserving the fact that no progression jump was earned.
- Impact: the in-session cue contradicts reduced-load coaching and can prescribe ordinary weight during the planned deload. Earlier H-02 phase wiring is present but not honored by these branches; this is a remaining defect, not a repeat of the previously fixed absent phase argument.
- Fix: determine the base training weight first for each branch, then apply day/phase scaling exactly once to every weighted output; preserve load-unit rounding and prevent consolidation from accidentally granting an extra increase. Add branch-specific cases for incomplete rep ranges and consolidation at light/readiness/deload scales.

### D2 / P2: Empty or different-movement bouts validate a coach swap

- Primary: app/src/main/java/com/forge/app/domain/coach/OutcomeWatcher.kt:85-101, especially97-101.
- At the end of the14-day window, “non-skipped row exists” is sufficient for an ok verdict. It neither requires a logged set nor checks performedExerciseId against the requested replacement in decision.payload.
- Real producer: DayExerciseHandlers.kt:60-69 lazily creates a LoggedExercise when a user writes only a note; the workout may finish with this row and sets elsewhere. SnapshotAssembler.kt:51-73 includes that empty row as a non-skipped ExerciseBout. A session-only substitution also preserves the base slot key while performedExerciseId changes.
- Reproduced with the actual OutcomeWatcher: a replacement db-row with zero sets yields ok; one real barbell-row set while the requested replacement is db-row also yields ok.
- Impact: the coach records an unperformed change as successful and it contributes to automatic-apply trust (TrustLedger.kt:68-88). Prior M-08 fixed the entirely absent-bout case, but these unperformed exposures still pass the new guard.
- Fix: require meaningful performed working sets on the requested replacement; count no exposure as OUTCOME_NOT_FOLLOWED. Keep skip handling and actual replacement failure separate. For legacy decisions without payload, choose an explicit conservative rule rather than treating arbitrary slot activity as success.

### D3 / P2: Midnight widget worker replaces and cancels itself before refresh

- Primary: app/src/main/java/com/forge/app/service/WidgetMidnightWorker.kt:34-38 and60.
- doWork calls schedule before its suspending widget refresh. schedule enqueues the same unique name forge_widget_midnight with ExistingWorkPolicy.REPLACE, so the currently running worker is part of the unfinished work being cancelled.
- Impact: cancellation can reach the CoroutineWorker while refreshForgeWidgets is suspended, aborting the intended midnight redraw. The next worker is scheduled, but each run repeats the same race. Independent Wear/services reviewer confirmed the same source path.
- Fix: make self-rearming use a successor chain or distinct next-date unique key so it does not replace the current worker; keep external clock-change replacement separate. Reordering the same REPLACE after refresh avoids losing that refresh but still reports/cancels the current worker, so a proper scheduling split is preferable.
- Evidence limit: source and WorkManager semantics; no device/WorkManager runtime reproduction here. Weekly recap and training-reminder scheduling findings are owned by the Wear/services reviewer and intentionally omitted from this report.

### D4 / P3: Final “not followed” outcomes display as still watching forever

- Primary: app/src/main/java/com/forge/app/domain/coach/CoachOutcome.kt:19-35.
- OUTCOME_NOT_FOLLOWED is a terminal verdict emitted by OutcomeWatcher and program generation. label handles only ok/failed; all other outcomes use the pending countdown.
- Reachable display: ui/coach/CoachUi.kt:91 calls this helper for applied decisions.
- Reproduced: label(applied, not_followed, appliedAt=1, now=20days) returns “still watching.”
- Fix: add explicit neutral terminal wording for OUTCOME_NOT_FOLLOWED and reserve countdown for pending. No schema change needed.

### D5 / P2: Imported weighted timed-only history crashes progression

- Primary: domain/adapt/ProgressionAdvisor.kt:347-358 and507; duplicate coarse filter at397-401.
- The bout filter admits any non-assisted set with weightLb. bestWorkingE1rm excludes duration sets (domain/adapt/E1rm.kt:47-48), but private bestE1rm force-unwraps its null result. Five admitted bouts are enough to enter the calculation and throw NullPointerException at507.
- Reproduced using actual app classes with WEIGHT slot and five bouts each containing45lb, reps0, durationSeconds60. Stack: evaluate358 -> bestE1rm507. AdaptationRepository recommendations/engineStatsRead and DeloadAdvisor.instrument call progression.
- Parent/data-path validation confirmed reachability: catalogue barbell-bench-press in five distinct finished tracked imported sessions, each with weightLb100, reps0 and durationSeconds60, is accepted unchanged by importer/repository. Opening statistics/adaptation after such a supported import reaches the crash. Built-in phone hold controls use BODYWEIGHT; this is an import-triggered defect.
- Fix: use the shared strength-set predicate or mapNotNull per-bout bestWorkingE1rm before checking sample count; remove the !! and align the duplicated filter. Test imported/heterogeneous set types.

## Cleanup and follow-up

- ProgressionAdvisor duplicates a less precise eligibility predicate instead of the shared E1rm helper. Centralizing that predicate fixes D5 and prevents timed/warmup/failure flag drift. InsightEngine also has local strength filters at60-62 and319-324; inspect them under the same semantics rather than asserting every weighted set is a strength sample.
- ui/gym/train/DayUiEvent.SetSessionType and corresponding intensity/untracked events have no discovered rendering caller; onSetExerciseUnit is passed into ExerciseCard but not invoked. Hydration issues behind those paths are not presented as live defects. They are candidates for removal or deliberate completion after the separate gym rendering pass.
- The custom timed watch bypass concern was rejected: ProgramRepository maps program slots to canonical library IDs; CustomExerciseRegistry has no timed capability; phone DayUiState uses the same library timed check. No supported producer was found, so it is not a finding.
- Academy text/catalog content was inspected for code/data wiring and malformed structures, not independently fact-checked as scientific/medical literature.
- No physical-device, session/background race, accessibility rendering or Compose lifecycle runtime claims are made by this phase. Parent owns the release build/tests/lint gate and other reviewers own app data and remaining UI/Wear surfaces.

## Reproduction artifacts

- [AvexDomainProbe.java](AvexDomainProbe.java), run through [run_probes.py](run_probes.py). Independently rerun against compiled app classes and project-pinned Kotlin stdlib 2.2.10; output in [validation.json](validation.json). No app source changes or additional Gradle invocation.
- Results: both reduced-load cases incorrectly100lb; timed-only weighted progression NPE; both empty and wrong-replacement swap verdicts ok; terminal not-followed label still watching.

## File coverage

All files below were inspected. Business branches were read across whole files; static catalogs/articles were read for their authored data and registration. Related call sites outside this list were traced for each finding. Files overlap a service reviewer where explicitly noted.

160 scoped source files, 22276 lines.

- app/src/main/java/com/forge/app/domain/academy/AcademyAutonomy.kt
- app/src/main/java/com/forge/app/domain/academy/AcademyContent.kt
- app/src/main/java/com/forge/app/domain/academy/AcademyEngine.kt
- app/src/main/java/com/forge/app/domain/academy/AcademyFundamentals.kt
- app/src/main/java/com/forge/app/domain/academy/AcademyProgramming.kt
- app/src/main/java/com/forge/app/domain/academy/AcademyRegistry.kt
- app/src/main/java/com/forge/app/domain/academy/AcademySignals.kt
- app/src/main/java/com/forge/app/domain/academy/AcademyYourNumbers.kt
- app/src/main/java/com/forge/app/domain/academy/Article.kt
- app/src/main/java/com/forge/app/domain/academy/ArticleRegistry.kt
- app/src/main/java/com/forge/app/domain/academy/Lesson.kt
- app/src/main/java/com/forge/app/domain/academy/LibraryHypertrophy.kt
- app/src/main/java/com/forge/app/domain/academy/LibraryNutrition.kt
- app/src/main/java/com/forge/app/domain/academy/LibraryRecovery.kt
- app/src/main/java/com/forge/app/domain/adapt/AdaptThresholds.kt
- app/src/main/java/com/forge/app/domain/adapt/AdaptationSnapshot.kt
- app/src/main/java/com/forge/app/domain/adapt/DeloadAdvisor.kt
- app/src/main/java/com/forge/app/domain/adapt/E1rm.kt
- app/src/main/java/com/forge/app/domain/adapt/EffortModel.kt
- app/src/main/java/com/forge/app/domain/adapt/InsightEngine.kt
- app/src/main/java/com/forge/app/domain/adapt/OrderingAdvisor.kt
- app/src/main/java/com/forge/app/domain/adapt/ProgressionAdvisor.kt
- app/src/main/java/com/forge/app/domain/adapt/ReadinessAdvisor.kt
- app/src/main/java/com/forge/app/domain/adapt/Recommendation.kt
- app/src/main/java/com/forge/app/domain/adapt/RecommendationArbiter.kt
- app/src/main/java/com/forge/app/domain/adapt/RestAdvisor.kt
- app/src/main/java/com/forge/app/domain/adapt/RestingHrTrend.kt
- app/src/main/java/com/forge/app/domain/adapt/SnapshotAssembler.kt
- app/src/main/java/com/forge/app/domain/adapt/VolumeResponse.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioActivity.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioCalorieEstimator.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioCompare.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioCondition.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioDetail.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioEffort.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioGlyphs.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioGuidelines.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioIcons.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioOptionalFields.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioPace.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioPaceTrend.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioRecords.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioType.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioWeekAggregate.kt
- app/src/main/java/com/forge/app/domain/cardio/CardioWeekSeries.kt
- app/src/main/java/com/forge/app/domain/cardio/CustomCardioType.kt
- app/src/main/java/com/forge/app/domain/cardio/WearableCardio.kt
- app/src/main/java/com/forge/app/domain/coach/AutoCoachPlanner.kt
- app/src/main/java/com/forge/app/domain/coach/BlockPlanner.kt
- app/src/main/java/com/forge/app/domain/coach/CoachGenBias.kt
- app/src/main/java/com/forge/app/domain/coach/CoachGoalKind.kt
- app/src/main/java/com/forge/app/domain/coach/CoachOutcome.kt
- app/src/main/java/com/forge/app/domain/coach/CoachSignal.kt
- app/src/main/java/com/forge/app/domain/coach/GoalPortfolio.kt
- app/src/main/java/com/forge/app/domain/coach/LifeEvents.kt
- app/src/main/java/com/forge/app/domain/coach/OutcomeWatcher.kt
- app/src/main/java/com/forge/app/domain/coach/PersonalProfile.kt
- app/src/main/java/com/forge/app/domain/coach/PreSessionBrief.kt
- app/src/main/java/com/forge/app/domain/coach/ProjectScanner.kt
- app/src/main/java/com/forge/app/domain/coach/SessionAdaptor.kt
- app/src/main/java/com/forge/app/domain/coach/SessionOpinion.kt
- app/src/main/java/com/forge/app/domain/coach/SuggestionCalibrator.kt
- app/src/main/java/com/forge/app/domain/coach/TodayDirective.kt
- app/src/main/java/com/forge/app/domain/coach/TrustLadder.kt
- app/src/main/java/com/forge/app/domain/coach/TrustLedger.kt
- app/src/main/java/com/forge/app/domain/coach/WeeklyReview.kt
- app/src/main/java/com/forge/app/domain/coach/WeightPhase.kt
- app/src/main/java/com/forge/app/domain/engine/AerobicBase.kt
- app/src/main/java/com/forge/app/domain/engine/ConditioningLoad.kt
- app/src/main/java/com/forge/app/domain/engine/ConditioningPlanner.kt
- app/src/main/java/com/forge/app/domain/engine/ConditioningProfile.kt
- app/src/main/java/com/forge/app/domain/engine/ZoneCoach.kt
- app/src/main/java/com/forge/app/domain/goal/CustomGoal.kt
- app/src/main/java/com/forge/app/domain/goal/GoalPins.kt
- app/src/main/java/com/forge/app/domain/health/ActiveCalorieEstimator.kt
- app/src/main/java/com/forge/app/domain/health/BodyFatSync.kt
- app/src/main/java/com/forge/app/domain/health/BodyweightSync.kt
- app/src/main/java/com/forge/app/domain/health/HcRecordKeys.kt
- app/src/main/java/com/forge/app/domain/health/LeanMassSync.kt
- app/src/main/java/com/forge/app/domain/health/MetCalories.kt
- app/src/main/java/com/forge/app/domain/health/RouteMatching.kt
- app/src/main/java/com/forge/app/domain/health/SessionHrAnalysis.kt
- app/src/main/java/com/forge/app/domain/health/StepBucketing.kt
- app/src/main/java/com/forge/app/domain/health/WatchWorkout.kt
- app/src/main/java/com/forge/app/domain/health/WearableBrand.kt
- app/src/main/java/com/forge/app/domain/measurement/BodyMeasurementType.kt
- app/src/main/java/com/forge/app/domain/mood/Mood.kt
- app/src/main/java/com/forge/app/domain/notify/Milestones.kt
- app/src/main/java/com/forge/app/domain/notify/PrMilestone.kt
- app/src/main/java/com/forge/app/domain/notify/QuietHoursSchedule.kt
- app/src/main/java/com/forge/app/domain/notify/TrainingReminder.kt
- app/src/main/java/com/forge/app/domain/parser/WeightParser.kt
- app/src/main/java/com/forge/app/domain/photo/PhotoPose.kt
- app/src/main/java/com/forge/app/domain/photo/PhotoTag.kt
- app/src/main/java/com/forge/app/domain/pr/PrDetector.kt
- app/src/main/java/com/forge/app/domain/program/ProgramGenerationIntent.kt
- app/src/main/java/com/forge/app/domain/rank/RankLadder.kt
- app/src/main/java/com/forge/app/domain/rank/StandingEngine.kt
- app/src/main/java/com/forge/app/domain/rank/XpEngine.kt
- app/src/main/java/com/forge/app/domain/schedule/TrainingRecovery.kt
- app/src/main/java/com/forge/app/domain/schedule/WeeklySchedule.kt
- app/src/main/java/com/forge/app/domain/session/SessionType.kt
- app/src/main/java/com/forge/app/domain/session/SetLogUseCase.kt
- app/src/main/java/com/forge/app/domain/trophy/TrophyEvaluator.kt
- app/src/main/java/com/forge/app/domain/trophy/TrophyExercises.kt
- app/src/main/java/com/forge/app/domain/trophy/TrophyStatsSnapshot.kt
- app/src/main/java/com/forge/app/domain/units/DecimalInput.kt
- app/src/main/java/com/forge/app/domain/units/DistanceFormatter.kt
- app/src/main/java/com/forge/app/domain/units/ElevationFormatter.kt
- app/src/main/java/com/forge/app/domain/units/HoldFormatter.kt
- app/src/main/java/com/forge/app/domain/units/LengthFormatter.kt
- app/src/main/java/com/forge/app/domain/units/WeightFormatter.kt
- app/src/main/java/com/forge/app/domain/vacation/VacationCalendar.kt
- app/src/main/java/com/forge/app/domain/volume/VolumeCalculator.kt
- app/src/main/java/com/forge/app/domain/warmup/MobilityCatalog.kt
- app/src/main/java/com/forge/app/domain/warmup/WarmupEngine.kt
- app/src/main/java/com/forge/app/domain/warmup/WarmupProtocol.kt
- app/src/main/java/com/forge/app/program/CustomExerciseRegistry.kt
- app/src/main/java/com/forge/app/program/ExerciseLibrary.kt
- app/src/main/java/com/forge/app/program/GoalProfiles.kt
- app/src/main/java/com/forge/app/program/Program.kt
- app/src/main/java/com/forge/app/program/ProgramGenerator.kt
- app/src/main/java/com/forge/app/program/SessionEstimate.kt
- app/src/main/java/com/forge/app/program/SplitTemplates.kt
- app/src/main/java/com/forge/app/program/Trophies.kt
- app/src/main/java/com/forge/app/program/Types.kt
- app/src/main/java/com/forge/app/program/VolumeModel.kt
- app/src/main/java/com/forge/app/program/VolumeTargets.kt
- app/src/main/java/com/forge/app/service/ForgeNotifications.kt
- app/src/main/java/com/forge/app/service/ReminderScheduler.kt
- app/src/main/java/com/forge/app/service/TimeChangeReceiver.kt
- app/src/main/java/com/forge/app/service/TrainingReminderWorker.kt
- app/src/main/java/com/forge/app/service/WeeklyRecapWorker.kt
- app/src/main/java/com/forge/app/service/WidgetMidnightWorker.kt
- app/src/main/java/com/forge/app/service/WorkoutSessionBridge.kt
- app/src/main/java/com/forge/app/service/WorkoutSessionService.kt
- app/src/main/java/com/forge/app/ui/coach/CoachViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/freestyle/ExerciseBrowserViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleDraft.kt
- app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleLogViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/freestyle/FreestyleTemplateViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/history/HistoryFiltering.kt
- app/src/main/java/com/forge/app/ui/gym/history/SessionHistoryViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/notes/NotesSearchViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/session/SessionDetailViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/stats/StatsViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayExerciseHandlers.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayListViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/train/DaySessionHandlers.kt
- app/src/main/java/com/forge/app/ui/gym/train/DaySwapHandlers.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayTimerHandlers.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayViewModel.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayViewModelBuilders.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayViewModelRefresh.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayWarmupBuilder.kt
- app/src/main/java/com/forge/app/ui/gym/train/DayWarmupHandlers.kt
- app/src/main/java/com/forge/app/ui/gym/train/state/DayUiState.kt
- app/src/main/java/com/forge/app/ui/programbuilder/ProgramBuilderDraft.kt
- app/src/main/java/com/forge/app/ui/programbuilder/ProgramBuilderModels.kt
- app/src/main/java/com/forge/app/ui/programbuilder/ProgramBuilderViewModel.kt
