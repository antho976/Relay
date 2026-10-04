package com.tally.app.ui.settings

import androidx.compose.runtime.Immutable
import com.tally.app.data.prefs.Accent
import com.tally.core.BackupFile
import com.tally.core.Copy
import com.tally.core.MoneyFormatter
import java.text.NumberFormat
import java.time.LocalDate
import java.util.Locale

/*
 * The pure pieces of Settings and onboarding: the currency list, the summaries the nav rows
 * carry, ordinals, file names. No Android here, so a plain JVM test holds every line.
 */

/** The currencies offered without a search: the common ones, Canada first. */
internal val COMMON_CURRENCIES: List<String> = listOf(
    "CAD", "USD", "EUR", "GBP", "AUD", "CHF", "JPY", "MXN",
    "NZD", "SEK", "NOK", "DKK", "INR", "BRL", "CNY", "ZAR",
)

/** The short list onboarding opens with, before "More currencies". */
internal val FIRST_CURRENCIES: List<String> = listOf("CAD", "USD", "EUR", "GBP")

/** The common list, with [current] put first when it is not one of them, so the pick is never hidden. */
internal fun currencyCodes(current: String): List<String> =
    if (current in COMMON_CURRENCIES) COMMON_CURRENCIES else listOf(current) + COMMON_CURRENCIES

/**
 * Onboarding's list: the device's own currency first, then the current pick (a re-run after an
 * erase keeps it), then the four most common. Expanded, every common currency follows.
 */
internal fun onboardingCurrencyCodes(device: String, current: String, expanded: Boolean): List<String> {
    val base = (listOf(device, current) + FIRST_CURRENCIES).distinct()
    return if (expanded) (base + COMMON_CURRENCIES).distinct() else base
}

/** One currency as a row reads it. */
@Immutable
internal data class CurrencyOption(
    val code: String,
    val name: String,
    val symbol: String,
    /** 1234.56 in this currency, at its own number of decimals. */
    val sample: String,
    val fractionDigits: Int,
) {
    /** "$ · e.g. $1,234.56"; the symbol drops out where it is just the code again. */
    val subtitle: String
        get() = listOfNotNull(symbol.takeIf { it != code }, "e.g. $sample").joinToString(" · ")

    val decimalsLine: String
        get() = when (fractionDigits) {
            0 -> "No decimals"
            1 -> "1 decimal place"
            else -> "$fractionDigits decimal places"
        }
}

/** 1234.56 in minor units for a currency with [fractionDigits] decimals (1234 for yen). */
internal fun sampleMinor(fractionDigits: Int): Long = 123_456L * MoneyFormatter.pow10(fractionDigits) / 100

internal fun currencyOption(code: String, locale: Locale): CurrencyOption {
    val fmt = MoneyFormatter(code, locale)
    val currency = fmt.currency
    val name = currency.getDisplayName(locale).replaceFirstChar { it.titlecase(locale) }
    return CurrencyOption(
        code = currency.currencyCode,
        name = name,
        symbol = fmt.symbol,
        sample = fmt.format(sampleMinor(fmt.fractionDigits)),
        fractionDigits = fmt.fractionDigits,
    )
}

/** "1st", "2nd", "3rd", "4th", "11th", "21st". */
internal fun ordinal(n: Int): String {
    val suffix = if (n % 100 in 11..13) "th" else when (n % 10) {
        1 -> "st"
        2 -> "nd"
        3 -> "rd"
        else -> "th"
    }
    return "$n$suffix"
}

/** "the 1st", "the 15th": how the month's start day reads in a sentence. */
internal fun monthStartLabel(day: Int): String = "the " + ordinal(day)

internal fun weekStartLabel(monday: Boolean): String = if (monday) "Monday" else "Sunday"

/** The Appearance row's subtitle: "Warm dark · Ember accent". */
internal fun appearanceSummary(amoled: Boolean, accentEnabled: Boolean, accent: Accent): String =
    groundLabel(amoled) + " · " + if (accentEnabled) "${accent.label} accent" else "no accent"

internal fun groundLabel(amoled: Boolean): String = if (amoled) "Pure black" else "Warm dark"

/** The Currency & dates row's subtitle: "CAD · month starts on the 1st · week from Monday". */
internal fun formatSummary(currency: String, monthStartDay: Int, weekStartsMonday: Boolean): String =
    "$currency · month starts on ${monthStartLabel(monthStartDay)} · week from ${weekStartLabel(weekStartsMonday)}"

/** The Accounts row's subtitle: "4 accounts · net $10,095". */
internal fun accountsSummary(count: Int, net: String): String =
    if (count == 0) "No accounts yet" else Copy.plural(count, "account") + " · net " + net

/** The Categories row's subtitle: "13 for spending · 4 for income". */
internal fun categoriesSummary(spending: Int, income: Int): String =
    if (spending + income == 0) "None yet" else "$spending for spending · $income for income"

/** A count with the locale's grouping: "1,284". */
internal fun countText(n: Int, locale: Locale): String = NumberFormat.getIntegerInstance(locale).format(n)

internal fun backupFileName(date: LocalDate): String = "tally-backup-$date.json"

internal fun csvFileName(date: LocalDate): String = "tally-entries-$date.csv"

/** What a picked backup holds, read before anything is replaced. */
@Immutable
data class RestorePreview(
    val entries: Int,
    val accounts: Int,
    /** The day the file was saved; null when the file does not say. */
    val savedOn: LocalDate?,
)

internal fun restorePreview(file: BackupFile): RestorePreview = RestorePreview(
    entries = file.transactions.size,
    accounts = file.accounts.size,
    savedOn = runCatching { LocalDate.parse(file.exportedAt) }.getOrNull(),
)

/** The restore confirm: "Replace everything with this backup? It holds 1,284 entries, saved 3 Oct." */
internal fun restorePrompt(entries: Int, savedOn: String?): String =
    "Replace everything with this backup? It holds " + Copy.plural(entries, "entry", "entries") +
        (if (savedOn != null) ", saved $savedOn." else ".")
