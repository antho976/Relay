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

    fun encode(rows: List<CsvRow>, currency: String, fractionDigits: Int): String {
        val sb = StringBuilder()
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
        // A leading =, +, - or @ turns into a formula in a spreadsheet; a quote prefix defuses it.
        val safe = if (field.isNotEmpty() && field[0] in "=+-@" && field.toDoubleOrNull() == null) "'$field" else field
        return if (safe.any { it == ',' || it == '"' || it == '\n' || it == '\r' }) {
            "\"" + safe.replace("\"", "\"\"") + "\""
        } else safe
    }

    /** Splits CSV text into records of fields, honouring quotes, doubled quotes and CRLF. */
    fun parseRecords(text: String): List<List<String>> {
        val records = ArrayList<List<String>>()
        val field = StringBuilder()
        var record = ArrayList<String>()
        var inQuotes = false
        var i = 0
        val src = text.removePrefix("﻿")
        while (i < src.length) {
            val c = src[i]
            if (inQuotes) {
                if (c == '"') {
                    if (i + 1 < src.length && src[i + 1] == '"') { field.append('"'); i++ } else inQuotes = false
                } else field.append(c)
            } else when (c) {
                '"' -> inQuotes = true
                ',' -> { record.add(field.toString()); field.clear() }
                '\r' -> {}
                '\n' -> { record.add(field.toString()); field.clear(); records.add(record); record = ArrayList() }
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
            val rawAmount = at(amountCol)
            val negative = rawAmount.trimStart().startsWith("-") || rawAmount.startsWith("(")
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
            val toAccount = at(toCol).ifEmpty { null }
            if (type == TxType.TRANSFER && toAccount == null) {
                errors += "Line $line: a transfer needs a to_account."
                return@forEachIndexed
            }
            rows += CsvRow(
                date = date,
                type = type,
                amount = amount,
                category = at(catCol).ifEmpty { null },
                account = at(accCol).ifEmpty { defaultAccount },
                toAccount = toAccount,
                note = at(noteCol),
            )
        }
        return CsvImport(rows, errors)
    }
}
