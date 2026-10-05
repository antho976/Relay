package com.tally.app.data.repo

import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.BudgetDao
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.BudgetRow
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.GoalDao
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.GoalWithSaved
import com.tally.app.data.db.RecurringDao
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.RecurringRow
import com.tally.app.data.db.TallyDatabase
import com.tally.core.BillSchedule
import com.tally.core.Recurrence
import com.tally.core.nextDueOnSave
import kotlinx.coroutines.flow.Flow
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PlanRepository @Inject constructor(
    private val db: TallyDatabase,
    private val budgets: BudgetDao,
    private val recurring: RecurringDao,
    private val goals: GoalDao,
    private val clock: Clock,
) {
    // ── Budgets ──────────────────────────────────────────────────────────────

    fun budgets(): Flow<List<BudgetRow>> = budgets.observeAll()

    /** Sets the standing monthly limit for [categoryId] (0 = overall). Zero or less removes it. */
    suspend fun setBudget(categoryId: Long, amount: Long) {
        if (amount <= 0) {
            budgets.deleteFor(categoryId)
            return
        }
        budgets.setAmount(categoryId, amount)
    }

    suspend fun budgetFor(categoryId: Long): BudgetEntity? = budgets.getFor(categoryId)

    /** Sets several limits as one write (zero removes one), so an undo restores all of them or none. */
    suspend fun setBudgets(amounts: Map<Long, Long>) = db.withTransaction {
        amounts.forEach { (categoryId, amount) -> setBudget(categoryId, amount) }
    }

    // ── Recurring ────────────────────────────────────────────────────────────

    fun recurring(): Flow<List<RecurringRow>> = recurring.observeAll()
    suspend fun recurringItem(id: Long): RecurringEntity? = recurring.get(id)

    /** The latest date entered against bill [id], or null when nothing has been. */
    suspend fun lastPosted(id: Long): LocalDate? = recurring.lastPosted(id)

    /**
     * Saves a bill, pausing or resuming it by [item]'s active flag. The stored row is the authority
     * on where the schedule stands: it is read in the same transaction as the write, so an editor
     * that sat open while the poster ran cannot rewind a date that already posted. [item]'s
     * nextDate is only the earliest start of a new bill; [nextDueOnSave] decides the rest, so a
     * new bill never back-fills, a resumed one does not owe its pause, one switched to posting
     * itself starts from today, and a changed rule never lands on a date the bill already posted.
     * A bill deleted while its editor was open comes back as a new one instead of vanishing.
     */
    suspend fun saveRecurring(item: RecurringEntity): Long = db.withTransaction { write(item) }

    /** Deletes a bill and hands it back for Undo. Entries it posted stay, unlinked. */
    suspend fun deleteRecurring(id: Long): RecurringEntity? {
        val row = recurring.get(id) ?: return null
        recurring.delete(id)
        return row
    }

    /** Puts a deleted bill back exactly, same id. */
    suspend fun restoreRecurring(item: RecurringEntity) { recurring.insert(item) }

    /** Pauses or resumes a bill. Resuming starts from today: a paused bill does not owe its pause. */
    suspend fun setRecurringActive(id: Long, active: Boolean) {
        db.withTransaction {
            recurring.get(id)?.let { write(it.copy(active = active)) }
        }
    }

    /** [saveRecurring]'s write, inside the caller's transaction. */
    private suspend fun write(item: RecurringEntity): Long {
        val existing = if (item.id != 0L) recurring.get(item.id) else null
        val next = nextDueOnSave(
            rule = Recurrence(item.anchorDate, item.frequency, item.interval),
            autoPost = item.autoPost,
            active = item.active,
            stored = existing?.let { BillSchedule(Recurrence(it.anchorDate, it.frequency, it.interval), it.nextDate, it.autoPost, it.active) },
            today = clock.today(),
            lastPosted = existing?.let { recurring.lastPosted(it.id) },
            notBefore = if (existing == null) item.nextDate else null,
        )
        val clean = item.copy(name = item.name.trim(), nextDate = next)
        if (existing == null) return recurring.insert(clean.copy(id = 0L))
        recurring.update(clean)
        return clean.id
    }

    // ── Goals ────────────────────────────────────────────────────────────────

    fun goals(): Flow<List<GoalWithSaved>> = goals.observeAll()
    suspend fun goal(id: Long): GoalEntity? = goals.get(id)
    fun contributions(goalId: Long): Flow<List<ContributionEntity>> = goals.observeContributions(goalId)

    suspend fun saveGoal(goal: GoalEntity): Long =
        if (goal.id == 0L) goals.insert(goal.copy(name = goal.name.trim()))
        else { goals.update(goal.copy(name = goal.name.trim())); goal.id }

    /** A deleted goal and its contributions, for Undo. */
    data class DeletedGoal(val goal: GoalEntity, val contributions: List<ContributionEntity>)

    suspend fun deleteGoal(id: Long): DeletedGoal? {
        val goal = goals.get(id) ?: return null
        val contributions = goals.contributionsFor(id)
        goals.delete(id)
        return DeletedGoal(goal, contributions)
    }

    suspend fun restoreGoal(deleted: DeletedGoal) {
        goals.insert(deleted.goal)
        goals.insertContributions(deleted.contributions)
    }

    suspend fun contribute(goalId: Long, amount: Long, note: String = ""): Long =
        goals.insertContribution(ContributionEntity(goalId = goalId, amount = amount, date = clock.today(), note = note.trim()))

    /** Deletes one contribution and hands it back for Undo. */
    suspend fun deleteContribution(id: Long): ContributionEntity? {
        val row = goals.contribution(id) ?: return null
        goals.deleteContribution(id)
        return row
    }

    suspend fun restoreContribution(c: ContributionEntity) { goals.insertContributions(listOf(c)) }
}
