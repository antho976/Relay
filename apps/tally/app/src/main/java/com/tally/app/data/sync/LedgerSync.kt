package com.tally.app.data.sync

import androidx.room.withTransaction
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.ActivityEntity
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.db.ContributionEntity
import com.tally.app.data.db.FxRateEntity
import com.tally.app.data.db.GoalEntity
import com.tally.app.data.db.HoldingEntity
import com.tally.app.data.db.IdUid
import com.tally.app.data.db.PriceEntity
import com.tally.app.data.db.RecurringEntity
import com.tally.app.data.db.RoomFactEntity
import com.tally.app.data.db.SecurityEntity
import com.tally.app.data.db.SyncDao
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.db.TombstoneEntity
import com.tally.app.data.db.TransactionEntity
import com.tally.core.AccountType
import com.tally.core.ActivityType
import com.tally.core.CategoryKind
import com.tally.core.Frequency
import com.tally.core.GoalKind
import com.tally.core.Registration
import com.tally.core.SecurityKind
import com.tally.core.TxType
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

/**
 * One row's change on the wire (docs/MONEY.md, "Sync"): `{ table, uid, updated_at, deleted, row }`.
 * [row] holds the fields camelCase, as Tally's backup names them, with references as uids.
 */
data class Change(
    val table: String,
    val uid: String,
    val updatedAt: Long,
    val deleted: Boolean = false,
    val row: JsonObject = JsonObject(emptyMap()),
) {
    fun toJson(): JsonObject = buildJsonObject {
        put("table", table)
        put("uid", uid)
        put("updated_at", updatedAt)
        put("deleted", deleted)
        put("row", row)
    }

    companion object {
        /** Null for anything that is not a change: it is skipped, not fatal. */
        fun fromJson(json: JsonElement): Change? = runCatching {
            val o = json.jsonObject
            Change(
                table = o.getValue("table").jsonPrimitive.content,
                uid = o.getValue("uid").jsonPrimitive.content,
                updatedAt = o.getValue("updated_at").jsonPrimitive.longOrNull ?: return null,
                deleted = o["deleted"]?.jsonPrimitive?.booleanOrNull ?: false,
                row = (o["row"] as? JsonObject) ?: JsonObject(emptyMap()),
            )
        }.getOrNull()
    }
}

/** The wire's table names, in the order the PC applies them; settings first. */
object SyncTables {
    const val SETTINGS = "settings"
    const val ACCOUNTS = "accounts"
    const val CATEGORIES = "categories"
    const val RECURRING = "recurring"
    const val GOALS = "goals"
    const val TRANSACTIONS = "transactions"
    const val BUDGETS = "budgets"
    const val CONTRIBUTIONS = "contributions"
    const val VALUES = "account_values"
    const val SECURITIES = "securities"
    const val HOLDINGS = "holdings"
    const val ACTIVITIES = "activities"
    const val PRICES = "prices"
    const val FX_RATES = "fx_rates"
    const val ROOM_FACTS = "room_facts"

    val ORDER = listOf(
        SETTINGS, ACCOUNTS, CATEGORIES, RECURRING, GOALS, TRANSACTIONS, BUDGETS, CONTRIBUTIONS, VALUES,
        SECURITIES, HOLDINGS, ACTIVITIES, PRICES, FX_RATES, ROOM_FACTS,
    )

    fun order(table: String): Int = ORDER.indexOf(table).let { if (it < 0) Int.MAX_VALUE else it }

    /**
     * The prefixes of uids both devices derive the same way: a posted bill's, and what an import
     * writes (docs/INVESTMENTS.md). Such a row is written only when its uid is neither present nor
     * deleted, so a deleted one's tombstone is kept for good (SyncDao.pruneTombstones lists the
     * same prefixes in SQL).
     */
    val DERIVED = listOf("bill:", "imp:", "sec:", "hold:", "px:", "fx:", "room:", "val:", "ws:")

    fun isDerived(uid: String): Boolean = DERIVED.any { uid.startsWith(it) }

    /** The Room table a wire table is kept in; only contributions are named differently. */
    fun room(table: String): String = if (table == CONTRIBUTIONS) "goal_contributions" else table

    fun wire(roomTable: String): String = if (roomTable == "goal_contributions") CONTRIBUTIONS else roomTable
}

/** What applying the PC's changes did: rows taken, and rows older than ours or not placeable here. */
data class Applied(val applied: Int, val skipped: Int)

/**
 * The ledger's side of a sync: the rows changed since the last one, as [Change]s, and the PC's
 * changes merged in. Newest `updated_at` wins per row, a tombstone wins like any edit, and
 * references are uids on the wire and ids here. Budgets are matched by their category.
 */
@Singleton
class LedgerSync @Inject constructor(private val db: TallyDatabase) {

    private val dao: SyncDao get() = db.sync()

    /**
     * Every row stamped at or after [since], and every tombstone, in the order the PC applies
     * them. [since] null is everything there is (a first sync), with no tombstones: the PC is
     * replaced by what is here.
     */
    suspend fun collect(since: Long?): List<Change> = db.withTransaction {
        val from = since ?: Long.MIN_VALUE
        val accounts = dao.accountUids().byId()
        val categories = dao.categoryUids().byId()
        val bills = dao.recurringUids().byId()
        val goals = dao.goalUids().byId()
        val securities = dao.securityUids().byId()
        val out = ArrayList<Change>()
        dao.accountsSince(from).forEach { out += it.change() }
        dao.categoriesSince(from).forEach { out += it.change() }
        dao.recurringSince(from).forEach { r -> r.change(accounts, categories)?.let { out += it } }
        dao.goalsSince(from).forEach { out += it.change(accounts) }
        dao.transactionsSince(from).forEach { t -> t.change(accounts, categories, bills)?.let { out += it } }
        dao.budgetsSince(from).forEach { b -> b.change(categories)?.let { out += it } }
        dao.contributionsSince(from).forEach { c -> c.change(goals)?.let { out += it } }
        dao.valuesSince(from).forEach { v -> v.change(accounts)?.let { out += it } }
        dao.securitiesSince(from).forEach { out += it.change() }
        dao.holdingsSince(from).forEach { h -> h.change(accounts, securities)?.let { out += it } }
        dao.activitiesSince(from).forEach { a -> a.change(accounts, securities)?.let { out += it } }
        dao.pricesSince(from).forEach { p -> p.change(securities)?.let { out += it } }
        dao.fxRatesSince(from).forEach { out += it.change() }
        dao.roomFactsSince(from).forEach { out += it.change() }
        if (since != null) dao.tombstonesSince(since).forEach { out += it.change() }
        out.sortedBy { SyncTables.order(it.table) }
    }

    /** Merges the PC's [changes] in one transaction. Settings are the caller's (they live in DataStore). */
    suspend fun apply(changes: List<Change>): Applied = db.withTransaction {
        dao.beginApplying()
        var applied = 0
        var skipped = 0
        for (c in changes.sortedBy { SyncTables.order(it.table) }) {
            val took = when (c.table) {
                SyncTables.ACCOUNTS -> applyAccount(c)
                SyncTables.CATEGORIES -> applyCategory(c)
                SyncTables.RECURRING -> applyRecurring(c)
                SyncTables.GOALS -> applyGoal(c)
                SyncTables.TRANSACTIONS -> applyTransaction(c)
                SyncTables.BUDGETS -> applyBudget(c)
                SyncTables.CONTRIBUTIONS -> applyContribution(c)
                SyncTables.VALUES -> applyValue(c)
                SyncTables.SECURITIES -> applySecurity(c)
                SyncTables.HOLDINGS -> applyHolding(c)
                SyncTables.ACTIVITIES -> applyActivity(c)
                SyncTables.PRICES -> applyPrice(c)
                SyncTables.FX_RATES -> applyFxRate(c)
                SyncTables.ROOM_FACTS -> applyRoomFact(c)
                else -> continue
            }
            if (took) applied++ else skipped++
        }
        dao.endApplying()
        Applied(applied, skipped)
    }

    /**
     * The PC's ledger, whole, in place of this one (after `money.sync_stale`). One transaction:
     * every row here goes, the PC's arrive with their own stamps and uids, and no tombstone is
     * left for what was removed, since the PC never had those rows in this ledger.
     */
    suspend fun replaceWith(changes: List<Change>): Applied = db.withTransaction {
        dao.beginApplying()
        dao.wipeLedger()
        dao.clearTombstones()
        dao.endApplying()
        val applied = apply(changes.filter { !it.deleted })
        dao.clearTombstones()
        // A derived row deleted on the PC stays deleted here: the poster skips a deleted bill's
        // date, and an import a deleted line.
        changes.filter { it.deleted && SyncTables.isDerived(it.uid) }
            .forEach { dao.insertTombstone(TombstoneEntity(SyncTables.room(it.table), it.uid, it.updatedAt)) }
        applied
    }

    /** Tombstones the PC has been told about; kept until a sync after them succeeds. */
    suspend fun pruneTombstones(before: Long) = dao.pruneTombstones(before)

    // ── Merging one row ──────────────────────────────────────────────────────

    /**
     * The rule every table shares. [local] is the row this change is about (by uid, or by
     * category for a budget). [build] makes the row to write, with the given local id, or null
     * when the change cannot be placed here (a reference to a row this phone does not have).
     */
    private suspend fun <E> merge(
        table: String,
        c: Change,
        local: E?,
        stamp: (E) -> Long,
        idOf: (E) -> Long,
        uidOf: (E) -> String,
        build: suspend (Long) -> E?,
        insert: suspend (E) -> Unit,
        update: suspend (E) -> Unit,
        delete: suspend (Long) -> Unit,
    ): Boolean {
        if (local != null && stamp(local) >= c.updatedAt) return false
        if (c.deleted) {
            if (local != null) {
                delete(idOf(local))
                // The delete trigger wrote a tombstone; the PC is where it came from.
                dao.forgetTombstone(table, uidOf(local))
            }
            // A derived row the PC deleted stays deleted here too, had or not, as the PC keeps the
            // ones this phone deletes: the next import of the same file, or the poster, would bring
            // it back otherwise (docs/INVESTMENTS.md). At the PC's stamp, so it is no news to the PC.
            if (SyncTables.isDerived(c.uid) && (dao.tombstone(table, c.uid)?.deletedAt ?: Long.MIN_VALUE) < c.updatedAt) {
                dao.insertTombstone(TombstoneEntity(table, c.uid, c.updatedAt))
            }
            return true
        }
        if (local == null) {
            // Deleted here after the PC's edit: the delete is newer and stands.
            val buried = dao.tombstone(table, c.uid)
            if (buried != null && buried.deletedAt >= c.updatedAt) return false
            val row = build(0L) ?: return false
            insert(row)
            dao.forgetTombstone(table, c.uid)
        } else {
            val row = build(idOf(local)) ?: return false
            update(row)
        }
        return true
    }

    private suspend fun accountId(uid: String?): Long? = uid?.let { dao.account(it)?.id }
    private suspend fun categoryId(uid: String?): Long? = uid?.let { dao.category(it)?.id }
    private suspend fun securityId(uid: String?): Long? = uid?.let { dao.security(it)?.id }

    private suspend fun applyAccount(c: Change): Boolean = merge(
        "accounts", c, dao.account(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            AccountEntity(
                id = id,
                name = r.str("name") ?: return@merge null,
                type = r.enum<AccountType>("type") ?: return@merge null,
                openingBalance = r.long("openingBalance") ?: 0L,
                archived = r.bool("archived") ?: false,
                sortOrder = r.int("sortOrder") ?: 0,
                // A device from before investments leaves these out: none, and empty.
                registration = r.enum<Registration>("registration"),
                institution = r.str("institution") ?: "",
                externalRef = r.str("externalRef") ?: "",
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertAccount(it) },
        update = { dao.updateAccount(it) },
        delete = { id ->
            dao.forgetAccount(id)
            dao.deleteAccount(id)
        },
    )

    private suspend fun applyCategory(c: Change): Boolean = merge(
        "categories", c, dao.category(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            CategoryEntity(
                id = id,
                name = r.str("name") ?: return@merge null,
                kind = r.enum<CategoryKind>("kind") ?: return@merge null,
                color = r.int("color") ?: 0,
                icon = r.str("icon") ?: "dots",
                archived = r.bool("archived") ?: false,
                sortOrder = r.int("sortOrder") ?: 0,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertCategory(it) },
        update = { dao.updateCategory(it) },
        delete = { id ->
            // As a local delete does: its budget goes with it.
            dao.budgetFor(id)?.let { b -> dao.deleteBudget(b.id) }
            dao.deleteCategory(id)
        },
    )

    private suspend fun applyRecurring(c: Change): Boolean = merge(
        "recurring", c, dao.recurring(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            val type = r.enum<TxType>("type") ?: return@merge null
            val toAccount = r.str("toAccount")?.let { accountId(it) ?: return@merge null }
            if (type == TxType.TRANSFER && toAccount == null) return@merge null
            RecurringEntity(
                id = id,
                name = r.str("name") ?: return@merge null,
                type = type,
                amount = r.long("amount") ?: return@merge null,
                accountId = accountId(r.str("account")) ?: return@merge null,
                toAccountId = toAccount,
                categoryId = categoryId(r.str("category")),
                frequency = r.enum<Frequency>("frequency") ?: return@merge null,
                interval = r.int("interval") ?: 1,
                anchorDate = r.date("anchorDate") ?: return@merge null,
                nextDate = r.date("nextDate") ?: return@merge null,
                endDate = r.date("endDate"),
                autoPost = r.bool("autoPost") ?: true,
                active = r.bool("active") ?: true,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertRecurring(it) },
        update = { dao.updateRecurring(it) },
        delete = { dao.deleteRecurring(it) },
    )

    private suspend fun applyGoal(c: Change): Boolean = merge(
        "goals", c, dao.goal(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            GoalEntity(
                id = id,
                name = r.str("name") ?: return@merge null,
                target = r.long("target") ?: 0L,
                targetDate = r.date("targetDate"),
                color = r.int("color") ?: 0,
                archived = r.bool("archived") ?: false,
                kind = r.enum<GoalKind>("kind") ?: GoalKind.SAVINGS,
                // Not a foreign key here: an account this phone does not have reads as every account.
                accountId = accountId(r.str("account")),
                percent = r.int("percent") ?: 0,
                startDate = r.date("startDate"),
                startAmount = r.long("startAmount") ?: 0L,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertGoal(it) },
        update = { dao.updateGoal(it) },
        delete = { dao.deleteGoal(it) },
    )

    private suspend fun applyTransaction(c: Change): Boolean = merge(
        "transactions", c, dao.transaction(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            val type = r.enum<TxType>("type") ?: return@merge null
            val toAccount = r.str("toAccount")?.let { accountId(it) ?: return@merge null }
            if (type == TxType.TRANSFER && toAccount == null) return@merge null
            TransactionEntity(
                id = id,
                type = type,
                amount = r.long("amount") ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                accountId = accountId(r.str("account")) ?: return@merge null,
                toAccountId = if (type == TxType.TRANSFER) toAccount else null,
                categoryId = if (type == TxType.TRANSFER) null else categoryId(r.str("category")),
                note = r.str("note") ?: "",
                recurringId = r.str("recurring")?.let { dao.recurring(it)?.id },
                createdAt = r.long("createdAt") ?: 0L,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertTransaction(it) },
        update = { dao.updateTransaction(it) },
        delete = { dao.deleteTransaction(it) },
    )

    /**
     * A budget is matched by its category (null is the overall one), whatever uid each device gave
     * it. A tombstone without a row is matched by uid.
     */
    private suspend fun applyBudget(c: Change): Boolean {
        val hasCategory = c.row.containsKey("category")
        val categoryUid = c.row.str("category")
        val categoryId: Long? = when {
            !hasCategory -> null
            categoryUid == null -> BudgetEntity.OVERALL
            else -> categoryId(categoryUid) ?: if (c.deleted) null else return false
        }
        // A live budget is its category's; a tombstone is found by its uid first, then by its
        // category (the PC names it either way).
        val local = when {
            c.deleted -> dao.budget(c.uid) ?: categoryId?.let { dao.budgetFor(it) }
            categoryId != null -> dao.budgetFor(categoryId)
            else -> dao.budget(c.uid)
        }
        return merge(
            "budgets", c, local, { it.updatedAt }, { it.id }, { it.uid },
            build = { id ->
                val category = categoryId ?: return@merge null
                // Take the PC's uid unless another budget here already holds it.
                val uid = dao.budget(c.uid)?.takeIf { it.id != id }?.let { local?.uid ?: return@merge null } ?: c.uid
                BudgetEntity(id = id, categoryId = category, amount = c.row.long("amount") ?: return@merge null, uid = uid, updatedAt = c.updatedAt)
            },
            insert = { dao.insertBudget(it) },
            update = { dao.updateBudget(it) },
            delete = { dao.deleteBudget(it) },
        )
    }

    private suspend fun applyContribution(c: Change): Boolean = merge(
        "goal_contributions", c, dao.contribution(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            ContributionEntity(
                id = id,
                goalId = r.str("goal")?.let { dao.goal(it)?.id } ?: return@merge null,
                amount = r.long("amount") ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                note = r.str("note") ?: "",
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertContribution(it) },
        update = { dao.updateContribution(it) },
        delete = { dao.deleteContribution(it) },
    )

    private suspend fun applyValue(c: Change): Boolean = merge(
        "account_values", c, dao.value(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            AccountValueEntity(
                id = id,
                accountId = accountId(r.str("account")) ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                value = r.long("value") ?: return@merge null,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertValue(it) },
        update = { dao.updateValue(it) },
        delete = { dao.deleteValue(it) },
    )

    // ── Investments (docs/INVESTMENTS.md) ────────────────────────────────────

    private suspend fun applySecurity(c: Change): Boolean = merge(
        "securities", c, dao.security(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            SecurityEntity(
                id = id,
                symbol = r.str("symbol") ?: return@merge null,
                name = r.str("name") ?: "",
                currency = r.str("currency") ?: return@merge null,
                // A kind a newer PC knows and this phone does not still holds its place.
                kind = r.enum<SecurityKind>("kind") ?: SecurityKind.OTHER,
                exchange = r.str("exchange") ?: "",
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertSecurity(it) },
        update = { dao.updateSecurity(it) },
        delete = { dao.deleteSecurity(it) },
    )

    private suspend fun applyHolding(c: Change): Boolean = merge(
        "holdings", c, dao.holding(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            HoldingEntity(
                id = id,
                accountId = accountId(r.str("account")) ?: return@merge null,
                securityId = securityId(r.str("security")) ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                quantity = r.long("quantity") ?: return@merge null,
                book = r.long("book") ?: return@merge null,
                bookMarket = r.long("bookMarket") ?: return@merge null,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertHolding(it) },
        update = { dao.updateHolding(it) },
        delete = { dao.deleteHolding(it) },
    )

    private suspend fun applyActivity(c: Change): Boolean = merge(
        "activities", c, dao.activity(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            // No security is fine (a deposit); one this phone does not have is not.
            val security = r.str("security")?.let { securityId(it) ?: return@merge null }
            ActivityEntity(
                id = id,
                accountId = accountId(r.str("account")) ?: return@merge null,
                securityId = security,
                type = r.enum<ActivityType>("type") ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                quantity = r.long("quantity") ?: 0L,
                amount = r.long("amount") ?: return@merge null,
                fee = r.long("fee") ?: 0L,
                currency = r.str("currency") ?: return@merge null,
                toAmount = r.long("toAmount"),
                toCurrency = r.str("toCurrency"),
                note = r.str("note") ?: "",
                source = r.str("source") ?: "MANUAL",
                createdAt = r.long("createdAt") ?: 0L,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertActivity(it) },
        update = { dao.updateActivity(it) },
        delete = { dao.deleteActivity(it) },
    )

    private suspend fun applyPrice(c: Change): Boolean = merge(
        "prices", c, dao.price(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            PriceEntity(
                id = id,
                securityId = securityId(r.str("security")) ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                price = r.long("price") ?: return@merge null,
                source = r.str("source") ?: "MANUAL",
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertPrice(it) },
        update = { dao.updatePrice(it) },
        delete = { dao.deletePrice(it) },
    )

    private suspend fun applyFxRate(c: Change): Boolean = merge(
        "fx_rates", c, dao.fxRate(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            FxRateEntity(
                id = id,
                base = r.str("base") ?: return@merge null,
                quote = r.str("quote") ?: return@merge null,
                date = r.date("date") ?: return@merge null,
                rate = r.long("rate") ?: return@merge null,
                source = r.str("source") ?: "MANUAL",
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertFxRate(it) },
        update = { dao.updateFxRate(it) },
        delete = { dao.deleteFxRate(it) },
    )

    private suspend fun applyRoomFact(c: Change): Boolean = merge(
        "room_facts", c, dao.roomFact(c.uid), { it.updatedAt }, { it.id }, { it.uid },
        build = { id ->
            val r = c.row
            RoomFactEntity(
                id = id,
                registration = r.enum<Registration>("registration") ?: return@merge null,
                year = r.int("year") ?: return@merge null,
                amount = r.long("amount") ?: return@merge null,
                uid = c.uid,
                updatedAt = c.updatedAt,
            )
        },
        insert = { dao.insertRoomFact(it) },
        update = { dao.updateRoomFact(it) },
        delete = { dao.deleteRoomFact(it) },
    )
}

// ── Rows as the wire carries them ────────────────────────────────────────────

private fun List<IdUid>.byId(): Map<Long, String> = associate { it.id to it.uid }

private fun JsonObject.str(key: String): String? = (this[key] as? JsonPrimitive)?.takeIf { it !is JsonNull }?.contentOrNull

private fun JsonObject.long(key: String): Long? {
    val p = (this[key] as? JsonPrimitive)?.takeIf { it !is JsonNull } ?: return null
    return p.longOrNull ?: p.contentOrNull?.toDoubleOrNull()?.toLong()
}

private fun JsonObject.int(key: String): Int? = long(key)?.toInt()

private fun JsonObject.bool(key: String): Boolean? {
    val p = (this[key] as? JsonPrimitive)?.takeIf { it !is JsonNull } ?: return null
    return p.booleanOrNull ?: p.longOrNull?.let { it != 0L }
}

private fun JsonObject.date(key: String): LocalDate? = str(key)?.let { runCatching { LocalDate.parse(it) }.getOrNull() }

private inline fun <reified T : Enum<T>> JsonObject.enum(key: String): T? =
    str(key)?.let { name -> enumValues<T>().firstOrNull { it.name == name } }

private fun change(table: String, uid: String, updatedAt: Long, row: JsonObject) = Change(table, uid, updatedAt, false, row)

private fun AccountEntity.change() = change(SyncTables.ACCOUNTS, uid, updatedAt, buildJsonObject {
    put("name", name)
    put("type", type.name)
    put("openingBalance", openingBalance)
    put("archived", archived)
    put("sortOrder", sortOrder)
    put("registration", registration?.name)
    put("institution", institution)
    put("externalRef", externalRef)
})

private fun CategoryEntity.change() = change(SyncTables.CATEGORIES, uid, updatedAt, buildJsonObject {
    put("name", name)
    put("kind", kind.name)
    put("color", color)
    put("icon", icon)
    put("archived", archived)
    put("sortOrder", sortOrder)
})

/** Null when a reference the row needs has no uid (it cannot be placed on the PC either). */
private fun RecurringEntity.change(accounts: Map<Long, String>, categories: Map<Long, String>): Change? {
    val account = accounts[accountId] ?: return null
    return change(SyncTables.RECURRING, uid, updatedAt, buildJsonObject {
        put("name", name)
        put("type", type.name)
        put("amount", amount)
        put("account", account)
        put("toAccount", toAccountId?.let { accounts[it] })
        put("category", categoryId?.let { categories[it] })
        put("frequency", frequency.name)
        put("interval", interval)
        put("anchorDate", anchorDate.toString())
        put("nextDate", nextDate.toString())
        put("endDate", endDate?.toString())
        put("autoPost", autoPost)
        put("active", active)
    })
}

private fun GoalEntity.change(accounts: Map<Long, String>) = change(SyncTables.GOALS, uid, updatedAt, buildJsonObject {
    put("name", name)
    put("target", target)
    put("targetDate", targetDate?.toString())
    put("color", color)
    put("archived", archived)
    put("kind", kind.name)
    put("account", accountId?.let { accounts[it] })
    put("percent", percent)
    put("startDate", startDate?.toString())
    put("startAmount", startAmount)
})

private fun TransactionEntity.change(accounts: Map<Long, String>, categories: Map<Long, String>, bills: Map<Long, String>): Change? {
    val account = accounts[accountId] ?: return null
    return change(SyncTables.TRANSACTIONS, uid, updatedAt, buildJsonObject {
        put("type", type.name)
        put("amount", amount)
        put("date", date.toString())
        put("account", account)
        put("toAccount", toAccountId?.let { accounts[it] })
        put("category", categoryId?.let { categories[it] })
        put("note", note)
        put("recurring", recurringId?.let { bills[it] })
        put("createdAt", createdAt)
    })
}

/** The overall budget travels with a null category. One whose category is gone is not sent. */
private fun BudgetEntity.change(categories: Map<Long, String>): Change? {
    val category = if (categoryId == BudgetEntity.OVERALL) null else categories[categoryId] ?: return null
    return change(SyncTables.BUDGETS, uid, updatedAt, buildJsonObject {
        put("category", category)
        put("amount", amount)
    })
}

private fun ContributionEntity.change(goals: Map<Long, String>): Change? {
    val goal = goals[goalId] ?: return null
    return change(SyncTables.CONTRIBUTIONS, uid, updatedAt, buildJsonObject {
        put("goal", goal)
        put("amount", amount)
        put("date", date.toString())
        put("note", note)
    })
}

private fun AccountValueEntity.change(accounts: Map<Long, String>): Change? {
    val account = accounts[accountId] ?: return null
    return change(SyncTables.VALUES, uid, updatedAt, buildJsonObject {
        put("account", account)
        put("date", date.toString())
        put("value", value)
    })
}

private fun SecurityEntity.change() = change(SyncTables.SECURITIES, uid, updatedAt, buildJsonObject {
    put("symbol", symbol)
    put("name", name)
    put("currency", currency)
    put("kind", kind.name)
    put("exchange", exchange)
})

private fun HoldingEntity.change(accounts: Map<Long, String>, securities: Map<Long, String>): Change? {
    val account = accounts[accountId] ?: return null
    val security = securities[securityId] ?: return null
    return change(SyncTables.HOLDINGS, uid, updatedAt, buildJsonObject {
        put("account", account)
        put("security", security)
        put("date", date.toString())
        put("quantity", quantity)
        put("book", book)
        put("bookMarket", bookMarket)
    })
}

private fun ActivityEntity.change(accounts: Map<Long, String>, securities: Map<Long, String>): Change? {
    val account = accounts[accountId] ?: return null
    val security = securityId?.let { securities[it] ?: return null }
    return change(SyncTables.ACTIVITIES, uid, updatedAt, buildJsonObject {
        put("account", account)
        put("security", security)
        put("type", type.name)
        put("date", date.toString())
        put("quantity", quantity)
        put("amount", amount)
        put("fee", fee)
        put("currency", currency)
        put("toAmount", toAmount)
        put("toCurrency", toCurrency)
        put("note", note)
        put("source", source)
        put("createdAt", createdAt)
    })
}

private fun PriceEntity.change(securities: Map<Long, String>): Change? {
    val security = securities[securityId] ?: return null
    return change(SyncTables.PRICES, uid, updatedAt, buildJsonObject {
        put("security", security)
        put("date", date.toString())
        put("price", price)
        put("source", source)
    })
}

private fun FxRateEntity.change() = change(SyncTables.FX_RATES, uid, updatedAt, buildJsonObject {
    put("base", base)
    put("quote", quote)
    put("date", date.toString())
    put("rate", rate)
    put("source", source)
})

private fun RoomFactEntity.change() = change(SyncTables.ROOM_FACTS, uid, updatedAt, buildJsonObject {
    put("registration", registration.name)
    put("year", year)
    put("amount", amount)
})

/** A budget's tombstone names its category, which is how the PC finds it; the rest carry none. */
private fun TombstoneEntity.change(): Change {
    val table = SyncTables.wire(tableName)
    val row = if (table == SyncTables.BUDGETS) buildJsonObject { put("category", category) } else JsonObject(emptyMap())
    return Change(table, uid, deletedAt, deleted = true, row = row)
}
