package com.tally.app.data.repo

import android.content.Context
import android.net.Uri
import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.NoteUse
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TransactionEntity
import com.tally.app.data.prefs.SettingsRepository
import com.tally.core.AccountType
import com.tally.core.BankStatements
import com.tally.core.TxType
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

/**
 * One statement line as the import writes it. [amount] is signed from the account's side: below
 * zero leaves it (an expense, or a transfer out to [counterpartId]), above zero comes in.
 */
data class ImportLine(
    val date: LocalDate,
    val note: String,
    val amount: Long,
    val categoryId: Long?,
    /** Set for a move between the owner's own accounts: the account at the other end. */
    val counterpartId: Long? = null,
)

/** Where an import lands: an account that exists, or one it creates (with the opening the statement implies). */
sealed interface ImportTarget {
    data class Existing(val accountId: Long) : ImportTarget
    data class New(val name: String, val type: AccountType, val openingBalance: Long) : ImportTarget
}

/**
 * The bank import's reads and its one write. The file is read here (the ViewModel holds no
 * Context); deciding what each line becomes is [com.tally.app.ui.settings.planImport]'s job,
 * and this writes the result in one transaction, so a failed import adds nothing.
 */
@Singleton
class ImportRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val db: TallyDatabase,
    private val ledger: LedgerRepository,
    private val settings: SettingsRepository,
    private val clock: Clock,
) {

    /** The file at [uri] as text, in whatever encoding the bank wrote it; null when it cannot be opened. */
    suspend fun readText(uri: Uri): String? = withContext(Dispatchers.IO) {
        runCatching {
            context.contentResolver.openInputStream(uri)?.use { BankStatements.decodeText(it.readBytes()) }
        }.getOrNull()
    }

    /**
     * What [accountId] already holds between [start] and [end], as signed effects by day: income
     * and transfers in above zero, spending and transfers out below. A statement line that finds
     * its twin here is already logged.
     */
    suspend fun existingEffects(accountId: Long, start: LocalDate, end: LocalDate): List<Pair<LocalDate, Long>> =
        withContext(Dispatchers.IO) {
            ledger.touching(accountId, start, end).map { t ->
                val signed = when {
                    t.type == TxType.TRANSFER && t.toAccountId == accountId && t.accountId != accountId -> t.amount
                    t.type == TxType.TRANSFER -> -t.amount
                    t.type == TxType.INCOME -> t.amount
                    else -> -t.amount
                }
                t.date to signed
            }
        }

    /** Notes filed under a category in the last year: what the import learns the owner's own payees from. */
    suspend fun learnedNotes(): List<NoteUse> = withContext(Dispatchers.IO) {
        ledger.notesSince(clock.today().minusYears(1))
    }

    /**
     * Writes [lines] into [target]: expenses, income, and transfers with their counterpart. A new
     * account is created first and becomes the default when there is no default yet. Returns the
     * account the lines went into, or null when nothing was written.
     */
    suspend fun write(target: ImportTarget, lines: List<ImportLine>): Long? = withContext(Dispatchers.IO) {
        runCatching {
            db.withTransaction {
                val accountId = when (target) {
                    is ImportTarget.Existing -> target.accountId
                    is ImportTarget.New -> ledger.saveAccount(
                        AccountEntity(name = target.name, type = target.type, openingBalance = target.openingBalance)
                    )
                }
                val now = clock.nowMillis()
                val rows = lines.filter { it.amount != 0L }.map { line ->
                    val size = kotlin.math.abs(line.amount)
                    val other = line.counterpartId?.takeIf { it != accountId }
                    when {
                        other != null && line.amount < 0 -> TransactionEntity(
                            type = TxType.TRANSFER, amount = size, date = line.date, accountId = accountId,
                            toAccountId = other, note = line.note, createdAt = now,
                        )
                        other != null -> TransactionEntity(
                            type = TxType.TRANSFER, amount = size, date = line.date, accountId = other,
                            toAccountId = accountId, note = line.note, createdAt = now,
                        )
                        else -> TransactionEntity(
                            type = if (line.amount < 0) TxType.EXPENSE else TxType.INCOME, amount = size, date = line.date,
                            accountId = accountId, categoryId = line.categoryId, note = line.note, createdAt = now,
                        )
                    }
                }
                db.transactions().insertAll(rows)
                accountId
            }
        }.getOrNull()?.also { id ->
            val s = settings.current()
            if (db.accounts().get(s.defaultAccountId) == null) settings.setDefaultAccount(id)
        }
    }
}
