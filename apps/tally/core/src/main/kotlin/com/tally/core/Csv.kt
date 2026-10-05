package com.tally.core

import java.time.LocalDate
import java.time.format.DateTimeParseException

/** One exported or imported row, with names instead of ids so a spreadsheet can read it. */
data class CsvRow(
    val date: LocalDate,
    val type: TxType,
    val amount: Long,
    val category: String?,
    val account: String,
    val toAccount: String?,
    val note: String,
)

data class CsvImport(val rows: List<CsvRow>, val errors: List<String>)

/**
 * RFC 4180 CSV for transactions. Amounts are written in major units with a dot ("12.50") whatever
 * the locale, because a CSV is read by spreadsheets in every locale and a comma there splits the
 * column.
 */
object CsvCodec {

    val HEADER = listOf("date", "type", "amount", "currency", "category", "account", "to_account", "note")

    /**
     * The UTF-8 byte order mark. Excel opens a CSV without one in the legacy code page, so
     * "Épicerie" would read "Ã‰picerie"; the reader drops it again.
     */
    const val BOM = "\uFEFF"

    /** The characters that start a formula in a spreadsheet cell. */
    private const val FORMULA_START = "=+-@"

    fun encode(rows: List<CsvRow>, currency: String, fractionDigits: Int): String {
        val sb = StringBuilder(BOM)
        sb.append(HEADER.joinToString(",")).append("\r\n")
        rows.forEach { r ->
            val fields = listOf(
                r.date.toString(),
                r.type.name.lowercase(),
                majorString(r.amount, fractionDigits),
                currency,
                r.category.orEmpty(),
                r.account,
                r.toAccount.orEmpty(),
                r.note,
            )
            sb.append(fields.joinToString(",") { quote(it) }).append("\r\n")
        }
        return sb.toString()
    }

    fun majorString(minor: Long, fractionDigits: Int): String {
        if (fractionDigits == 0) return minor.toString()
        val scale = MoneyFormatter.pow10(fractionDigits)
        val sign = if (minor < 0) "-" else ""
        val abs = kotlin.math.abs(minor)
        return sign + (abs / scale) + "." + (abs % scale).toString().padStart(fractionDigits, '0')
    }

    private fun quote(field: String): String {
        // A leading =, +, - or @ turns into a formula in a spreadsheet; an apostrophe defuses it.
        // A field that already starts with apostrophes before one gets one more, so [unguard]
        // always takes off exactly the one that was added.
        val safe = if (needsGuard(field.trimStart('\''))) "'$field" else field
        return if (safe.any { it == ',' || it == '"' || it == '\n' || it == '\r' }) {
            "\"" + safe.replace("\"", "\"\"") + "\""
        } else safe
    }

    private val PLAIN_NUMBER = Regex("""[+-]?\d+(\.\d+)?""")

    /** True when [text] would start a formula: it opens with =, +, - or @ and is not a plain number. */
    private fun needsGuard(text: String): Boolean =
        text.isNotEmpty() && text[0] in FORMULA_START && !PLAIN_NUMBER.matches(text)

    /** Takes off the apostrophe [quote] put in front of a formula-like field, and nothing else. */
    fun unguard(field: String): String =
        if (field.startsWith("'") && needsGuard(field.trimStart('\''))) field.substring(1) else field

    /**
     * Splits CSV text into records of fields, honouring quotes, doubled quotes and CRLF. A quote
     * opens a quoted section only at the start of a field (spaces before it are dropped); anywhere
     * else it is a plain character, as spreadsheets read it, so the inch mark in `TV 55" stand`
     * cannot swallow the rows that follow it. [delimiter] separates fields: a comma by default, a
     * semicolon or a tab for the bank exports that use one.
     */
    fun parseRecords(text: String, delimiter: Char = ','): List<List<String>> {
        val records = ArrayList<List<String>>()
        val field = StringBuilder()
        var record = ArrayList<String>()
        var inQuotes = false
        // Whether the current field already had its quoted section; a second quote is plain text.
        var quoted = false
        var i = 0
        val src = text.removePrefix(BOM)
        while (i < src.length) {
            val c = src[i]
            if (inQuotes) {
                if (c == '"') {
                    if (i + 1 < src.length && src[i + 1] == '"') { field.append('"'); i++ } else inQuotes = false
                } else field.append(c)
            } else when (c) {
                '"' -> if (!quoted && field.isBlank()) {
                    field.clear()
                    inQuotes = true
                    quoted = true
                } else {
                    field.append(c)
                }
                delimiter -> { record.add(field.toString()); field.clear(); quoted = false }
                '\r' -> {}
                '\n' -> { record.add(field.toString()); field.clear(); quoted = false; records.add(record); record = ArrayList() }
                else -> field.append(c)
            }
            i++
        }
        if (field.isNotEmpty() || record.isNotEmpty()) { record.add(field.toString()); records.add(record) }
        return records.filter { r -> r.any { it.isNotBlank() } }
    }

    /**
     * Reads a CSV with a header row. Needs `date` and `amount`; `type`, `category`, `account`,
     * `to_account` and `note` are optional. Without a type column a negative amount is an expense
     * and a positive one income, which is how most bank exports write it.
     */
    fun decode(text: String, fractionDigits: Int, defaultAccount: String): CsvImport {
        val records = parseRecords(text)
        if (records.isEmpty()) return CsvImport(emptyList(), listOf("The file is empty."))
        val header = records.first().map { it.trim().lowercase() }
        val col = { name: String -> header.indexOf(name) }
        val dateCol = col("date")
        val amountCol = col("amount")
        if (dateCol < 0 || amountCol < 0) {
            return CsvImport(emptyList(), listOf("The first row needs a date column and an amount column."))
        }
        val typeCol = col("type")
        val catCol = col("category")
        val accCol = col("account")
        val toCol = col("to_account")
        val noteCol = col("note").takeIf { it >= 0 } ?: col("description")
        val rows = ArrayList<CsvRow>()
        val errors = ArrayList<String>()
        records.drop(1).forEachIndexed { idx, r ->
            val line = idx + 2
            fun at(i: Int) = if (i >= 0 && i < r.size) r[i].trim() else ""
            val date = try {
                LocalDate.parse(at(dateCol))
            } catch (e: DateTimeParseException) {
                errors += "Line $line: the date \"${at(dateCol)}\" is not in YYYY-MM-DD form."
                return@forEachIndexed
            }
            // The names and the note come back without the apostrophe the export put before a formula.
            fun textAt(i: Int) = unguard(at(i))
            val rawAmount = at(amountCol)
            // "-4.25", "−4.25", "-$4.25", "$-4.25", "4.25-" and "(4.25)" are all money going out.
            val negative = MoneyFormatter.hasSign(rawAmount)
            val amount = MoneyFormatter.parseAmount(rawAmount, fractionDigits)
            if (amount == null || amount == 0L) {
                errors += "Line $line: \"$rawAmount\" is not an amount."
                return@forEachIndexed
            }
            val type = when (at(typeCol).lowercase()) {
                "expense" -> TxType.EXPENSE
                "income" -> TxType.INCOME
                "transfer" -> TxType.TRANSFER
                "" -> if (negative) TxType.EXPENSE else TxType.INCOME
                else -> {
                    errors += "Line $line: the type \"${at(typeCol)}\" is not expense, income or transfer."
                    return@forEachIndexed
                }
            }
            val toAccount = textAt(toCol).ifEmpty { null }
            if (type == TxType.TRANSFER && toAccount == null) {
                errors += "Line $line: a transfer needs a to_account."
                return@forEachIndexed
            }
            rows += CsvRow(
                date = date,
                type = type,
                amount = amount,
                category = textAt(catCol).ifEmpty { null },
                account = textAt(accCol).ifEmpty { defaultAccount },
                toAccount = toAccount,
                note = textAt(noteCol),
            )
        }
        return CsvImport(rows, errors)
    }
}
