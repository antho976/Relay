package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.unit.Density
import com.forge.app.data.db.entities.CoachDecision
import com.forge.app.data.db.entities.CoachGoal
import com.forge.app.data.db.entities.CoachPass
import com.forge.app.data.db.entities.CoachProject
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.data.repo.CoachBrief
import com.forge.app.data.repo.CoachRepository
import com.forge.app.data.repo.CoachTimeline
import com.forge.app.data.repo.CoachWatch
import com.forge.app.data.repo.LearnedBias
import com.forge.app.data.repo.RecoverySignal
import com.forge.app.data.repo.TrackedLift
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.data.repo.CoachMilestone
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.CoachGoalKind
import com.forge.app.domain.coach.GoalPortfolio
import com.forge.app.domain.coach.PersonalProfile
import com.forge.app.domain.coach.TypeTrust
import com.forge.app.program.MuscleGroup
import com.forge.app.domain.coach.WeeklyReviewData
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.coach.CoachActions
import com.forge.app.ui.coach.CoachLedger
import com.forge.app.ui.coach.CoachViewModel
import com.forge.app.ui.coach.accountItemCount
import com.forge.app.ui.theme.ForgeTheme
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Golden screenshots of the Coach ledger (`design/MAP.md`).
 *
 * The old Coach page shipped with no golden at all, which is part of how it drifted into eight
 * sections nobody had looked at together. The account is the app's one non-recipe layout with its
 * own vocabulary — a spine, nodes, stamps, and exactly one filled tile — so it is pinned here:
 * at 100% and 200% font scale, on a brand-new account, and on AMOLED.
 *
 * Record or re-record:  gradle -p forge-android :app:recordRoborazziDebug
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class CoachLedgerScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    /**
     * [scrollTo] names the anchor to bring to the top of the frame, rather than an item index.
     * Index arithmetic silently shot a frame of empty ground when it was off by one; scrolling to a
     * named node fails the test instead, which is what a golden of a region below the fold owes.
     * Name the LAST block of the region, not its heading: the scroll brings a node just into view,
     * so scrolling to a heading parks it at the bottom edge with its whole section still below.
     */
    private fun shoot(
        name: String,
        amoled: Boolean = false,
        scrollTo: String? = null,
        content: @Composable () -> Unit
    ) {
        compose.setContent {
            ForgeTheme(amoledMode = amoled) {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                    content()
                }
            }
        }
        if (scrollTo != null) {
            compose.onAllNodes(hasScrollAction()).onFirst()
                .performScrollToNode(hasText(scrollTo, substring = true))
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    private fun ledger(state: CoachViewModel.UiState) = @Composable {
        CoachLedger(
            state = state,
            weightUnit = WeightUnit.LB,
            now = NOW,
            actions = CoachActions(connectHealth = {})
        )
    }

    @Test
    fun ledger() = shoot("coach-ledger") { ledger(activeState())() }

    /** §14: the whole account must survive the biggest font without clipping or lost content. */
    @Test
    fun ledgerAtLargeFontScale() {
        compose.setContent {
            ForgeTheme {
                CompositionLocalProvider(
                    LocalDensity provides Density(
                        LocalDensity.current.density,
                        fontScale = 2f
                    )
                ) { ledger(activeState())() }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/coach-ledger-200.png", options)
    }

    /** The state every first-run user sees: no calls, no history, a baseline still filling. */
    @Test
    fun ledgerOnANewAccount() = shoot("coach-ledger-baseline") { ledger(baselineState())() }

    /**
     * The account below the fold: the weeks before this one, each entry stamped with what became
     * of it. A LazyColumn golden pins one viewport, so the regions past the first get their own.
     */
    @Test
    fun ledgerRecord() = shoot("coach-ledger-record", scrollTo = "DIDN'T STICK") {
        ledger(activeState())()
    }

    /** SIGNALS: the live reading, straight after the account. */
    @Test
    fun ledgerStand() = shoot("coach-ledger-stand", scrollTo = "SIGNALS") {
        ledger(activeState())()
    }

    /** The block and what the account is climbing toward. */
    @Test
    fun ledgerBlock() = shoot("coach-ledger-block", scrollTo = "BLOCK") {
        ledger(activeState())()
    }

    @Test
    fun ledgerLearned() = shoot("coach-ledger-learned", scrollTo = "Best spacing") {
        ledger(activeState())()
    }

    /**
     * `accountItemCount` mirrors `coachAccount`'s item emission by hand — the deep link that used
     * to open the Signals lens scrolls by it — so the two are pinned together here. Scrolling by
     * that count must land on SIGNALS, the first thing after the account.
     */
    @Test
    fun theDeepLinkIndexLandsOnWhereYouStand() {
        val state = activeState()
        compose.setContent {
            ForgeTheme {
                CoachLedger(
                    state = state,
                    weightUnit = WeightUnit.LB,
                    now = NOW,
                    actions = CoachActions(),
                    listState = rememberLazyListState(
                        initialFirstVisibleItemIndex = accountItemCount(state)
                    )
                )
            }
        }
        compose.onNodeWithText("SIGNALS").assertIsDisplayed()
    }

    /** AMOLED is a shipped ground, not a variant. */
    @Test
    fun ledgerAmoled() = shoot("coach-ledger-amoled", amoled = true) { ledger(activeState())() }
}

private const val NOW = 1_755_648_000_000L // 2025-08-20T00:00:00Z, fixed so goldens are stable.
private const val DAY = 24L * 60 * 60 * 1000

/** A mid-journey account: two open calls, settled history, real readings. */
private fun activeState() = CoachViewModel.UiState(
    loading = false,
    brief = CoachBrief(
        pass = CoachPass("2025-W34", NOW, CoachRepository.STATUS_PROPOSED, null),
        decisions = listOf(
            CoachDecision(
                id = 1, weekId = "2025-W34", type = "volume_up",
                targetKey = "bench_press", targetName = "Bench Press",
                summary = "Add a set to Bench Press (3 → 4)",
                reason = "Chest is responding but is your slowest-climbing muscle right now, and " +
                    "last week hit the target with recovery fresh.",
                status = CoachRepository.STATUS_PROPOSED, payload = "4"
            ),
            CoachDecision(
                id = 2, weekId = "2025-W34", type = "swap",
                targetKey = "db_row", targetName = "DB Row",
                summary = "Rotate DB Row → Wide Lat Pulldown",
                reason = "Four sessions without a rep gain. A variation resets the stimulus " +
                    "without dropping the work.",
                status = CoachRepository.STATUS_PROPOSED, payload = "wide_lat_pulldown"
            )
        ),
        review = WeeklyReviewData(
            sessionsLastWeek = 4, sessionsTarget = 4,
            volumeLastWeekLb = 42_180.0, volumePriorWeekLb = 39_400.0,
            prsLastWeek = 2, trackedLifts = 6, stalledLifts = 1,
            fatigueScore = 4, fatigueBand = "Building",
            focusLine = "Own the top set on bench before adding load."
        ),
        sessionsLogged = 34, minSessions = 4
    ),
    watch = CoachWatch(
        sessionsLogged = 34, minSessions = 4, sessionsToGo = 0, autopilot = false,
        fatigueScore = 4, fatigueThreshold = 7,
        recoveryGateSessions = 6, historyDays = 28, recoveryWindowDays = 28,
        fatigueChecks = listOf(
            DeloadAdvisor.FatigueCheck("Hard sets", "38% at RPE 9+", 2, fired = true),
            DeloadAdvisor.FatigueCheck("Session rating", "6.8 avg", 0, fired = false),
            DeloadAdvisor.FatigueCheck("Sleep", "6.1h avg", 2, fired = true)
        ),
        trackedLifts = listOf(
            TrackedLift("Bench Press", 9, concrete = true, slotId = "bench_press"),
            TrackedLift("Back Squat", 8, concrete = true, slotId = "back_squat"),
            TrackedLift("DB Row", 7, concrete = true, stalling = true, slotId = "db_row"),
            TrackedLift("Overhead Press", 1, concrete = false, slotId = "ohp")
        ),
        recoverySignals = listOf(
            RecoverySignal("Sleep", "12 nights in the last two weeks", true),
            RecoverySignal("Resting heart rate", "14 readings", true),
            RecoverySignal("Session effort", "34 rated sessions", true),
            RecoverySignal("Rest-day flags", "2 in the last two weeks", true)
        ),
        learnedBiases = listOf(
            LearnedBias("Slower on pressing", "Weight nudges on bench land better every third week."),
            LearnedBias("Rotations stick", "You keep swapped variations, so they come sooner now.")
        )
    ),
    timeline = CoachTimeline(
        trust = listOf(
            TypeTrust("weight_nudge", 3, 3, earned = true),
            TypeTrust("swap", 1, 3, earned = false),
            TypeTrust("volume_up", 2, 3, earned = false)
        ),
        milestones = listOf(
            CoachMilestone("First call", "The coach made its first change.", true),
            CoachMilestone("Ten applied", "Reached on the week of Jul 14.", true),
            CoachMilestone("First autopilot", "Weight nudges earned it first.", true),
            CoachMilestone("Twenty-five applied", "Nine to go at your current pace.", false),
            CoachMilestone("A full block", "Run one block start to finish.", false)
        ),
        weeks = listOf(
            CoachRepository.CoachHistoryEntry(
                CoachPass("2025-W33", NOW - 7 * DAY, CoachRepository.STATUS_PROPOSED, null),
                listOf(
                    CoachDecision(
                        id = 10, weekId = "2025-W33", type = "weight_nudge",
                        targetKey = "back_squat", targetName = "Back Squat",
                        summary = "Add load to Back Squat (275 → 285)",
                        reason = "Two clean sessions at the top of the range.",
                        status = CoachRepository.STATUS_APPLIED,
                        appliedAt = NOW - 6 * DAY, outcome = "pending", undoData = "{}"
                    ),
                    CoachDecision(
                        id = 11, weekId = "2025-W33", type = "rep_shift",
                        targetKey = "db_curl", targetName = "DB Curl",
                        summary = "Shift DB Curl from 8 to 12 reps",
                        reason = "Stalled at the bottom of the range for three sessions.",
                        status = CoachRepository.STATUS_SKIPPED, payload = "12"
                    )
                )
            ),
            CoachRepository.CoachHistoryEntry(
                CoachPass("2025-W32", NOW - 14 * DAY, CoachRepository.STATUS_PROPOSED, null),
                listOf(
                    CoachDecision(
                        id = 12, weekId = "2025-W32", type = "volume_up",
                        targetKey = "lat_pulldown", targetName = "Lat Pulldown",
                        summary = "Add a set to Lat Pulldown (3 → 4)",
                        reason = "Back is your slowest-climbing muscle.",
                        status = CoachRepository.STATUS_APPLIED,
                        appliedAt = NOW - 14 * DAY, outcome = "ok", undoData = "{}"
                    )
                )
            ),
            CoachRepository.CoachHistoryEntry(
                CoachPass("2025-W31", NOW - 21 * DAY, CoachRepository.STATUS_HOLD, "Vacation mode was on."),
                emptyList()
            ),
            CoachRepository.CoachHistoryEntry(
                CoachPass("2025-W30", NOW - 28 * DAY, CoachRepository.STATUS_PROPOSED, null),
                listOf(
                    CoachDecision(
                        id = 13, weekId = "2025-W30", type = "swap",
                        targetKey = "leg_press", targetName = "Leg Press",
                        summary = "Rotate Leg Press → Hack Squat",
                        reason = "Six sessions without a rep gain.",
                        status = CoachRepository.STATUS_APPLIED,
                        appliedAt = NOW - 28 * DAY, outcome = "failed", undoData = "{}"
                    )
                )
            )
        )
    ),
    e1rmBySlot = mapOf(
        "bench_press" to listOf(212.0, 215.0, 214.0, 219.0, 221.0, 220.0, 226.0, 228.0, 231.0),
        "back_squat" to listOf(305.0, 309.0, 312.0, 311.0, 318.0, 322.0, 327.0, 330.0),
        "db_row" to listOf(148.0, 150.0, 149.0, 151.0, 150.0, 149.0, 150.0)
    ),
    goals = listOf(
        goalState("bench_press", CoachGoalKind.LIFT_1RM, 315.0, "231 of 315 lb · +2.1 lb a week", 8, true),
        goalState("back", CoachGoalKind.MUSCLE_VOLUME, 18.0, "14 of 18 sets a week", 14, false)
    ),
    block = TrainingBlock(
        id = 1, phase = BlockPhase.ACCUMULATE.code, weekIndex = 3, plannedWeeks = 6,
        intent = "push the top set every session", startedAt = NOW - 21 * DAY
    ),
    project = CoachProject(
        id = 1, kind = "muscle", name = "Bring up your rear delts",
        why = "Rear delts trail your pressing by a wide margin.",
        plan = "Two extra rear-delt sets on pull days, placed before the heavy rows.",
        finishLine = "Done when rear delts hold 14 sets a week for four weeks.",
        targetKey = "rear_delts", weeks = 4, startedAt = NOW - 7 * DAY
    ),
    profile = PersonalProfile.Profile(
        volumeCaps = mapOf(MuscleGroup.CHEST to 16, MuscleGroup.BACK to 20),
        recoveryDays = 2,
        sweetSpotReps = emptyMap(),
        strongestHour = null
    ),
    health = CoachViewModel.HealthSeries(
        sleepHours = listOf(6.9f, 6.2f, 5.8f, 7.1f, 6.4f, 5.9f, 6.6f, 6.1f, 7.3f, 6.0f),
        sleepFloorHours = 6.5f,
        restingHr = listOf(54, 55, 57, 56, 58, 59, 57, 56),
        hrWindowAvg = 57, hrBaseline = 55,
        hrvWindowAvg = 62, hrvBaseline = 68
    ),
    daysToNextBrief = 3
)

private fun goalState(
    targetKey: String,
    kind: CoachGoalKind,
    target: Double,
    reading: String,
    etaWeeks: Int,
    onTrack: Boolean
) = GoalPortfolio.GoalState(
    goal = CoachGoal(id = 1, kind = kind.code, targetKey = targetKey, targetValue = target, createdAt = NOW),
    kind = kind,
    current = null,
    target = target,
    perWeek = null,
    etaWeeks = etaWeeks,
    onTrack = onTrack,
    reachedNow = false,
    reading = reading
)

/** A brand-new account: the baseline still filling, nothing to decide, nothing to look back on. */
private fun baselineState() = CoachViewModel.UiState(
    loading = false,
    brief = CoachBrief(
        pass = CoachPass("2025-W34", NOW, CoachRepository.STATUS_HOLD, "Still learning — 2 session(s) logged."),
        decisions = emptyList(),
        review = null,
        sessionsLogged = 2,
        minSessions = 4
    ),
    watch = CoachWatch(
        sessionsLogged = 2, minSessions = 4, sessionsToGo = 2, autopilot = false,
        fatigueScore = null, fatigueThreshold = 7,
        recoveryGateSessions = 6, historyDays = 5, recoveryWindowDays = 28,
        fatigueChecks = emptyList(),
        trackedLifts = listOf(
            TrackedLift("Bench Press", 1, concrete = false, slotId = "bench_press"),
            TrackedLift("Back Squat", 1, concrete = false, slotId = "back_squat")
        ),
        recoverySignals = listOf(
            RecoverySignal("Sleep", "not connected", false),
            RecoverySignal("Resting heart rate", "not connected", false),
            RecoverySignal("Session effort", "2 rated sessions", true)
        ),
        learnedBiases = emptyList()
    ),
    timeline = CoachTimeline(trust = emptyList(), milestones = emptyList(), weeks = emptyList()),
    daysToNextBrief = 5
)
