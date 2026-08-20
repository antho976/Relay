package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.CoachProjectDao
import com.forge.app.data.db.entities.CoachProject
import com.forge.app.domain.coach.PersonalProfile
import com.forge.app.domain.coach.ProjectScanner
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Proactive projects (Coach v3 D): the coach's standing answer to "what should I improve?".
 *
 * Propose-only at this phase — the scanner surfaces the biggest lever and the user accepts it. The
 * coach starting its own projects is a T3 behavior and waits for Phase E, because a coach that
 * starts work you didn't agree to has to have earned that first.
 */
@Singleton
class ProjectRepository @Inject constructor(
    private val projectDao: CoachProjectDao,
    private val adaptationRepository: AdaptationRepository,
    private val academyRepository: AcademyRepository,
    private val clock: Clock
) {

    fun observeActive(): Flow<CoachProject?> = projectDao.observeActive()

    suspend fun active(): CoachProject? = projectDao.active()

    suspend fun history(): List<CoachProject> = projectDao.all()

    /**
     * The project the coach would propose next, or null when nothing is worth interrupting for.
     * Returns null while one is already running: one at a time is the entire discipline.
     */
    suspend fun proposal(): ProjectScanner.Candidate? {
        if (active() != null) return null
        val snapshot = runCatching { adaptationRepository.snapshotCached() }.getOrNull() ?: return null
        val profile = PersonalProfile.build(snapshot)
        // Don't re-propose something the user already abandoned; they answered that question.
        val abandoned = projectDao.all().filter { it.abandonedAt != null }
            .mapNotNull { p -> ProjectScanner.Kind.entries.firstOrNull { it.code == p.kind } }
            .toSet()
        return ProjectScanner.top(snapshot, profile, excludeKinds = abandoned)
    }

    /**
     * Every project the coach would consider right now, best first — not just the top one.
     *
     * The scanner has always ranked a list; only its head was ever surfaced, which made the coach
     * look like it had one idea and no reasoning. Picking from the list is the user's call: the
     * ranking says which is most useful, not which they want to spend four weeks on.
     */
    suspend fun proposals(): List<ProjectScanner.Candidate> {
        if (active() != null) return emptyList()
        val snapshot = runCatching { adaptationRepository.snapshotCached() }.getOrNull() ?: return emptyList()
        val profile = PersonalProfile.build(snapshot)
        val abandoned = projectDao.all().filter { it.abandonedAt != null }
            .mapNotNull { p -> ProjectScanner.Kind.entries.firstOrNull { it.code == p.kind } }
            .toSet()
        return ProjectScanner.scan(snapshot, profile).filterNot { it.kind in abandoned }
    }

    /** Accept a proposal. Its lesson unlocks here, because the concept is now live for this user. */
    suspend fun accept(candidate: ProjectScanner.Candidate): CoachProject {
        active()?.let { return it }
        val project = CoachProject(
            kind = candidate.kind.code,
            name = candidate.name,
            why = candidate.why,
            plan = candidate.plan,
            finishLine = candidate.finishLine,
            targetKey = candidate.targetKey,
            weeks = candidate.weeks,
            startedAt = clock.nowMs()
        )
        val id = projectDao.insert(project)
        runCatching {
            academyRepository.unlock("coach.what_a_project_is")
            if (candidate.kind == ProjectScanner.Kind.IMBALANCE) {
                academyRepository.unlock("programming.imbalances")
            }
        }
        return project.copy(id = id)
    }

    suspend fun complete(id: Long) = projectDao.markCompleted(id, clock.nowMs())

    suspend fun abandon(id: Long) = projectDao.markAbandoned(id, clock.nowMs())

    /**
     * Has the running project run its course? Time-based rather than metric-based on purpose: the
     * finish line is stated in the user's terms and they are the ones who judge it, so the coach
     * asks rather than silently deciding it succeeded.
     */
    suspend fun dueForReview(): CoachProject? = active()?.takeIf {
        clock.nowMs() - it.startedAt >= it.weeks * 7L * 24 * 60 * 60 * 1000
    }
}
