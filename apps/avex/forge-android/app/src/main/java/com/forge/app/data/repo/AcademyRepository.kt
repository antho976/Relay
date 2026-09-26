package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.LessonEventDao
import com.forge.app.data.db.entities.LessonEvent
import com.forge.app.domain.academy.AcademyRegistry
import com.forge.app.domain.academy.LessonEventKind
import com.forge.app.domain.adapt.ProgressionAdvisor
import com.forge.app.domain.adapt.countsForProgression
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The Academy's data layer (Coach v3 A2): an append-only ledger in, derived state out.
 *
 * Unlocks are idempotent by construction — [unlock] no-ops when the lesson already has an unlock
 * event — so a moment that fires every week (a stall the coach keeps holding) can call it freely.
 * Nothing here mutates: the same recompute rule as `CoachGenBias.from(decisions)`.
 */
@Singleton
class AcademyRepository @Inject constructor(
    private val lessonEventDao: LessonEventDao,
    private val adaptationRepository: AdaptationRepository,
    private val coachDao: com.forge.app.data.db.dao.CoachDao,
    private val clock: Clock
) {

    fun observeStates(): Flow<List<AcademyRegistry.LessonState>> =
        lessonEventDao.observeAll().map { AcademyRegistry.stateFrom(it) }

    suspend fun states(): List<AcademyRegistry.LessonState> =
        AcademyRegistry.stateFrom(lessonEventDao.all())

    suspend fun unlocked(): List<AcademyRegistry.LessonState> =
        AcademyRegistry.unlocked(lessonEventDao.all())

    suspend fun upcoming(): List<AcademyRegistry.LessonState> =
        AcademyRegistry.upcoming(lessonEventDao.all())

    /** Unlocked but never opened — the "new lesson" chip's count. */
    suspend fun newCount(): Int = states().count { it.isNew }

    /** Record a moment. Idempotent: the first unlock is the truth, later firings are ignored. */
    suspend fun unlock(lessonId: String) = record(lessonId, LessonEventKind.UNLOCKED, once = true)

    /** The user opened the lesson. Recorded once; re-reads don't append. */
    suspend fun markOpened(lessonId: String) = record(lessonId, LessonEventKind.OPENED, once = true)

    /** The user reached the end. Recorded once. */
    suspend fun markCompleted(lessonId: String) = record(lessonId, LessonEventKind.COMPLETED, once = true)

    private suspend fun record(lessonId: String, kind: LessonEventKind, once: Boolean) {
        // A retired id (a coach reason written before the 2026-09-26 cut) records against the lesson
        // that absorbed it, so the ledger only ever grows under ids that still ship.
        val id = AcademyRegistry.lesson(lessonId)?.id ?: return
        if (once && lessonEventDao.has(id, kind.code)) return
        lessonEventDao.insert(
            LessonEvent(lessonId = id, kind = kind.code, atMs = clock.nowMs())
        )
    }

    /**
     * Fire whatever moments the current state implies (B3). Called from the coach's own read paths
     * and from the Academy itself — every unlock is idempotent, so calling it often is free.
     *
     * Derived from the snapshot rather than sprinkled through hot paths on purpose: a set-logging
     * loop should not be doing ledger writes, and "has this user ever logged a set?" is a question
     * the snapshot already answers.
     */
    suspend fun syncCoachMoments() = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) {
        syncCoachMomentsOnWorker()
    }

    private suspend fun syncCoachMomentsOnWorker() {
        val snapshot = runCatching { adaptationRepository.snapshotCached() }.getOrElse {
            if (it is kotlinx.coroutines.CancellationException) throw it else null
        } ?: return

        // Training: the chapter unlocks as the reader meets each idea in their own sessions.
        if (snapshot.program.isNotEmpty()) unlock(LESSON_GETTING_STRONGER)
        val anySets = snapshot.exerciseHistory.values.any { bouts -> bouts.any { it.sets.isNotEmpty() } }
        if (anySets) {
            unlock(LESSON_EFFORT)
            unlock(LESSON_FORM)
        }
        if (snapshot.sessions.isNotEmpty()) unlock(LESSON_RECOVERY)
        if (snapshot.exerciseHistory.values.any { bouts -> bouts.any { !it.countsForProgression } }) {
            unlock(LESSON_FORM)
        }
        // Soreness or illness ever flagged, from either the check-in or the older rest-day reason.
        if (snapshot.cardio.any { it.restReason == "sore" || it.restReason == "sick" }) {
            unlock(LESSON_SORENESS)
        }
        val profile = com.forge.app.domain.coach.PersonalProfile.build(snapshot)
        if (profile.volumeCaps.isNotEmpty()) unlock(LESSON_VOLUME)
        if (ProgressionAdvisor.cutSuppressedStalls(snapshot).isNotEmpty()) unlock(LESSON_PROTEIN)

        // Your coach: once it has enough sessions to speak, or has acted under its own authority.
        if (snapshot.sessions.size >= 3) {
            unlock(LESSON_HOW_IT_DECIDES)
            unlock(LESSON_READINESS)
        }
        if (autoAppliedEver()) unlock(LESSON_HOW_IT_DECIDES)
        if (snapshot.health.hrv.size >= 6) unlock(LESSON_READINESS)

        // Cardio: from the athlete's own conditioning, not from a phase flag.
        val activeCardio = snapshot.cardio.filter { it.restReason == null }
        if (activeCardio.isNotEmpty()) unlock(LESSON_ZONE2)
        if (activeCardio.any { it.intervalCount != null && it.intervalCount > 0 } ||
            com.forge.app.domain.engine.ConditioningLoad.interferencePenalty(snapshot.cardio, snapshot.nowMs) > 0
        ) {
            unlock(LESSON_INTERVALS)
        }
    }

    /** Has the coach ever applied a change under its own earned authority? */
    private suspend fun autoAppliedEver(): Boolean = runCatching {
        com.forge.app.domain.coach.TrustLedger.earnedTypes(coachDao.allDecisions()).isNotEmpty()
    }.getOrDefault(false)

    /** Fired when the goal portfolio flags a real conflict — C2's moment. */
    suspend fun onGoalConflict() = unlock(LESSON_BLOCKS)

    /**
     * The cold-start lesson to carry on today's directive, or null once the track is read. Below
     * the coach's data gates this is what keeps the directive card from ever going blank.
     */
    suspend fun coldStartLesson(): com.forge.app.domain.academy.Lesson? =
        runCatching { AcademyRegistry.nextColdStartLesson(lessonEventDao.all()) }.getOrNull()

    companion object {
        const val LESSON_GETTING_STRONGER = "training.getting_stronger"
        const val LESSON_EFFORT = "training.effort"
        const val LESSON_VOLUME = "training.volume"
        const val LESSON_FORM = "training.form"
        const val LESSON_RECOVERY = "training.recovery"
        const val LESSON_SORENESS = "training.soreness"
        const val LESSON_PROTEIN = "training.protein"
        const val LESSON_HOW_IT_DECIDES = "coach.how_it_decides"
        const val LESSON_READINESS = "coach.readiness"
        const val LESSON_BLOCKS = "coach.blocks"
        const val LESSON_ZONE2 = "cardio.zone2"
        const val LESSON_INTERVALS = "cardio.intervals"
    }
}
