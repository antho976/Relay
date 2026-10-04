package com.tally.app.data.repo

import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.RecurringDao
import com.tally.app.data.db.TransactionDao
import com.tally.app.data.db.TransactionEntity
import com.tally.core.Recurrence
import com.tally.core.TxType
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Writes every auto-posting bill that has come due into the ledger, catching up on any days the
 * app was not opened. Runs at launch and from the daily worker; the mutex and the per-bill
 * transaction mean two overlapping runs can never post the same date twice.
 */
@Singleton
class RecurringPoster @Inject constructor(
    private val db: TallyDatabase,
    private val recurring: RecurringDao,
    private val transactions: TransactionDao,
    private val clock: Clock,
) {
    private val mutex = Mutex()

    /** Returns how many entries were posted. */
    suspend fun postDue(): Int = mutex.withLock {
        val today = clock.today()
        var posted = 0
        recurring.due(today).forEach { item ->
            db.withTransaction {
                // Re-read inside the transaction: the row may have moved since the query above.
                val fresh = recurring.get(item.id) ?: return@withTransaction
                if (!fresh.active || !fresh.autoPost || fresh.nextDate.isAfter(today)) return@withTransaction
                val rule = Recurrence(fresh.anchorDate, fresh.frequency, fresh.interval)
                val last = fresh.endDate?.takeIf { it.isBefore(today) } ?: today
                val dates = rule.between(fresh.nextDate, last, limit = CATCH_UP_LIMIT)
                dates.forEach { date ->
                    transactions.insert(
                        TransactionEntity(
                            type = fresh.type,
                            amount = fresh.amount,
                            date = date,
                            accountId = fresh.accountId,
                            toAccountId = if (fresh.type == TxType.TRANSFER) fresh.toAccountId else null,
                            categoryId = if (fresh.type == TxType.TRANSFER) null else fresh.categoryId,
                            note = fresh.name,
                            recurringId = fresh.id,
                            createdAt = clock.nowMillis(),
                        )
                    )
                }
                posted += dates.size
                val next = rule.after(dates.lastOrNull() ?: today)
                val ended = fresh.endDate != null && next.isAfter(fresh.endDate)
                recurring.update(fresh.copy(nextDate = next, active = fresh.active && !ended))
            }
        }
        posted
    }

    companion object {
        /** A weekly bill a year overdue; anything beyond that is a data problem, not a backlog. */
        const val CATCH_UP_LIMIT = 60
    }
}
