package com.tally.core

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.Charset
import java.nio.charset.CodingErrorAction
import java.text.Normalizer
import java.time.DateTimeException
import java.time.LocalDate

/** Where a statement came from, as the reader recognised it. */
enum class StatementFormat(val label: String) {
    DESJARDINS("Desjardins AccèsD"),
    WEALTHSIMPLE("Wealthsimple"),
    BANK("Bank CSV"),
}

/**
 * One line of a bank statement. [amount] is signed as the bank wrote it: below zero is money out
 * of the account, above zero money in. [account] names the account the line belongs to when the
 * file holds several (a Desjardins export can), else null.
 */
data class StatementRow(
    val date: LocalDate,
    val description: String,
    val amount: Long,
    val balance: Long? = null,
    val account: String? = null,
    /** The bank's own transaction code when it writes one (Wealthsimple's "SPEND", "AFT_IN"). */
    val code: String? = null,
)

/** A statement read whole: its lines in file order, and the lines that could not be read. */
data class Statement(
    val format: StatementFormat,
    val rows: List<StatementRow>,
    val errors: List<String> = emptyList(),
) {
    /** The accounts the file names, in the order they first appear; empty when it names none. */
    val accounts: List<String> get() = rows.mapNotNull { it.account }.distinct()
    val first: LocalDate? get() = rows.minOfOrNull { it.date }
    val last: LocalDate? get() = rows.maxOfOrNull { it.date }

    /** The share of lines above zero. A card statement that writes purchases as positive reads mostly so. */
    val positiveShare: Float get() = if (rows.isEmpty()) 0f else rows.count { it.amount > 0 }.toFloat() / rows.size
}

sealed interface StatementRead {
    data class Ok(val statement: Statement) : StatementRead

    /** A file in Tally's own export layout: it names its accounts and types, so the plain CSV import reads it. */
    data object TallyCsv : StatementRead

    /**
     * One of Wealthsimple's investment files (a holdings report, an activities export, or a monthly
     * statement with buys, sells or dividends): money put to work is not income or spending, so the
     * investment import ([Wealthsimple.read]) reads it.
     */
    data object Investments : StatementRead

    /** [reason] is shown as-is, so it names the problem and not the parser. */
    data class Invalid(val reason: String) : StatementRead
}

/**
 * Bank statements as their banks export them, read on the phone. No bank offers an open API to an
 * offline app, so the CSV the owner downloads is the bridge: Desjardins AccèsD (with French or
 * English headers, or its header-less positional layout), Wealthsimple's cash statements, and any
 * other bank that writes a date and an amount (or money out and money in) per line. Wealthsimple's
 * investment files are handed to [Wealthsimple] instead.
 */
object BankStatements {

    /**
     * Reads [text]. [dayFirst] settles a numeric date like 03/04/2026 that the file itself leaves
     * ambiguous (no day above 12 anywhere in it): true reads 3 April, false 4 March.
     */
    fun read(text: String, fractionDigits: Int, dayFirst: Boolean = true): StatementRead {
        val body = text.removePrefix(CsvCodec.BOM)
        if (body.isBlank()) return StatementRead.Invalid("The file is empty.")
        if (Wealthsimple.read(body) is WsRead.Ok) return StatementRead.Investments
        val records = CsvCodec.parseRecords(body, delimiterOf(body)).map { r -> r.map { it.trim() } }
        if (records.isEmpty()) return StatementRead.Invalid("The file is empty.")

        val headerAt = records.take(HEADER_SCAN).indexOfFirst { isHeader(it) }
        if (headerAt >= 0) {
            val header = records[headerAt].map(::normalize)
            if (isTallyHeader(header)) return StatementRead.TallyCsv
            return finish(withHeader(header, records.drop(headerAt + 1), headerAt + 2, fractionDigits, dayFirst))
        }
        if (records.any { r -> r.any { DESJARDINS_DATE.matches(it) } }) {
            return finish(desjardinsPositional(records, fractionDigits))
        }
        return finish(headerless(records, fractionDigits, dayFirst))
    }

    private fun finish(statement: Statement): StatementRead =
        if (statement.rows.isEmpty()) {
            StatementRead.Invalid(statement.errors.firstOrNull() ?: "No transactions were found in that file.")
        } else {
            StatementRead.Ok(statement)
        }

    /**
     * The file's bytes as text. UTF-8 when they are UTF-8; a UTF-16 file says so with its byte
     * order mark; anything else is read as Windows-1252, which is what an older bank export (or a
     * French Excel save) writes "Épicerie" in.
     */
    fun decodeText(bytes: ByteArray): String {
        if (bytes.size >= 2) {
            val b0 = bytes[0].toInt() and 0xFF
            val b1 = bytes[1].toInt() and 0xFF
            if ((b0 == 0xFF && b1 == 0xFE) || (b0 == 0xFE && b1 == 0xFF)) return String(bytes, Charsets.UTF_16)
        }
        val strict = Charsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
        return try {
            strict.decode(ByteBuffer.wrap(bytes)).toString()
        } catch (e: CharacterCodingException) {
            String(bytes, Charset.forName("windows-1252"))
        }
    }

    /** The field separator the first lines agree on: a tab, a semicolon, else a comma. */
    fun delimiterOf(text: String): Char {
        val lines = text.lineSequence().filter { it.isNotBlank() }.take(8).toList()
        if (lines.isEmpty()) return ','
        fun steady(c: Char): Int = lines.minOf { line -> countOutsideQuotes(line, c) }
        return listOf('\t', ';', ',').maxByOrNull { c -> steady(c) * 10 + if (c == ',') 1 else 0 }
            ?.takeIf { steady(it) > 0 } ?: ','
    }

    private fun countOutsideQuotes(line: String, c: Char): Int {
        var quoted = false
        var n = 0
        line.forEach { ch ->
            if (ch == '"') quoted = !quoted else if (ch == c && !quoted) n++
        }
        return n
    }

    // ── Headers ──────────────────────────────────────────────────────────────

    /** How many leading lines may come before the header (an account name, a period line). */
    private const val HEADER_SCAN = 12

    private enum class Role { DATE, POSTED, DESCRIPTION, AMOUNT, DEBIT, CREDIT, BALANCE, CODE, ACCOUNT }

    /** Lowercase, no accents, no brackets or what is in them, punctuation as spaces: "AMOUNT (CAD)" reads "amount". */
    fun normalize(text: String): String {
        val noAccents = Normalizer.normalize(text, Normalizer.Form.NFD).replace(Regex("\\p{Mn}+"), "")
        return noAccents.lowercase()
            .replace(Regex("\\(.*?\\)"), " ")
            .replace(Regex("[^a-z0-9]+"), " ")
            .trim()
    }

    private val DATE_NAMES = setOf(
        "date", "transaction date", "date de transaction", "date de la transaction", "date de l operation",
        "date operation", "date d operation", "trans date", "date transaction", "jour",
    )
    private val POSTED_NAMES = setOf(
        "posted date", "post date", "posting date", "date posted", "date d inscription", "date inscription",
        "date de publication", "settlement date", "date de valeur", "value date",
    )
    private val DESCRIPTION_NAMES = setOf(
        "description", "details", "detail", "merchant", "merchant name", "payee", "name", "memo", "libelle",
        "narrative", "transaction description", "description 1", "nom", "commercant", "beneficiaire", "note",
    )
    private val DEBIT_NAMES = setOf(
        "debit", "debits", "withdrawal", "withdrawals", "retrait", "retraits", "money out", "debit amount",
        "sortie", "sorties", "paid out", "out", "montant debit", "achat", "achats",
    )
    private val CREDIT_NAMES = setOf(
        "credit", "credits", "deposit", "deposits", "depot", "depots", "money in", "credit amount", "entree",
        "entrees", "paid in", "in", "montant credit", "paiement", "paiements",
    )
    private val CODE_NAMES = setOf("transaction", "type", "code", "transaction type", "type de transaction", "activity type")
    private val ACCOUNT_NAMES = setOf("account", "compte", "account number", "numero de compte", "account name")

    private fun role(h: String): Role? = when {
        h in DATE_NAMES -> Role.DATE
        h in POSTED_NAMES -> Role.POSTED
        h in DESCRIPTION_NAMES -> Role.DESCRIPTION
        h == "amount" || h.startsWith("amount ") || h == "montant" || h.startsWith("montant ") -> Role.AMOUNT
        h in DEBIT_NAMES -> Role.DEBIT
        h in CREDIT_NAMES -> Role.CREDIT
        h.startsWith("balance") || h.startsWith("solde") -> Role.BALANCE
        h in CODE_NAMES -> Role.CODE
        h in ACCOUNT_NAMES -> Role.ACCOUNT
        else -> null
    }

    private fun isHeader(record: List<String>): Boolean {
        val roles = record.map { role(normalize(it)) }
        val dated = Role.DATE in roles || Role.POSTED in roles
        val money = Role.AMOUNT in roles || Role.DEBIT in roles || Role.CREDIT in roles
        return dated && money
    }

    private fun isTallyHeader(header: List<String>): Boolean =
        "type" in header && "account" in header && "amount" in header && "to account" in header

    private fun withHeader(
        header: List<String>,
        lines: List<List<String>>,
        firstLine: Int,
        fractionDigits: Int,
        dayFirst: Boolean,
    ): Statement {
        val roles = header.map(::role)
        fun col(r: Role): Int = roles.indexOf(r)
        val dateCol = col(Role.DATE).takeIf { it >= 0 } ?: col(Role.POSTED)
        val descCol = col(Role.DESCRIPTION).takeIf { it >= 0 } ?: col(Role.CODE)
        val codeCol = if (col(Role.DESCRIPTION) >= 0) col(Role.CODE) else -1
        val amountCol = col(Role.AMOUNT)
        val debitCol = col(Role.DEBIT)
        val creditCol = col(Role.CREDIT)
        val balanceCol = col(Role.BALANCE)
        val accountCol = col(Role.ACCOUNT)
        val wealthsimple = header.any { it == "posted date" } && amountCol >= 0 && balanceCol >= 0 ||
            (codeCol >= 0 && header.getOrNull(codeCol) == "transaction" && header.size <= 6)
        val format = when {
            wealthsimple -> StatementFormat.WEALTHSIMPLE
            header.any { it == "montant" || it == "retrait" || it == "depot" || it == "solde" } -> StatementFormat.DESJARDINS
            else -> StatementFormat.BANK
        }
        val order = StatementDates.order(lines.mapNotNull { it.getOrNull(dateCol) }) ?: dayFirst
        val rows = ArrayList<StatementRow>()
        val errors = ArrayList<String>()
        lines.forEachIndexed { i, r ->
            fun at(c: Int) = if (c >= 0) r.getOrNull(c).orEmpty() else ""
            val amount = if (amountCol >= 0) {
                signedAmount(at(amountCol), fractionDigits)
            } else {
                val out = magnitude(at(debitCol), fractionDigits) ?: 0L
                val inn = magnitude(at(creditCol), fractionDigits) ?: 0L
                if (out == 0L && inn == 0L) null else inn - out
            }
            val rawDate = at(dateCol)
            val date = StatementDates.parse(rawDate, order)
            when {
                date == null && amount == null -> Unit // a blank line, a total, a footnote
                date == null -> errors += "Line ${firstLine + i}: the date \"$rawDate\" could not be read."
                amount == null || amount == 0L -> Unit // a zero line moves nothing
                else -> rows += StatementRow(
                    date = date,
                    description = at(descCol).ifBlank { at(codeCol) },
                    amount = amount,
                    balance = if (balanceCol >= 0) signedAmount(at(balanceCol), fractionDigits) else null,
                    account = at(accountCol).ifBlank { null },
                    code = at(codeCol).ifBlank { null },
                )
            }
        }
        return Statement(format, rows, errors)
    }

    // ── Desjardins, header-less ─────────────────────────────────────────────

    private val DESJARDINS_DATE = Regex("""\d{4}/\d{2}/\d{2}""")

    /**
     * AccèsD's own CSV has no header: caisse, folio, account type, date (YYYY/MM/DD), sequence,
     * description, cheque number, withdrawal, deposit, interest, capital paid, advance, repayment
     * and balance, in that order. Read relative to the date, so a leading column more or less
     * (an export of one account only) still lines up. Withdrawals and advances are money out;
     * deposits and repayments money in.
     */
    private fun desjardinsPositional(records: List<List<String>>, fractionDigits: Int): Statement {
        val rows = ArrayList<StatementRow>()
        val errors = ArrayList<String>()
        records.forEachIndexed { i, r ->
            val d = r.indexOfFirst { DESJARDINS_DATE.matches(it) }
            if (d < 0) return@forEachIndexed
            val date = StatementDates.parse(r[d], dayFirst = false)
            if (date == null) {
                errors += "Line ${i + 1}: the date \"${r[d]}\" could not be read."
                return@forEachIndexed
            }
            fun num(offset: Int): Long = magnitude(r.getOrNull(d + offset).orEmpty(), fractionDigits) ?: 0L
            val out = num(4) + num(8)
            val inn = num(5) + num(9)
            val amount = if (out == 0L && inn == 0L) num(6) else inn - out
            if (amount == 0L) return@forEachIndexed
            val account = listOfNotNull(r.getOrNull(d - 1), r.getOrNull(d - 2))
                .filter { it.isNotBlank() }
                .joinToString(" ")
                .ifBlank { null }
            rows += StatementRow(
                date = date,
                description = r.getOrNull(d + 2).orEmpty(),
                amount = amount,
                balance = r.getOrNull(d + 10)?.let { signedAmount(it, fractionDigits) },
                account = account,
            )
        }
        return Statement(StatementFormat.DESJARDINS, rows, errors)
    }

    // ── Anything else without a header ───────────────────────────────────────

    /**
     * A file with no header at all: per line, the first cell that reads as a date, the first text
     * after it as the description, then either money out and money in (one of the two blank) or a
     * signed amount, and a balance after them when there is one.
     */
    private fun headerless(records: List<List<String>>, fractionDigits: Int, dayFirst: Boolean): Statement {
        val order = StatementDates.order(records.mapNotNull { r -> r.firstOrNull { StatementDates.looksLikeDate(it) } }) ?: dayFirst
        val rows = ArrayList<StatementRow>()
        records.forEach { r ->
            val d = r.indexOfFirst { StatementDates.looksLikeDate(it) }
            if (d < 0) return@forEach
            val date = StatementDates.parse(r[d], order) ?: return@forEach
            val k = (d + 1 until r.size).firstOrNull { c -> r[c].isNotBlank() && magnitude(r[c], fractionDigits) == null } ?: return@forEach
            val a = r.getOrNull(k + 1).orEmpty()
            val b = r.getOrNull(k + 2).orEmpty()
            val split = r.size >= k + 3 && (a.isBlank() != b.isBlank())
            val amount = if (split) {
                (magnitude(b, fractionDigits) ?: 0L) - (magnitude(a, fractionDigits) ?: 0L)
            } else {
                signedAmount(a, fractionDigits) ?: return@forEach
            }
            if (amount == 0L) return@forEach
            rows += StatementRow(
                date = date,
                description = r[k],
                amount = amount,
                balance = (if (split) r.getOrNull(k + 3) else b.takeIf { it.isNotBlank() })?.let { signedAmount(it, fractionDigits) },
            )
        }
        return Statement(StatementFormat.BANK, rows)
    }

    // ── Amounts ──────────────────────────────────────────────────────────────

    private val CREDIT_MARK = Regex("""(?i)\b(cr|credit)\b""")
    private val DEBIT_MARK = Regex("""(?i)\b(dr|debit)\b""")

    /** An amount's size, whatever its sign or symbol; null when the cell holds no number. */
    internal fun magnitude(text: String, fractionDigits: Int): Long? {
        if (text.none { it.isDigit() }) return null
        return MoneyFormatter.parseAmount(text, fractionDigits)
    }

    /**
     * An amount with its sign: a minus (any of the dashes a statement prints), brackets or a "DR"
     * read as money out; "CR" as money in, the way card statements mark a payment or a refund.
     */
    internal fun signedAmount(text: String, fractionDigits: Int): Long? {
        val size = magnitude(text, fractionDigits) ?: return null
        val negative = when {
            CREDIT_MARK.containsMatchIn(text) -> false
            DEBIT_MARK.containsMatchIn(text) -> true
            else -> MoneyFormatter.hasSign(text)
        }
        return if (negative) -size else size
    }
}

/** The dates statements write: ISO, numeric either way round, compact, and with month names in English or French. */
internal object StatementDates {

    private val ISO = Regex("""^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})(?:$|[ T].*)""")
    private val COMPACT = Regex("""^(\d{4})(\d{2})(\d{2})$""")
    private val NUMERIC = Regex("""^(\d{1,2})[-/.](\d{1,2})[-/.](\d{2}|\d{4})(?:$|[ T].*)""")
    private val DAY_MONTH_YEAR = Regex("""^(\d{1,2})[ -]([^\d\s,.-]+)\.?,?[ -](\d{4})$""")
    private val MONTH_DAY_YEAR = Regex("""^([^\d\s,.-]+)\.?\s+(\d{1,2}),?\s+(\d{4})$""")

    fun looksLikeDate(text: String): Boolean = parse(text, dayFirst = true) != null

    fun parse(text: String, dayFirst: Boolean): LocalDate? {
        val t = text.trim()
        if (t.isEmpty()) return null
        return try {
            ISO.matchEntire(t)?.let { m -> return date(m.groupValues[1], m.groupValues[2], m.groupValues[3]) }
            COMPACT.matchEntire(t)?.let { m -> return date(m.groupValues[1], m.groupValues[2], m.groupValues[3]) }
            NUMERIC.matchEntire(t)?.let { m ->
                val (a, b, y) = m.destructured
                return if (dayFirst) date(y, b, a) else date(y, a, b)
            }
            DAY_MONTH_YEAR.matchEntire(t)?.let { m ->
                val month = monthOf(m.groupValues[2]) ?: return null
                return date(m.groupValues[3], month.toString(), m.groupValues[1])
            }
            MONTH_DAY_YEAR.matchEntire(t)?.let { m ->
                val month = monthOf(m.groupValues[1]) ?: return null
                return date(m.groupValues[3], month.toString(), m.groupValues[2])
            }
            null
        } catch (e: DateTimeException) {
            null
        } catch (e: NumberFormatException) {
            null
        }
    }

    /**
     * Whether a file's numeric dates put the day first: true once any first part is above 12,
     * false once any second part is, null when every date could be either.
     */
    fun order(samples: List<String>): Boolean? {
        var dayFirst = false
        var monthFirst = false
        samples.forEach { s ->
            val m = NUMERIC.matchEntire(s.trim()) ?: return@forEach
            val a = m.groupValues[1].toInt()
            val b = m.groupValues[2].toInt()
            if (a > 12) dayFirst = true
            if (b > 12) monthFirst = true
        }
        return when {
            dayFirst && !monthFirst -> true
            monthFirst && !dayFirst -> false
            else -> null
        }
    }

    private fun date(year: String, month: String, day: String): LocalDate {
        val y = year.toInt().let { if (it < 100) 2000 + it else it }
        return LocalDate.of(y, month.toInt(), day.toInt())
    }

    /** "janv.", "Jan", "février", "Aug" → the month's number. */
    private fun monthOf(word: String): Int? {
        val w = BankStatements.normalize(word).replace(" ", "")
        if (w.length < 3) return null
        return when {
            w.startsWith("jan") -> 1
            w.startsWith("feb") || w.startsWith("fev") -> 2
            w.startsWith("mar") -> 3
            w.startsWith("apr") || w.startsWith("avr") -> 4
            w.startsWith("may") || w.startsWith("mai") -> 5
            w.startsWith("juin") || w == "jun" || w.startsWith("june") -> 6
            w.startsWith("juil") || w == "jul" || w.startsWith("july") -> 7
            w.startsWith("aug") || w.startsWith("aou") -> 8
            w.startsWith("sep") -> 9
            w.startsWith("oct") -> 10
            w.startsWith("nov") -> 11
            w.startsWith("dec") -> 12
            else -> null
        }
    }
}
