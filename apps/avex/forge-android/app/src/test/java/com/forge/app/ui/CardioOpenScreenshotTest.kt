package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.domain.cardio.CardioType
import com.forge.app.domain.cardio.CardioWearableDay
import com.forge.app.domain.cardio.HourlySteps
import com.forge.app.domain.cardio.cardioActivityRecords
import com.forge.app.domain.cardio.cardioPaceSeries
import com.forge.app.domain.cardio.cardioWeekSeries
import com.forge.app.domain.cardio.cardioWeekStreak
import com.forge.app.domain.cardio.lastWeekToDateKm
import com.forge.app.domain.cardio.recentCardioTypes
import com.forge.app.domain.health.HrPoint
import com.forge.app.domain.health.WatchWorkout
import com.forge.app.ui.cardio.CardioHub
import com.forge.app.ui.cardio.CardioHubActions
import com.forge.app.ui.cardio.CardioViewModel
import com.forge.app.ui.cardio.CardioWeeksContent
import com.forge.app.ui.cardio.CardioWeeksState
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.cardio.components.CardioSessionDetailSheet
import com.forge.app.ui.cardio.state.CardioLens
import com.forge.app.ui.cardio.state.CardioUiState
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.OpenColors
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId

/**
 * Golden screenshots of the Cardio screens rebuilt 2026-10-21 to the "Open panel" redesign: the hub
 * on both tabs, the weeks page, a session, and the log sheet, from fake state. Pages are shot on a
 * tall frame so a whole page reads in one image, at 412dp and at Antho's 384dp, and once at 130%.
 *
 * Record or re-record:  ./gradlew :app:recordRoborazziDebug --tests '*CardioOpenScreenshotTest*'
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w412dp-h1400dp-normal-long-notround-any-420dpi-keyshidden-nonav")
class CardioOpenScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f))

    private val zone: ZoneId = ZoneId.of("America/New_York")
    private val today: LocalDate = LocalDate.of(2026, 10, 21)
    private val todayStartMs = today.atStartOfDay(zone).toInstant().toEpochMilli()

    private fun at(date: LocalDate, hour: Int, minute: Int = 0): Long =
        LocalDateTime.of(date, java.time.LocalTime.of(hour, minute)).atZone(zone).toInstant().toEpochMilli()

    private fun mi(v: Double) = v * 1.609344

    /** Twelve weeks of runs, walks and rides, with this week's three sessions on top. */
    private val entries: List<CardioEntry> by lazy {
        val list = mutableListOf<CardioEntry>()
        var id = 1L
        val monday = today.with(java.time.DayOfWeek.MONDAY)
        // This week, as drawn.
        list += CardioEntry(id++, at(monday, 6, 10), CardioType.RUN.code, 40, mi(4.2), effort = "easy", hrZone = "2", conditions = "cold")
        list += CardioEntry(id++, at(monday.plusDays(1), 12, 30), CardioType.WALK.code, 25, mi(1.4))
        list += CardioEntry(id++, at(monday.plusDays(2), 7, 0), CardioType.CYCLE.code, 30, mi(7.1))
        // Earlier weeks: runs getting a little quicker, walks and a ride between.
        val minutes = listOf(40, 55, 30, 45, 35, 60, 40, 50, 25, 45, 50)
        for (w in 1..11) {
            val m = monday.minusWeeks(w.toLong())
            val pace = 9.9 - (11 - w) * 0.04 // min per mile, falling toward now
            val runMi = listOf(3.1, 4.0, 3.5, 6.2, 3.8, 4.4, 3.2, 5.0, 3.6, 4.1, 3.9)[w - 1]
            list += CardioEntry(id++, at(m, 6, 15), CardioType.RUN.code, (runMi * pace).toInt(), mi(runMi))
            list += CardioEntry(id++, at(m.plusDays(2), 18, 0), CardioType.WALK.code, minutes[w - 1], mi(minutes[w - 1] / 18.0))
            if (w % 2 == 0) list += CardioEntry(id++, at(m.plusDays(5), 9, 0), CardioType.CYCLE.code, 50, mi(11.0))
            if (w % 3 == 0) list += CardioEntry(id++, at(m.plusDays(4), 7, 0), CardioType.RUN.code, 38, mi(4.0))
        }
        list.sortedByDescending { it.date }
    }

    private val watchWalk = WatchWorkout(
        recordId = "w1", startMs = at(today, 7, 40), endMs = at(today, 8, 12),
        exerciseType = androidx.health.connect.client.records.ExerciseSessionRecord.EXERCISE_TYPE_WALKING,
        title = null, distanceKm = mi(1.6), kcal = 140.0
    )

    private fun hubState(lens: CardioLens): CardioUiState {
        val weekStartMs = today.with(java.time.DayOfWeek.MONDAY).atStartOfDay(zone).toInstant().toEpochMilli()
        val week = entries.filter { it.date >= weekStartMs }
        return CardioUiState(
            isLoading = false,
            todayStartMs = todayStartMs,
            weekMinutes = week.sumOf { it.durationMin },
            cardioDaysThisWeek = CardioViewModel.countActiveDays(week, zone),
            weekTargetMin = 0,
            weekDays = CardioViewModel.buildWeekDays(week, zone, today),
            weekDistanceKm = week.sumOf { it.distanceKm ?: 0.0 },
            cardioRecords = cardioActivityRecords(entries),
            cardioPaceSeries = cardioPaceSeries(entries),
            lens = lens,
            entries = entries,
            stepsConnected = true,
            todayWearable = CardioWearableDay(
                hourlySteps = listOf(6 to 300, 7 to 1900, 8 to 600, 9 to 400, 10 to 900, 11 to 1200, 12 to 600, 13 to 310)
                    .map { (h, s) -> HourlySteps(h, s) }
            ),
            usualStepsByNow = 7800,
            stepsHour = 13,
            useMiles = true,
            importSuggestions = listOf(watchWalk),
            lastWeekToDateKm = lastWeekToDateKm(entries, zone, today),
            cardioStreakWeeks = cardioWeekStreak(entries, zone, today),
            recentTypes = recentCardioTypes(entries),
        )
    }

    private fun shoot(name: String, fontScale: Float = 1f, content: @Composable () -> Unit) {
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

    @Test
    fun hubWeek() = shoot("cardio-hub-week") {
        CardioHub(hubState(CardioLens.WEEK), zone, CardioHubActions(onBack = {}), showWatchInvite = false)
    }

    @Test
    fun hubProgress() = shoot("cardio-hub-progress") {
        CardioHub(hubState(CardioLens.PROGRESS), zone, CardioHubActions(onBack = {}), showWatchInvite = false)
    }

    @Test
    @Config(qualifiers = "+w384dp")
    fun hubWeekNarrow() = shoot("cardio-hub-week-384") {
        CardioHub(hubState(CardioLens.WEEK), zone, CardioHubActions(onBack = {}), showWatchInvite = false)
    }

    @Test
    @Config(qualifiers = "+w384dp")
    fun hubProgressNarrowLargeText() = shoot("cardio-hub-progress-384-fs130", fontScale = 1.3f) {
        CardioHub(hubState(CardioLens.PROGRESS), zone, CardioHubActions(onBack = {}), showWatchInvite = false)
    }

    /** A brand-new account: every reading at zero, and the watch invite in the steps slot. */
    @Test
    fun hubEmpty() = shoot("cardio-hub-empty") {
        CardioHub(
            CardioUiState(isLoading = false, todayStartMs = todayStartMs, useMiles = true),
            zone, CardioHubActions(onBack = {}), showWatchInvite = true
        )
    }

    private fun weeksState() = CardioWeeksState(
        loaded = true,
        weeks = cardioWeekSeries(entries, todayStartMs, 12, zone),
        entries = entries,
        weekTargetMin = 0,
        useMiles = true,
        todayStartMs = todayStartMs,
        zone = zone,
    )

    @Test
    fun weeks() = shoot("cardio-weeks") {
        CardioWeeksContent(weeksState(), pagesBack = 0, onPage = {}, onOpenWeek = {}, onBack = {})
    }

    @Test
    @Config(qualifiers = "+w384dp")
    fun weeksNarrow() = shoot("cardio-weeks-384") {
        CardioWeeksContent(weeksState(), pagesBack = 0, onPage = {}, onOpenWeek = {}, onBack = {})
    }

    private fun hr(): List<HrPoint> {
        val t0 = entries.last { it.type == CardioType.RUN.code && it.date >= todayStartMs - 3 * 86_400_000L }.date
        val bpm = listOf(104, 118, 128, 134, 138, 140, 141, 143, 144, 144, 143, 145, 147, 150, 152, 155, 158)
        return bpm.mapIndexed { i, b -> HrPoint(t0 + i * 150_000L, b) }
    }

    private val monRun get() = entries.first { it.type == CardioType.RUN.code && it.date >= todayStartMs - 3 * 86_400_000L }

    @Test
    fun session() = shoot("cardio-session") {
        val h = hr()
        CardioSessionDetailSheet(
            entry = monRun, allEntries = entries, useMiles = true, hr = h, hrAvgBpm = 142, hrMaxBpm = 161,
            onEdit = {}, onDelete = {}, onBack = {}, zone = zone
        )
    }

    @Test
    @Config(qualifiers = "+w384dp")
    fun sessionNarrowLargeText() = shoot("cardio-session-384-fs130", fontScale = 1.3f) {
        CardioSessionDetailSheet(
            entry = monRun, allEntries = entries, useMiles = true, hr = hr(), hrAvgBpm = 142, hrMaxBpm = 161,
            onEdit = {}, onDelete = {}, onBack = {}, zone = zone
        )
    }

    @Composable
    private fun SheetFrame(content: @Composable () -> Unit) {
        Box(Modifier.fillMaxSize().background(OpenColors.OnAccentDark), contentAlignment = Alignment.BottomCenter) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp))
                    .background(MaterialTheme.colorScheme.surface)
            ) {
                Box(Modifier.fillMaxWidth().padding(top = 12.dp), contentAlignment = Alignment.Center) {
                    Box(Modifier.width(36.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(OpenColors.Rule))
                }
                content()
            }
        }
    }

    @Test
    fun logSheet() = shoot("cardio-log") {
        SheetFrame {
            CardioLogSheet(
                onDismiss = {}, onSave = { _, _, _, _, _, _, _, _, _, _, _, _, _ -> },
                useMiles = true, lastUsedType = "run", recentTypes = listOf("run", "walk", "cycle"),
                watchSessions = listOf(watchWalk)
            )
        }
    }

    @Test
    @Config(qualifiers = "+w384dp")
    fun logSheetNarrowLargeText() = shoot("cardio-log-384-fs130", fontScale = 1.3f) {
        SheetFrame {
            CardioLogSheet(
                onDismiss = {}, onSave = { _, _, _, _, _, _, _, _, _, _, _, _, _ -> },
                useMiles = true, lastUsedType = "run", recentTypes = listOf("run", "walk", "cycle"),
                watchSessions = listOf(watchWalk)
            )
        }
    }
}
