package com.tally.core

import kotlinx.serialization.Serializable
import java.math.BigDecimal
import java.math.BigInteger
import java.math.RoundingMode
import java.security.MessageDigest
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.temporal.ChronoUnit
import java.util.Currency
import java.util.TreeMap
import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.ln1p
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow

// ── The rows the reading takes ───────────────────────────────────────────────

/** An INVESTMENT account, as the portfolio reads it. */
data class InvestAccountRow(
    val id: Long,
    val uid: String,
    val name: String,
    val registration: Registration?,
    val institution: String = "",
)

data class SecurityRow(
    val id: Long,
    val uid: String,
    val symbol: String,
    val name: String = "",
    val currency: String,
    val kind: SecurityKind,
    val exchange: String = "",
)

/**
 * One line of an account's holdings snapshot. [quantity] is at [Invest.QTY_SCALE]; [book] is in the
 * ledger currency, [bookMarket] in the security's own, both minor units.
 */
data class HoldingRow(
    val accountId: Long,
    val securityId: Long,
    val date: LocalDate,
    val quantity: Long,
    val book: Long,
    val bookMarket: Long,
)

/**
 * One investment activity. [quantity], [amount] and [fee] are never negative: [type] gives the
 * direction. [amount] and [fee] are minor units of [currency]; an FX activity's [amount] leaves in
 * [currency] and [toAmount] arrives in [toCurrency].
 */
data class ActivityRow(
    val id: Long,
    val uid: String,
    val accountId: Long,
    val securityId: Long?,
    val type: ActivityType,
    val date: LocalDate,
    val quantity: Long = 0,
    val amount: Long,
    val fee: Long = 0,
    val currency: String,
    val toAmount: Long? = null,
    val toCurrency: String? = null,
)

/** A security's price on a day, at [Invest.PRICE_SCALE] of its currency's major unit. */
data class PriceRow(val securityId: Long, val date: LocalDate, val price: Long)

/** Units of [quote] for one [base] on a day, at [Invest.RATE_SCALE]. */
data class FxRateRow(val base: String, val quote: String, val date: LocalDate, val rate: Long)

/** The room CRA gives for a registration and year, in the ledger currency. */
data class RoomFactRow(val registration: Registration, val year: Int, val amount: Long)

/** A Tally TRANSFER touching an investment account: [amount] is above zero into it, below zero out of it. */
data class TransferRow(val accountId: Long, val date: LocalDate, val amount: Long)

/** Everything [Invest.portfolio] reads, as plain rows, so the phone and the PC read the same numbers. */
data class PortfolioInput(
    val currency: String,
    val today: LocalDate,
    val accounts: List<InvestAccountRow> = emptyList(),
    val securities: List<SecurityRow> = emptyList(),
    val holdings: List<HoldingRow> = emptyList(),
    val activities: List<ActivityRow> = emptyList(),
    val prices: List<PriceRow> = emptyList(),
    val fxRates: List<FxRateRow> = emptyList(),
    val roomFacts: List<RoomFactRow> = emptyList(),
    val transfers: List<TransferRow> = emptyList(),
)

// ── The reading ──────────────────────────────────────────────────────────────

/**
 * The investments as of [today]. Money is minor units of [currency]; dates are ISO strings. Nothing
 * here is stored or synced: it is read again from the rows each time.
 */
@Serializable
data class Portfolio(
    val currency: String,
    val today: String,
    /** No investment account at all. */
    val empty: Boolean,
    /** The newest snapshot or price date the values rest on, or null when there is none. */
    val asOf: String?,
    val value: Long,
    val book: Long,
    val gain: Long,
    val gainBps: Long?,
    val cash: Long,
    val income12m: Long,
    /** Twelve calendar months, oldest first, the current one last. */
    val incomeByMonth: List<IncomeMonth>,
    val accounts: List<PortfolioAccount>,
    /** By registration, largest first. */
    val allocation: List<AllocationShare>,
    /** By kind of security, largest first; cash counts as [SecurityKind.CASH]. */
    val kinds: List<KindShare>,
    /** Largest value first. */
    val holdings: List<PortfolioHolding>,
    /** Newest first, at most ten. */
    val income: List<IncomeEntry>,
    val room: List<RoomLine>,
    val issues: List<String>,
)

@Serializable
data class IncomeMonth(val month: String, val amount: Long)

@Serializable
data class PortfolioAccount(
    val id: Long,
    val name: String,
    val registration: Registration?,
    val institution: String,
    val value: Long,
    val book: Long,
    val gain: Long,
    val gainBps: Long?,
    val cash: Long,
    /** How many positions it holds. */
    val holdings: Int,
    val valuedOn: String?,
    val returnBps: Long?,
    /** True when [returnBps] is a yearly rate; false when it is the return over a period shorter than a year. */
    val returnAnnual: Boolean,
    val returnSince: String?,
)

@Serializable
data class AllocationShare(val registration: Registration?, val value: Long, val shareBps: Long)

@Serializable
data class KindShare(val kind: SecurityKind, val value: Long, val shareBps: Long)

@Serializable
data class PortfolioHolding(
    val accountId: Long,
    val account: String,
    val registration: Registration?,
    val securityId: Long,
    val symbol: String,
    val name: String,
    val kind: SecurityKind,
    val currency: String,
    val quantity: Long,
    val price: Long?,
    val priceDate: String?,
    val value: Long,
    val book: Long,
    val gain: Long,
    val gainBps: Long?,
    val weightBps: Long,
    /** The value or the book was converted without a rate for the day: by the position's own book ratio, or one to one. */
    val fxEstimated: Boolean,
    /** No price on or before today: the value shown is the book. */
    val noPrice: Boolean,
)

@Serializable
data class IncomeEntry(val date: String, val accountId: Long, val symbol: String?, val type: ActivityType, val amount: Long)

/**
 * A registration's room in [year]: the CRA figure ([room], null when none was entered) minus what
 * went in over the window that ends on [deadline].
 */
@Serializable
data class RoomLine(
    val registration: Registration,
    val year: Int,
    val room: Long?,
    val contributed: Long,
    val withdrawn: Long,
    val left: Long?,
    val over: Long,
    val overTaxed: Long,
    val deadline: String,
    val limit: Long?,
)

// ── Intermediate readings, for the tests and for anyone who needs less than the whole ─────────────

/** What an account holds of a security once its snapshot and the activities after it are applied. */
data class Position(
    val accountId: Long,
    val securityId: Long,
    val quantity: Long,
    val book: Long,
    val bookMarket: Long,
    val fxEstimated: Boolean,
)

/** An account's cash in one currency, minor units of that currency. */
data class CashBalance(val accountId: Long, val currency: String, val amount: Long)

data class Positions(val positions: List<Position>, val cash: List<CashBalance>, val issues: List<String>)

/** A fixed-point figure read from text, and whether reading it dropped digits. */
data class Scaled(val value: Long, val rounded: Boolean)

/** One money movement for [Invest.xirr]: below zero is money put in, above zero money taken out. */
data class CashFlow(val date: LocalDate, val amount: Long)

/**
 * Holdings, their value and the room in registered accounts. Units, prices and rates are [Long]s at
 * a fixed scale; every product is taken exactly and rounded once, half-even, so the phone and the PC
 * land on the same cent. A twin of `invest.rs`, held to the same tests.
 */
object Invest {

    /** Units held, to 1e-8 of a unit (a satoshi). */
    const val QTY_SCALE = 100_000_000L

    /** A price, to 1e-8 of its currency's major unit. */
    const val PRICE_SCALE = 100_000_000L

    /** An exchange rate, units of the quote currency per one base, to 1e-8. */
    const val RATE_SCALE = 100_000_000L

    /** The first $2,000 over an RRSP's limit is not taxed. */
    const val RRSP_BUFFER = 200_000L

    const val FHSA_ANNUAL = 800_000L
    const val FHSA_LIFETIME = 4_000_000L

    // ── Fixed point ──────────────────────────────────────────────────────────

    /** Digits after the decimal point for an ISO 4217 code; 2 for a code that is not one. */
    fun fractionDigits(currency: String): Int = digitsOf(currency) ?: 2

    /** True for an ISO 4217 code of a currency with a set number of decimals ("CAD", "JPY"; not "XAU"). */
    fun isCurrency(code: String): Boolean = digitsOf(code) != null

    private fun digitsOf(code: String): Int? =
        runCatching { Currency.getInstance(code).defaultFractionDigits }.getOrNull()?.takeIf { it >= 0 }

    /** a·b/d, rounded half-even. Throws [ArithmeticException] when it does not fit a [Long] or [d] is zero. */
    fun mulDivHalfEven(a: Long, b: Long, d: Long): Long =
        divide(BigInteger.valueOf(a).multiply(BigInteger.valueOf(b)), BigInteger.valueOf(d))

    /** [quantity] at [price], in minor units of the price's currency (which has [fractionDigits] decimals). */
    fun marketValue(quantity: Long, price: Long, fractionDigits: Int): Long =
        divide(BigInteger.valueOf(quantity).multiply(BigInteger.valueOf(price)), BigInteger.TEN.pow(16 - fractionDigits))

    /** [minor] of a currency with [fromDigits] decimals, in one with [toDigits], at [rate] (units of it per one of the first). */
    fun convert(minor: Long, fromDigits: Int, toDigits: Int, rate: Long): Long =
        divide(
            BigInteger.valueOf(minor).multiply(BigInteger.valueOf(rate)).multiply(BigInteger.TEN.pow(toDigits)),
            BigInteger.valueOf(RATE_SCALE).multiply(BigInteger.TEN.pow(fromDigits)),
        )

    private fun divide(n: BigInteger, d: BigInteger): Long =
        BigDecimal(n).divide(BigDecimal(d), 0, RoundingMode.HALF_EVEN).longValueExact()

    // Digits are spelled out: Android's regex engine (ICU) reads `\d` as any decimal digit, which
    // BigDecimal would then accept, and the Rust reads ASCII digits only.
    private val DECIMAL = Regex("""[+-]?[0-9][0-9,]*(?:\.[0-9]+)?""")

    /**
     * "1,234.5" or "-3" at [digits] decimals, rounded half-even, and whether that dropped digits: an
     * optional sign, digits (commas among them group), and optionally a point and more digits.
     * Anything else (a currency sign, ".5", "5.", "1e5") is not a number and reads null, as does a
     * figure that does not fit a [Long].
     */
    fun parseScaled(text: String, digits: Int): Scaled? {
        val t = text.trim()
        if (!DECIMAL.matches(t)) return null
        val exact = BigDecimal(t.replace(",", "").removePrefix("+"))
        val scaled = exact.setScale(digits, RoundingMode.HALF_EVEN)
        val value = runCatching { scaled.unscaledValue().longValueExact() }.getOrNull() ?: return null
        return Scaled(value, scaled.compareTo(exact) != 0)
    }

    /**
     * The uid an imported activity gets, the same on every device: `imp:` and the first 32 hex digits
     * of SHA-256 over [parts] and [occurrence] (how many identical lines came before it in the file),
     * joined by U+001F.
     */
    fun importUid(parts: List<String>, occurrence: Int): String {
        val text = (parts + occurrence.toString()).joinToString("\u001F")
        val hash = MessageDigest.getInstance("SHA-256").digest(text.toByteArray(Charsets.UTF_8))
        val hex = StringBuilder("imp:")
        hash.take(16).forEach { b ->
            val v = b.toInt() and 0xFF
            hex.append(HEX[v shr 4]).append(HEX[v and 0xF])
        }
        return hex.toString()
    }

    private const val HEX = "0123456789abcdef"

    /**
     * Units of [to] per one [from] on [date]: the newest [from]→[to] rate on or before it, else the
     * inverse (1e16 / r, half-even) of the newest [to]→[from] one. Null without either.
     */
    fun rateOn(rates: List<FxRateRow>, from: String, to: String, date: LocalDate): Long? {
        if (from == to) return RATE_SCALE
        fun newest(base: String, quote: String) =
            rates.filter { it.base == base && it.quote == quote && it.date <= date && it.rate > 0 }.sortedBy { it.date }.lastOrNull()?.rate
        return newest(from, to) ?: newest(to, from)?.let { mulDivHalfEven(RATE_SCALE, RATE_SCALE, it) }
    }

    // ── Positions and cash ───────────────────────────────────────────────────

    private class Pos {
        var quantity = 0L
        var book = 0L
        var bookMarket = 0L
        var fxEstimated = false
    }

    private val ORDER = compareBy<ActivityRow>({ it.date }, { it.type.ordinal }, { it.uid })

    /**
     * [minor] of [from] in [to] on [date]: by a rate, else by a position's [book] over its
     * [bookMarket] (when both are above zero), else one to one; and true when it was not a rate.
     */
    private fun toLedger(
        minor: Long,
        from: String,
        to: String,
        date: LocalDate,
        rates: List<FxRateRow>,
        book: Long = 0,
        bookMarket: Long = 0,
    ): Pair<Long, Boolean> {
        if (from == to) return minor to false
        rateOn(rates, from, to, date)?.let { return convert(minor, fractionDigits(from), fractionDigits(to), it) to false }
        if (book > 0 && bookMarket > 0) return mulDivHalfEven(minor, book, bookMarket) to true
        return convert(minor, fractionDigits(from), fractionDigits(to), RATE_SCALE) to true
    }

    /**
     * Each account's positions and cash: its newest snapshot (its holdings on their newest date),
     * moved by its activities dated after it (all of them without a snapshot), in date, type and uid
     * order. The book is the average cost. Positions with no units and no book are dropped.
     */
    fun positions(input: PortfolioInput): Positions {
        val securities = input.securities.associateBy { it.id }
        val positions = ArrayList<Position>()
        val cash = ArrayList<CashBalance>()
        val issues = ArrayList<String>()
        input.accounts.forEach { account ->
            val snapshot = input.holdings.filter { it.accountId == account.id }.maxOfOrNull { it.date }
            val held = TreeMap<Long, Pos>()
            val money = TreeMap<String, Long>()
            fun move(currency: String, by: Long) {
                money[currency] = (money[currency] ?: 0L) + by
            }
            input.holdings.filter { it.accountId == account.id && it.date == snapshot }.forEach { h ->
                val security = securities[h.securityId]
                if (security != null && security.kind == SecurityKind.CASH) {
                    move(security.currency, marketValue(h.quantity, PRICE_SCALE, fractionDigits(security.currency)))
                } else {
                    val p = held.getOrPut(h.securityId) { Pos() }
                    p.quantity += h.quantity
                    p.book += h.book
                    p.bookMarket += h.bookMarket
                }
            }
            val after = input.activities.filter { it.accountId == account.id && (snapshot == null || it.date > snapshot) }
            after.sortedWith(ORDER).forEach { a ->
                val amount = a.amount
                val fee = a.fee
                when (a.type) {
                    ActivityType.DEPOSIT, ActivityType.DIVIDEND, ActivityType.INTEREST, ActivityType.CREDIT,
                    ActivityType.RETURN_OF_CAPITAL -> move(a.currency, amount)
                    ActivityType.WITHDRAWAL, ActivityType.FEE, ActivityType.TAX -> move(a.currency, -amount)
                    ActivityType.BUY -> move(a.currency, -(amount + fee))
                    ActivityType.SELL -> move(a.currency, amount - fee)
                    ActivityType.TRANSFER_IN -> if (a.securityId == null) move(a.currency, amount)
                    ActivityType.TRANSFER_OUT -> if (a.securityId == null) move(a.currency, -amount)
                    ActivityType.FX -> if (a.toAmount != null && a.toCurrency != null) {
                        move(a.currency, -amount)
                        move(a.toCurrency, a.toAmount)
                    }
                    ActivityType.REINVEST, ActivityType.NOTIONAL_DISTRIBUTION, ActivityType.SPLIT -> Unit
                }
                val securityId = a.securityId ?: return@forEach
                val p = held.getOrPut(securityId) { Pos() }
                fun ledger(x: Long): Long {
                    val (v, estimated) = toLedger(x, a.currency, input.currency, a.date, input.fxRates, p.book, p.bookMarket)
                    if (estimated) p.fxEstimated = true
                    return v
                }
                when (a.type) {
                    ActivityType.BUY, ActivityType.REINVEST -> {
                        val cost = amount + fee
                        val converted = ledger(cost)
                        p.quantity += a.quantity
                        p.bookMarket += cost
                        p.book += converted
                    }
                    ActivityType.TRANSFER_IN -> {
                        val converted = ledger(amount)
                        p.quantity += a.quantity
                        p.bookMarket += amount
                        p.book += converted
                    }
                    ActivityType.SPLIT -> p.quantity += a.quantity
                    ActivityType.NOTIONAL_DISTRIBUTION -> {
                        val converted = ledger(amount)
                        p.bookMarket += amount
                        p.book += converted
                    }
                    ActivityType.RETURN_OF_CAPITAL -> {
                        val converted = ledger(amount)
                        p.bookMarket = max(0, p.bookMarket - amount)
                        p.book = max(0, p.book - converted)
                    }
                    ActivityType.SELL, ActivityType.TRANSFER_OUT -> if (a.quantity > p.quantity) {
                        val symbol = securities[securityId]?.symbol ?: "?"
                        val issue = if (a.type == ActivityType.SELL) {
                            "Sold more $symbol than the ledger holds"
                        } else {
                            "Moved out more $symbol than the ledger holds"
                        }
                        if (issue !in issues) issues += issue
                        p.quantity = 0
                        p.book = 0
                        p.bookMarket = 0
                    } else if (a.quantity > 0) {
                        val removedBook = mulDivHalfEven(p.book, a.quantity, p.quantity)
                        val removedMarket = mulDivHalfEven(p.bookMarket, a.quantity, p.quantity)
                        p.quantity -= a.quantity
                        p.book -= removedBook
                        p.bookMarket -= removedMarket
                    }
                    else -> Unit
                }
            }
            held.forEach { (securityId, p) ->
                if (p.quantity != 0L || p.book != 0L) {
                    positions += Position(account.id, securityId, p.quantity, p.book, p.bookMarket, p.fxEstimated)
                }
            }
            money.forEach { (currency, amount) -> cash += CashBalance(account.id, currency, amount) }
        }
        return Positions(positions, cash, issues)
    }

    // ── Money-weighted return ────────────────────────────────────────────────

    private val GRID = doubleArrayOf(
        -0.99, -0.95, -0.9, -0.8, -0.7, -0.6, -0.5, -0.4, -0.3, -0.2, -0.1, -0.05, 0.0, 0.05, 0.1, 0.2, 0.3,
        0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0, 100.0, 1000.0,
    )

    /**
     * The yearly rate that brings [flows] to zero (an XIRR on an actual/365 day count), or null when
     * there is no money both put in and taken out. Where several rates do, the one nearest zero.
     */
    fun xirr(flows: List<CashFlow>): Double? {
        if (flows.none { it.amount > 0 } || flows.none { it.amount < 0 }) return null
        val sorted = flows.sortedBy { it.date }
        val scale = sorted.maxOf { abs(it.amount.toDouble()) }
        val c = DoubleArray(sorted.size) { sorted[it].amount / scale }
        val start = sorted.first().date
        val t = DoubleArray(sorted.size) { ChronoUnit.DAYS.between(start, sorted[it].date) / 365.0 }
        fun f(r: Double): Double {
            val l = ln1p(r)
            var sum = 0.0
            for (i in c.indices) sum += c[i] * exp(-t[i] * l)
            return sum
        }
        fun slope(r: Double): Double {
            val l = ln1p(r)
            var sum = 0.0
            for (i in c.indices) sum += -t[i] * c[i] * exp(-(t[i] + 1) * l)
            return sum
        }
        val atGrid = DoubleArray(GRID.size) { f(GRID[it]) }
        val brackets = ArrayList<Pair<Double, Double>>()
        for (k in GRID.indices) {
            if (atGrid[k] == 0.0) {
                brackets += GRID[k] to GRID[k]
            } else if (k + 1 < GRID.size && atGrid[k + 1] != 0.0 && (atGrid[k] < 0) != (atGrid[k + 1] < 0)) {
                brackets += GRID[k] to GRID[k + 1]
            }
        }
        if (brackets.isEmpty()) return null
        fun dist(b: Pair<Double, Double>) = if (b.first <= 0.0 && 0.0 <= b.second) 0.0 else min(abs(b.first), abs(b.second))
        val nearest = brackets.minOf { dist(it) }
        val roots = brackets.filter { dist(it) == nearest }.map { (low, high) ->
            var lo = low
            var hi = high
            val negativeAtLo = f(lo) < 0
            var r = (lo + hi) / 2
            run solve@{
                repeat(100) {
                    val fr = f(r)
                    if (fr == 0.0) return@solve
                    if ((fr < 0) == negativeAtLo) lo = r else hi = r
                    val d = slope(r)
                    var n = r - fr / d
                    if (d == 0.0 || !(n > lo && n < hi)) n = (lo + hi) / 2
                    if (abs(n - r) < 1e-12) {
                        r = n
                        return@solve
                    }
                    r = n
                }
            }
            r
        }
        return roots.minWithOrNull(compareBy<Double> { abs(it) }.thenByDescending { it })
    }

    /** A yearly [rate] over [days]: the return over that period, not annualized. */
    fun periodReturn(rate: Double, days: Long): Double = (1 + rate).pow(days / 365.0) - 1

    /** A return in basis points, rounded as Java's `Math.round`. */
    fun bps(rate: Double): Long = floor(rate * 10_000 + 0.5).toLong()

    // ── Room ─────────────────────────────────────────────────────────────────

    /** The TFSA dollar limit for [year], in cents; null before 2009 and for years not announced yet. */
    fun tfsaLimit(year: Int): Long? = when (year) {
        in 2009..2012 -> 500_000L
        2013, 2014 -> 550_000L
        2015 -> 1_000_000L
        in 2016..2018 -> 550_000L
        in 2019..2022 -> 600_000L
        2023 -> 650_000L
        in 2024..2026 -> 700_000L
        else -> null
    }

    /** The RRSP dollar limit for [year], in cents; null for a year not in the table. */
    fun rrspLimit(year: Int): Long? = when (year) {
        2024 -> 3_156_000L
        2025 -> 3_249_000L
        2026 -> 3_381_000L
        2027 -> 3_539_000L
        else -> null
    }

    /** The TFSA limits from [firstYear] (2009 at the earliest) through [year], summed. A hint, not CRA's figure. */
    fun tfsaCumulative(firstYear: Int, year: Int): Long =
        (max(2009, firstYear)..year).sumOf { tfsaLimit(it) ?: 0L }

    /** Next year's TFSA room: what is left this year, what was taken out (it comes back on 1 January), and next year's limit. */
    fun tfsaNextRoom(room: Long, contributed: Long, withdrawn: Long, nextLimit: Long): Long =
        room - contributed + withdrawn + nextLimit

    /**
     * An FHSA's room in [year]: $8,000 in the year it opened; after that $8,000 plus what went unused
     * last year (at most $8,000), never more than what is left of the $40,000 for life.
     */
    fun fhsaRoom(openYear: Int, contributionsByYear: Map<Int, Long>, year: Int): Long {
        if (year < openYear) return 0
        fun before(y: Int) = contributionsByYear.filterKeys { it < y }.values.sum()
        var room = max(0, min(FHSA_ANNUAL, FHSA_LIFETIME - before(openYear)))
        for (y in openYear + 1..year) {
            val carry = min(FHSA_ANNUAL, max(0, room - (contributionsByYear[y - 1] ?: 0L)))
            room = max(0, min(FHSA_ANNUAL + carry, FHSA_LIFETIME - before(y)))
        }
        return room
    }

    /** The last day to contribute to an RRSP for [year]: the 60th day of the next year, moved off a weekend to the Monday. */
    fun rrspDeadline(year: Int): LocalDate {
        val day = LocalDate.ofYearDay(year + 1, 60)
        return when (day.dayOfWeek) {
            DayOfWeek.SATURDAY -> day.plusDays(2)
            DayOfWeek.SUNDAY -> day.plusDays(1)
            else -> day
        }
    }

    /** The registrations Tally keeps room for, in [Registration] order. */
    val ROOM_REGISTRATIONS = listOf(Registration.TFSA, Registration.RRSP, Registration.FHSA)

    /** The first and last day of [registration]'s window for [year]. */
    fun roomWindow(registration: Registration, year: Int): ClosedRange<LocalDate> =
        if (registration == Registration.RRSP) {
            rrspDeadline(year - 1).plusDays(1)..rrspDeadline(year)
        } else {
            LocalDate.of(year, 1, 1)..LocalDate.of(year, 12, 31)
        }

    /** The year [registration]'s room reads on [today]: the calendar year, but an RRSP's is last year until last year's deadline. */
    fun roomYear(registration: Registration, today: LocalDate): Int =
        if (registration == Registration.RRSP && today <= rrspDeadline(today.year - 1)) today.year - 1 else today.year

    /** What is left of [room] after [contributed], and what is over it. An RRSP's first $2,000 over is not taxed. */
    fun roomLine(registration: Registration, year: Int, room: Long?, contributed: Long, withdrawn: Long): RoomLine {
        val over = if (room == null) 0L else max(0, contributed - room)
        return RoomLine(
            registration = registration,
            year = year,
            room = room,
            contributed = contributed,
            withdrawn = withdrawn,
            left = room?.let { max(0, it - contributed) },
            over = over,
            overTaxed = if (registration == Registration.RRSP) max(0, over - RRSP_BUFFER) else over,
            deadline = roomWindow(registration, year).endInclusive.toString(),
            limit = when (registration) {
                Registration.TFSA -> tfsaLimit(year)
                Registration.RRSP -> rrspLimit(year)
                Registration.FHSA -> FHSA_ANNUAL
                else -> null
            },
        )
    }

    // ── The portfolio ────────────────────────────────────────────────────────

    private val INCOME = setOf(ActivityType.DIVIDEND, ActivityType.INTEREST, ActivityType.REINVEST)

    private fun share(part: Long, total: Long): Long = if (total == 0L) 0 else mulDivHalfEven(part, 10_000, total)

    private fun gainBps(gain: Long, book: Long): Long? = if (book > 0) mulDivHalfEven(gain, 10_000, book) else null

    private fun newest(a: LocalDate?, b: LocalDate?): LocalDate? = if (a == null) b else if (b == null || a >= b) a else b

    /**
     * The portfolio from the ledger's rows: positions valued at the newest price on or before today,
     * cash, gains, income, money-weighted returns and the registered room.
     */
    fun portfolio(input: PortfolioInput): Portfolio {
        val ledger = input.currency
        val today = input.today
        val rates = input.fxRates
        val securities = input.securities.associateBy { it.id }
        val read = positions(input)
        val issues = ArrayList(read.issues)
        fun note(issue: String) {
            if (issue !in issues) issues += issue
        }
        fun priceOn(securityId: Long) = input.prices.filter { it.securityId == securityId && it.date <= today }.sortedBy { it.date }.lastOrNull()
        val holdings = ArrayList<PortfolioHolding>()
        val accounts = ArrayList<PortfolioAccount>()
        val byKind = TreeMap<SecurityKind, Long>()
        var asOf: LocalDate? = null
        input.accounts.forEach { account ->
            var valuedOn = input.holdings.filter { it.accountId == account.id }.maxOfOrNull { it.date }
            var value = 0L
            var book = 0L
            var count = 0
            read.positions.filter { it.accountId == account.id }.forEach { p ->
                val security = securities[p.securityId]
                val kind = security?.kind ?: SecurityKind.OTHER
                val currency = security?.currency ?: ledger
                val priced: Pair<Long, LocalDate?>? =
                    if (kind == SecurityKind.CASH) PRICE_SCALE to null else priceOn(p.securityId)?.let { it.price to it.date }
                val (v, estimated) = if (priced == null) {
                    p.book to false
                } else {
                    toLedger(marketValue(p.quantity, priced.first, fractionDigits(currency)), currency, ledger, today, rates, p.book, p.bookMarket)
                }
                val priceDate = priced?.second
                valuedOn = newest(valuedOn, priceDate)
                value += v
                book += p.book
                count++
                byKind[kind] = (byKind[kind] ?: 0L) + v
                holdings += PortfolioHolding(
                    accountId = account.id,
                    account = account.name,
                    registration = account.registration,
                    securityId = p.securityId,
                    symbol = security?.symbol ?: "?",
                    name = security?.name.orEmpty(),
                    kind = kind,
                    currency = currency,
                    quantity = p.quantity,
                    price = priced?.first,
                    priceDate = priceDate?.toString(),
                    value = v,
                    book = p.book,
                    gain = v - p.book,
                    gainBps = gainBps(v - p.book, p.book),
                    weightBps = 0,
                    fxEstimated = p.fxEstimated || estimated,
                    noPrice = priced == null,
                )
            }
            var cash = 0L
            read.cash.filter { it.accountId == account.id && it.amount != 0L }.forEach { c ->
                val (v, estimated) = toLedger(c.amount, c.currency, ledger, today, rates)
                if (estimated) note("No ${c.currency} to $ledger rate: ${c.currency} cash is counted one to one")
                cash += v
            }
            byKind[SecurityKind.CASH] = (byKind[SecurityKind.CASH] ?: 0L) + cash
            value += cash
            book += cash
            asOf = newest(asOf, valuedOn)
            val r = accountReturn(input, account.id, value)
            accounts += PortfolioAccount(
                id = account.id,
                name = account.name,
                registration = account.registration,
                institution = account.institution,
                value = value,
                book = book,
                gain = value - book,
                gainBps = gainBps(value - book, book),
                cash = cash,
                holdings = count,
                valuedOn = valuedOn?.toString(),
                returnBps = r.bps,
                returnAnnual = r.annual,
                returnSince = r.since?.toString(),
            )
        }
        val value = accounts.sumOf { it.value }
        val book = accounts.sumOf { it.book }
        val weighed = holdings.map { it.copy(weightBps = share(it.value, value)) }
            .sortedWith(compareByDescending<PortfolioHolding> { it.value }.thenBy { it.symbol }.thenBy { it.accountId })
        // On a tie, no registration sorts first, then the registrations in their order (as `invest.rs` sorts them).
        val byRegistration = LinkedHashMap<Registration?, Long>()
        accounts.forEach { byRegistration[it.registration] = (byRegistration[it.registration] ?: 0L) + it.value }
        val allocation = byRegistration.filter { it.value != 0L }
            .map { (registration, v) -> AllocationShare(registration, v, share(v, value)) }
            .sortedWith(compareByDescending<AllocationShare> { it.value }.thenBy { it.registration?.ordinal ?: -1 })
        val kinds = byKind.filter { it.value != 0L }
            .map { (kind, v) -> KindShare(kind, v, share(v, value)) }
            .sortedWith(compareByDescending<KindShare> { it.value }.thenBy { it.kind.ordinal })

        // Income: what dividends, interest and reinvested distributions brought, in the ledger currency.
        val investing = input.accounts.map { it.id }.toSet()
        val earned = input.activities
            .filter { it.accountId in investing && it.date <= today && it.type in INCOME }
            .map { it to toLedger(it.amount, it.currency, ledger, it.date, rates).first }
            .sortedWith(compareByDescending<Pair<ActivityRow, Long>> { it.first.date }.thenByDescending { it.first.id })
        val yearAgo = today.minusDays(365)
        val thisMonth = YearMonth.from(today)
        return Portfolio(
            currency = ledger,
            today = today.toString(),
            empty = input.accounts.isEmpty(),
            asOf = asOf?.toString(),
            value = value,
            book = book,
            gain = value - book,
            gainBps = gainBps(value - book, book),
            cash = accounts.sumOf { it.cash },
            income12m = earned.filter { it.first.date > yearAgo }.sumOf { it.second },
            incomeByMonth = (11 downTo 0).map { back ->
                val month = thisMonth.minusMonths(back.toLong())
                IncomeMonth(month.toString(), earned.filter { YearMonth.from(it.first.date) == month }.sumOf { it.second })
            },
            accounts = accounts,
            allocation = allocation,
            kinds = kinds,
            holdings = weighed,
            income = earned.take(10).map { (a, v) ->
                IncomeEntry(a.date.toString(), a.accountId, a.securityId?.let { securities[it]?.symbol }, a.type, v)
            },
            room = room(input),
            issues = issues,
        )
    }

    private class Return(val bps: Long?, val annual: Boolean, val since: LocalDate?)

    /**
     * An account's money-weighted return in basis points, whether it is yearly, and the day its flows
     * start. Only for an account whose deposits Tally has seen from the start: one with a [ActivityType.DEPOSIT]
     * and either no snapshot or an activity on or before its first one.
     */
    private fun accountReturn(input: PortfolioInput, accountId: Long, value: Long): Return {
        val activities = input.activities.filter { it.accountId == accountId }
        val firstSnapshot = input.holdings.filter { it.accountId == accountId }.minOfOrNull { it.date }
        val fromTheStart = firstSnapshot == null || activities.any { it.date <= firstSnapshot }
        if (activities.none { it.type == ActivityType.DEPOSIT } || !fromTheStart) return Return(null, false, null)
        val flows = activities.mapNotNull { a ->
            val sign = when (a.type) {
                ActivityType.DEPOSIT, ActivityType.TRANSFER_IN -> -1
                ActivityType.WITHDRAWAL, ActivityType.TRANSFER_OUT -> 1
                else -> return@mapNotNull null
            }
            CashFlow(a.date, sign * toLedger(a.amount, a.currency, input.currency, a.date, input.fxRates).first)
        } + CashFlow(input.today, value)
        val since = flows.minOf { it.date }
        val rate = xirr(flows) ?: return Return(null, false, since)
        val days = ChronoUnit.DAYS.between(since, input.today)
        return if (days >= 365) Return(bps(rate), true, since) else Return(bps(periodReturn(rate, days)), false, since)
    }

    /**
     * TFSA, RRSP and FHSA room in the year each reads today ([roomYear]), for each that has an account
     * or a room figure. In RRSP season an RRSP reads last year: what goes in then counts toward it.
     */
    private fun room(input: PortfolioInput): List<RoomLine> {
        return ROOM_REGISTRATIONS.mapNotNull { registration ->
            val year = roomYear(registration, input.today)
            val accounts = input.accounts.filter { it.registration == registration }
            val fact = input.roomFacts.firstOrNull { it.registration == registration && it.year == year }?.amount
            if (accounts.isEmpty() && fact == null) return@mapNotNull null
            val window = roomWindow(registration, year)
            var contributed = 0L
            var withdrawn = 0L
            accounts.forEach { account ->
                val activities = input.activities.filter { it.accountId == account.id }
                if (activities.isEmpty()) {
                    input.transfers.filter { it.accountId == account.id && it.date in window }.forEach { t ->
                        if (t.amount > 0) contributed += t.amount else withdrawn -= t.amount
                    }
                    return@forEach
                }
                activities.filter { it.date in window }.forEach { a ->
                    val v = toLedger(a.amount, a.currency, input.currency, a.date, input.fxRates).first
                    when (a.type) {
                        ActivityType.DEPOSIT, ActivityType.TRANSFER_IN -> contributed += v
                        ActivityType.WITHDRAWAL, ActivityType.TRANSFER_OUT -> withdrawn += v
                        else -> Unit
                    }
                }
            }
            roomLine(registration, year, fact, contributed, withdrawn)
        }
    }
}
