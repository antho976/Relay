package com.tally.app.ui.invest

import androidx.compose.runtime.Immutable
import com.tally.app.data.db.AccountBalance
import com.tally.core.AccountType
import com.tally.core.ActivityType
import com.tally.core.BankStatements
import com.tally.core.Copy
import com.tally.core.IncomeEntry
import com.tally.core.IncomeMonth
import com.tally.core.Invest
import com.tally.core.MoneyFormatter
import com.tally.core.Portfolio
import com.tally.core.PortfolioHolding
import com.tally.core.Registration
import com.tally.core.RoomLine
import com.tally.core.SecurityKind
import java.math.BigDecimal
import java.time.LocalDate
import java.time.YearMonth
import java.time.temporal.ChronoUnit
import kotlin.math.abs

/*
 * The Investments screen's pure half: registrations and kinds of security as words and hues, the
 * room meter's pace, the sentences the page prints, the reading a kind lens narrows the page to,
 * and the investment import's preview. No Android, so InvestLogicTest holds every line of it. The
 * numbers themselves come from core's Invest.portfolio, the same reading the PC makes.
 */

/** The import page's source key that opens it on Wealthsimple's investment files. */
internal const val INVEST_SOURCE = "investments"

/** A figure older than this many days reads as stale: the page says when it was last brought up to date. */
internal const val STALE_DAYS = 31L

/** The holdings the page lists before "show N more". */
internal const val HOLDINGS_SHOWN = 8

/** The latest payouts the Income section lists. */
internal const val INCOME_SHOWN = 4

/** The first $2,000 over an RRSP's room is not taxed (CRA); said in the room line, not reckoned here. */
private const val RRSP_GRACE = 200_000L

// ── Words ────────────────────────────────────────────────────────────────────

/** A registration as words. Null is an investment account whose kind was never set. */
internal fun registrationLabel(r: Registration?): String = when (r) {
    Registration.NON_REGISTERED -> "Non-registered"
    Registration.TFSA -> "TFSA"
    Registration.RRSP -> "RRSP"
    Registration.FHSA -> "FHSA"
    Registration.RESP -> "RESP"
    Registration.LIRA -> "LIRA"
    Registration.RRIF -> "RRIF"
    Registration.OTHER -> "Other"
    null -> "Kind not set"
}

/** The few letters a registration's tile carries; null for one with no short name, which takes the account glyph. */
internal fun registrationCode(r: Registration?): String? = when (r) {
    Registration.TFSA -> "TFSA"
    Registration.RRSP -> "RRSP"
    Registration.FHSA -> "FHSA"
    Registration.RESP -> "RESP"
    Registration.LIRA -> "LIRA"
    Registration.RRIF -> "RRIF"
    Registration.NON_REGISTERED -> "NR"
    Registration.OTHER, null -> null
}

/** The registrations that have a yearly room Tally reads against a CRA figure. */
internal val ROOM_KINDS: Set<Registration> = setOf(Registration.TFSA, Registration.RRSP, Registration.FHSA)

/** The order the account editor offers the kinds in: the three with room first. */
internal val REGISTRATION_CHOICES: List<Registration> = listOf(
    Registration.TFSA,
    Registration.RRSP,
    Registration.FHSA,
    Registration.NON_REGISTERED,
    Registration.RESP,
    Registration.LIRA,
    Registration.RRIF,
    Registration.OTHER,
)

/** A kind of security as the allocation's legend names it. */
internal fun kindLabel(k: SecurityKind): String = when (k) {
    SecurityKind.STOCK -> "Stocks"
    SecurityKind.ETF -> "ETFs"
    SecurityKind.MUTUAL_FUND -> "Funds"
    SecurityKind.BOND -> "Bonds"
    SecurityKind.CRYPTO -> "Crypto"
    SecurityKind.CASH -> "Cash"
    SecurityKind.OTHER -> "Other"
}

/**
 * Account kinds and kinds of security are two data series, each with hues of its own from the
 * category palette (indices into it). Never on text: a hue marks a tile, a segment, a legend dot.
 */
internal fun registrationHue(r: Registration?): Int = when (r) {
    Registration.TFSA -> 0
    Registration.RESP -> 1
    Registration.RRSP -> 2
    Registration.FHSA -> 3
    Registration.OTHER -> 5
    Registration.LIRA -> 8
    Registration.NON_REGISTERED -> 9
    Registration.RRIF -> 11
    null -> 10
}

internal fun kindHue(k: SecurityKind): Int = when (k) {
    SecurityKind.ETF -> 4
    SecurityKind.OTHER -> 5
    SecurityKind.STOCK -> 6
    SecurityKind.CRYPTO -> 7
    SecurityKind.MUTUAL_FUND -> 8
    SecurityKind.CASH -> 10
    SecurityKind.BOND -> 11
}

/** An income line's type as words. */
internal fun incomeTypeLabel(type: ActivityType): String = when (type) {
    ActivityType.DIVIDEND -> "Dividend"
    ActivityType.INTEREST -> "Interest"
    ActivityType.REINVEST -> "Reinvested"
    else -> "Income"
}

/** A symbol as its tile shows it: upper case, without a Canadian exchange's suffix ("VFV.TO" is VFV). */
internal fun tileSymbol(symbol: String): String =
    symbol.trim().uppercase().replace(Regex("""\.(TO|V|NE|CN)$"""), "").ifEmpty { "?" }

/** Units held, as people write them: "12.5", "0.0042", "1200". */
internal fun quantityText(quantity: Long): String {
    val units = BigDecimal.valueOf(quantity).movePointLeft(8)
    return if (units.signum() == 0) "0" else units.stripTrailingZeros().toPlainString()
}

/** "12.5 shares" for a stock or a fund on an exchange, "0.0042 units" for anything else. */
internal fun unitsLine(quantity: Long, kind: SecurityKind): String {
    val text = quantityText(quantity)
    val one = text == "1"
    return when (kind) {
        SecurityKind.STOCK, SecurityKind.ETF -> "$text " + if (one) "share" else "shares"
        else -> "$text " + if (one) "unit" else "units"
    }
}

/** Basis points as a percent to one place: "8.1%", signed when asked ("+8.1%"); a loss always carries its minus. */
internal fun percentText(bps: Long, signed: Boolean = false): String {
    val tenths = (abs(bps) + 5) / 10
    val body = "${tenths / 10}.${tenths % 10}%"
    return when {
        tenths == 0L -> body
        bps < 0 -> "−$body"
        signed -> "+$body"
        else -> body
    }
}

/** A share in whole percent, "18%"; a sliver reads "under 1%" rather than a false zero. */
internal fun sharePercent(bps: Long): String = when {
    bps <= 0L -> "0%"
    bps < 50L -> "under 1%"
    else -> "${(bps + 50) / 100}%"
}

/** [part] of [total] in basis points, rounded as the ledger rounds; 0 when there is no total. */
internal fun shareBps(part: Long, total: Long): Long =
    if (total <= 0L) 0L else Invest.mulDivHalfEven(part, 10_000, total)

/** A gain with its sign written out, money and percent, so colour is never the only cue: "+$412 · +8.1%". */
internal fun gainText(gain: Long, gainBps: Long?, money: MoneyFormatter): String {
    val amount = if (gain > 0L) "+" + money.formatWhole(gain) else money.formatWhole(gain)
    return if (gainBps == null) amount else "$amount · " + percentText(gainBps, signed = true)
}

/** The hero's line under the worth: how far above or below what the holdings cost it stands. */
internal fun costLine(gain: Long, cost: Long, money: MoneyFormatter): String = when {
    cost <= 0L -> "No cost on record yet"
    gain > 0L -> money.formatWhole(gain) + " above what the holdings cost"
    gain < 0L -> money.formatWhole(-gain) + " below what the holdings cost"
    else -> "Level with what the holdings cost"
}

/** An ISO day from the portfolio, or null when there is none (or it does not read as one). */
internal fun parseDay(text: String?): LocalDate? =
    text?.let { runCatching { LocalDate.parse(it) }.getOrNull() }

/** The first day of an income month ("2026-10"), or null when it does not read as one. */
internal fun monthStart(m: IncomeMonth): LocalDate? =
    runCatching { YearMonth.parse(m.month).atDay(1) }.getOrNull()

// ── Freshness ────────────────────────────────────────────────────────────────

internal fun isStale(newest: LocalDate, today: LocalDate): Boolean = ChronoUnit.DAYS.between(newest, today) > STALE_DAYS

/** The page's context line: "3 accounts · valued 9 Oct", "· last valued 12 Aug" once it is stale. */
internal fun investContext(accounts: Int, newest: LocalDate?, today: LocalDate, short: (LocalDate) -> String): String {
    val count = Copy.plural(accounts, "account")
    return when {
        newest == null -> "$count · no values yet"
        isStale(newest, today) -> "$count · last valued " + short(newest)
        else -> "$count · valued " + short(newest)
    }
}

/** How old the newest figure is, said plainly: "40 days since the last report", "2 months since the last report". */
internal fun staleLine(newest: LocalDate, today: LocalDate): String {
    val days = ChronoUnit.DAYS.between(newest, today)
    val months = (days / 30).toInt()
    return if (months >= 2) "$months months since the last report" else Copy.plural(days.toInt(), "day") + " since the last report"
}

// ── Room ─────────────────────────────────────────────────────────────────────

/** The last day to contribute for [year]: 31 December, or the RRSP deadline in the year after. */
internal fun roomDeadline(registration: Registration, year: Int): LocalDate =
    if (registration == Registration.RRSP) Invest.rrspDeadline(year) else LocalDate.of(year, 12, 31)

/** The first day that counts toward [year]'s room: 1 January, or the day after last year's RRSP deadline. */
internal fun roomStart(registration: Registration, year: Int): LocalDate =
    if (registration == Registration.RRSP) Invest.rrspDeadline(year - 1).plusDays(1) else LocalDate.of(year, 1, 1)

/**
 * The room meter's tick: where an even pace to use [year]'s room by its deadline stands on
 * [today], 0..1. The pace meter re-read for a tax room; today counts, as it does on a budget.
 */
internal fun roomPaceFraction(registration: Registration, year: Int, today: LocalDate): Float {
    val start = roomStart(registration, year)
    val days = ChronoUnit.DAYS.between(start, roomDeadline(registration, year)) + 1
    val elapsed = (ChronoUnit.DAYS.between(start, today) + 1).coerceIn(0, days)
    return elapsed.toFloat() / days
}

/**
 * What each month needs from here, this one included, to use [left] by [deadline], rounded up to a
 * whole [unit] so the months add up to at least the room; null once nothing is left or the deadline passed.
 */
internal fun monthlyToFill(left: Long, today: LocalDate, deadline: LocalDate, unit: Long = 1L): Long? {
    if (left <= 0L || deadline.isBefore(today)) return null
    val months = (deadline.year * 12 + deadline.monthValue) - (today.year * 12 + today.monthValue) + 1
    val each = (left + months - 1) / months
    return (each + unit - 1) / unit * unit
}

/** The room row's line: what is left and the monthly pace that uses it, or how far over it went and what that costs. */
internal fun roomText(line: RoomLine, today: LocalDate, money: MoneyFormatter, short: (LocalDate) -> String): String {
    if (line.room == null) return "Add your ${line.year} room from CRA My Account"
    val left = line.left ?: 0L
    return when {
        line.over > 0L && line.overTaxed == 0L ->
            "Over by " + money.formatWhole(line.over) + ", inside the " + money.formatWhole(RRSP_GRACE) + " an RRSP may go over"
        line.over > 0L ->
            "Over by " + money.formatWhole(line.over) + ". The CRA charges 1% a month on " + money.formatWhole(line.overTaxed)
        left == 0L -> "The ${line.year} room is used"
        else -> {
            val deadline = roomDeadline(line.registration, line.year)
            val monthly = monthlyToFill(left, today, deadline, MoneyFormatter.pow10(money.fractionDigits))
            money.formatWhole(left) + " left" +
                (monthly?.let { " · " + money.formatWhole(it) + " a month uses it by " + short(deadline) } ?: "")
        }
    }
}

/** The room row's reading at its end: "$4,200 of $7,000", or "$4,200 in" with no room set. */
internal fun roomReading(line: RoomLine, money: MoneyFormatter): String {
    val room = line.room
    return if (room == null) money.formatWhole(line.contributed) + " in" else Copy.ofBudget(line.contributed, room, money)
}

/** The room meter's spoken reading. */
internal fun roomDescription(line: RoomLine, pace: Float?, money: MoneyFormatter): String {
    val label = registrationLabel(line.registration)
    val room = line.room
    if (room == null || pace == null) return "$label: " + money.formatWhole(line.contributed) + " in for ${line.year}, no room set"
    val even = Math.round(room.toDouble() * pace)
    return "$label: " + money.formatWhole(line.contributed) + " in of " + money.formatWhole(room) + " room. " +
        "An even pace would be " + money.formatWhole(even) + " by today."
}

/** Where the owner finds a registration's room, said under its editor, and what Tally does with it. */
internal fun roomSource(r: Registration): String = when (r) {
    Registration.RRSP ->
        "Your RRSP deduction limit is on your latest Notice of Assessment and in CRA My Account. " +
            "Tally takes off what goes in between last year's deadline and this one."
    Registration.FHSA ->
        "Your FHSA participation room is in CRA My Account. Tally takes off what goes in this year."
    else ->
        "Your room on 1 January is in CRA My Account. Tally takes off what goes in this year. " +
            "The yearly limit is your room only if last year's was used up; what comes out returns next year."
}

// ── The reading, for every account or one kind ───────────────────────────────

/** One investment account as the page reads it: from its holdings when it has any, else from the value recorded by hand. */
@Immutable
data class InvestLine(
    val id: Long,
    val name: String,
    val registration: Registration?,
    /** What it is worth: its holdings and cash, or its balance when it has none. */
    val worth: Long,
    /** What its holdings and cash cost; zero for an account read by hand. */
    val cost: Long,
    val cash: Long,
    val holdings: Int,
    /** No holdings or cash on record, so [worth] is the value recorded by hand (or what went in). */
    val byHand: Boolean,
    val valuedOn: LocalDate?,
    val returnBps: Long?,
    val returnAnnual: Boolean,
    val returnSince: LocalDate?,
)

/** The open investment accounts, each read from the portfolio when it holds something there, else from its balance. */
internal fun investLines(portfolio: Portfolio?, balances: List<AccountBalance>): List<InvestLine> {
    val read = portfolio?.accounts.orEmpty().associateBy { it.id }
    return balances.filter { it.type == AccountType.INVESTMENT && !it.archived }.map { b ->
        val p = read[b.id]
        if (p != null && (p.holdings > 0 || p.cash != 0L || p.book != 0L)) {
            InvestLine(
                id = b.id,
                name = b.name,
                registration = p.registration,
                worth = p.value,
                cost = p.book,
                cash = p.cash,
                holdings = p.holdings,
                byHand = false,
                valuedOn = parseDay(p.valuedOn),
                returnBps = p.returnBps,
                returnAnnual = p.returnAnnual,
                returnSince = parseDay(p.returnSince),
            )
        } else {
            InvestLine(
                id = b.id,
                name = b.name,
                registration = p?.registration,
                worth = b.balance,
                cost = 0L,
                cash = 0L,
                holdings = 0,
                byHand = true,
                valuedOn = b.valuedOn,
                returnBps = p?.returnBps,
                returnAnnual = p?.returnAnnual ?: false,
                returnSince = parseDay(p?.returnSince),
            )
        }
    }
}

/** One kind the lens can narrow the page to. [key] survives a rotation: the registration's name, or [LENS_UNSET]. */
@Immutable
data class InvestLens(val key: String, val label: String, val registration: Registration?)

internal const val LENS_UNSET = "UNSET"

/** The kinds held, in [Registration] order with the unset ones last; empty below two kinds, which need no lens. */
internal fun investLenses(lines: List<InvestLine>): List<InvestLens> {
    val held = lines.map { it.registration }.distinct()
    if (held.size < 2) return emptyList()
    return held.sortedBy { it?.ordinal ?: Int.MAX_VALUE }
        .map { r -> InvestLens(r?.name ?: LENS_UNSET, registrationLabel(r), r) }
}

/** One segment of an allocation bar and its legend row. [hue] indexes the category palette. */
@Immutable
data class ShareLine(val label: String, val hue: Int, val value: Long, val shareBps: Long)

/** Everything the page draws for the lens picked, folded once here rather than in composition. */
@Immutable
data class InvestView(
    val lenses: List<InvestLens> = emptyList(),
    /** The lens picked; null reads every account. */
    val lens: InvestLens? = null,
    val accounts: List<InvestLine> = emptyList(),
    /** What the accounts are worth together: the hero's figure. */
    val worth: Long = 0,
    /** The part of [worth] read from holdings and cash, and what those cost. */
    val priced: Long = 0,
    val cost: Long = 0,
    /** The part of [worth] recorded by hand, on accounts with no holdings on record. */
    val byHand: Long = 0,
    val cash: Long = 0,
    /** The newest day any figure rests on. */
    val newest: LocalDate? = null,
    /** Largest first. */
    val holdings: List<PortfolioHolding> = emptyList(),
    /** By account kind; empty under a lens, which is one kind. */
    val allocation: List<ShareLine> = emptyList(),
    /** By kind of security, cash included. */
    val kinds: List<ShareLine> = emptyList(),
    /** Newest first. */
    val income: List<IncomeEntry> = emptyList(),
    /** Twelve months, oldest first; null under a lens, since the months are read for every account together. */
    val incomeByMonth: List<IncomeMonth>? = null,
    val income12m: Long? = null,
    val room: List<RoomLine> = emptyList(),
    val issues: List<String> = emptyList(),
) {
    val gain: Long get() = priced - cost
    val gainBps: Long? get() = if (cost > 0L) Invest.mulDivHalfEven(gain, 10_000, cost) else null

    /** Some accounts are read from holdings: the cost and the gain mean something. */
    val hasHoldings: Boolean get() = accounts.any { !it.byHand }
    val fxEstimated: Int get() = holdings.count { it.fxEstimated }
    val noPrice: Int get() = holdings.count { it.noPrice }
}

/** The page for [lensKey] (null: every account). A key that no longer matches a kind held reads every account. */
internal fun investView(portfolio: Portfolio?, balances: List<AccountBalance>, lensKey: String?): InvestView {
    val all = investLines(portfolio, balances)
    val lenses = investLenses(all)
    val lens = lenses.firstOrNull { it.key == lensKey }
    val lines = if (lens == null) all else all.filter { it.registration == lens.registration }
    val ids = lines.map { it.id }.toSet()
    val priced = lines.filter { !it.byHand }
    val holdings = portfolio?.holdings.orEmpty().filter { it.accountId in ids }
    val cash = priced.sumOf { it.cash }
    val worth = lines.sumOf { it.worth }
    return InvestView(
        lenses = lenses,
        lens = lens,
        accounts = lines,
        worth = worth,
        priced = priced.sumOf { it.worth },
        cost = priced.sumOf { it.cost },
        byHand = lines.filter { it.byHand }.sumOf { it.worth },
        cash = cash,
        newest = lines.mapNotNull { it.valuedOn }.maxOrNull() ?: if (lens == null) parseDay(portfolio?.asOf) else null,
        holdings = holdings,
        allocation = if (lens == null) allocationLines(lines) else emptyList(),
        kinds = kindLines(holdings, cash),
        income = portfolio?.income.orEmpty().filter { it.accountId in ids },
        incomeByMonth = if (lens == null) portfolio?.incomeByMonth else null,
        income12m = if (lens == null) portfolio?.income12m else null,
        room = portfolio?.room.orEmpty().filter { lens == null || it.registration == lens.registration },
        issues = portfolio?.issues.orEmpty(),
    )
}

/** What each account kind is worth, largest first, by-hand values included so the bar adds up to the hero. */
internal fun allocationLines(lines: List<InvestLine>): List<ShareLine> {
    val total = lines.filter { it.worth > 0L }.sumOf { it.worth }
    return lines.filter { it.worth > 0L }
        .groupBy { it.registration }
        .map { (r, ls) -> ls.sumOf { it.worth }.let { v -> ShareLine(registrationLabel(r), registrationHue(r), v, shareBps(v, total)) } }
        .sortedByDescending { it.value }
}

/** What each kind of security is worth, the accounts' cash counted as cash, largest first. */
internal fun kindLines(holdings: List<PortfolioHolding>, cash: Long): List<ShareLine> {
    val byKind = LinkedHashMap<SecurityKind, Long>()
    holdings.forEach { h -> byKind[h.kind] = (byKind[h.kind] ?: 0L) + h.value }
    if (cash != 0L) byKind[SecurityKind.CASH] = (byKind[SecurityKind.CASH] ?: 0L) + cash
    val positive = byKind.filterValues { it > 0L }
    val total = positive.values.sum()
    return positive.map { (k, v) -> ShareLine(kindLabel(k), kindHue(k), v, shareBps(v, total)) }.sortedByDescending { it.value }
}

/** A holding's line under its name: "12.5 shares · TFSA · 18%". */
internal fun holdingLine(h: PortfolioHolding, weightBps: Long): String = listOf(
    unitsLine(h.quantity, h.kind),
    if (h.registration != null) registrationLabel(h.registration) else h.account,
    sharePercent(weightBps),
).joinToString(" · ")

/** A holding as TalkBack reads it: "XEQT, iShares Core Equity ETF Portfolio, $4,512, up $412 or 8.1 percent, 12.5 shares in TFSA". */
internal fun holdingDescription(h: PortfolioHolding, money: MoneyFormatter): String {
    val where = if (h.registration != null) registrationLabel(h.registration) else h.account
    val move = when {
        h.noPrice -> "no price yet, shown at cost"
        h.gain > 0L -> "up " + money.formatWhole(h.gain) + (h.gainBps?.let { " or " + percentText(it).removeSuffix("%") + " percent" } ?: "")
        h.gain < 0L -> "down " + money.formatWhole(-h.gain) + (h.gainBps?.let { " or " + percentText(-it).removeSuffix("%") + " percent" } ?: "")
        else -> "level with its cost"
    }
    return listOf(tileSymbol(h.symbol), h.name, money.formatWhole(h.value), move, unitsLine(h.quantity, h.kind) + " in " + where)
        .filter { it.isNotBlank() }
        .joinToString(", ")
}

/** An account's line under its name: "TFSA · valued 9 Oct · 5 holdings · +6.2% a year since 4 Mar 2024". */
internal fun accountMeta(a: InvestLine, short: (LocalDate) -> String): String = buildList {
    add(registrationLabel(a.registration))
    val on = a.valuedOn
    when {
        on != null -> add((if (a.byHand) "valued by hand " else "valued ") + short(on))
        a.byHand -> add("no value recorded")
    }
    if (!a.byHand) add(Copy.plural(a.holdings, "holding"))
    returnText(a, short)?.let { add(it) }
}.joinToString(" · ")

/** An account's money-weighted return in words: "+6.2% a year since 4 Mar 2024", "+2.0% since 1 Jan". */
internal fun returnText(a: InvestLine, short: (LocalDate) -> String): String? {
    val bps = a.returnBps ?: return null
    val since = a.returnSince?.let { " since " + short(it) }.orEmpty()
    return percentText(bps, signed = true) + (if (a.returnAnnual) " a year" else "") + since
}

/** The Income section's line: "$86 in dividends and interest over 12 months". */
internal fun incomeLine(total: Long, money: MoneyFormatter): String =
    if (total <= 0L) "No dividends or interest over 12 months" else money.formatWhole(total) + " in dividends and interest over 12 months"

// ── The investment import ────────────────────────────────────────────────────

/** Which of Wealthsimple's files a preview read, in the contract's words. */
internal const val KIND_HOLDINGS = "holdings"
internal const val KIND_ACTIVITIES = "activities"
internal const val KIND_STATEMENT = "statement"

/** An account a Wealthsimple file names, and the Tally account already holding its number (null: none). */
@Immutable
data class FileAccount(val number: String, val name: String, val registration: Registration?, val accountId: Long?, val rows: Int)

/** What the investment import found in a file, before anything is written. */
@Immutable
data class InvestPreview(
    /** [KIND_HOLDINGS], [KIND_ACTIVITIES] or [KIND_STATEMENT]. */
    val kind: String,
    val asOf: LocalDate?,
    val accounts: List<FileAccount>,
    val holdings: Int,
    val activities: Int,
    /** Activities not yet in the ledger, and the ones already there. */
    val new: Int,
    val duplicates: Int,
    /** Each line left out, and why. */
    val skipped: List<String>,
) {
    /** What the import adds: a report's holdings, or the new lines of an export or a statement. */
    val count: Int get() = if (kind == KIND_HOLDINGS) holdings else new
}

/** An investment import under way: the preview, where each file account goes, and a statement's account. */
@Immutable
data class InvestImport(
    val preview: InvestPreview,
    /** Each file account's number to the Tally account it goes into; null makes a new one. */
    val mapping: Map<String, Long?>,
    /** A statement names no account: the one it belongs to. */
    val statementAccountId: Long? = null,
) {
    /** A statement can go in once it has an account; a report or an export always can. */
    val ready: Boolean get() = preview.kind != KIND_STATEMENT || statementAccountId != null
}

/** A registration read from its stored name; null for none, or a name this version does not know. */
internal fun registrationNamed(name: String?): Registration? = Registration.entries.firstOrNull { it.name == name }

/** The words that name a registration in an account's name, English and French. */
private fun registrationWords(r: Registration?): Set<String> = when (r) {
    Registration.TFSA -> setOf("tfsa", "celi")
    Registration.RRSP -> setOf("rrsp", "reer")
    Registration.FHSA -> setOf("fhsa", "celiapp")
    Registration.RESP -> setOf("resp", "reee")
    Registration.LIRA -> setOf("lira", "cri")
    Registration.RRIF -> setOf("rrif", "ferr")
    else -> emptySet()
}

/**
 * Where each account in the file goes, before the owner says: the account already holding its
 * number; else the one open investment account whose name says its kind ("Wealthsimple TFSA" for
 * a TFSA) that nothing else took, so an account added by hand is not made twice; else a new one.
 */
internal fun guessMapping(file: List<FileAccount>, held: List<AccountBalance>): Map<String, Long?> {
    val open = held.filter { it.type == AccountType.INVESTMENT && !it.archived }
    val taken = file.mapNotNull { it.accountId }.toMutableSet()
    return file.associate { f ->
        val id = f.accountId ?: run {
            val words = registrationWords(f.registration)
            val named = if (words.isEmpty()) emptyList() else open.filter { a ->
                a.id !in taken && BankStatements.normalize(a.name).split(" ").any { it in words }
            }
            named.singleOrNull()?.id?.also { taken += it }
        }
        f.number to id
    }
}

/** What an account the import creates is called: the file's name for it, or "Wealthsimple TFSA". */
internal fun newAccountName(f: FileAccount): String = f.name.trim().ifEmpty { "Wealthsimple " + registrationLabel(f.registration) }

/** The page head's context for a file read: "Wealthsimple holdings report · as of 8 May". */
internal fun investFileLine(p: InvestPreview, short: (LocalDate) -> String): String = when (p.kind) {
    KIND_HOLDINGS -> "Wealthsimple holdings report" + (p.asOf?.let { " · as of " + short(it) } ?: "")
    KIND_ACTIVITIES -> "Wealthsimple activities export · " + Copy.plural(p.activities, "line")
    else -> "Wealthsimple monthly statement · " + Copy.plural(p.activities, "line")
}

/** The line under the preview's figure: "holdings in 3 accounts", "new activities · 12 already in Tally". */
internal fun investPlanLine(p: InvestPreview, into: String?): String = when (p.kind) {
    KIND_HOLDINGS -> (if (p.holdings == 1) "holding" else "holdings") + " in " + Copy.plural(p.accounts.size, "account")
    KIND_ACTIVITIES -> (if (p.new == 1) "new activity" else "new activities") + " across " + Copy.plural(p.accounts.size, "account")
    else -> (if (p.new == 1) "new line" else "new lines") + " into " + (into ?: "the account you pick")
}

/** The import's act: "Import 12 holdings", "Import 34 activities", or that nothing is new. */
internal fun investActionLabel(p: InvestPreview): String = when {
    p.count == 0 -> "Nothing new to import"
    p.kind == KIND_HOLDINGS -> "Import " + Copy.plural(p.count, "holding")
    p.kind == KIND_ACTIVITIES -> "Import " + Copy.plural(p.count, "activity", "activities")
    else -> "Import " + Copy.plural(p.count, "line")
}

/** What the import did, for the done page and the notice: "Imported 12 holdings · 2 accounts added". */
internal fun investImportedLine(holdings: Int, activities: Int, accountsCreated: Int, duplicates: Int): String {
    val added = listOfNotNull(
        Copy.plural(holdings, "holding").takeIf { holdings > 0 },
        Copy.plural(activities, "activity", "activities").takeIf { activities > 0 },
    )
    return listOfNotNull(
        if (added.isEmpty()) "Nothing new to add" else "Imported " + added.joinToString(" and "),
        (Copy.plural(accountsCreated, "account") + " added").takeIf { accountsCreated > 0 },
        "$duplicates already in Tally".takeIf { duplicates > 0 },
    ).joinToString(" · ")
}

/**
 * Where to find Wealthsimple's files, step by step, as the import page lists them. The site is
 * written wealthsimple.com, as the bank row writes it: my.wealthsimple.com is longer than the
 * column beside a Choose file key and broke mid-word.
 */
internal const val WS_HOLDINGS_STEPS =
    "Sign in at wealthsimple.com in a browser: your profile, then Documents, then Generate document. Pick Holdings report (CSV), today, every account"

internal const val WS_ACTIVITIES_STEPS =
    "The same page: Activities export (CSV), over the longest period the first time. A TFSA's or an RRSP's monthly statement reads too"

internal const val INVEST_FOOTER =
    "Buys, sells and dividends stay inside the account, never spending or income. The file is read on this phone and not kept."
