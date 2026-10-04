package com.margin.app.data.repo

import com.margin.app.data.Clock
import com.margin.app.data.db.BudgetDao
import com.margin.app.data.db.BudgetEntity
import com.margin.app.data.db.BudgetRow
import com.margin.app.data.db.ContributionEntity
import com.margin.app.data.db.GoalDao
import com.margin.app.data.db.GoalEntity
import com.margin.app.data.db.GoalWithSaved
import com.margin.app.data.db.RecurringDao
import com.margin.app.data.db.RecurringEntity
import com.margin.app.data.db.RecurringRow
import com.margin.core.Recurrence
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PlanRepository @Inject constructor(
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
        val existing = budgets.getFor(categoryId)
        budgets.upsert(BudgetEntity(id = existing?.id ?: 0, categoryId = categoryId, amount = amount))
    }

    suspend fun budgetFor(categoryId: Long): BudgetEntity? = budgets.getFor(categoryId)

    // ── Recurring ────────────────────────────────────────────────────────────

    fun recurring(): Flow<List<RecurringRow>> = recurring.observeAll()
    suspend fun recurringItem(id: Long): RecurringEntity? = recurring.get(id)

    /**
     * Saves a bill. A new or re-anchored one starts at its first occurrence from today, so making
     * a bill never back-fills months of entries the owner did not ask for.
     */
    suspend fun saveRecurring(item: RecurringEntity): Long {
        val rule = Recurrence(item.anchorDate, item.frequency, item.interval)
        val today = clock.today()
        val existing = if (item.id != 0L) recurring.get(item.id) else null
        val ruleChanged = existing == null || existing.anchorDate != item.anchorDate ||
            existing.frequency != item.frequency || existing.interval != item.interval
        val next = if (ruleChanged) rule.onOrAfter(if (item.anchorDate.isAfter(today)) item.anchorDate else today) else item.nextDate
        val clean = item.copy(name = item.name.trim(), nextDate = next)
        return if (clean.id == 0L) recurring.insert(clean) else { recurring.update(clean); clean.id }
    }

    /** Deletes a bill and hands it back for Undo. Entries it posted stay, unlinked. */
    suspend fun deleteRecurring(id: Long): RecurringEntity? {
        val row = recurring.get(id) ?: return null
        recurring.delete(id)
        return row
    }

    /** Puts a deleted bill back exactly, same id. */
    suspend fun restoreRecurring(item: RecurringEntity) { recurring.insert(item) }

    suspend fun setRecurringActive(id: Long, active: Boolean) {
        val item = recurring.get(id) ?: return
        val today = clock.today()
        // Resuming starts from today: a paused bill does not owe the months it was paused for.
        val next = if (active && item.nextDate.isBefore(today)) {
            Recurrence(item.anchorDate, item.frequency, item.interval).onOrAfter(today)
        } else item.nextDate
        recurring.update(item.copy(active = active, nextDate = next))
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
