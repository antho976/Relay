package com.tally.app.data.db

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey
import com.tally.core.AccountType
import com.tally.core.ActivityType
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.Registration
import com.tally.core.SecurityKind
import com.tally.core.TxType
import java.time.LocalDate
import java.util.UUID

// Every amount is a Long of minor units (cents). Enums are stored by name; renaming a constant
// is a migration, and release keeps enum names (proguard-rules.pro).
//
// Every synced row ends in [uid] and [updatedAt] (version 3). They come last so the positional
// constructors the backup restore uses still read the same; deletes leave a [TombstoneEntity].

fun newUid(): String = UUID.randomUUID().toString()

@Entity(tableName = "accounts", indices = [Index(value = ["uid"], unique = true)])
data class AccountEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val type: AccountType,
    val openingBalance: Long = 0,
    val archived: Boolean = false,
    val sortOrder: Int = 0,
    /** An INVESTMENT account's tax wrapper (docs/INVESTMENTS.md, version 4); null on every other account. */
    val registration: Registration? = null,
    /** Who holds it ("Wealthsimple"); empty when nobody said. */
    val institution: String = "",
    /** The institution's own number for it ("HQ7XFMC41CAD"): how an import finds it again. */
    val externalRef: String = "",
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

@Entity(tableName = "categories", indices = [Index("kind"), Index(value = ["uid"], unique = true)])
data class CategoryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val kind: CategoryKind,
    /** Index into the twelve-hue category palette. */
    val color: Int,
    val icon: String,
    val archived: Boolean = false,
    val sortOrder: Int = 0,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

@Entity(
    tableName = "transactions",
    foreignKeys = [
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["toAccountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = CategoryEntity::class, parentColumns = ["id"], childColumns = ["categoryId"], onDelete = ForeignKey.SET_NULL),
        ForeignKey(entity = RecurringEntity::class, parentColumns = ["id"], childColumns = ["recurringId"], onDelete = ForeignKey.SET_NULL),
    ],
    indices = [
        Index("date"), Index("accountId"), Index("toAccountId"), Index("categoryId"), Index("recurringId"), Index(value = ["type", "date"]),
        Index(value = ["uid"], unique = true),
    ],
)
data class TransactionEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val type: TxType,
    /** Always positive; [type] carries the direction. */
    val amount: Long,
    val date: LocalDate,
    val accountId: Long,
    /** The receiving account of a transfer; null otherwise. */
    val toAccountId: Long? = null,
    /** Null for transfers, and for entries whose category was deleted. */
    val categoryId: Long? = null,
    val note: String = "",
    val recurringId: Long? = null,
    @ColumnInfo(defaultValue = "0") val createdAt: Long = 0,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * A standing monthly limit. [categoryId] 0 is the overall budget: a real column value rather than
 * NULL, because SQLite lets any number of NULLs through a unique index and there must be one.
 */
@Entity(tableName = "budgets", indices = [Index(value = ["categoryId"], unique = true), Index(value = ["uid"], unique = true)])
data class BudgetEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val categoryId: Long,
    val amount: Long,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
) {
    companion object { const val OVERALL = 0L }
}

@Entity(
    tableName = "recurring",
    foreignKeys = [
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["toAccountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = CategoryEntity::class, parentColumns = ["id"], childColumns = ["categoryId"], onDelete = ForeignKey.SET_NULL),
    ],
    indices = [Index("accountId"), Index("toAccountId"), Index("categoryId"), Index("nextDate"), Index(value = ["uid"], unique = true)],
)
data class RecurringEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val type: TxType,
    val amount: Long,
    val accountId: Long,
    val toAccountId: Long? = null,
    val categoryId: Long? = null,
    val frequency: Frequency,
    val interval: Int = 1,
    val anchorDate: LocalDate,
    /** The next date this posts on; advanced when it posts. */
    val nextDate: LocalDate,
    val endDate: LocalDate? = null,
    /** Off means it is a reminder: shown as upcoming, never written to the ledger on its own. */
    val autoPost: Boolean = true,
    val active: Boolean = true,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * A goal. [kind] says what it measures (see [GoalKind]); the columns past [archived] came with
 * version 2 and default to a plain savings goal, which is what every version 1 goal was.
 */
@Entity(tableName = "goals", indices = [Index(value = ["uid"], unique = true)])
data class GoalEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    /** The amount to reach; for a monthly goal set as a share of income, unused (0). */
    val target: Long,
    val targetDate: LocalDate? = null,
    val color: Int = 0,
    val archived: Boolean = false,
    @ColumnInfo(defaultValue = "SAVINGS") val kind: GoalKind = GoalKind.SAVINGS,
    /**
     * The account a balance goal reads, or the one an invest goal counts money into. Null reads
     * everything: the net of every open account, or every investment account. Not a foreign key:
     * deleting the account clears it ([GoalDao.forgetAccount]).
     */
    val accountId: Long? = null,
    /** A monthly goal's share of what came in, 1 to 100; 0 when [target] is a set amount a month. */
    @ColumnInfo(defaultValue = "0") val percent: Int = 0,
    /** When a balance goal was set, and what it read then: the start of its pace. */
    val startDate: LocalDate? = null,
    @ColumnInfo(defaultValue = "0") val startAmount: Long = 0,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

@Entity(
    tableName = "goal_contributions",
    foreignKeys = [ForeignKey(entity = GoalEntity::class, parentColumns = ["id"], childColumns = ["goalId"], onDelete = ForeignKey.CASCADE)],
    indices = [Index("goalId"), Index(value = ["uid"], unique = true)],
)
data class ContributionEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val goalId: Long,
    /** Negative for a withdrawal. */
    val amount: Long,
    val date: LocalDate,
    val note: String = "",
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * What an investment account was worth on [date], as the owner read it off a statement. The
 * newest one stands in for the opening balance and every entry dated on or before it, so market
 * moves land in the balance without passing for income or spending.
 */
@Entity(
    tableName = "account_values",
    foreignKeys = [ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE)],
    indices = [Index(value = ["accountId", "date"]), Index(value = ["uid"], unique = true)],
)
data class AccountValueEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val accountId: Long,
    val date: LocalDate,
    val value: Long,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

// ── Investments (docs/INVESTMENTS.md, version 4) ────────────────────────────────────────────
//
// Each table has the same name on the PC, on the wire and here. Units held are at
// Invest.QTY_SCALE, prices and rates at Invest.PRICE_SCALE and RATE_SCALE. Positions, values,
// gains and room are read from these rows (Invest.portfolio), never stored. An import writes
// rows under uids both devices derive the same way (sec:, hold:, imp:, px:, val:, ws:).

/** A thing held: a stock, a fund, cash in a currency. uid `sec:<SYMBOL>`. */
@Entity(tableName = "securities", indices = [Index(value = ["uid"], unique = true)])
data class SecurityEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** Upper case, as its uid is made from it. */
    val symbol: String,
    val name: String = "",
    /** The ISO code its prices and its market book are in. */
    val currency: String,
    val kind: SecurityKind,
    val exchange: String = "",
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * One line of an account's holdings snapshot: its rows on its newest [date] are what it held then.
 * [book] is in the ledger currency, [bookMarket] in the security's. uid
 * `hold:<account uid>:<security uid>:<date>` when imported.
 */
@Entity(
    tableName = "holdings",
    foreignKeys = [
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = SecurityEntity::class, parentColumns = ["id"], childColumns = ["securityId"], onDelete = ForeignKey.CASCADE),
    ],
    indices = [Index(value = ["accountId", "date"]), Index("securityId"), Index(value = ["uid"], unique = true)],
)
data class HoldingEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val accountId: Long,
    val securityId: Long,
    val date: LocalDate,
    val quantity: Long,
    val book: Long,
    val bookMarket: Long,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * A buy, a sale, a dividend, money in or out. Never a [TransactionEntity], so it never counts as
 * spending or income. [quantity], [amount] and [fee] are never negative: [type] gives the
 * direction. [amount] and [fee] are minor units of [currency]; an FX activity's [amount] leaves in
 * [currency] and [toAmount] arrives in [toCurrency]. uid `imp:<hash>` when imported.
 */
@Entity(
    tableName = "activities",
    foreignKeys = [
        ForeignKey(entity = AccountEntity::class, parentColumns = ["id"], childColumns = ["accountId"], onDelete = ForeignKey.CASCADE),
        ForeignKey(entity = SecurityEntity::class, parentColumns = ["id"], childColumns = ["securityId"], onDelete = ForeignKey.SET_NULL),
    ],
    indices = [Index(value = ["accountId", "date"]), Index("securityId"), Index(value = ["uid"], unique = true)],
)
data class ActivityEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val accountId: Long,
    val securityId: Long? = null,
    val type: ActivityType,
    val date: LocalDate,
    val quantity: Long = 0,
    val amount: Long,
    val fee: Long = 0,
    val currency: String,
    val toAmount: Long? = null,
    val toCurrency: String? = null,
    val note: String = "",
    /** MANUAL, or WEALTHSIMPLE for an imported line. */
    val source: String = "MANUAL",
    val createdAt: Long = 0,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/** A security's price on [date] (IMPORT or MANUAL). uid `px:<security uid>:<date>`. */
@Entity(
    tableName = "prices",
    foreignKeys = [ForeignKey(entity = SecurityEntity::class, parentColumns = ["id"], childColumns = ["securityId"], onDelete = ForeignKey.CASCADE)],
    indices = [Index(value = ["securityId", "date"]), Index(value = ["uid"], unique = true)],
)
data class PriceEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val securityId: Long,
    val date: LocalDate,
    val price: Long,
    val source: String,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/** Units of [quote] for one [base] on [date] (MANUAL or BANK_OF_CANADA). uid `fx:<BASE>:<QUOTE>:<date>`. */
@Entity(tableName = "fx_rates", indices = [Index(value = ["uid"], unique = true)])
data class FxRateEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val base: String,
    val quote: String,
    val date: LocalDate,
    val rate: Long,
    val source: String,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * The contribution room CRA gives a registration in [year] (My Account, or the Notice of
 * Assessment for an RRSP), in the ledger currency. uid `room:<REGISTRATION>:<year>`.
 */
@Entity(tableName = "room_facts", indices = [Index(value = ["uid"], unique = true)])
data class RoomFactEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val registration: Registration,
    val year: Int,
    val amount: Long,
    /** Permanent id across devices (docs/MONEY.md, "Sync"); never changes once written. */
    val uid: String = newUid(),
    /** When this row last changed, in epoch millis. The database keeps it current (SyncSchema). */
    val updatedAt: Long = System.currentTimeMillis(),
)

/**
 * A row deleted on this phone, kept until the paired PC has been told. Written by the database's
 * own delete triggers (SyncSchema), so a cascade leaves one for every row it takes. [category] is
 * set for budgets only, which sync by their category: the category's uid, or null for the overall
 * budget.
 */
@Entity(tableName = "sync_tombstones", primaryKeys = ["tableName", "uid"], indices = [Index("deletedAt")])
data class TombstoneEntity(
    val tableName: String,
    val uid: String,
    val deletedAt: Long,
    val category: String? = null,
)

/**
 * The sync's own flags. While the row "applying" is present, the triggers that stamp local edits
 * stand aside, so rows taken from the PC keep the PC's change time. Only ever set inside the
 * transaction that applies a sync.
 */
@Entity(tableName = "sync_state")
data class SyncStateEntity(
    @PrimaryKey val key: String,
    val value: Long,
)
