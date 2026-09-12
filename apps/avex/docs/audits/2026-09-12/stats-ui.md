# Avex Stats and Overview source audit

Read-only bounded follow-up at HEAD `9637809`, 2026-09-12. No Gradle, production edits, screenshots, device input, or speculative layout claims. Inspected all Kotlin rendering/state/ViewModel files under `ui/gym/stats/` and `ui/overview/`, plus the data producers and click modifier needed to verify findings. The generated anatomy file was checked for constants, muscle mapping and consumers; individual SVG coordinates were not visually validated.

Loaded local Impeccable context/native audit guidance and Forge design router. This is source-level correctness/accessibility review, not a visual conformance score: no numerical accessibility/performance/theme/adaptivity score or native-rendering pass is claimed without runtime evidence. Positive patterns include theme-role colors, lifecycle-aware collection, asynchronous cached anatomy parsing, bounded recent lists, user-unit conversion, and stale day-detail jobs canceled on selection/dismissal.

## Confirmed actionable findings

### S1. P2: “This week” muscle bars include last week's workouts

- Primary location: `app/src/main/java/com/forge/app/ui/gym/stats/StatsContent.kt:257-258`.
- Producer: `app/src/main/java/com/forge/app/data/repo/StatsRepository.kt:484-488,502-503`.
- Other affected rendering: `StatsOverview.kt:81-90`; target-completion treatment in `StatsVolume.kt:66,79-97`.
- Trigger: train on Sunday, open Stats on Monday before training again.
- Evidence: the displayed section is “Sets per muscle this week”; its caption says the tracks are weekly targets and filling them means on plan. The producer deliberately uses `clock.nowMs() - WEEK_MS`, a rolling seven-day window, rather than Monday start. Sunday still fills Monday's bars and heatmap. Meanwhile the hero's period comparison uses the calendar week and correctly displays zero new-week workouts/volume.
- Impact: the same screen reports two different periods as “this week”, and shows progress against the new weekly plan from previous-week work.
- Fix: use the same calendar-week boundary for current-week muscle progress/hero heatmap, or explicitly present a rolling seven-day metric with a matching comparison basis. The calendar-week anchor is the consistent choice with the existing UI and plan targets.
- Validation: direct source trace and deterministic date arithmetic for Monday 2026-09-14 with Sunday 2026-09-13. No Android rendering required to establish the mismatched inclusion rule.

### S2. P2: Untracked recent sessions receive invalid progression comparisons and BEST badges

- Primary location: `app/src/main/java/com/forge/app/ui/overview/OverviewUiStateMapper.kt:48-57`.
- Producer: `data/repo/StatsRepository.kt:144-147` intentionally includes untracked recent-history rows; `data/db/dao/SessionDao.kt:376-381` computes `DayVolumeStats` only over tracked sessions.
- Rendering: `ui/overview/OverviewScreen.kt:180-190,201-209`.
- Trigger A: same day key has tracked sessions of 10,000 and 20,000 lb total volume plus a recent untracked session of 12,000 lb.
- Evidence: the mapper assumes the recent row is included in the tracked aggregate, subtracts its 12,000 from the tracked sum 30,000, divides by `(2-1)`, and compares to 18,000. Home renders `-33% avg`. A comparison to the actual tracked average 15,000 would be `-20%`; an explicitly excluded session should not receive a progression comparison at all.
- Trigger B: a recent untracked 25,000-lb session against those same tracked rows is labeled `BEST`, despite its exclusion from record statistics.
- Impact: users deliberately exclude a session but still see record badges and incorrect comparison arithmetic in Home history. The inclusive history list itself is appropriate; the comparison needs a separate eligibility gate.
- Fix: retain the row in recent history, but leave `vsAvgPct=null` and `isBest=false` when `session.isUntracked`. If product intent is to compare excluded sessions anyway, use the tracked average directly without subtracting the excluded row, and do not label it a tracked record.
- Validation: production source trace with an independent arithmetic check (`-33` versus `-20`). No actual Kotlin mapper runtime probe. This is an uncovered consumer beyond the prior H5 tracked-only aggregate fixes, not a repetition of the fixed SQL.

### S3. P2: Calendar training-day actions have no accessible date or count

- Primary location: `app/src/main/java/com/forge/app/ui/gym/stats/components/CalendarHeatmap.kt:126-134`.
- Supporting location: `app/src/main/java/com/forge/app/ui/common/BounceClick.kt:25-54`.
- Trigger: use TalkBack or switch-access navigation in Stats → Days with any lit training days.
- Evidence: each lit cell is a bare `Box` with color and `bounceClick`; there is no Text, content description, state description or labeled click action identifying the date or set count. `bounceClick` delegates to unlabeled `Modifier.clickable` and cannot supply this missing information. The only available text describes the whole page's date range and total days.
- Impact: the individual day actions are indistinguishable to a screen-reader user, so they cannot select the intended day to open its log using the information exposed by the app.
- Fix: give each interactive cell a localized full-date/set-count description, a button role, and an “Open day log” action label. Consider a text/list alternative if dense chart navigation is burdensome. This finding concerns missing semantics, not an untested touch-target/layout assertion.
- Validation: semantics source trace. No TalkBack/device session was run.

## Smaller confirmed edge cases and explicit limits

### S4. P3: Calendar's current date is frozen for its entire composition

`CalendarHeatmap.kt:60` uses `remember { LocalDate.now() }`. Its range, week columns and future-date coloring continue to use that date even when `StatsRepository` re-emits on `timeSignals.dayStarts()` (`StatsRepository.kt:479`). Keeping the screen composed through midnight/device-date change leaves the page ending on yesterday; a new week's grid remains the old week. Pass the current date from the reactive time state or key it to the same day-boundary signal. Source-confirmed; no midnight device test. The adjacent training-day summary also computes a rolling 182-day count while the calendar draws 26 calendar-week columns, so counts can include several dates outside its grid.

### S5. P3: Several statistical charts expose no series values to accessibility

`components/Sparkline.kt:63`, `components/ScatterChart.kt:35`, and `StatsInterest.kt:133` draw series in Canvas without chart semantics. The RPE histogram at `StatsInterest.kt:45-55` exposes RPE labels but never bucket counts. Some neighboring min/max/current/average text is available, so this is less severe than the unlabeled interactive calendar; detailed trends/distributions themselves remain inaccessible. Add a concise meaningful chart summary and an accessible values/table path, then verify TalkBack. `BodyHeatmap` is similarly visual, but muscle totals have a textual alternative in the Volume lens, so do not double-count it as another issue.

## Rejected or optional cleanup

- **Do not report equal-volume top-lift caching as a reachable user bug.** `OverviewViewModel.kt:404-412` uses `(volume,unit)` as the cache signature. A same-volume weight/reps edit would stale it, but the domain reviewer confirmed the current app has no finished-history set editor. Keep this as a future invalidation note if such editing is introduced. The cache comment's immutability assumption is adequate for the presently reachable surface.
- The Overview still carries older coach state, callback parameters and helper surfaces that the current screen does not render. Likewise StatsUiState carries data for removed panels. Candidate cleanup needs a full reference check including previews/tests before deleting, and should not become release-blocking refactoring.
- Strength tier thresholds are generic across lifts, but the code/documented intent is a coarse heuristic. No sports-science judgment or numerical correctness finding is asserted without reviewing that intended model and authoritative references.
- No clipping, large-font, tablet, contrast, gesture-performance, or animation-runtime defect is asserted from fixed dimensions alone.

## File coverage

30 files, 4992 lines. All executable rendering/state/ViewModel code inspected. BodyAnatomy generated SVG data had metadata/mapping review only.

- `app/src/main/java/com/forge/app/ui/gym/stats/StatsAdherence.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsBody.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsCommon.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsContent.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsInterest.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsOverview.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsReadiness.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsStrength.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsViewModel.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/StatsVolume.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/Bars.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/BodyAnatomy.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/BodyHeatmap.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/CalendarHeatmap.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/LineChart.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/MuscleFigure.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/ScatterChart.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/components/Sparkline.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/state/StatsChartData.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/state/StatsEngineUi.kt`
- `app/src/main/java/com/forge/app/ui/gym/stats/state/StatsUiState.kt`
- `app/src/main/java/com/forge/app/ui/overview/HistoryComponents.kt`
- `app/src/main/java/com/forge/app/ui/overview/OverviewScreen.kt`
- `app/src/main/java/com/forge/app/ui/overview/OverviewUiStateMapper.kt`
- `app/src/main/java/com/forge/app/ui/overview/OverviewViewModel.kt`
- `app/src/main/java/com/forge/app/ui/overview/SummarySheet.kt`
- `app/src/main/java/com/forge/app/ui/overview/components/HomeSurfaceCards.kt`
- `app/src/main/java/com/forge/app/ui/overview/components/NavTile.kt`
- `app/src/main/java/com/forge/app/ui/overview/components/OverviewComponents.kt`
- `app/src/main/java/com/forge/app/ui/overview/state/OverviewUiState.kt`
