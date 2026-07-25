package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CheckinDao
import com.forge.app.data.db.dao.InjuryRestrictionDao
import com.forge.app.data.db.entities.CheckinEntry
import com.forge.app.data.db.entities.InjuryRestriction
import com.forge.app.program.MuscleGroup
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The morning check-in and injury restrictions (Coach v3 B1).
 *
 * Owns two rules the UI must not re-implement:
 *  - **one row per calendar day**, keyed ISO (`yyyy-MM-dd`), upserted so answering twice corrects;
 *  - **adaptive prompting** — someone who dismisses the sheet every morning is telling you
 *    something, and the coach that keeps asking anyway is the coach they turn off.
 */
@Singleton
class CheckinRepository @Inject constructor(
    private val checkinDao: CheckinDao,
    private val injuryDao: InjuryRestrictionDao,
    private val clock: Clock
) {

    private val dateFmt = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    fun todayKey(zone: ZoneId = ZoneId.systemDefault()): String =
        Instant.ofEpochMilli(clock.nowMs()).atZone(zone).toLocalDate().format(dateFmt)

    suspend fun today(): CheckinEntry? = checkinDao.forDate(todayKey())

    /** Recent check-ins for the engine's windows (readiness reads days, not history). */
    suspend fun recentForEngine(windowDays: Int = ENGINE_WINDOW_DAYS): List<CheckinEntry> =
        checkinDao.since(clock.nowMs() - windowDays * DAY_MS)

    /**
     * Save today's answers. Any subset may be null — a partial check-in beats an abandoned one.
     */
    suspend fun save(
        sleepQuality: Int? = null,
        soreness: Int? = null,
        stress: Int? = null,
        motivation: Int? = null,
        sick: Boolean = false,
        soreMuscles: Set<MuscleGroup> = emptySet()
    ) {
        checkinDao.upsert(
            CheckinEntry(
                id = today()?.id ?: 0,
                dateKey = todayKey(),
                sleepQuality = sleepQuality,
                soreness = soreness,
                stress = stress,
                motivation = motivation,
                sick = sick,
                soreMuscles = soreMuscles.joinToString(",") { it.code },
                skipped = false,
                recordedAt = clock.nowMs()
            )
        )
    }

    /** Record a dismissal so [shouldPrompt] can learn to stop asking. */
    suspend fun skipToday() {
        if (today()?.hasAnswers == true) return
        checkinDao.upsert(
            CheckinEntry(
                id = today()?.id ?: 0,
                dateKey = todayKey(),
                skipped = true,
                recordedAt = clock.nowMs()
            )
        )
    }

    /**
     * Whether to show the sheet at today's first app-open.
     *
     * False once today is answered or dismissed, and false after [SKIPS_BEFORE_BACKING_OFF]
     * consecutive dismissals — at which point the check-in lives on as a manual action instead of
     * a daily interruption. One answered day resets it: the user re-opted in by answering.
     */
    suspend fun shouldPrompt(): Boolean {
        val today = today()
        if (today != null) return false
        val recent = checkinDao.recent(SKIPS_BEFORE_BACKING_OFF)
        if (recent.size < SKIPS_BEFORE_BACKING_OFF) return true
        return !recent.all { it.skipped }
    }

    // ── Injury restrictions ────────────────────────────────────────────────────

    suspend fun activeRestrictions(): List<InjuryRestriction> = injuryDao.active()

    fun observeActiveRestrictions() = injuryDao.observeActive()

    suspend fun restrictMuscle(muscle: MuscleGroup, note: String = ""): Long =
        injuryDao.insert(
            InjuryRestriction(
                scope = InjuryRestriction.SCOPE_MUSCLE,
                targetKey = muscle.code,
                note = note,
                startedAt = clock.nowMs()
            )
        )

    suspend fun restrictExercise(exerciseId: String, note: String = ""): Long =
        injuryDao.insert(
            InjuryRestriction(
                scope = InjuryRestriction.SCOPE_EXERCISE,
                targetKey = exerciseId,
                note = note,
                startedAt = clock.nowMs()
            )
        )

    /** Cleared, not deleted: "I hurt my shoulder in July" explains July's numbers forever. */
    suspend fun clearRestriction(id: Long) = injuryDao.clear(id, clock.nowMs())

    private companion object {
        const val DAY_MS = 24L * 60 * 60 * 1000
        const val ENGINE_WINDOW_DAYS = 30
        const val SKIPS_BEFORE_BACKING_OFF = 5
    }
}
