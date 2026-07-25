package com.forge.app.domain.adapt

/**
 * Every tunable for the adaptation engine, in one place. Each advisor takes this as a
 * parameter (defaulting to `AdaptThresholds()`) so tests can tighten or loosen gates
 * without touching the rules, and a Settings page can expose them later.
 *
 * The data-count gates are the confidence guard: below them an advisor returns nothing
 * rather than guessing — sparse data must never produce a confident-wrong suggestion.
 */
data class AdaptThresholds(
    // ── Load steps (System 1: progression) ─────────────────────────────────────
    /** Smallest meaningful dumbbell increment, and the grid suggestions snap to. */
    val dumbbellStepLb: Double = 2.5,
    /** Per-set RPE at or below this counts as "room to progress" (outranks the exercise rating). */
    val rpeEasyMax: Double = 8.0,
    /** Per-set RPE at or above this counts as brutal → back off. */
    val rpeBrutalMin: Double = 9.5,
    /** Per-set RPE at or above this reads as a near-maximal bout in [EffortModel] (stall signature). */
    val rpeHighEffortMin: Double = 9.0,
    /** Multipliers applied to dumbbell suggestions for the session's intensity intent (#123). */
    val intensityLightScale: Double = 0.9,
    val intensityHardScale: Double = 1.05,

    // ── Plateau detection (System 1) ───────────────────────────────────────────
    /** An e1RM must beat the prior best by this fraction to count as improvement (noise floor). */
    val stallTolerance: Double = 0.005,
    /** Bouts with no e1RM improvement before anything fires. Below this: silent. */
    val plateauMinBouts: Int = 4,
    /** Stall length at which the suggestion escalates to a rep-range change. */
    val repShiftAfterStalledBouts: Int = 6,
    /** Stall length at which the suggestion escalates to a variation swap. */
    val swapAfterStalledBouts: Int = 8,
    /** Stall length at which plateau calls are HIGH confidence (else MEDIUM). */
    val highConfidenceStall: Int = 6,
    /** Weight reset for a high-effort stall ("drop ~10% and build back up"). */
    val resetFraction: Double = 0.10,
    /** HARD/BRUTAL (or RPE ≥ 9) bouts in the recent window for a stall to read as "high effort". */
    val highEffortCountInWindow: Int = 2,
    /** Max variation candidates carried on a swap suggestion. */
    val swapCandidateCount: Int = 3,

    // ── Rest tuning (System 2: adaptive rest) ──────────────────────────────────
    /** Qualified rest samples a role bucket needs before tuning applies. Below: canonical defaults. */
    val minRestSamples: Int = 8,
    /** Personal correction stays within ±this fraction of the canonical prescription. */
    val maxRestAdjust: Double = 0.40,
    /** Tuned durations snap to this grid (timer-friendly values). */
    val restRoundSeconds: Int = 15,
    /** Realized rests outside these bounds are mistaps / walked-away — excluded from tuning. */
    val minSaneRestSeconds: Int = 15,
    val maxSaneRestSeconds: Int = 600,
    /** Extra rest after a BRUTAL-rated previous set (pre-existing rule, now owned here). */
    val brutalRestBonusSeconds: Int = 30,

    // ── Deload (System 5: accumulated fatigue) ─────────────────────────────────
    /** The fatigue window: signals are measured over the most recent N days. */
    val deloadWindowDays: Int = 14,
    /** Finished sessions (and ≥ one full window of history) required before any deload call. */
    val deloadMinSessions: Int = 8,
    /** Additive driver score at which the suggestion fires / reads HIGH confidence. */
    val deloadScoreThreshold: Int = 5,
    val deloadHighScore: Int = 7,
    /** A deload inside the last N days mutes the advisor — recovery is already happening. */
    val deloadRecentDeloadSuppressDays: Int = 10,
    /** Effort inflation: HARD/BRUTAL share of rated bouts (min count) at flat-or-down volume. */
    val deloadEffortShare: Double = 0.5,
    val deloadMinRatedBouts: Int = 6,
    /** Intra-session rep drop-off (first set → last set) that reads as fatigue, with min bouts. */
    val deloadDropoffThreshold: Double = 0.30,
    val deloadMinDropoffBouts: Int = 6,
    /** The "prior month" baseline span (days before the fatigue window) used by the e1RM-regression
     *  and resting-HR drivers. Health data must be fetched far enough back to cover
     *  deloadWindowDays + this — [com.forge.app.data.repo.AdaptationRepository] derives its HC read
     *  range from it so the two can't silently drift apart. */
    val deloadPriorBaselineDays: Int = 28,
    /** e1RM regression: window best below this fraction of the prior month's best, on ≥ N lifts. */
    val deloadRegressionFraction: Double = 0.95,
    val deloadRegressionLifts: Int = 2,
    /** Sleep-debt driver (Health Connect): nights needed in-window, and the avg-minutes ceiling
     *  (390 = 6.5 h) at/below which short sleep reads as a recovery drag. */
    val deloadMinSleepNights: Int = 5,
    val deloadSleepDebtMinutes: Int = 390,
    /** Resting-HR driver (Health Connect): samples needed in each of window/prior, and the bpm
     *  rise over the prior-month baseline that reads as "not recovered". */
    val deloadMinRestingHrSamples: Int = 4,
    val deloadRestingHrDeltaBpm: Int = 5,
    /** Mood driver (A1): post-session moods needed in-window, and the share of them at
     *  drained/off that reads as accumulated fatigue rather than one rough day. */
    val deloadMinMoods: Int = 4,
    val deloadLowMoodShare: Double = 0.5,
    /** HRV driver (A1, Health Connect W6): RMSSD samples needed in each of window/prior, and the
     *  fractional drop below the prior-month baseline that reads as autonomic fatigue. HRV is
     *  noisy per-night, so this is a trend-vs-own-baseline read, never an absolute number. */
    val deloadMinHrvSamples: Int = 5,
    val deloadHrvDropFraction: Double = 0.12,
    /** Daily-steps driver (A1, Health Connect W6): days needed in-window, and the average daily
     *  step count above which off-gym movement is itself a meaningful recovery cost. */
    val deloadMinStepDays: Int = 7,
    val deloadHighDailySteps: Int = 14_000,
    /** "Overdue" driver: no deload week inside the last N weeks of training history. */
    val deloadNoDeloadWeeks: Int = 8,
    /** Plateau driver: ≥ N currently-stalled lifts (from System 1). */
    val deloadPlateauCount: Int = 3,

    // ── Auto-coach weekly pass (Phase 1 + 3 planner / Phase 5 watcher) ──────────
    /** Consolidation nudge: a fatigue score this many points below [deloadScoreThreshold] reads
     *  as "building", so a quiet weekly pass holds with a consolidation-week message (hold your
     *  weights, fill the ranges) instead of a generic "plan is working" line. */
    val consolidateBandPoints: Int = 2,
    /** Mesocycle phase line (Week Brief focus cue): a training block runs ~this many weeks of
     *  accumulation → intensification → peak before a deload; the cue tracks the week-in-block. */
    val mesocycleWeeks: Int = 5,
    /** Watcher: a rep-range shift whose best e1RM ends below this fraction of the pre-change best
     *  (once its window closes) is judged FAILED — the shift backfired rather than restarting it. */
    val repShiftRegressFraction: Double = 0.97,

    // ── Readiness (System 6: daily autoregulation) ─────────────────────────────
    /** Data gate: below this many sessions the scale stays neutral (silent). */
    val readinessMinSessions: Int = 3,
    /** The scale is bounded to ±this percent — autoregulation nudges, never lurches. */
    val readinessMaxPercent: Int = 5,
    /** Active (non-rest) cardio minutes in the last day past which it reads as a recovery cost. */
    val readinessCardioLoadMinutes: Int = 60,
    /** Percent shaved off readiness when [readinessCardioLoadMinutes] is exceeded in the last day. */
    val readinessCardioLoadPenalty: Int = 1,
    /** Mood window (A1): how far back a post-session mood still speaks about today's readiness. */
    val readinessMoodHours: Int = 48,
    /** Readiness at/below −this reads as "today should not be a hard session" (B2 directive). */
    val readinessRestThreshold: Int = 3,
    /** Check-in window (B1): how long this morning's answers speak for. */
    val readinessCheckinHours: Int = 20,
    /** Percent removed while a sick flag is live — illness outranks every other reading (B1). */
    val readinessSickPenalty: Int = 5,
    /** Percent removed during the return ramp after a layoff (B1). */
    val readinessComebackPenalty: Int = 3,
    /** Measured sleep (HC): minutes at/below which last night reads short, and at/above which it reads long. */
    val readinessShortSleepMinutes: Int = 360,
    val readinessGoodSleepMinutes: Int = 480,
    /** Resting-HR readiness read: prior-fortnight samples needed, and the bpm rise that speaks. */
    val readinessMinRestingHrSamples: Int = 4,
    val readinessRestingHrDeltaBpm: Int = 5,
    /** HRV drop (%) below your own fortnight baseline that shaves a readiness point (F). */
    val readinessHrvDropPercent: Int = 12,
    /** Steps yesterday past which a long day on your feet counts against today's session. */
    val readinessHighStepDay: Int = 18_000,
    /** Readiness percent for a drained/off (−) or strong (+) mood inside [readinessMoodHours]. */
    val readinessLowMoodPenalty: Int = 2,
    val readinessStrongMoodBonus: Int = 1,

    // ── Insights (System 4) ────────────────────────────────────────────────────
    /** Time-of-day rule: total sets before the pattern is worth stating. */
    val insightTimeOfDayMinSets: Int = 30,
    /** Most-improved rule: sessions on the lift, and the minimum % gain worth celebrating. */
    val insightImprovedMinSessions: Int = 4,
    val insightImprovedMinPct: Int = 5,
    /** Weekly muscle dominance: share of volume + minimum weekly sets before flagging. */
    val insightBalanceDominantShare: Double = 0.5,
    val insightBalanceMinWeekSets: Int = 10,
    /** Structural ratios (push/pull, quad/ham): window, per-side set minimum, healthy bands. */
    val insightRatioWindowDays: Int = 28,
    val insightRatioMinSetsPerSide: Int = 10,
    val insightPushPullLow: Double = 0.6,
    val insightPushPullHigh: Double = 1.6,
    val insightQuadHamLow: Double = 0.5,
    val insightQuadHamHigh: Double = 2.5,
    /** Adherence rules: of the last N bouts, how many skips / session-swaps before flagging. */
    val insightAdherenceWindow: Int = 5,
    val insightSkipCount: Int = 3,
    val insightSwapCount: Int = 3,
    /** Session-estimate calibration: sessions per day type, and the drift (min) worth surfacing. */
    val insightEstimateMinSessions: Int = 4,
    val insightEstimateDriftMinutes: Int = 15,
    /** Sweet-spot rep range: sets needed in a bucket to score it, and the e1RM edge (fraction)
     *  the best bucket must beat the runner-up by before it's worth calling a "sweet spot". */
    val insightSweetSpotMinSetsPerBucket: Int = 5,
    val insightSweetSpotEdge: Double = 0.03,
    /** Lagging lift (within a muscle): the growth % that reads as "climbing", and the ceiling
     *  at/below which the other lift reads as "stalled" — only flagged when both occur. */
    val insightLagGrowthPct: Int = 5,
    val insightLagFlatPct: Int = 0,

    // ── Insights · data-hungry (System 4, Tier 5 — gated on months of data) ─────
    /** Time-of-day performance: the hour that splits "earlier" vs "later", the min within-lift
     *  normalized bouts needed on EACH side, and the % e1RM gap worth reporting. */
    val insightTimePerfSplitHour: Int = 14,
    val insightTimePerfMinBouts: Int = 12,
    val insightTimePerfPct: Int = 4,
    /** Volume response (A1 — MEV/MRV flavour): training weeks a muscle needs, the min weeks in
     *  EACH volume tier (above/at-or-below its median weekly sets), and the per-week e1RM-gain
     *  gap (lb) between high- and low-volume weeks worth calling responsive vs near-ceiling. */
    val insightVolumeMinWeeks: Int = 8,
    val insightVolumeMinPerTier: Int = 3,
    val insightVolumeDeltaGapLb: Double = 1.0,
    /** Rest-day response (recovery curve): sessions needed with a computable prior gap, the min
     *  per spacing tier, the rest-day split, and the normalized-volume % difference worth surfacing. */
    val insightRestMinSessions: Int = 12,
    val insightRestMinPerTier: Int = 4,
    val insightRestSplitDays: Int = 2,
    val insightRestPct: Int = 8,

    // ── Ordering (System 3: fatigue-aware session order) ───────────────────────
    /** Same-muscle back-to-back pairings the day must have before a reorder is suggested. */
    val orderingMinAdjacencies: Int = 1,
    /** Pairings fixed at which the suggestion reads HIGH confidence (else MEDIUM). */
    val orderingHighConfidenceFix: Int = 2
)
