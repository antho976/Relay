package com.tally.app.data.repo

import androidx.room.withTransaction
import com.tally.app.data.Clock
import com.tally.app.data.db.AccountEntity
import com.tally.app.data.db.AccountValueEntity
import com.tally.app.data.db.ActivityEntity
import com.tally.app.data.db.HoldingEntity
import com.tally.app.data.db.InvestDao
import com.tally.app.data.db.PriceEntity
import com.tally.app.data.db.RoomFactEntity
import com.tally.app.data.db.SecurityEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.prefs.SettingsRepository
import com.tally.core.AccountType
import com.tally.core.ActivityRow
import com.tally.core.FxRateRow
import com.tally.core.HoldingRow
import com.tally.core.ImportPlan
import com.tally.core.Invest
import com.tally.core.InvestAccountRow
import com.tally.core.PlanInput
import com.tally.core.Portfolio
import com.tally.core.PortfolioInput
import com.tally.core.PriceRow
import com.tally.core.Registration
import com.tally.core.RoomFactRow
import com.tally.core.SecurityRow
import com.tally.core.TransferRow
import com.tally.core.Wealthsimple
import com.tally.core.WsAccount
import com.tally.core.WsFile
import com.tally.core.WsKind
import com.tally.core.WsRead
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import java.time.LocalDate
import javax.inject.Inject
import javax.inject.Singleton

/** One account a Wealthsimple file names, and where its lines would go. */
data class PreviewAccount(
    /** Wealthsimple's number for it ("HQ7XFMC41CAD"). */
    val number: String,
    /** The file's name for it, or "Wealthsimple TFSA" when the file gives none. */
    val name: String,
    val registration: Registration,
    /** The account already holding [number]; null when the import would make a new one. */
    val accountId: Long?,
    /** How many of the file's lines are this account's. */
    val rows: Int,
)

/** What importing a file would do, read before anything is written. */
data class ImportPreviewData(
    val kind: WsKind,
    /** A holdings report's day; null for an activities export or a statement. */
    val asOf: LocalDate?,
    /** In the order the file names them; a statement names none. */
    val accounts: List<PreviewAccount>,
    /** The snapshot lines and the activities the file holds. */
    val holdings: Int,
    val activities: Int,
    /** Of those, the lines not in the ledger yet... */
    val new: Int,
    /** ...and the ones already in it, or deleted from it, which an import never brings back. */
    val duplicates: Int,
    /** The lines that could not be read, each as "Line 12: Options aren't tracked yet". */
    val skipped: List<String>,
)

/** What an import wrote. */
data class ImportCounts(
    val accountsCreated: Int,
    val securities: Int,
    val holdings: Int,
    val activities: Int,
    /** Lines left out because they were already in, or were deleted. */
    val duplicates: Int,
    /** Prices and account values written or changed; one already so is not counted. */
    val prices: Int,
    val values: Int,
    /** Lines that could not be read. */
    val skipped: Int,
)

/** Why a file was not previewed or imported, worded to be shown as it is. Nothing was written. */
class ImportRefused(reason: String) : IllegalArgumentException(reason)

/**
 * The investments (docs/INVESTMENTS.md): the portfolio read from the ledger's rows, the Wealthsimple
 * import, and the room and registration the owner sets. The rules are core's ([Invest],
 * [Wealthsimple]), the PC's twins; this only reads and writes rows. An import writes under the uids
 * the plan derives, so a file imported here and on the PC merges on sync, and a line goes in only
 * when its uid is neither here nor deleted: importing a file twice adds it once.
 */
@Singleton
class InvestRepository @Inject constructor(
    private val db: TallyDatabase,
    private val settings: SettingsRepository,
    private val clock: Clock,
) {
    private val dao: InvestDao get() = db.invest()

    // ── The portfolio ────────────────────────────────────────────────────────

    /**
     * The portfolio as of today, read again whenever a table it rests on or the currency changes.
     * Every row is read in one transaction, so an import is seen whole or not at all. A ledger
     * whose figures do not fit a Long fails the flow with [ArithmeticException] (Invest's rule).
     */
    fun portfolio(): Flow<Portfolio> =
        combine(
            db.invalidationTracker.createFlow(*PORTFOLIO_TABLES, emitInitialState = true),
            settings.settings.map { it.currency }.distinctUntilChanged(),
        ) { _, currency -> currency }
            .map { currency -> Invest.portfolio(input(currency, clock.today())) }
            .flowOn(Dispatchers.Default)

    private suspend fun input(currency: String, today: LocalDate): PortfolioInput = db.withTransaction {
        PortfolioInput(
            currency = currency,
            today = today,
            // As on the PC: the open investment accounts; their rows come whatever they hold.
            accounts = db.accounts().all().filter { it.type == AccountType.INVESTMENT && !it.archived }.map {
                InvestAccountRow(id = it.id, uid = it.uid, name = it.name, registration = it.registration, institution = it.institution)
            },
            securities = dao.securities().map {
                SecurityRow(id = it.id, uid = it.uid, symbol = it.symbol, name = it.name, currency = it.currency, kind = it.kind, exchange = it.exchange)
            },
            holdings = dao.holdings().map {
                HoldingRow(accountId = it.accountId, securityId = it.securityId, date = it.date, quantity = it.quantity, book = it.book, bookMarket = it.bookMarket)
            },
            activities = dao.activities().map {
                ActivityRow(
                    id = it.id, uid = it.uid, accountId = it.accountId, securityId = it.securityId, type = it.type, date = it.date,
                    quantity = it.quantity, amount = it.amount, fee = it.fee, currency = it.currency,
                    toAmount = it.toAmount, toCurrency = it.toCurrency,
                )
            },
            prices = dao.prices().map { PriceRow(securityId = it.securityId, date = it.date, price = it.price) },
            fxRates = fxRates(),
            roomFacts = dao.roomFacts().map { RoomFactRow(registration = it.registration, year = it.year, amount = it.amount) },
            transfers = dao.investmentTransfers().map { TransferRow(accountId = it.accountId, date = it.date, amount = it.amount) },
        )
    }

    private suspend fun fxRates(): List<FxRateRow> =
        dao.fxRates().map { FxRateRow(base = it.base, quote = it.quote, date = it.date, rate = it.rate) }

    // ── Wealthsimple files ───────────────────────────────────────────────────

    /**
     * What importing [text] would do: its accounts, each with the account already holding its
     * number, and how many of its lines are new or already in. A statement names no account, so
     * until one is chosen its lines all count as new. Throws [ImportRefused] for a file that is
     * not one of Wealthsimple's investment files, or one this ledger cannot take.
     */
    suspend fun preview(text: String): ImportPreviewData = refusing(PREVIEW_FAILED) {
        val file = read(text)
        val currency = settings.current().currency
        db.withTransaction {
            val found = file.accounts.associate { it.number to db.accounts().byExternalRef(it.number) }
            val plan = plan(
                file,
                PlanInput(
                    currency = currency,
                    accounts = found.mapNotNull { (number, account) -> account?.let { number to it.uid } }.toMap(),
                    statementAccount = if (file.kind == WsKind.STATEMENT) "" else null,
                    fxRates = fxRates(),
                ),
            )
            val duplicates = plan.holdings.count { holdingKnown(it.uid) } + plan.activities.count { activityKnown(it.uid) }
            ImportPreviewData(
                kind = file.kind,
                asOf = file.asOf,
                accounts = file.accounts.map { a ->
                    PreviewAccount(a.number, a.name, a.registration, accountId = found[a.number]?.id, rows = rowsOf(file, a.number))
                },
                holdings = plan.holdings.size,
                activities = plan.activities.size,
                new = plan.holdings.size + plan.activities.size - duplicates,
                duplicates = duplicates,
                skipped = plan.skipped.map { "Line ${it.line}: ${it.reason}" },
            )
        }
    }

    /**
     * Imports [text] in one transaction: a failure writes nothing. Each Wealthsimple account goes
     * where [mapping] says (by its number): an id into that investment account, which then keeps
     * the number so the next import finds it; null into a new account. A number [mapping] leaves
     * out goes into the account already holding it, else a new one. A statement goes into
     * [statementAccountId]. Throws [ImportRefused] with the reason to show.
     */
    suspend fun import(text: String, mapping: Map<String, Long?>, statementAccountId: Long?): ImportCounts = refusing(IMPORT_FAILED) {
        val file = read(text)
        val currency = settings.current().currency
        db.withTransaction {
            val accounts = HashMap<String, String>()
            val remember = ArrayList<Pair<AccountEntity, WsAccount>>()
            file.accounts.forEach { a ->
                if (a.number in mapping) {
                    val id = mapping[a.number] ?: return@forEach
                    val account = investment(id)
                    accounts[a.number] = account.uid
                    remember += account to a
                } else {
                    db.accounts().byExternalRef(a.number)?.let { accounts[a.number] = it.uid }
                }
            }
            val statementAccount = if (file.kind == WsKind.STATEMENT) {
                investment(statementAccountId ?: throw ImportRefused("Choose the account this statement belongs to")).uid
            } else {
                null
            }
            val plan = plan(file, PlanInput(currency = currency, accounts = accounts, statementAccount = statementAccount, fxRates = fxRates()))
            // An account chosen by hand keeps the number, the institution and the registration.
            remember.forEach { (account, ws) ->
                val kept = account.copy(
                    externalRef = ws.number,
                    institution = account.institution.ifBlank { Wealthsimple.INSTITUTION },
                    registration = account.registration ?: ws.registration,
                )
                if (kept != account) db.accounts().update(kept)
            }
            write(plan, file.kind)
        }
    }

    /** Writes [plan]. Runs inside the caller's transaction. */
    private suspend fun write(plan: ImportPlan, kind: WsKind): ImportCounts {
        val now = clock.nowMillis()
        var created = 0
        plan.accounts.forEach { a ->
            val there = db.accounts().byUid(a.uid)
            if (there == null) {
                db.accounts().insert(
                    AccountEntity(
                        name = a.name, type = AccountType.INVESTMENT, sortOrder = db.accounts().nextSortOrder(),
                        registration = a.registration, institution = a.institution, externalRef = a.externalRef, uid = a.uid,
                    )
                )
                created++
            } else {
                val kept = there.copy(type = AccountType.INVESTMENT, registration = a.registration, institution = a.institution, externalRef = a.externalRef)
                if (kept != there) db.accounts().update(kept)
            }
        }
        val accountIds = HashMap<String, Long>()
        suspend fun accountId(uid: String): Long = accountIds.getOrPut(uid) { db.accounts().byUid(uid)?.id ?: error("No account $uid") }

        var securities = 0
        val securityIds = HashMap<String, Long>()
        plan.securities.forEach { s ->
            val found = dao.security(s.uid) ?: dao.securityBySymbol(s.symbol)
            securityIds[s.uid] = when {
                found == null -> {
                    securities++
                    dao.insertSecurity(SecurityEntity(symbol = s.symbol, name = s.name, currency = s.currency, kind = s.kind, exchange = s.exchange, uid = s.uid))
                }
                // A holdings report knows a security's name, currency, kind and exchange; an activity does not.
                kind == WsKind.HOLDINGS -> {
                    val kept = found.copy(name = s.name, currency = s.currency, kind = s.kind, exchange = s.exchange)
                    if (kept != found) dao.updateSecurity(kept)
                    found.id
                }
                else -> found.id
            }
        }

        var holdings = 0
        var activities = 0
        var duplicates = 0
        plan.holdings.forEach { h ->
            if (db.sync().tombstone("holdings", h.uid) != null) {
                duplicates++
                return@forEach
            }
            val id = dao.insertHoldingOrIgnore(
                HoldingEntity(
                    accountId = accountId(h.accountUid), securityId = securityIds.getValue(h.securityUid), date = h.date,
                    quantity = h.quantity, book = h.book, bookMarket = h.bookMarket, uid = h.uid,
                )
            )
            if (id == -1L) duplicates++ else holdings++
        }
        plan.activities.forEach { a ->
            if (db.sync().tombstone("activities", a.uid) != null) {
                duplicates++
                return@forEach
            }
            val id = dao.insertActivityOrIgnore(
                ActivityEntity(
                    accountId = accountId(a.accountUid), securityId = a.securityUid?.let { securityIds.getValue(it) }, type = a.type,
                    date = a.date, quantity = a.quantity, amount = a.amount, fee = a.fee, currency = a.currency,
                    toAmount = a.toAmount, toCurrency = a.toCurrency, note = a.note, source = Wealthsimple.SOURCE, createdAt = now, uid = a.uid,
                )
            )
            if (id == -1L) duplicates++ else activities++
        }

        var prices = 0
        plan.prices.forEach { p ->
            val securityId = securityIds.getValue(p.securityUid)
            val there = dao.price(p.uid)
            if (there == null) {
                dao.insertPrice(PriceEntity(securityId = securityId, date = p.date, price = p.price, source = Wealthsimple.PRICE_SOURCE, uid = p.uid))
                prices++
            } else {
                val kept = there.copy(securityId = securityId, date = p.date, price = p.price, source = Wealthsimple.PRICE_SOURCE)
                if (kept != there) {
                    dao.updatePrice(kept)
                    prices++
                }
            }
        }

        // The report's value stands in for the account's balance up to its day (the account_values
        // rule), so Home, net worth and goals see the import with nothing of their own.
        var values = 0
        plan.values.forEach { v ->
            val accountId = accountId(v.accountUid)
            val there = db.values().byUid(v.uid)
            if (there == null) {
                db.values().insert(AccountValueEntity(accountId = accountId, date = v.date, value = v.value, uid = v.uid))
                values++
            } else {
                val kept = there.copy(accountId = accountId, date = v.date, value = v.value)
                if (kept != there) {
                    db.values().update(kept)
                    values++
                }
            }
            // One value per account per day, as the editor keeps them: the report's takes the day.
            db.values().deleteOthersOn(accountId, v.date, v.uid)
        }

        return ImportCounts(
            accountsCreated = created,
            securities = securities,
            holdings = holdings,
            activities = activities,
            duplicates = duplicates,
            prices = prices,
            values = values,
            skipped = plan.skipped.size,
        )
    }

    private fun read(text: String): WsFile = when (val r = Wealthsimple.read(text)) {
        is WsRead.Ok -> r.file
        is WsRead.Invalid -> throw ImportRefused(r.reason)
    }

    private fun plan(file: WsFile, input: PlanInput): ImportPlan =
        Wealthsimple.plan(file, input).also { p -> p.refused?.let { throw ImportRefused(it) } }

    /** An investment account by id, or a refusal: lines go into nothing else. */
    private suspend fun investment(id: Long): AccountEntity =
        db.accounts().get(id)?.takeIf { it.type == AccountType.INVESTMENT } ?: throw ImportRefused("Pick an investment account")

    /** A line under [uid] is in the ledger, or was deleted from it: an import never adds it. */
    private suspend fun holdingKnown(uid: String): Boolean = dao.holding(uid) != null || db.sync().tombstone("holdings", uid) != null

    private suspend fun activityKnown(uid: String): Boolean = dao.activity(uid) != null || db.sync().tombstone("activities", uid) != null

    private fun rowsOf(file: WsFile, number: String): Int =
        file.holdings.count { it.account == number } + file.activities.count { it.account == number }

    /**
     * Runs [block] on the IO dispatcher. Anything but a refusal (a read or a write that failed)
     * becomes one saying [failed], so the caller always has a sentence to show; any transaction
     * has rolled back by then.
     */
    private suspend fun <T> refusing(failed: String, block: suspend () -> T): T = withContext(Dispatchers.IO) {
        try {
            block()
        } catch (e: ImportRefused) {
            throw e
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            throw ImportRefused(failed)
        }
    }

    // ── What the owner sets ──────────────────────────────────────────────────

    /**
     * The room CRA gives [registration] in [year] (a TFSA's, an RRSP's or an FHSA's), in minor
     * units of the ledger currency. Zero or less removes it.
     */
    suspend fun setRoom(registration: Registration, year: Int, amount: Long) {
        require(registration in ROOM) { "Room is kept for a TFSA, an RRSP or an FHSA" }
        require(year in 2009..2200) { "Not a year for room: $year" }
        val uid = "room:${registration.name}:$year"
        withContext(Dispatchers.IO) {
            db.withTransaction<Unit> {
                val there = dao.roomFact(uid)
                when {
                    there == null && amount > 0 -> dao.insertRoomFact(RoomFactEntity(registration = registration, year = year, amount = amount, uid = uid))
                    there == null -> Unit
                    amount <= 0 -> dao.deleteRoomFact(there.id)
                    there.amount != amount -> dao.updateRoomFact(there.copy(amount = amount))
                    else -> Unit
                }
            }
        }
    }

    /**
     * An account's registration (null for none) and institution. Only an investment account has a
     * registration; clearing it is always allowed.
     */
    suspend fun setRegistration(accountId: Long, registration: Registration?, institution: String) {
        withContext(Dispatchers.IO) {
            db.withTransaction<Unit> {
                val account = db.accounts().get(accountId) ?: return@withTransaction
                require(registration == null || account.type == AccountType.INVESTMENT) { "Only an investment account has a registration" }
                val kept = account.copy(registration = registration, institution = institution.trim())
                if (kept != account) db.accounts().update(kept)
            }
        }
    }

    companion object {
        private const val PREVIEW_FAILED = "That file could not be read."
        private const val IMPORT_FAILED = "The import did not finish. Nothing was added."

        /** The registrations Tally keeps room for. */
        private val ROOM = setOf(Registration.TFSA, Registration.RRSP, Registration.FHSA)

        /** What the portfolio is read from: a change to any of them reads it again. */
        private val PORTFOLIO_TABLES = arrayOf("accounts", "transactions", "securities", "holdings", "activities", "prices", "fx_rates", "room_facts")
    }
}
