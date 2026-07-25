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
        if (AcademyRegistry.lesson(lessonId) == null) return
        if (once && lessonEventDao.has(lessonId, kind.code)) return
        lessonEventDao.insert(
            LessonEvent(lessonId = lessonId, kind = kind.code, atMs = clock.nowMs())
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
    suspend fun syncCoachMoments() {
        val snapshot = runCatching { adaptationRepository.snapshotCached() }.getOrNull() ?: return

        // Cold start: the curriculum's first lesson exists from the moment there's a program.
        if (snapshot.program.isNotEmpty()) unlock("fundamentals.what_a_program_is")

        val anySets = snapshot.exerciseHistory.values.any { bouts -> bouts.any { it.sets.isNotEmpty() } }
        if (anySets) {
            unlock("fundamentals.sets_reps_rpe")
            unlock("fundamentals.warmups")
        }
        // Two sessions in and progression suggestions become real, so does their lesson.
        if (snapshot.sessions.size >= 2) unlock("fundamentals.progressive_overload")
        if (snapshot.sessions.isNotEmpty()) unlock("fundamentals.rest_and_recovery")
        if (snapshot.sessions.size >= 3) {
            unlock("fundamentals.how_the_coach_works")
            unlock("fundamentals.what_readiness_means")
            unlock("coach.readiness_built_from")
        }
        // A technique-tagged session means the athlete has met the idea; F3 explains why it's kept
        // out of the coach's progress reads.
        if (snapshot.exerciseHistory.values.any { bouts -> bouts.any { !it.countsForProgression } }) {
            unlock("fundamentals.form_vs_load")
        }
        // Soreness or illness ever flagged — from either the check-in or the older rest-day reason.
        if (snapshot.cardio.any { it.restReason == "sore" || it.restReason == "sick" }) {
            unlock("fundamentals.soreness_vs_injury")
        }
        // Finishing the track earns the closing lesson about the price of all of it.
        val events = lessonEventDao.all()
        val readTrack = AcademyRegistry.coldStartTrack.count { lesson ->
            events.any { it.lessonId == lesson.id && it.kind == LessonEventKind.OPENED.code }
        }
        if (readTrack >= AcademyRegistry.coldStartTrack.size - 1) unlock("fundamentals.log_honestly")

        // D: the learning loop's moments — a personal number that has started changing decisions.
        val profile = com.forge.app.domain.coach.PersonalProfile.build(snapshot)
        if (profile.volumeCaps.isNotEmpty()) unlock("programming.your_volume_landmarks")
        if (profile.recoveryDays != null) unlock("programming.your_recovery_curve")
        if (profile.sweetSpotReps.isNotEmpty()) unlock("programming.sweet_spot_reps")

        // E: the ladder itself becomes a concept the moment the coach has any standing on it, and
        // "how to take it back" the moment it has ever applied something without being asked twice.
        if (snapshot.sessions.size >= 12) unlock("coach.trust_tiers")
        if (autoAppliedEver()) unlock("coach.taking_decisions_back")
        // F: HRV becomes a concept once there's enough of it to read a trend from.
        if (snapshot.health.hrv.size >= 6) unlock("signals.stress_hrv")

        // Engine: conditioning concepts unlock from the athlete's own cardio, not from a phase flag.
        val activeCardio = snapshot.cardio.filter { it.restReason == null }
        if (activeCardio.isNotEmpty()) {
            unlock("engine.why_aerobic_base")
            unlock("engine.what_zone2_is")
        }
        if (com.forge.app.domain.engine.ConditioningLoad.interferencePenalty(snapshot.cardio, snapshot.nowMs) > 0) {
            unlock("engine.interference")
        }
        if (activeCardio.any { it.intervalCount != null && it.intervalCount > 0 }) {
            unlock("engine.intervals")
        }
        if (activeCardio.any { it.hrZone != null }) unlock("engine.reading_hr")
        if (activeCardio.count { it.distanceKm != null } >= 8) unlock("engine.base_without_a_lab")

        // The coach's own counterintuitive moment: a stall it deliberately did not escalate.
        if (ProgressionAdvisor.cutSuppressedStalls(snapshot).isNotEmpty()) {
            unlock(LESSON_STRENGTH_ON_A_CUT)
        }
    }

    /** Has the coach ever applied a change under its own earned authority? */
    private suspend fun autoAppliedEver(): Boolean = runCatching {
        com.forge.app.domain.coach.TrustLedger.earnedTypes(coachDao.allDecisions()).isNotEmpty()
    }.getOrDefault(false)

    /** Fired when the goal portfolio flags a real conflict — C2's moment. */
    suspend fun onGoalConflict() = unlock("coach.why_goals_fight")

    /**
     * The cold-start lesson to carry on today's directive, or null once the track is read. Below
     * the coach's data gates this is what keeps the directive card from ever going blank.
     */
    suspend fun coldStartLesson(): com.forge.app.domain.academy.Lesson? =
        runCatching { AcademyRegistry.nextColdStartLesson(lessonEventDao.all()) }.getOrNull()

    companion object {
        const val LESSON_STRENGTH_ON_A_CUT = "coach.strength_on_a_cut"
    }
}
