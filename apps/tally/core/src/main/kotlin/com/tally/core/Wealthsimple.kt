package com.tally.core

import java.time.DateTimeException
import java.time.LocalDate
import kotlin.math.abs
import kotlin.math.max

/** Which of Wealthsimple's three investment files a text is. */
enum class WsKind { HOLDINGS, ACTIVITIES, STATEMENT }

/** An account a Wealthsimple file names, by its number ("HQ7XFMC41CAD"). */
data class WsAccount(val number: String, val name: String, val registration: Registration)

/**
 * One line of a holdings report. [quantity] is at [Invest.QTY_SCALE] and [price] at
 * [Invest.PRICE_SCALE]; [book] is in Canadian cents, [bookMarket] and [marketValue] minor units of
 * [currency], the security's own.
 */
data class WsHolding(
    val line: Int,
    val account: String,
    val symbol: String,
    val name: String,
    val exchange: String,
    val kind: SecurityKind,
    val currency: String,
    val quantity: Long,
    val price: Long?,
    val book: Long,
    val bookMarket: Long,
    val marketValue: Long?,
)

/**
 * One activity as the file gives it, already in Tally's types: [quantity], [amount] and [fee] are
 * never negative, [type] gives the direction. [account] is the account's number, null on a
 * statement, which names none. [price] is the unit price the file wrote, at [Invest.PRICE_SCALE].
 */
data class WsActivity(
    val line: Int,
    val account: String?,
    val date: LocalDate,
    val type: ActivityType,
    val symbol: String?,
    val name: String,
    val currency: String,
    val quantity: Long,
    val price: Long?,
    val amount: Long,
    val fee: Long,
    val toAmount: Long? = null,
    val toCurrency: String? = null,
    val note: String = "",
)

/** A line the reader left out, and why. Lines count from 1 at the header. */
data class WsSkipped(val line: Int, val reason: String)

/** A file read whole. [asOf] is the holdings report's "As of" day; null for the other two. */
data class WsFile(
    val kind: WsKind,
    val asOf: LocalDate?,
    /** In the order they first appear. */
    val accounts: List<WsAccount>,
    val holdings: List<WsHolding>,
    val activities: List<WsActivity>,
    val skipped: List<WsSkipped>,
)

sealed interface WsRead {
    data class Ok(val file: WsFile) : WsRead

    /** [reason] is shown as-is, so it names the problem and not the parser. */
    data class Invalid(val reason: String) : WsRead
}

/**
 * What [Wealthsimple.plan] needs besides the file. [accounts] maps a Wealthsimple account number to
 * the uid of the Tally account it goes into; a number not in it gets `ws:<number>`, the account the
 * import creates. [statementAccount] is the uid of the account a statement belongs to.
 */
data class PlanInput(
    val currency: String,
    val accounts: Map<String, String> = emptyMap(),
    val statementAccount: String? = null,
    val fxRates: List<FxRateRow> = emptyList(),
)

/** An INVESTMENT account the import creates: uid `ws:<number>`, [externalRef] the number. */
data class PlanAccount(val uid: String, val name: String, val registration: Registration, val institution: String, val externalRef: String)

data class PlanSecurity(val uid: String, val symbol: String, val name: String, val currency: String, val kind: SecurityKind, val exchange: String)

data class PlanHolding(
    val uid: String,
    val accountUid: String,
    val securityUid: String,
    val date: LocalDate,
    val quantity: Long,
    val book: Long,
    val bookMarket: Long,
)

data class PlanActivity(
    val uid: String,
    val line: Int,
    val accountUid: String,
    val securityUid: String?,
    val type: ActivityType,
    val date: LocalDate,
    val quantity: Long,
    val amount: Long,
    val fee: Long,
    val currency: String,
    val toAmount: Long?,
    val toCurrency: String?,
    val note: String,
)

/** A holding's market price on the report's day; `prices.source` is [Wealthsimple.PRICE_SOURCE]. */
data class PlanPrice(val uid: String, val securityUid: String, val date: LocalDate, val price: Long)

/** An account's market value in Canadian cents on the report's day: the `account_values` row the import records. */
data class PlanValue(val uid: String, val accountUid: String, val date: LocalDate, val value: Long)

/**
 * The rows an import writes, in file order, each with a uid both devices derive the same way, so a
 * file imported on the phone and on the PC adds its lines once. References are uids; a row goes in
 * only when its uid is neither present nor deleted. [accounts] are the ones to create (numbers
 * [PlanInput.accounts] does not map). [refused] says why nothing can be written; every list is
 * empty then.
 */
data class ImportPlan(
    val kind: WsKind? = null,
    val accounts: List<PlanAccount> = emptyList(),
    val securities: List<PlanSecurity> = emptyList(),
    val holdings: List<PlanHolding> = emptyList(),
    val activities: List<PlanActivity> = emptyList(),
    val prices: List<PlanPrice> = emptyList(),
    val values: List<PlanValue> = emptyList(),
    val skipped: List<WsSkipped> = emptyList(),
    val refused: String? = null,
)

/**
 * Wealthsimple's own files: the holdings report, the activities export and the monthly statements.
 * Wealthsimple has no public API for a person, so the files its web app exports are the bridge, and
 * nothing leaves the person's devices. [read] reads any of the three; [plan] turns one into the rows
 * the ledger writes. Columns are found by their header name, in any case and order; unknown columns
 * are ignored. A twin of `wealthsimple.rs`.
 */
object Wealthsimple {

    /** What a cash statement is told: it belongs to the bank import. */
    const val NOT_INVESTMENT = "not an investment file"

    const val INSTITUTION = "Wealthsimple"

    /** `activities.source` for an imported line, and `prices.source` for a report's price. */
    const val SOURCE = "WEALTHSIMPLE"
    const val PRICE_SOURCE = "IMPORT"

    private const val OPTIONS = "Options aren't tracked yet"

    /** How many leading lines may come before the header. */
    private const val HEADER_SCAN = 12

    /** The codes that make a monthly statement an investment account's. */
    private val INVESTMENT_CODES = setOf("BUY", "SELL", "DIV", "CONT", "NRT", "FPLINT", "LOAN", "RECALL")

    private val OPTION_TRADES = setOf("STO", "BTO", "STC", "BTC")

    /**
     * "XEQT - iShares Core Equity ETF Portfolio: Bought 10.0000 shares at $38.12 per share". Spaces
     * and tabs are spelled out so the Rust reads the same descriptions.
     */
    private val TRADE = Regex("""^([A-Za-z0-9.]+)[ \t]*-[ \t]*[^\n\r]+:[ \t]*(Bought|Sold)[ \t]+([0-9.,]+)[ \t]+shares?[ \t]+at[ \t]+\$([0-9.,]+)""")

    /** Reads a holdings report, an activities export or an investment account's monthly statement. A byte order mark and CRLF are fine. */
    fun read(text: String): WsRead {
        val records = CsvCodec.parseRecords(text).map { r -> r.map { it.trim() } }
        if (records.isEmpty()) return WsRead.Invalid("The file is empty.")
        records.take(HEADER_SCAN).forEachIndexed { at, record ->
            val header = record.map { it.lowercase() }
            fun has(name: String) = name in header
            val body = records.drop(at + 1)
            if (has("symbol") && has("quantity") && (has("book value (cad)") || has("market value"))) return holdings(header, body)
            if (has("transaction_date") && has("activity_type")) return activities(header, body)
            if (has("date") && has("transaction") && has("amount")) return statement(header, body)
        }
        return WsRead.Invalid("This is not a Wealthsimple holdings report, activities export or monthly statement.")
    }

    /** The cell under [name] on [row]; "" when the column or the cell is missing. */
    private fun cell(header: List<String>, row: List<String>, name: String): String {
        val i = header.indexOf(name)
        return if (i >= 0) row.getOrNull(i).orEmpty() else ""
    }

    private fun currencyOrCad(text: String): String {
        val c = text.uppercase()
        return if (Invest.isCurrency(c)) c else "CAD"
    }

    private fun isoDate(text: String): LocalDate? =
        if (text.length != 10) null else try {
            LocalDate.parse(text)
        } catch (e: DateTimeException) {
            null
        }

    /** An account type as Wealthsimple writes it, in English or French, to a registration: the first name it contains, lower-cased. */
    fun registrationOf(accountType: String): Registration {
        val t = accountType.lowercase()
        return REGISTRATIONS.firstOrNull { (names, _) -> names.any { t.contains(it) } }?.second ?: Registration.OTHER
    }

    private val REGISTRATIONS = listOf(
        listOf("fhsa", "celiapp") to Registration.FHSA,
        listOf("tfsa", "celi") to Registration.TFSA,
        listOf("rrsp", "reer") to Registration.RRSP,
        listOf("resp", "reee") to Registration.RESP,
        listOf("lira", "cri") to Registration.LIRA,
        listOf("rrif", "ferr") to Registration.RRIF,
        listOf(
            "non-registered", "non registered", "non_registered", "non enregistré", "personal", "individual",
            "joint", "margin", "cash", "crypto",
        ) to Registration.NON_REGISTERED,
    )

    /** A security type as the holdings report writes it; null for an option, which is not tracked. */
    fun kindOf(securityType: String): SecurityKind? = when (securityType.uppercase()) {
        "OPTION" -> null
        "EQUITY" -> SecurityKind.STOCK
        "EXCHANGE_TRADED_FUND" -> SecurityKind.ETF
        "MUTUAL_FUND" -> SecurityKind.MUTUAL_FUND
        "BOND", "FIXED_INCOME" -> SecurityKind.BOND
        "CRYPTOCURRENCY", "CRYPTO" -> SecurityKind.CRYPTO
        "CASH" -> SecurityKind.CASH
        else -> SecurityKind.OTHER
    }

    private fun noteAccount(accounts: MutableList<WsAccount>, number: String, name: String, registration: Registration) {
        if (accounts.none { it.number == number }) {
            accounts += WsAccount(number, name.ifEmpty { "Wealthsimple " + registration.label }, registration)
        }
    }

    // ── The holdings report ──────────────────────────────────────────────────

    private fun holdings(header: List<String>, body: List<List<String>>): WsRead {
        var asOf: LocalDate? = null
        val accounts = ArrayList<WsAccount>()
        val holdings = ArrayList<WsHolding>()
        val skipped = ArrayList<WsSkipped>()
        body.forEachIndexed { i, row ->
            val line = i + 2
            fun at(name: String) = cell(header, row, name)
            // The footer: "As of 2026-05-08 12:00 GMT-04:00".
            val first = row.firstOrNull { it.isNotBlank() }.orEmpty()
            if (first.lowercase().startsWith("as of ")) {
                asOf = isoDate(first.substring(6).trim().take(10))
                return@forEachIndexed
            }
            val symbol = at("symbol").uppercase()
            val number = at("account number")
            if (symbol.isEmpty() || number.isEmpty()) {
                skipped += WsSkipped(line, "A line without an account number or a symbol")
                return@forEachIndexed
            }
            val kind = kindOf(at("security type"))
            if (kind == null) {
                skipped += WsSkipped(line, OPTIONS)
                return@forEachIndexed
            }
            val quantity = Invest.parseScaled(at("quantity"), 8)
            if (quantity == null) {
                skipped += WsSkipped(line, "The quantity \"${at("quantity")}\" could not be read")
                return@forEachIndexed
            }
            val currency = listOf(at("market price currency"), at("market value currency"), at("book value currency (market)"))
                .firstOrNull { it.isNotEmpty() }?.let(::currencyOrCad) ?: "CAD"
            fun money(name: String, currency: String) = Invest.parseScaled(at(name), Invest.fractionDigits(currency))?.value
            val book = money("book value (cad)", "CAD") ?: if (currency == "CAD") money("book value (market)", "CAD") else null
            if (book == null) {
                skipped += WsSkipped(line, "The book value \"${at("book value (cad)")}\" could not be read")
                return@forEachIndexed
            }
            noteAccount(accounts, number, at("account name"), registrationOf(at("account type")))
            holdings += WsHolding(
                line = line,
                account = number,
                symbol = symbol,
                name = at("name"),
                exchange = at("exchange"),
                kind = kind,
                currency = currency,
                quantity = quantity.value,
                price = Invest.parseScaled(at("market price"), 8)?.value,
                book = book,
                bookMarket = money("book value (market)", currency) ?: book,
                marketValue = money("market value", currency),
            )
        }
        return WsRead.Ok(WsFile(WsKind.HOLDINGS, asOf, accounts, holdings, emptyList(), skipped))
    }

    // ── The activities export ────────────────────────────────────────────────

    /** One side of a currency exchange, waiting for the other. */
    private class Leg(val line: Int, val account: String, val date: LocalDate, val currency: String, val net: Long)

    private fun activities(header: List<String>, body: List<List<String>>): WsRead {
        if ("net_cash_amount" !in header) return WsRead.Invalid("The activities export has no net_cash_amount column.")
        val accounts = ArrayList<WsAccount>()
        val activities = ArrayList<WsActivity>()
        val skipped = ArrayList<WsSkipped>()
        val legs = ArrayList<Leg>()
        body.forEachIndexed { i, row ->
            val line = i + 2
            fun at(name: String) = cell(header, row, name)
            val date = StatementDates.parse(at("transaction_date"), dayFirst = false)
            if (date == null) {
                // A footer line closes the file.
                if (i + 1 < body.size) skipped += WsSkipped(line, "The date \"${at("transaction_date")}\" could not be read")
                return@forEachIndexed
            }
            val number = at("account_id")
            if (number.isEmpty()) {
                skipped += WsSkipped(line, "A line without an account")
                return@forEachIndexed
            }
            val kind = at("activity_type")
            val currency = currencyOrCad(at("currency"))
            val digits = Invest.fractionDigits(currency)
            val net = Invest.parseScaled(at("net_cash_amount"), digits)?.value
            if (net == null) {
                skipped += WsSkipped(line, "The amount \"${at("net_cash_amount")}\" could not be read")
                return@forEachIndexed
            }
            val units = Invest.parseScaled(at("quantity"), 8)?.value ?: 0L
            val commission = abs(Invest.parseScaled(at("commission"), digits)?.value ?: 0L)
            val sub = at("activity_sub_type").uppercase()
            val symbol = at("symbol").uppercase().ifEmpty { null }
            val registration = registrationOf(at("account_type"))
            val type: ActivityType = when (kind.lowercase()) {
                "trade" -> when {
                    sub in OPTION_TRADES -> null.also { skipped += WsSkipped(line, OPTIONS) }
                    sub == "BUY" || sub == "DRIP" || (sub != "SELL" && units > 0) -> ActivityType.BUY
                    sub == "SELL" || units < 0 -> ActivityType.SELL
                    else -> null.also { skipped += WsSkipped(line, "A trade without units") }
                }
                "optionexercise" -> null.also { skipped += WsSkipped(line, OPTIONS) }
                "dividend" -> if (net < 0) null.also { skipped += WsSkipped(line, "A reversed dividend") } else ActivityType.DIVIDEND
                "interest" -> ActivityType.INTEREST
                "moneymovement" -> if (net >= 0) ActivityType.DEPOSIT else ActivityType.WITHDRAWAL
                "fxexchange" -> {
                    legs += Leg(line, number, date, currency, net)
                    noteAccount(accounts, number, "", registration)
                    null
                }
                "nonresidenttax" -> ActivityType.TAX
                "fee" -> if (net > 0) ActivityType.CREDIT else ActivityType.FEE
                "refund", "bonuspayment", "administrativepayment" -> if (net < 0) ActivityType.FEE else ActivityType.CREDIT
                "returnofcapital" -> ActivityType.RETURN_OF_CAPITAL
                "noncashdistribution" -> ActivityType.NOTIONAL_DISTRIBUTION
                "securitytransfer", "internalsecuritytransfer" -> if (units > 0) ActivityType.TRANSFER_IN else ActivityType.TRANSFER_OUT
                "corporateaction" -> if (units > 0) ActivityType.SPLIT else null.also { skipped += WsSkipped(line, "Tally doesn't read $kind lines yet") }
                else -> null.also { skipped += WsSkipped(line, "Tally doesn't read $kind lines yet") }
            } ?: return@forEachIndexed
            if (symbol == null && (type == ActivityType.BUY || type == ActivityType.SELL || type == ActivityType.SPLIT)) {
                skipped += WsSkipped(line, "A trade without a symbol")
                return@forEachIndexed
            }
            val (amount, fee) = when (type) {
                ActivityType.BUY -> max(0, abs(net) - commission) to commission
                ActivityType.SELL -> abs(net) + commission to commission
                ActivityType.SPLIT -> 0L to 0L
                else -> abs(net) to 0L
            }
            noteAccount(accounts, number, "", registration)
            activities += WsActivity(
                line = line,
                account = number,
                date = date,
                type = type,
                symbol = symbol,
                name = at("name"),
                currency = currency,
                quantity = abs(units),
                price = Invest.parseScaled(at("unit_price"), 8)?.value,
                amount = amount,
                fee = fee,
            )
        }
        // An exchange is two lines: money out in one currency, in in the other, on one day. Each out
        // leg takes the first unpaired other leg of its account and day.
        val paired = BooleanArray(legs.size)
        legs.forEachIndexed { o, out ->
            if (out.net >= 0 || paired[o]) return@forEachIndexed
            val n = legs.indices.firstOrNull { !paired[it] && legs[it].net >= 0 && legs[it].account == out.account && legs[it].date == out.date }
                ?: return@forEachIndexed
            paired[o] = true
            paired[n] = true
            val into = legs[n]
            activities += WsActivity(
                line = out.line,
                account = out.account,
                date = out.date,
                type = ActivityType.FX,
                symbol = null,
                name = "",
                currency = out.currency,
                quantity = 0,
                price = null,
                amount = abs(out.net),
                fee = 0,
                toAmount = into.net,
                toCurrency = into.currency,
            )
        }
        legs.forEachIndexed { k, leg -> if (!paired[k]) skipped += WsSkipped(leg.line, "A currency exchange without its other side") }
        return WsRead.Ok(
            WsFile(WsKind.ACTIVITIES, null, accounts, emptyList(), activities.sortedBy { it.line }, skipped.sortedBy { it.line }),
        )
    }

    // ── A monthly statement ──────────────────────────────────────────────────

    private fun statement(header: List<String>, body: List<List<String>>): WsRead {
        fun codeOf(row: List<String>) = cell(header, row, "transaction").uppercase()
        if (body.none { codeOf(it) in INVESTMENT_CODES }) return WsRead.Invalid(NOT_INVESTMENT)
        val activities = ArrayList<WsActivity>()
        val skipped = ArrayList<WsSkipped>()
        body.forEachIndexed { i, row ->
            val line = i + 2
            fun at(name: String) = cell(header, row, name)
            val date = StatementDates.parse(at("date"), dayFirst = false)
            if (date == null) {
                skipped += WsSkipped(line, "The date \"${at("date")}\" could not be read")
                return@forEachIndexed
            }
            val code = codeOf(row)
            val currency = currencyOrCad(at("currency"))
            val amount = Invest.parseScaled(at("amount"), Invest.fractionDigits(currency))?.value?.let { abs(it) }
            if (amount == null) {
                skipped += WsSkipped(line, "The amount \"${at("amount")}\" could not be read")
                return@forEachIndexed
            }
            val description = at("description")
            val beforeDash = description.substringBefore(" - ", "").trim().uppercase().ifEmpty { null }
            val activity = WsActivity(line, null, date, ActivityType.DEPOSIT, null, "", currency, 0, null, amount, 0, note = description)
            activities += when (code) {
                "BUY", "SELL" -> {
                    val m = TRADE.find(description)
                    val units = m?.let { Invest.parseScaled(it.groupValues[3], 8) }
                    val price = m?.let { Invest.parseScaled(it.groupValues[4], 8) }
                    if (m == null || units == null || price == null) {
                        skipped += WsSkipped(line, "The description does not say how many shares or at what price")
                        return@forEachIndexed
                    }
                    activity.copy(
                        type = if (code == "BUY") ActivityType.BUY else ActivityType.SELL,
                        symbol = m.groupValues[1].uppercase(),
                        quantity = units.value,
                        price = price.value,
                    )
                }
                "DIV" -> activity.copy(type = ActivityType.DIVIDEND, symbol = beforeDash)
                "CONT" -> activity
                "NRT" -> activity.copy(type = ActivityType.TAX)
                "FPLINT", "INT" -> activity.copy(type = ActivityType.INTEREST)
                "FEE" -> activity.copy(type = ActivityType.FEE)
                "WD", "WDL" -> activity.copy(type = ActivityType.WITHDRAWAL)
                "LOAN", "RECALL" -> {
                    skipped += WsSkipped(line, "Securities lending isn't tracked")
                    return@forEachIndexed
                }
                else -> {
                    skipped += WsSkipped(line, "Tally doesn't read $code lines yet")
                    return@forEachIndexed
                }
            }
        }
        return WsRead.Ok(WsFile(WsKind.STATEMENT, null, emptyList(), emptyList(), activities, skipped))
    }

    // ── The import plan ──────────────────────────────────────────────────────

    /** A security's uid: its symbol, upper-cased and trimmed. */
    fun securityUid(symbol: String): String = "sec:" + symbol.trim().uppercase()

    /** The rows [file] becomes, with the uids both devices derive. Pure, so the phone and the PC write the same rows from the same file. */
    fun plan(file: WsFile, input: PlanInput): ImportPlan {
        if (input.currency != "CAD") {
            return ImportPlan(refused = "Wealthsimple reports are in Canadian dollars; this ledger keeps ${input.currency}")
        }
        val created = LinkedHashMap<String, PlanAccount>()
        fun accountUid(number: String): String {
            input.accounts[number]?.let { return it }
            val uid = "ws:$number"
            if (uid !in created) {
                val found = file.accounts.firstOrNull { it.number == number }
                val registration = found?.registration ?: Registration.OTHER
                created[uid] = PlanAccount(uid, found?.name ?: ("Wealthsimple " + registration.label), registration, INSTITUTION, number)
            }
            return uid
        }
        val securities = LinkedHashMap<String, PlanSecurity>()

        if (file.kind == WsKind.HOLDINGS) {
            val asOf = file.asOf ?: return ImportPlan(refused = "The holdings report has no \"As of\" line, so its day is unknown")
            val holdings = LinkedHashMap<String, PlanHolding>()
            val prices = LinkedHashMap<String, PlanPrice>()
            val values = LinkedHashMap<String, PlanValue>()
            file.holdings.forEach { h ->
                val account = accountUid(h.account)
                val security = securityUid(h.symbol)
                securities.getOrPut(security) { PlanSecurity(security, h.symbol.trim().uppercase(), h.name, h.currency, h.kind, h.exchange) }
                val uid = "hold:$account:$security:$asOf"
                val before = holdings[uid]
                holdings[uid] = before?.copy(quantity = before.quantity + h.quantity, book = before.book + h.book, bookMarket = before.bookMarket + h.bookMarket)
                    ?: PlanHolding(uid, account, security, asOf, h.quantity, h.book, h.bookMarket)
                if (h.price != null) prices.getOrPut("px:$security:$asOf") { PlanPrice("px:$security:$asOf", security, asOf, h.price) }
                val valueUid = "val:$account:$asOf"
                values[valueUid] = PlanValue(valueUid, account, asOf, (values[valueUid]?.value ?: 0L) + inCad(h, asOf, input.fxRates))
            }
            return ImportPlan(
                kind = file.kind,
                accounts = created.values.toList(),
                securities = securities.values.toList(),
                holdings = holdings.values.toList(),
                prices = prices.values.toList(),
                values = values.values.toList(),
                skipped = file.skipped,
            )
        }

        val seen = HashMap<List<String>, Int>()
        val activities = file.activities.map { a ->
            val account = when {
                a.account != null -> accountUid(a.account)
                input.statementAccount != null -> input.statementAccount
                else -> return ImportPlan(refused = "Say which account this statement belongs to")
            }
            val security = a.symbol?.let { symbol ->
                securityUid(symbol).also { uid ->
                    securities.getOrPut(uid) { PlanSecurity(uid, symbol.trim().uppercase(), a.name, a.currency, SecurityKind.OTHER, "") }
                }
            }
            val parts = listOf(
                account, a.date.toString(), a.type.name, security.orEmpty(), a.quantity.toString(), a.amount.toString(),
                a.currency, a.fee.toString(),
            )
            val occurrence = seen[parts] ?: 0
            seen[parts] = occurrence + 1
            PlanActivity(
                uid = Invest.importUid(parts, occurrence),
                line = a.line,
                accountUid = account,
                securityUid = security,
                type = a.type,
                date = a.date,
                quantity = a.quantity,
                amount = a.amount,
                fee = a.fee,
                currency = a.currency,
                toAmount = a.toAmount,
                toCurrency = a.toCurrency,
                note = a.note,
            )
        }
        return ImportPlan(
            kind = file.kind,
            accounts = created.values.toList(),
            securities = securities.values.toList(),
            activities = activities,
            skipped = file.skipped,
        )
    }

    /**
     * A holding's market value in Canadian cents on [date]: by a rate, else by its own book in both
     * currencies, else one to one; without a market value or a price, its book.
     */
    private fun inCad(h: WsHolding, date: LocalDate, rates: List<FxRateRow>): Long {
        val digits = Invest.fractionDigits(h.currency)
        val market = h.marketValue ?: h.price?.let { Invest.marketValue(h.quantity, it, digits) } ?: return h.book
        if (h.currency == "CAD") return market
        Invest.rateOn(rates, h.currency, "CAD", date)?.let { return Invest.convert(market, digits, 2, it) }
        if (h.book > 0 && h.bookMarket > 0) return Invest.mulDivHalfEven(market, h.book, h.bookMarket)
        return Invest.convert(market, digits, 2, Invest.RATE_SCALE)
    }
}
