package com.margin.core

import java.math.BigDecimal
import java.math.RoundingMode
import java.text.DecimalFormat
import java.text.NumberFormat
import java.util.Currency
import java.util.Locale
import kotlin.math.abs

/**
 * Money is a [Long] of MINOR units (cents for CAD) everywhere in this app. Floating point never
 * touches an amount: a sum of a thousand doubles drifts, and a budget that is off by a cent on
 * the last day reads as a bug in the one place the owner checks to the cent.
 *
 * One instance per (currency, locale). Not thread-safe, because [NumberFormat] is not: build one
 * per screen state (it is cheap) rather than sharing one across threads.
 */
class MoneyFormatter(currencyCode: String, val locale: Locale) {

    val currency: Currency = runCatching { Currency.getInstance(currencyCode) }
        .getOrElse { Currency.getInstance("CAD") }

    /** Digits after the decimal point for this currency: 2 for CAD, 0 for JPY. */
    val fractionDigits: Int = currency.defaultFractionDigits.coerceAtLeast(0)

    private val scale: Long = pow10(fractionDigits)

    private val full: NumberFormat = NumberFormat.getCurrencyInstance(locale).apply {
        currency = this@MoneyFormatter.currency
        minimumFractionDigits = fractionDigits
        maximumFractionDigits = fractionDigits
    }

    private val whole: NumberFormat = NumberFormat.getCurrencyInstance(locale).apply {
        currency = this@MoneyFormatter.currency
        minimumFractionDigits = 0
        maximumFractionDigits = 0
        roundingMode = RoundingMode.HALF_EVEN
    }

    private val plain: NumberFormat = NumberFormat.getNumberInstance(locale).apply {
        minimumFractionDigits = 0
        maximumFractionDigits = 1
        roundingMode = RoundingMode.HALF_EVEN
    }

    /** The symbol and its side, read off the locale's own pattern ("$12" in en-CA, "12 $" in fr-CA). */
    private val prefix: String = (full as? DecimalFormat)?.positivePrefix.orEmpty()
    private val suffix: String = (full as? DecimalFormat)?.positiveSuffix.orEmpty()

    val symbol: String get() = currency.getSymbol(locale)

    /** The locale's decimal separator, for the keypad. */
    val decimalSeparator: Char = (full as? DecimalFormat)?.decimalFormatSymbols?.monetaryDecimalSeparator ?: '.'

    fun toMajor(minor: Long): BigDecimal = BigDecimal.valueOf(minor).movePointLeft(fractionDigits)

    /** "$1,284.50". Negative amounts take a true minus sign, never a hyphen. */
    fun format(minor: Long): String = signed(minor, full.format(toMajor(abs(minor))))

    /**
     * "$1,285": whole units, for figures where cents are noise (the margin, a budget, a month
     * total). Rounds half-even so a column of rounded figures does not drift one way.
     */
    fun formatWhole(minor: Long): String = signed(minor, whole.format(toMajor(abs(minor))))

    /**
     * "$12.4k" at 10,000 and above, whole units below. For tight places (a chart axis, a figure
     * row at 200% font) where "$12,431" would wrap.
     */
    fun formatCompact(minor: Long): String {
        val major = toMajor(abs(minor))
        if (major < BigDecimal(10_000)) return formatWhole(minor)
        val (value, unit) = if (major >= BigDecimal(1_000_000)) {
            major.divide(BigDecimal(1_000_000)) to "M"
        } else {
            major.divide(BigDecimal(1_000)) to "k"
        }
        return signed(minor, prefix + plain.format(value.setScale(1, RoundingMode.HALF_EVEN)) + unit + suffix)
    }

    /** Income reads "+$1,200.00"; an expense is just the amount (the row's context says where it went). */
    fun formatSigned(minor: Long): String = if (minor > 0) "+" + format(minor) else format(minor)

    /** The bare number for an editable field: "12.50", no symbol, no grouping. */
    fun formatInput(minor: Long): String {
        if (fractionDigits == 0) return abs(minor).toString()
        val major = toMajor(abs(minor)).setScale(fractionDigits, RoundingMode.UNNECESSARY)
        return major.toPlainString()
    }

    /**
     * Parses what a person types into an amount field: "12", "12.5", "12,50", "$1,234.56",
     * "1 234,56 $". Returns null for anything that is not a non-negative amount.
     */
    fun parse(text: String): Long? = parseAmount(text, fractionDigits)

    private fun signed(minor: Long, body: String) = if (minor < 0) "−$body" else body

    companion object {
        fun pow10(n: Int): Long {
            var r = 1L
            repeat(n) { r *= 10 }
            return r
        }

        /** Locale-tolerant amount parsing; see [MoneyFormatter.parse]. */
        fun parseAmount(text: String, fractionDigits: Int): Long? {
            val cleaned = text.filter { it.isDigit() || it == '.' || it == ',' }
            if (cleaned.isEmpty() || cleaned.none { it.isDigit() }) return null
            // The LAST separator is the decimal one when it is followed by 1..fractionDigits digits;
            // every other separator is grouping. "1,234.56" and "1.234,56" both read as 1234.56,
            // and "1,234" (three digits after) reads as one thousand two hundred thirty-four.
            val lastSep = cleaned.indexOfLast { it == '.' || it == ',' }
            val (intPart, fracPart) = if (lastSep >= 0) {
                val tail = cleaned.substring(lastSep + 1)
                if (tail.length in 1..fractionDigits.coerceAtLeast(0) && fractionDigits > 0) {
                    cleaned.substring(0, lastSep) to tail
                } else if (tail.isEmpty() && fractionDigits > 0) {
                    cleaned.substring(0, lastSep) to ""
                } else {
                    cleaned to ""
                }
            } else cleaned to ""
            val digits = intPart.filter { it.isDigit() }.ifEmpty { "0" }
            if (digits.length > 13) return null
            val frac = fracPart.padEnd(fractionDigits, '0').take(fractionDigits)
            return (digits + frac).toLongOrNull()
        }
    }
}
