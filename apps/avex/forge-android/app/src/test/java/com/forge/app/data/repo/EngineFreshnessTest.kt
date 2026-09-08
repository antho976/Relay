package com.forge.app.data.repo

import android.app.Application
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.core.time.TimeSignals
import com.forge.app.data.db.*
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.program.Program
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class)
class EngineFreshnessTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val db = inMemoryForgeDb()
    private var now = 1_800_000_000_000L
    private val clock = Clock { now }
    private val settings = SettingsRepository(context, clock)
    private val custom = ProgramCustomizationRepository(db.programCustomizationDao())
    private val program = ProgramRepository(db, db.programDao(), db.programCustomizationDao(), custom,
        db.exerciseCustomizationDao(), db.sessionDao(), db.coachDao(), settings,
        dagger.Lazy { error("generation is not used") }, clock, context)
    private val adaptation = AdaptationRepository(db.sessionDao(), db.loggedExerciseDao(), db.loggedSetDao(),
        db.moodDao(), db.cardioDao(), db.bodyweightDao(), db.checkinDao(), db.injuryRestrictionDao(),
        db.adviceEventDao(), db.vacationDao(), program, custom,
        CustomizationRepository(db.exerciseCustomizationDao(), db.dayNameOverrideDao()), settings,
        HealthConnectManager(context), clock)
    private val original = Program.days
    @After fun close() { Program.setActive(original); db.close() }

    @Test fun `fresh snapshot sees edits reset midnight and backward clock within former TTL`() = runBlocking {
        Program.setActive(Program.seedDays)
        val first = adaptation.snapshotCached()
        val id = db.sessionDao().insert(session(startedAt = now - 3000, finishedAt = now - 1000))
        val second = adaptation.snapshotCached()
        assertEquals(first.sessions.size + 1, second.sessions.size)
        now += 86_400_000L
        assertEquals(now, adaptation.snapshotCached().nowMs)
        now -= 172_800_000L
        assertEquals(now, adaptation.snapshotCached().nowMs)
        db.sessionDao().delete(db.sessionDao().get(id)!!)
        assertEquals(first.sessions.size, adaptation.snapshotCached().sessions.size)
    }

    @Test fun `stats ignores active journal changes but responds to same-count finished set edits`() = runBlocking {
        Program.setActive(Program.seedDays)
        val sid = db.sessionDao().insert(session())
        val eid = db.loggedExerciseDao().insert(loggedExercise(sessionId = sid))
        val setId = db.loggedSetDao().insert(loggedSet(loggedExerciseId = eid))
        val activeId = db.sessionDao().insert(session(finishedAt = null))
        val signals = EngineInputSignals(db, settings, program, TimeSignals(clock))
        val events = Channel<Unit>(Channel.UNLIMITED)
        val job = launch { signals.changes().collect { events.send(it) } }
        try {
            withTimeout(5000) { events.receive() }
            db.sessionDao().setJournal(activeId, "writing during workout")
            assertNull(withTimeoutOrNull(300) { events.receive() })
            db.loggedSetDao().setRpe(setId, 8.5)
            withTimeout(5000) { events.receive() }
        } finally { job.cancelAndJoin() }
    }
    @Test fun `retained gym stats age out rolling sets and refresh customized targets`() = runBlocking {
        Program.setActive(Program.seedDays)
        val day = Program.days.first()
        val exercise = day.exercises.first()
        val sid = db.sessionDao().insert(session(startedAt = now - 7 * 86_400_000L + 3_600_000L,
            finishedAt = now - 7 * 86_400_000L + 7_200_000L, dayKey = day.key))
        val eid = db.loggedExerciseDao().insert(loggedExercise(sessionId = sid, exerciseId = exercise.id))
        db.loggedSetDao().insert(loggedSet(loggedExerciseId = eid, weightLb = 50.0, reps = 10))
        val time = TimeSignals(clock)
        val stats = StatsRepository(db.sessionDao(), db.cardioDao(), db.loggedExerciseDao(), db.loggedSetDao(),
            db.vacationDao(), BodyweightRepository(db.bodyweightDao(), clock, HealthConnectManager(context), settings),
            settings, time, clock, program, custom)
        val events = Channel<StatsRepository.GymStats>(Channel.UNLIMITED)
        val job = launch { stats.observeGymStats().collect { events.send(it) } }
        try {
            val before = withTimeout(5000) { events.receive() }
            assertTrue(before.weeklySetsByMuscle.sumOf { it.sets } > 0)
            val target = before.plannedSetsByMuscle.getValue(exercise.muscle)
            custom.setSetsOverride(day.key, exercise.id, exercise.sets + 2)
            withTimeout(5000) {
                while (events.receive().plannedSetsByMuscle[exercise.muscle] != target + 2) { }
            }
            now += 86_400_000L
            time.onSystemTimeChanged()
            withTimeout(5000) {
                while (events.receive().weeklySetsByMuscle.sumOf { it.sets } != 0) { }
            }
        } finally { job.cancelAndJoin() }
    }

    @Test fun `weekly count volume and dots all attribute cross-midnight training to its start`() = runBlocking {
        val oldZone = java.util.TimeZone.getDefault()
        try {
            for (zoneName in listOf("UTC", "America/Toronto")) {
                java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone(zoneName))
                val zone = java.time.ZoneId.of(zoneName)
                val monday = java.time.LocalDate.of(2026, 3, 9).atStartOfDay(zone)
                now = monday.plusHours(1).toInstant().toEpochMilli()
                val sid = db.sessionDao().insert(session(
                    startedAt = monday.minusMinutes(10).toInstant().toEpochMilli(),
                    finishedAt = monday.plusMinutes(10).toInstant().toEpochMilli()
                ).copy(totalVolumeLb = 500.0))
                val stats = StatsRepository(db.sessionDao(), db.cardioDao(), db.loggedExerciseDao(), db.loggedSetDao(),
                    db.vacationDao(), BodyweightRepository(db.bodyweightDao(), clock, HealthConnectManager(context), settings),
                    settings, TimeSignals(clock), clock, program, custom).observeWeeklyStats().first()
                assertEquals(0, stats.workouts)
                assertEquals(0.0, stats.volumeLb, 0.0)
                assertTrue(stats.weekDaysTrained.isEmpty())
                db.sessionDao().delete(db.sessionDao().get(sid)!!)
            }
        } finally { java.util.TimeZone.setDefault(oldZone) }
    }

}
