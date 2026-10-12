package com.tally.app.ui.settings

import androidx.compose.runtime.Immutable
import com.tally.app.data.prefs.Accent
import com.tally.core.BackupFile
import com.tally.core.BankStatements
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

/**
 * The confirm before a switch that rounds: "Japanese yen has no decimals, so 37 amounts will be
 * rounded to fit. Switching back later does not restore them."
 */
internal fun roundingPrompt(name: String, fractionDigits: Int, rounded: Int): String {
    val decimals = when (fractionDigits) {
        0 -> "no decimals"
        1 -> "1 decimal place"
        else -> "$fractionDigits decimal places"
    }
    return "$name has $decimals, so " + Copy.plural(rounded, "amount") + " will be rounded to fit. " +
        "Switching back later does not restore " + (if (rounded == 1) "it." else "them.")
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

/** A month's entries as their own file: "tally-entries-2026-10.csv". */
internal fun monthCsvFileName(start: LocalDate): String = "tally-entries-" + start.toString().take(7) + ".csv"

// ── The Settings list, Avex's way ───────────────────────────────────────────

/** Where a Settings row or search hit goes. */
enum class SettingsDest { APPEARANCE, FORMAT, ACCOUNTS, CATEGORIES, IMPORT, BACKUP, EXPORT, PC, SAMPLE, ERASE, ABOUT, INVESTMENTS }

/** The group a destination sits in on the Settings list, a search hit's "In ..." line. */
internal fun groupOf(dest: SettingsDest): String = when (dest) {
    SettingsDest.APPEARANCE, SettingsDest.FORMAT -> "General"
    SettingsDest.ACCOUNTS, SettingsDest.CATEGORIES, SettingsDest.IMPORT, SettingsDest.INVESTMENTS -> "Money"
    SettingsDest.BACKUP, SettingsDest.EXPORT -> "Data"
    SettingsDest.PC -> PC_GROUP
    SettingsDest.SAMPLE, SettingsDest.ERASE -> "Reset"
    SettingsDest.ABOUT -> "About"
}

/** One searchable thing: a page, or a setting on one. [tags] are the other words people use for it. */
@Immutable
internal data class SettingsEntry(val key: String, val name: String, val dest: SettingsDest, val tags: String = "") {
    /** Where the hit lives, in the words of the list: a page's group, or the page an item sits on. */
    val where: String get() = when {
        // The PC's page is the one row of its own group: the list it sits on says more.
        key == "page:pc" -> "Settings"
        key.startsWith("page:") -> groupOf(dest)
        else -> pageName(dest)
    }
}

internal fun pageName(dest: SettingsDest): String = when (dest) {
    SettingsDest.APPEARANCE -> "Appearance"
    SettingsDest.FORMAT -> "Currency & dates"
    SettingsDest.ACCOUNTS -> "Accounts"
    SettingsDest.CATEGORIES -> "Categories"
    SettingsDest.IMPORT -> "Import from your bank"
    SettingsDest.BACKUP -> "Backup"
    SettingsDest.EXPORT -> "Export"
    SettingsDest.PC -> PC_GROUP
    SettingsDest.SAMPLE -> "Load sample data"
    SettingsDest.ERASE -> "Erase everything"
    SettingsDest.ABOUT -> "About Tally"
    SettingsDest.INVESTMENTS -> "Investments"
}

/** Every page, and the settings inside them, that the search finds. */
internal val SETTINGS_ENTRIES: List<SettingsEntry> = listOf(
    SettingsEntry("page:appearance", "Appearance", SettingsDest.APPEARANCE, "theme look colour color dark"),
    SettingsEntry("page:format", "Currency & dates", SettingsDest.FORMAT, "money currency format dates"),
    SettingsEntry("page:accounts", "Accounts", SettingsDest.ACCOUNTS, "bank card cash balance chequing savings investment tfsa rrsp"),
    SettingsEntry("page:categories", "Categories", SettingsDest.CATEGORIES, "icons colours groceries spending income"),
    SettingsEntry("page:import", "Import from your bank", SettingsDest.IMPORT, "desjardins wealthsimple accesd statement csv bank download transactions"),
    SettingsEntry("page:backup", "Backup", SettingsDest.BACKUP, "auto weekly copy restore folder safe"),
    SettingsEntry("page:export", "Export", SettingsDest.EXPORT, "csv spreadsheet excel sheets file json share"),
    SettingsEntry("page:pc", "Relay on your PC", SettingsDest.PC, "sync computer desktop laptop relay pair link wifi tailscale both devices"),
    SettingsEntry("page:sample", "Load sample data", SettingsDest.SAMPLE, "demo try example fake"),
    SettingsEntry("page:erase", "Erase everything", SettingsDest.ERASE, "delete reset wipe clear start over"),
    SettingsEntry("page:about", "About Tally", SettingsDest.ABOUT, "version privacy offline licence license"),
    SettingsEntry("item:accent", "Accent colour", SettingsDest.APPEARANCE, "color ember red gold sage teal sky iris rose"),
    SettingsEntry("item:black", "Pure black", SettingsDest.APPEARANCE, "amoled oled dark black"),
    SettingsEntry("item:mono", "Use the accent", SettingsDest.APPEARANCE, "monochrome no colour grey"),
    SettingsEntry("item:currency", "Currency", SettingsDest.FORMAT, "cad usd eur dollar euro money"),
    SettingsEntry("item:month", "Month starts on", SettingsDest.FORMAT, "payday budget period cycle 15th"),
    SettingsEntry("item:week", "Week starts on", SettingsDest.FORMAT, "monday sunday"),
    SettingsEntry("item:desjardins", "Desjardins statement", SettingsDest.IMPORT, "accesd caisse csv"),
    SettingsEntry("item:wealthsimple", "Wealthsimple statement", SettingsDest.IMPORT, "chequing cash tfsa csv"),
    SettingsEntry("item:tally-csv", "Import a Tally CSV", SettingsDest.IMPORT, "spreadsheet csv file"),
    SettingsEntry("item:auto", "Weekly backup", SettingsDest.BACKUP, "automatic auto schedule"),
    SettingsEntry("item:folder", "Backup folder", SettingsDest.BACKUP, "uninstall keep drive files"),
    SettingsEntry("item:restore", "Restore a backup", SettingsDest.BACKUP, "bring back recover undo"),
    SettingsEntry("item:now", "Back up now", SettingsDest.BACKUP, "save copy"),
    SettingsEntry("item:csv", "Entries as CSV", SettingsDest.EXPORT, "spreadsheet excel numbers sheets"),
    SettingsEntry("item:file", "Save a backup file", SettingsDest.EXPORT, "json copy share"),
    SettingsEntry("item:pair", "Pair with your PC", SettingsDest.PC, "relay pairing link code qr address connect"),
    SettingsEntry("item:sync", "Sync now", SettingsDest.PC, "update refresh send pc computer"),
    SettingsEntry("item:forget", "Forget this PC", SettingsDest.PC, "unpair disconnect remove stop syncing"),
    SettingsEntry("item:investment", "Investment accounts", SettingsDest.ACCOUNTS, "tfsa rrsp fhsa celi reer wealthsimple value"),
    // Investments is a page of its own, not a setting; the search finds it by the words people use for it.
    SettingsEntry(
        "page:investments",
        "Investments",
        SettingsDest.INVESTMENTS,
        "portfolio holdings stocks etf dividends tfsa celi rrsp reer fhsa celiapp resp reee wealthsimple",
    ),
    SettingsEntry("item:room", "Contribution room", SettingsDest.INVESTMENTS, "tfsa rrsp fhsa celi reer celiapp droits cotisation cra limit"),
    SettingsEntry("item:ws-holdings", "Wealthsimple holdings report", SettingsDest.IMPORT, "investments activities export csv tfsa celi rrsp reer"),
)

private fun fold(text: String): String = BankStatements.normalize(text)

/**
 * The entries [query] finds, ranked: a name that starts with it, then a name holding it, then a
 * tag. In any case and with or without accents, so "epargne" finds what "épargne" would.
 */
internal fun searchSettings(query: String, entries: List<SettingsEntry> = SETTINGS_ENTRIES): List<SettingsEntry> {
    val q = fold(query)
    if (q.isEmpty()) return emptyList()
    return entries.mapNotNull { e ->
        val name = fold(e.name)
        val rank = when {
            name.startsWith(q) -> 0
            q in name -> 1
            q in fold(e.tags) -> 2
            else -> return@mapNotNull null
        }
        rank to e
    }.sortedWith(compareBy({ it.first }, { it.second.name.lowercase() })).map { it.second }
}

/**
 * The Backup row's subtitle: whether it runs, and when it last did ("Weekly · last 3 Oct"), or
 * that it failed. [lastOn] is the last day it wrote, already in words; null before the first.
 */
internal fun backupSummary(auto: Boolean, lastOn: String?, failed: Boolean): String = when {
    failed -> "Last backup failed · back up now"
    lastOn == null -> (if (auto) "Weekly" else "Off") + " · no backup yet"
    else -> (if (auto) "Weekly" else "Off") + " · last " + lastOn
}

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

// ── Relay on your PC ────────────────────────────────────────────────────────

/** The Settings group, and the page it opens. */
internal const val PC_GROUP = "Relay on your PC"

/** Where sync stands, in words: a short [title], the [detail] under it, and whether it [failed]. */
@Immutable
internal data class SyncLine(val title: String, val detail: String, val failed: Boolean = false)

/** "3 sent, 2 received", "Nothing new either way". */
internal fun syncCounts(sent: Int, received: Int): String =
    if (sent == 0 && received == 0) "Nothing new either way" else "$sent sent, $received received"

/**
 * The last sync as the page and the Settings row read it. [lastOn] is when it last worked, already
 * in words ("Today, 14:20"); null before the first.
 */
internal fun syncLine(paired: Boolean, lastOn: String?, error: String?, sent: Int, received: Int): SyncLine = when {
    !paired -> SyncLine("Not paired", "This ledger is on this phone only")
    error != null -> SyncLine("Last sync failed", error, failed = true)
    lastOn == null -> SyncLine("Not synced yet", "The first sync runs as soon as the PC can be reached")
    else -> SyncLine("Synced", lastOn + " · " + syncCounts(sent, received))
}

/**
 * The Settings row's subtitle for the paired PC: "Synced · Today, 14:20", or why it is not, or
 * the [note] a sync left (the phone took the PC's ledger).
 */
internal fun pcSummary(paired: Boolean, lastOn: String?, error: String?, note: String? = null): String = when {
    !paired -> "Keep this ledger on your computer too"
    error != null -> "Last sync failed · open for why"
    note != null -> note
    lastOn == null -> "Paired · not synced yet"
    else -> "Synced · $lastOn"
}

/** What the first sync does, said before it runs. */
internal const val FIRST_SYNC_NOTE =
    "The first sync makes the PC's ledger a copy of this phone's: anything only on the PC is replaced. After that, a change on either side reaches the other."
