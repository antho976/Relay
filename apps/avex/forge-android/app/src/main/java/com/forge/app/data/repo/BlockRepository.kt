package com.forge.app.data.repo

import com.forge.app.core.time.Clock
import com.forge.app.data.db.dao.TrainingBlockDao
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner
import com.forge.app.domain.coach.CoachGoalKind
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The training block's data layer (Coach v3 C).
 *
 * The block is opt-in: with none started the coach behaves exactly as it did in v2 (reactive
 * deloads, no phase modulation). Once one exists, the weekly pass advances it — idempotently, keyed
 * by ISO week — and every advisor that cares can read its phase.
 */
@Singleton
class BlockRepository @Inject constructor(
    private val blockDao: TrainingBlockDao,
    private val adaptationRepository: AdaptationRepository,
    private val coachGoalRepository: CoachGoalRepository,
    private val academyRepository: AcademyRepository,
    private val clock: Clock
) {

    fun observeActive(): Flow<TrainingBlock?> = blockDao.observeActive()

    suspend fun active(): TrainingBlock? = blockDao.active()

    suspend fun history(): List<TrainingBlock> = blockDao.all()

    /** The phase the coach should plan against, or null when no block is running. */
    suspend fun phase(): BlockPhase? = active()?.let { BlockPhase.fromCode(it.phase) }

    /**
     * Start a block around the athlete's top-priority goal, so the block has something to be FOR.
     * Its intent line is written from that goal rather than generated from a template.
     */
    suspend fun start(weeks: Int = BlockPlanner.DEFAULT_WEEKS, weekId: String): TrainingBlock {
        active()?.let { return it }
        val focus = runCatching { coachGoalRepository.active().firstOrNull() }.getOrNull()
        val intent = when (val kind = focus?.let { CoachGoalKind.fromCode(it.kind) }) {
            null -> "Build steadily, then take a planned week back."
            CoachGoalKind.LIFT_1RM -> "Build toward your strength target, then test it."
            CoachGoalKind.MUSCLE_VOLUME -> "Add working volume where you want size, then recover it."
            CoachGoalKind.BODYWEIGHT -> "Hold your strength while the weight moves."
            CoachGoalKind.CONSISTENCY -> "Make the weeks repeatable before making them harder."
            CoachGoalKind.CONDITIONING -> "Build the aerobic base alongside your lifting."
            CoachGoalKind.BALANCE -> "Even out what's lagging before pushing the whole again."
        }
        val block = BlockPlanner.start(
            nowMs = clock.nowMs(),
            weekId = weekId,
            plannedWeeks = weeks,
            focusGoalId = focus?.id ?: 0,
            intent = intent
        )
        val id = blockDao.insert(block)
        // A block is a coach concept the user can now see, so its lessons exist from this moment.
        runCatching {
            academyRepository.unlock("programming.what_a_block_is")
            academyRepository.unlock("programming.reading_your_block_card")
        }
        return block.copy(id = id)
    }

    /**
     * Advance the live block for this ISO week. Safe to call on every pass: the block records the
     * week it last moved on, so a second call in the same week is a no-op.
     *
     * The fatigue score is passed through so the tripwire can pull a deload FORWARD — the schedule
     * decides when rest is planned, the body can still decide it's needed sooner.
     */
    suspend fun advanceForWeek(weekId: String): TrainingBlock? {
        val block = active() ?: return null
        val fatigue = runCatching {
            DeloadAdvisor.fatigue(adaptationRepository.snapshotCached())?.score ?: 0
        }.getOrDefault(0)
        val advanced = BlockPlanner.advance(block, weekId, clock.nowMs(), fatigue)
        if (advanced != block) {
            blockDao.update(advanced)
            if (advanced.phase != block.phase) {
                runCatching { academyRepository.unlock("programming.four_phases") }
                if (advanced.phase == BlockPhase.DELOAD.code) {
                    runCatching { academyRepository.unlock("programming.deloads_are_earned") }
                }
            }
        }
        return advanced
    }

    /** End the block early — the user's veto, always available. */
    suspend fun end() {
        active()?.let { blockDao.update(it.copy(endedAt = clock.nowMs())) }
    }
}
