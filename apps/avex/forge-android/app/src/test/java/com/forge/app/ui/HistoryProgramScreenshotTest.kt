package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Density
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.data.db.entities.Session
import com.forge.app.domain.health.HrPoint
import com.forge.app.domain.health.SessionHrView
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.history.HistoryActions
import com.forge.app.ui.gym.history.HistoryItem
import com.forge.app.ui.gym.history.HistoryRowDetail
import com.forge.app.ui.gym.history.SessionHistoryPage
import com.forge.app.ui.gym.history.SessionHistoryUiState
import com.forge.app.ui.gym.history.historyMeta
import com.forge.app.ui.gym.history.groupByMonth
import com.forge.app.ui.gym.session.SessionDetailPage
import com.forge.app.ui.gym.session.state.ExerciseDetail
import com.forge.app.ui.gym.session.state.SessionDetailData
import com.forge.app.ui.gym.session.state.SessionDetailUiState
import com.forge.app.ui.gym.session.state.SessionMetric
import com.forge.app.ui.gym.session.state.SetDetail
import com.forge.app.ui.gym.stats.state.MuscleSetCount
import com.forge.app.ui.programbuilder.BuilderDay
import com.forge.app.ui.programbuilder.BuilderExercise
import com.forge.app.ui.programbuilder.DayEditorPage
import com.forge.app.ui.programbuilder.PlannedLoad
import com.forge.app.ui.programbuilder.ProgramPage
import com.forge.app.ui.programbuilder.ProgramPageActions
import com.forge.app.ui.programbuilder.ProgramPageModel
import com.forge.app.ui.theme.ForgeTheme
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.time.LocalDate
import java.time.ZoneId

/**
 * Goldens for the 2026-10-21 redesign of History, Session detail (on each metric tab) and Your
 * program (view, editing, Edit day), drawn from fake state. Each page is shot tall enough to show
 * it whole, at 412dp (the design's width) and at 384dp (Antho's phone), and at 130% font scale.
 *
 * Record or re-record:  ./gradlew :app:recordRoborazziDebug --tests '*HistoryProgramScreenshotTest*'
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w412dp-h1500dp-xxhdpi")
class HistoryProgramScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(name: String, narrow: Boolean = false, fontScale: Float = 1f, content: @Composable () -> Unit) {
        if (narrow) RuntimeEnvironment.setQualifiers("w384dp-h1500dp-xxhdpi")
        compose.setContent {
            ForgeTheme {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                    CompositionLocalProvider(
                        LocalDensity provides Density(LocalDensity.current.density, fontScale = fontScale)
                    ) { content() }
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    // ── History ─────────────────────────────────────────────────────────────────

    @Composable
    private fun historyPage() = SessionHistoryPage(historyState(), "", HistoryActions(), detail = { HISTORY_DETAILS[it] })

    @Test fun history() = shoot("history") { historyPage() }
    @Test fun history384() = shoot("history-384", narrow = true) { historyPage() }
    @Test fun history384Large() = shoot("history-384-130", narrow = true, fontScale = 1.3f) { historyPage() }
    @Test fun historySearch() = shoot("history-search") {
        SessionHistoryPage(historyState(), "bench", HistoryActions(), detail = { HISTORY_DETAILS[it] })
    }
    @Test fun historyEmpty() = shoot("history-empty") {
        SessionHistoryPage(SessionHistoryUiState(loaded = true, meta = historyMeta(emptyList())), "", HistoryActions(), detail = { null })
    }

    // ── Session detail ──────────────────────────────────────────────────────────

    @Composable
    private fun detail(metric: SessionMetric, hr: Boolean = true) = SessionDetailPage(
        state = SessionDetailUiState(isLoading = false, data = sessionData(), hrView = if (hr) hrView() else null, hrChecked = true),
        onBack = {}, onExport = {}, onOpenLiftTrend = {}, initialMetric = metric
    )

    @Test fun detailWeight() = shoot("session-detail-weight") { detail(SessionMetric.WEIGHT) }
    @Test fun detailVolume() = shoot("session-detail-volume") { detail(SessionMetric.VOLUME) }
    @Test fun detailReps() = shoot("session-detail-reps") { detail(SessionMetric.REPS) }
    @Test fun detailEffort() = shoot("session-detail-effort") { detail(SessionMetric.RPE) }
    @Test fun detail384() = shoot("session-detail-384", narrow = true) { detail(SessionMetric.WEIGHT) }
    @Test fun detail384Large() = shoot("session-detail-384-130", narrow = true, fontScale = 1.3f) { detail(SessionMetric.VOLUME) }
    @Test fun detailNoWatch() = shoot("session-detail-no-watch") { detail(SessionMetric.WEIGHT, hr = false) }

    /** The metric tabs switch what every lift reads. */
    @Test
    fun theTabsSwitch() {
        compose.setContent { ForgeTheme { detail(SessionMetric.WEIGHT) } }
        compose.onNodeWithText("TOP WEIGHT PER EXERCISE").assertIsDisplayed()
        compose.onNodeWithText("VOLUME").performClick()
        compose.onNodeWithText("VOLUME PER EXERCISE · LB").assertIsDisplayed()
        compose.onNodeWithText("EFFORT").performClick()
        compose.onNodeWithText("EFFORT PER EXERCISE · AVG RPE").assertIsDisplayed()
    }

    // ── Program ─────────────────────────────────────────────────────────────────

    @Composable
    private fun program(editing: Boolean) = ProgramPage(
        ProgramPageModel(
            days = PLAN, selected = 0, editing = editing, canEdit = !editing, loaded = true,
            meta = "Push · Legs · Pull · week 3 of 8, Accumulate",
            coachChanges = mapOf("push-a|incline-db-press" to "↑ 1 rep"),
            loads = LOADS, canStart = !editing, today = LocalDate.of(2026, 10, 19)
        ),
        ProgramPageActions()
    )

    @Test fun programView() = shoot("program") { program(editing = false) }
    @Test fun programEdit() = shoot("program-edit") { program(editing = true) }
    @Test fun program384() = shoot("program-384", narrow = true) { program(editing = false) }
    @Test fun programEdit384Large() = shoot("program-edit-384-130", narrow = true, fontScale = 1.3f) { program(editing = true) }

    @Composable
    private fun editDay() = DayEditorPage(
        day = PLAN[0], weekdayPlan = true, onBack = {}, onRename = {}, onSetType = {}, onSetAccent = {},
        onToggleWeekday = {}, onClearWeekdays = {}, onOpenExercise = {}, onRemoveExercise = {},
        onMoveExercise = { _, _ -> }, onAddExercise = {}, onDuplicateDay = {}, onRemoveDay = {}
    )

    @Test fun programDay() = shoot("program-day") { editDay() }
    @Test fun programDay384Large() = shoot("program-day-384-130", narrow = true, fontScale = 1.3f) { editDay() }
}

// ── Fake data ───────────────────────────────────────────────────────────────────

private val ZONE: ZoneId = ZoneId.systemDefault()
private fun at(y: Int, m: Int, d: Int, h: Int = 18) = LocalDate.of(y, m, d).atTime(h, 0).atZone(ZONE).toInstant().toEpochMilli()
private const val MIN = 60_000L

private fun workout(id: Long, day: String, start: Long, minutes: Int, prs: Int = 0, tags: String = "") = Session(
    id = id, dayKey = day, startedAt = start, finishedAt = start + minutes * MIN, totalVolumeLb = 12_000.0,
    prCount = prs, setCount = 18, activeSeconds = minutes * 60, tags = tags
)

private val HISTORY_ITEMS: List<HistoryItem> = listOf(
    HistoryItem.Workout(workout(1, "legs-a", at(2026, 10, 20), 52)),
    HistoryItem.Workout(workout(2, "push-a", at(2026, 10, 19), 48, prs = 1)),
    HistoryItem.Workout(workout(3, "pull-a", at(2026, 10, 17), 55)),
    HistoryItem.Cardio(CardioEntry(id = 9, date = at(2026, 10, 16, 8), type = "run", durationMin = 40, distanceKm = 6.76)),
    HistoryItem.Workout(workout(4, "legs-b", at(2026, 10, 16), 50, tags = "heavy")),
    HistoryItem.Workout(workout(5, "push-b", at(2026, 9, 30), 51)),
    HistoryItem.Workout(workout(6, "pull-b", at(2026, 9, 29), 54, prs = 1)),
)

private val HISTORY_DETAILS = mapOf(
    1L to HistoryRowDetail("Squat 275×5", listOf(6f, 3f, 0f, 4f, -2f)),
    2L to HistoryRowDetail("Bench press 195×5", listOf(7f, 3f, 0.6f, 4f, -1f)),
    3L to HistoryRowDetail("Deadlift 315×5", listOf(0f, 6f, 3f, 2f, -1f)),
    4L to HistoryRowDetail("Squat 265×6", listOf(6f, 2f, -1f, 4f, 0f)),
    5L to HistoryRowDetail("Overhead press 115×6", listOf(5f, 3f, 2f, 1f, 1.5f)),
    6L to HistoryRowDetail("Row 160×8", listOf(null, 5f, 4f, 3f, -1f)),
)

private fun historyState() = SessionHistoryUiState(
    months = groupByMonth(HISTORY_ITEMS, LocalDate.of(2026, 10, 21)),
    meta = historyMeta(HISTORY_ITEMS),
    availableTags = listOf("heavy", "focus"),
    loaded = true
)

private fun set(n: Int, lb: Double, reps: Int, rpe: Double?, top: Boolean = false) = SetDetail(
    number = n, weightLb = lb, weightText = lb.toInt().toString(), reps = reps, rpe = rpe, isTopSet = top,
    isAmrap = false, toFailure = false, isAssisted = false, dropAnnotation = null, setType = null
)

private fun lift(name: String, id: String, sets: List<SetDetail>, e1rm: Double? = null, prevTop: Double? = null, pr: Boolean = false) = ExerciseDetail(
    name = name, isPr = pr, effort = null, hitFullTarget = true, note = null,
    topWeightLb = sets.maxOf { it.weightLb ?: 0.0 }, totalReps = sets.sumOf { it.reps },
    volumeLb = sets.sumOf { it.volumeLb }, e1rmLb = e1rm, e1rmIsBest = false, sets = sets,
    exerciseId = id, prevTopWeightLb = prevTop
)

private fun sessionData() = SessionDetailData(
    sessionId = 2, title = "Push A", dateMs = at(2026, 10, 19), tag = "focus", durationMin = 48,
    volumeLb = 19_800.0, prCount = 1, setCount = 18, prevVolumeLb = 18_900.0, prevSetCount = 17,
    intensity = "heavy", sessionType = "normal", deload = false,
    journal = "Shoulder felt fine on bench today. Pause reps on the last set.",
    avgRpe = 7.5,
    muscleSplit = listOf(MuscleSetCount(MuscleGroup.CHEST, 8), MuscleSetCount(MuscleGroup.SHOULDERS, 4), MuscleSetCount(MuscleGroup.TRICEPS, 6)),
    exercises = listOf(
        lift("Bench press", "bench-press", listOf(set(1, 195.0, 5, 7.5, true), set(2, 195.0, 5, 8.0), set(3, 195.0, 5, 8.5), set(4, 185.0, 7, 8.5)), e1rm = 228.0, prevTop = 190.0, pr = true),
        lift("Overhead press", "ohp", listOf(set(1, 115.0, 6, 8.0), set(2, 115.0, 6, 8.5), set(3, 115.0, 5, 9.0))),
        lift("Incline dumbbell press", "incline-db-press", listOf(set(1, 60.0, 10, 7.0), set(2, 60.0, 10, 7.5), set(3, 60.0, 9, 8.0))),
        lift("Dips", "dips", listOf(set(1, 25.0, 8, 8.0), set(2, 25.0, 8, 8.0), set(3, 25.0, 7, 8.0))),
        lift("Triceps pushdown", "triceps-pushdown", listOf(set(1, 50.0, 12, 7.0), set(2, 50.0, 12, 7.0))),
    )
)

private fun hrView(): SessionHrView {
    val t0 = at(2026, 10, 19)
    val bpm = listOf(92, 110, 128, 116, 104, 122, 140, 126, 110, 130, 152, 128, 106, 118, 132, 120, 108, 116, 126, 118, 104)
    return SessionHrView(
        points = bpm.mapIndexed { i, b -> HrPoint(t0 + i * 48L * MIN / (bpm.size - 1), b) },
        avgBpm = 118, maxBpm = 152, setMarkersMs = emptyList(), exerciseBoundariesMs = emptyList(),
        perExercise = listOf(SessionHrView.ExerciseHr("Bench press", 131), SessionHrView.ExerciseHr("Overhead press", 124)),
        avgHrr60 = 22
    )
}

private fun ex(uid: String, lib: String, name: String, muscle: String, sets: Int, reps: String) =
    BuilderExercise(uid, lib, name, muscle, sets, reps)

private val PLAN = listOf(
    BuilderDay(
        "d1", "push-a", "Push A", "push", "#E85D4A",
        listOf(
            ex("e1", "bench-press", "Bench press", "Chest", 4, "5"),
            ex("e2", "ohp", "Overhead press", "Shoulders", 3, "6"),
            ex("e3", "incline-db-press", "Incline dumbbell press", "Upper chest", 3, "8-10"),
            ex("e4", "dips", "Dips", "Chest, triceps", 3, "8"),
            ex("e5", "triceps-pushdown", "Triceps pushdown", "Triceps", 3, "12"),
        ),
        weekdays = setOf(0)
    ),
    BuilderDay("d2", "legs-a", "Legs A", "legs", "#D4A017", List(19) { ex("l$it", "squat", "Squat", "Quads", 1, "5") }, weekdays = setOf(1)),
    BuilderDay("d3", "pull-b", "Pull B", "pull", "#5B9279", List(18) { ex("p$it", "row", "Row", "Back", 1, "8") }, weekdays = setOf(2)),
    BuilderDay("d4", "push-b", "Push B", "push", "#7B6CB5", List(17) { ex("q$it", "ohp", "Press", "Shoulders", 1, "6") }, weekdays = setOf(4)),
    BuilderDay("d5", "legs-b", "Legs B", "legs", "#4A90D9", List(20) { ex("r$it", "dl", "Deadlift", "Back", 1, "5") }, weekdays = setOf(5)),
)

private val LOADS = mapOf(
    "bench-press" to PlannedLoad(195.0, false),
    "ohp" to PlannedLoad(115.0, false),
    "incline-db-press" to PlannedLoad(60.0, false),
    "dips" to PlannedLoad(25.0, true),
    "triceps-pushdown" to PlannedLoad(50.0, false),
)
