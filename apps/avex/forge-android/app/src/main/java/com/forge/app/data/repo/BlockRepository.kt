package com.forge.app.data.repo

import androidx.room.withTransaction
import com.forge.app.core.time.Clock
import com.forge.app.core.time.deloadWeekEndMs
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.TrainingBlockDao
import com.forge.app.data.db.entities.TrainingBlock
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.adapt.DeloadAdvisor
import com.forge.app.domain.coach.BlockPhase
import com.forge.app.domain.coach.BlockPlanner
import com.forge.app.domain.coach.CoachGoalKind
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    private val settingsRepository: SettingsRepository,
    private val database: ForgeDatabase,
    private val clock: Clock
) {

    /**
     * Serialises [start], [advanceForWeek] and [end] within the process. Each is a read-then-write
     * on the singleton live row, and two of them interleaving is how a second active block came to
     * exist (M-13). Not held by [active]: the lifecycle methods call it while holding this.
     */
    private val lifecycleMutex = Mutex()

    fun observeActive(): Flow<TrainingBlock?> = blockDao.observeActive()

    /**
     * The live block, repairing the singleton invariant on the way: any extra open row is ended in
     * the same transaction, so a duplicate left behind by an older build cannot outlive the block
     * the user can see and reappear after they end it.
     */
    suspend fun active(): TrainingBlock? = database.withTransaction {
        val open = blockDao.allActive()
        val visible = open.firstOrNull() ?: return@withTransaction null
        if (open.size > 1) blockDao.endAllExcept(keepId = visible.id, endedAt = clock.nowMs())
        visible
    }

    suspend fun history(): List<TrainingBlock> = blockDao.all()

    /** The phase the coach should plan against, or null when no block is running. */
    suspend fun phase(): BlockPhase? = active()?.let { BlockPhase.fromCode(it.phase) }

    /**
     * Start a block around the athlete's top-priority goal, so the block has something to be FOR.
     * Its intent line is written from that goal rather than generated from a template.
     */
    suspend fun start(weeks: Int = BlockPlanner.DEFAULT_WEEKS, weekId: String): TrainingBlock = lifecycleMutex.withLock {
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
        // "Is one running?" and the insert are one transaction, so two starts that both got here
        // cannot both insert: the second reads the first's committed row and returns it (M-13).
        val (block, created) = database.withTransaction {
            active()?.let { running -> return@withTransaction running to false }
            val planned = BlockPlanner.start(
                nowMs = clock.nowMs(),
                weekId = weekId,
                plannedWeeks = weeks,
                focusGoalId = focus?.id ?: 0,
                intent = intent
            )
            val id = blockDao.insert(planned)
            planned.copy(id = id) to true
        }
        // A block is a coach concept the user can now see, so its lessons exist from this moment.
        if (created) runCatching {
            academyRepository.unlock("programming.what_a_block_is")
            academyRepository.unlock("programming.reading_your_block_card")
        }
        block
    }

    /**
     * Advance the live block for this ISO week. Safe to call on every pass: the block records the
     * week it last moved on, so a second call in the same week is a no-op.
     *
     * The fatigue score is passed through so the tripwire can pull a deload FORWARD — the schedule
     * decides when rest is planned, the body can still decide it's needed sooner.
     *
     * Entering the deload week is not a label change: it serves the deload. The scheduled deload
     * goes through the SAME regeneration the coach's reactive "deload" proposal and the Overview
     * card use ([AdaptationRepository.applyDeloadWeek]) — one deload generator, three entry points.
     * The weekly pass advances the block before it snapshots the program, so the pass that follows
     * plans against the deloaded week rather than the one it replaced.
     */
    suspend fun advanceForWeek(weekId: String): TrainingBlock? = lifecycleMutex.withLock {
        val block = active() ?: return@withLock null
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
            if (BlockPlanner.entersDeload(block, advanced)) serveScheduledDeload()
        }
        advanced
    }

    /**
     * Generate the block's deload week through the existing deload path, unless a deload applied
     * through either of the other entry points is still governing today — regenerating on top of
     * a running deload would reroll the week and push its window out (the same guard the Coach's
     * apply uses). Failure is swallowed on purpose: the block has already moved, and the weekly
     * pass falls back to proposing the deload as a decision when none is running.
     */
    private suspend fun serveScheduledDeload() {
        val startedAt = settingsRepository.deloadWeekStartMs.first()
        if (startedAt > 0L && clock.nowMs() < deloadWeekEndMs(startedAt)) return
        runCatching { adaptationRepository.applyDeloadWeek() }
    }

    /**
     * End the block early — the user's veto, always available.
     *
     * Ends EVERY open row, not only the visible one: a hidden duplicate that survived the visible
     * block's end used to reappear as the live block and keep driving phase behaviour (M-13).
     */
    suspend fun end() = lifecycleMutex.withLock {
        val now = clock.nowMs()
        database.withTransaction {
            blockDao.allActive().forEach { blockDao.update(it.copy(endedAt = now)) }
        }
    }
}
