package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.Session
import com.forge.app.data.repo.AppNotice
import com.forge.app.data.repo.ExtendedGoalRepository
import com.forge.app.data.repo.GoalRepository
import com.forge.app.data.repo.NoticeAction
import com.forge.app.data.repo.NoticeGlyph
import com.forge.app.data.repo.NoticeKind
import com.forge.app.domain.goal.GoalMetric
import com.forge.app.domain.goal.GoalPeriod
import com.forge.app.domain.goal.liftPinKey
import com.forge.app.domain.goal.customPinKey
import com.forge.app.domain.units.WeightUnit
import com.forge.app.ui.checkin.CheckinContent
import com.forge.app.ui.checkin.CheckinViewModel
import com.forge.app.ui.goals.ChooseTypeStep
import com.forge.app.ui.goals.CustomTargetStep
import com.forge.app.ui.goals.EditorStep
import com.forge.app.ui.goals.GoalsPage
import com.forge.app.ui.goals.GoalsViewModel
import com.forge.app.ui.goals.LiftTargetStep
import com.forge.app.ui.notifications.NotificationsPage
import com.forge.app.ui.recap.RecapPage
import com.forge.app.ui.recap.RecapSet
import com.forge.app.ui.recap.RecapTab
import com.forge.app.ui.recap.RecapUiState
import com.forge.app.ui.recap.buildMonthRecap
import com.forge.app.ui.recap.buildYearRecap
import com.forge.app.ui.theme.ForgeTheme
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneOffset
import java.util.Locale

/**
 * Goldens for the redesigned Goals (list, Add a goal, Set the target), Recap (Month, Year),
 * Notifications feed and Check-in sheet (2026-10-21, "pg-goals", "pg-recap", "pg-notif"), on fake
 * state. Each page is shot at 412dp (the design's width), at 384dp (Antho's phone) and at 384dp with
 * a 1.3 font scale, on a tall frame so the whole page is one image.
 *
 * Record or re-record:  ./gradlew :app:recordRoborazziDebug --tests '*GoalsRecapNoticesScreenshotTest*'
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w412dp-h1300dp-xxhdpi")
class GoalsRecapNoticesScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(name: String, fontScale: Float = 1f, sheet: Boolean = false, content: @Composable () -> Unit) {
        compose.setContent {
            ForgeTheme {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                    CompositionLocalProvider(
                        LocalDensity provides Density(LocalDensity.current.density, fontScale = fontScale)
                    ) {
                        if (sheet) {
                            Column(Modifier.fillMaxSize()) {
                                Spacer(Modifier.height(40.dp))
                                Box(
                                    Modifier.fillMaxWidth()
                                        .clip(RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp))
                                        .background(MaterialTheme.colorScheme.surface)
                                        .padding(top = 28.dp)
                                ) { content() }
                            }
                        } else content()
                    }
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    // ─── Goals ──────────────────────────────────────────────────────────────

    private val goalsPage = @Composable {
        GoalsPage(
            state = goalsState(),
            pinned = listOf(liftPinKey("bench"), liftPinKey("deadlift"), customPinKey(1)),
            weightUnit = WeightUnit.LB,
            useMiles = true,
            locale = Locale.US,
            onAdd = {}, onOpen = {}, onTogglePin = {},
            zone = ZoneOffset.UTC
        )
    }

    @Test fun goalsList() = shoot("goals-list") { goalsPage() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun goalsList384() = shoot("goals-list-384") { goalsPage() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun goalsList384Large() = shoot("goals-list-384-fs13", fontScale = 1.3f) { goalsPage() }

    @Test fun goalsEmpty() = shoot("goals-list-empty") {
        GoalsPage(GoalsViewModel.UiState(loading = false, todayStartMs = TODAY_MS), emptyList(), WeightUnit.LB, true, Locale.US, {}, {}, {}, zone = ZoneOffset.UTC)
    }

    private val addGoal = @Composable { ChooseTypeStep(onPickLift = {}, onPickMetric = {}) }
    @Test fun goalsAdd() = shoot("goals-add") { addGoal() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun goalsAdd384Large() = shoot("goals-add-384-fs13", fontScale = 1.3f) { addGoal() }

    private val liftTarget = @Composable {
        LiftTargetStep(
            step = EditorStep.LiftWeight("bench", "Bench press", null),
            history = GoalsViewModel.LiftHistory(205.0 to 5, benchTopSets()),
            pinned = false,
            otherPins = 2,
            onSet = { _, _ -> }, onClear = {},
            nowMs = TODAY_MS,
            today = TODAY,
        )
    }
    @Test fun goalsTarget() = shoot("goals-target") { liftTarget() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun goalsTarget384() = shoot("goals-target-384") { liftTarget() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun goalsTarget384Large() = shoot("goals-target-384-fs13", fontScale = 1.3f) { liftTarget() }

    private val workoutsTarget = @Composable {
        CustomTargetStep(
            metric = GoalMetric.SESSIONS, goal = null,
            loadHistory = { _, _ -> emptyList() }, loadWeighIns = { emptyList() },
            pinned = false, otherPins = 3,
            onCreate = { _, _, _, _ -> }, onSave = { _, _ -> }, onDelete = {},
            today = TODAY,
            initialHistory = listOf(4.0, 5.0, 3.0, 4.0, 5.0, 4.0, 4.0, 3.0),
        )
    }
    @Test fun goalsTargetWorkouts() = shoot("goals-target-workouts") { workoutsTarget() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun goalsTargetWorkouts384Large() = shoot("goals-target-workouts-384-fs13", fontScale = 1.3f) { workoutsTarget() }

    // ─── Recap ──────────────────────────────────────────────────────────────

    private fun recap(tab: RecapTab) = @Composable {
        RecapPage(
            state = recapState(), tab = tab, onTab = {}, weightUnit = WeightUnit.LB,
            onStepMonth = {}, onStepYear = {}, onShare = {}
        )
    }
    @Test fun recapMonth() = shoot("recap-month") { recap(RecapTab.MONTH)() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun recapMonth384Large() = shoot("recap-month-384-fs13", fontScale = 1.3f) { recap(RecapTab.MONTH)() }
    @Test fun recapYear() = shoot("recap-year") { recap(RecapTab.YEAR)() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun recapYear384Large() = shoot("recap-year-384-fs13", fontScale = 1.3f) { recap(RecapTab.YEAR)() }

    // ─── Notifications ──────────────────────────────────────────────────────

    private val feed = @Composable { NotificationsPage(notices = notices(), onAct = {}, onDismiss = {}) }
    @Test fun notifications() = shoot("notifications-feed") { feed() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun notifications384Large() = shoot("notifications-feed-384-fs13", fontScale = 1.3f) { feed() }
    @Test fun notificationsEmpty() = shoot("notifications-feed-empty") { NotificationsPage(notices = emptyList(), onAct = {}, onDismiss = {}) }

    private fun checkin(state: CheckinViewModel.UiState) = @Composable {
        CheckinContent(state, {}, {}, {}, {}, {}, {}, {}, {}, {})
    }
    private val answered = CheckinViewModel.UiState(visible = true, sleepQuality = 4, soreness = 2, stress = 2, motivation = 4)
    @Test fun checkinSheet() = shoot("checkin-sheet", sheet = true) { checkin(answered)() }
    @Test @Config(qualifiers = "w384dp-h1300dp-xxhdpi") fun checkinSheet384Large() = shoot("checkin-sheet-384-fs13", fontScale = 1.3f, sheet = true) { checkin(answered)() }
    @Test fun checkinSheetSore() = shoot("checkin-sheet-sore", sheet = true) {
        checkin(CheckinViewModel.UiState(visible = true, sleepQuality = 2, soreness = 4, soreMuscles = setOf(com.forge.app.program.MuscleGroup.entries.first())))()
    }
}

private val TODAY: LocalDate = LocalDate.of(2026, 10, 21)
private val TODAY_MS = TODAY.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() + 15L * 3_600_000
private const val DAY = 86_400_000L

private fun benchTopSets(): List<GoalRepository.TopSet> = listOf(
    84 to 185.0, 77 to 185.0, 70 to 190.0, 63 to 190.0, 56 to 190.0, 49 to 195.0,
    42 to 195.0, 35 to 195.0, 28 to 195.0, 21 to 200.0, 14 to 200.0, 7 to 205.0, 2 to 205.0,
).map { (ago, w) -> GoalRepository.TopSet(TODAY_MS - ago * DAY, w) }

private fun goalsState() = GoalsViewModel.UiState(
    loading = false,
    todayStartMs = TODAY.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
    liftGoals = listOf(
        GoalRepository.GoalProgress("bench", "Bench press", 225.0, 205.0, TODAY_MS - 40 * DAY),
        GoalRepository.GoalProgress("deadlift", "Deadlift", 405.0, 365.0, TODAY_MS - 60 * DAY),
        GoalRepository.GoalProgress("squat", "Squat", 275.0, 280.0, TODAY_MS - 90 * DAY),
    ),
    customGoals = listOf(
        ExtendedGoalRepository.Progress(1, GoalMetric.SESSIONS, GoalPeriod.WEEK, 5.0, 3.0, "Train 5 days a week", 0.6f, false, TODAY_MS - 20 * DAY),
        ExtendedGoalRepository.Progress(2, GoalMetric.BODYWEIGHT, GoalPeriod.ALL, 178.0, 182.4, "", 0.55f, false, TODAY_MS - 50 * DAY, baselineValue = 188.0),
        ExtendedGoalRepository.Progress(3, GoalMetric.CARDIO_DISTANCE, GoalPeriod.WEEK, 8.0, 9.1, "Run 5 mi a week", 1f, true, TODAY_MS - 10 * DAY),
    ),
    liftChange30d = mapOf("bench" to 10.0, "deadlift" to 15.0),
    liftReachedAtMs = mapOf("squat" to TODAY_MS - 5 * DAY),
    weighIns = listOf(TODAY_MS - 32 * DAY to 183.8, TODAY_MS - 15 * DAY to 183.0, TODAY_MS - DAY to 182.4),
)

private fun session(id: Long, date: LocalDate, volume: Double, prs: Int = 0, sets: Int = 16, day: String = "push_a", type: String = "normal"): Session {
    val start = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() + 17L * 3_600_000
    return Session(
        id = id, dayKey = day, startedAt = start, finishedAt = start + 52 * 60_000L,
        totalVolumeLb = volume, prCount = prs, setCount = sets, sessionType = type, activeSeconds = 52 * 60
    )
}

private fun recapState(): RecapUiState {
    var id = 0L
    val oct = listOf(1, 3, 5, 8, 10, 12, 15, 17, 19, 20).map { d ->
        session(++id, LocalDate.of(2026, 10, d), if (d in 15..17) 9000.0 else 15000.0, prs = if (d == 19) 2 else if (d == 8) 1 else 0, sets = 18, type = if (d in 15..17) "deload" else "normal")
    }
    val sep = (1..30 step 3).map { d -> session(++id, LocalDate.of(2026, 9, d), 14000.0, prs = if (d == 10) 1 else 0) }
    val earlier = (1..8).flatMap { m ->
        (1..(14 + m % 5)).map { k -> session(++id, LocalDate.of(2026, m, ((k * 2) % 27) + 1), 13000.0, prs = if (k % 9 == 0) 1 else 0) }
    }
    val all = earlier + sep + oct
    val sets = listOf(
        RecapSet("bench", null, LocalDate.of(2026, 9, 10).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(), 195.0, 5),
        RecapSet("bench", null, LocalDate.of(2026, 10, 19).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(), 205.0, 5),
        RecapSet("deadlift", null, LocalDate.of(2026, 1, 5).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(), 315.0, 5),
        RecapSet("deadlift", null, LocalDate.of(2026, 10, 8).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(), 365.0, 5),
    )
    val names = mapOf("bench" to "Bench press", "deadlift" to "Deadlift")
    val month = buildMonthRecap(
        YearMonth.of(2026, 10), oct, sep, sets, "Barbell row" to 9, TODAY, WeightUnit.LB, Locale.US, ZoneOffset.UTC,
        liftName = { idd, _ -> names[idd] ?: idd }, dayName = { "Push A" }
    )
    val year = buildYearRecap(
        2026, all, sets, "Bench press" to 82, 23, TODAY, WeightUnit.LB, Locale.US, ZoneOffset.UTC,
        liftName = { idd, _ -> names[idd] ?: idd }
    )
    return RecapUiState(isLoading = false, month = YearMonth.of(2026, 10), year = 2026, monthUi = month, yearUi = year, firstMonth = YearMonth.of(2025, 3))
}

private fun notices() = listOf(
    AppNotice("checkin.2026-10-21", NoticeKind.CHECK_IN, NoticeGlyph.COACH, "TODAY", "How are you feeling today?",
        "Sleep, soreness, stress, drive. Shapes today's targets.", NoticeAction.OpenCheckin, dismissible = false),
    AppNotice("coach.2026-W43", NoticeKind.COACH_BRIEF, NoticeGlyph.COACH, "COACH BRIEF", "Monday's brief",
        "Holding steady this week · 1 change applied", NoticeAction.OpenCoachBrief),
    AppNotice("lessons:a,b,c,d,e,f", NoticeKind.ACADEMY, NoticeGlyph.ACADEMY, "NEW LESSONS", "6 new lessons",
        "What readiness means, How the coach decides and 4 more", NoticeAction.OpenAcademy),
    AppNotice("milestone.month", NoticeKind.MILESTONE, NoticeGlyph.MILESTONE, "MILESTONE", "A month since your first session",
        "212 workouts in, since Mar 2025"),
    AppNotice("connect.notifications", NoticeKind.SETUP, NoticeGlyph.BELL, "CONNECT", "Turn on notifications",
        "Training nudges, the weekly recap, rest timer pings.", NoticeAction.EnableNotifications),
    AppNotice("connect.wearable", NoticeKind.SETUP, NoticeGlyph.WATCH, "CONNECT", "Connect a watch or ring",
        "Steps by the hour and your route on each session.", NoticeAction.ConnectWearable),
)
