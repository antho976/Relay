package com.tally.app.ui.settings

import com.tally.app.data.prefs.Accent
import com.tally.core.BackupFile
import com.tally.core.Copy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.util.Locale

class SettingsLogicTest {

    @Test fun ordinalsReadAsEnglish() {
        val expected = mapOf(
            1 to "1st", 2 to "2nd", 3 to "3rd", 4 to "4th", 10 to "10th", 11 to "11th", 12 to "12th",
            13 to "13th", 15 to "15th", 21 to "21st", 22 to "22nd", 23 to "23rd", 28 to "28th",
        )
        expected.forEach { (n, word) -> assertEquals(word, ordinal(n)) }
        assertEquals("the 1st", monthStartLabel(1))
        assertEquals("the 15th", monthStartLabel(15))
    }

    @Test fun currencyListKeepsThePickVisible() {
        assertEquals(COMMON_CURRENCIES, currencyCodes("CAD"))
        val odd = currencyCodes("ISK")
        assertEquals("ISK", odd.first())
        assertEquals(COMMON_CURRENCIES.size + 1, odd.size)
        assertEquals(16, COMMON_CURRENCIES.size)
        assertEquals(COMMON_CURRENCIES.size, COMMON_CURRENCIES.distinct().size)
    }

    @Test fun onboardingListPutsTheDeviceFirst() {
        assertEquals(listOf("CAD", "USD", "EUR", "GBP"), onboardingCurrencyCodes("CAD", "CAD", expanded = false))
        assertEquals(listOf("JPY", "CAD", "USD", "EUR", "GBP"), onboardingCurrencyCodes("JPY", "CAD", expanded = false))
        assertEquals(listOf("EUR", "SEK", "CAD", "USD", "GBP"), onboardingCurrencyCodes("EUR", "SEK", expanded = false))
        val all = onboardingCurrencyCodes("CAD", "CAD", expanded = true)
        assertEquals(all.distinct(), all)
        assertTrue(all.containsAll(COMMON_CURRENCIES))
    }

    @Test fun sampleAmountFollowsTheCurrencysDecimals() {
        assertEquals(123_456L, sampleMinor(2))
        assertEquals(1_234L, sampleMinor(0))
        assertEquals(1_234_560L, sampleMinor(3))
        val cad = currencyOption("CAD", Locale.CANADA)
        assertEquals("CAD", cad.code)
        assertEquals(2, cad.fractionDigits)
        assertTrue(cad.sample, cad.sample.contains("1,234.56"))
        val yen = currencyOption("JPY", Locale.CANADA)
        assertEquals(0, yen.fractionDigits)
        assertTrue(yen.sample, yen.sample.contains("1,234"))
        assertFalse(yen.sample, yen.sample.contains("."))
        assertEquals("No decimals", yen.decimalsLine)
        assertEquals("2 decimal places", cad.decimalsLine)
    }

    @Test fun subtitleDropsASymbolThatIsJustTheCode() {
        assertEquals("e.g. CHF 1,234.56", CurrencyOption("CHF", "Swiss Franc", "CHF", "CHF 1,234.56", 2).subtitle)
        assertEquals("$ · e.g. $1,234.56", CurrencyOption("CAD", "Canadian Dollar", "$", "$1,234.56", 2).subtitle)
    }

    @Test fun rowSummaries() {
        assertEquals("Warm dark · Ember accent", appearanceSummary(amoled = false, accentEnabled = true, accent = Accent.EMBER))
        assertEquals("Pure black · Sky accent", appearanceSummary(amoled = true, accentEnabled = true, accent = Accent.SKY))
        assertEquals("Pure black · no accent", appearanceSummary(amoled = true, accentEnabled = false, accent = Accent.SKY))
        assertEquals("CAD · month starts on the 1st · week from Monday", formatSummary("CAD", 1, true))
        assertEquals("EUR · month starts on the 15th · week from Sunday", formatSummary("EUR", 15, false))
        assertEquals("No accounts yet", accountsSummary(0, "$0"))
        assertEquals("1 account · net $120", accountsSummary(1, "$120"))
        assertEquals("4 accounts · net $10,096", accountsSummary(4, "$10,096"))
        assertEquals("13 for spending · 4 for income", categoriesSummary(13, 4))
        assertEquals("None yet", categoriesSummary(0, 0))
    }

    @Test fun fileNamesCarryTheDay() {
        val day = LocalDate.of(2026, 10, 4)
        assertEquals("tally-backup-2026-10-04.json", backupFileName(day))
        assertEquals("tally-entries-2026-10-04.csv", csvFileName(day))
    }

    @Test fun restorePreviewReadsTheFile() {
        val file = BackupFile(exportedAt = "2026-10-03", currency = "CAD")
        val preview = restorePreview(file)
        assertEquals(0, preview.entries)
        assertEquals(LocalDate.of(2026, 10, 3), preview.savedOn)
        assertNull(restorePreview(file.copy(exportedAt = "last week")).savedOn)
        assertEquals("Replace everything with this backup? It holds 1 entry.", restorePrompt(1, null))
        assertEquals(
            "Replace everything with this backup? It holds 1284 entries, saved 3 Oct.",
            restorePrompt(1284, "3 Oct"),
        )
    }

    @Test fun countsGroupByLocale() {
        assertEquals("1,284", countText(1_284, Locale.CANADA))
        assertEquals("0", countText(0, Locale.CANADA))
    }

    @Test fun roundingPromptSaysWhatIsLostAndThatItStaysLost() {
        assertEquals(
            "Japanese yen has no decimals, so 37 amounts will be rounded to fit. Switching back later does not restore them.",
            roundingPrompt("Japanese yen", 0, 37),
        )
        assertEquals(
            "Canadian dollar has 2 decimal places, so 1 amount will be rounded to fit. Switching back later does not restore it.",
            roundingPrompt("Canadian dollar", 2, 1),
        )
        assertTrue(roundingPrompt("Malagasy ariary", 1, 2).startsWith("Malagasy ariary has 1 decimal place, so 2 amounts"))
    }

    @Test fun copyKeepsTheVoice() {
        val lines = listOf(
            appearanceSummary(false, false, Accent.RED), formatSummary("USD", 2, true), accountsSummary(2, "$1"),
            categoriesSummary(1, 1), restorePrompt(3, "1 Oct"), restorePrompt(0, null),
            roundingPrompt("Japanese yen", 0, 37), roundingPrompt("Kuwaiti dinar", 3, 1),
        ) + DataOp.entries.map { it.working }
        lines.forEach { line -> Copy.banned.forEach { bad -> assertFalse("\"$line\" contains \"$bad\"", line.contains(bad, ignoreCase = true)) } }
    }

    @Test fun searchFindsPagesBySynonymAndNamesFirst() {
        assertEquals(SettingsDest.IMPORT, searchSettings("desjardins").first().dest)
        assertEquals(SettingsDest.IMPORT, searchSettings("Wealthsimple").first().dest)
        // A name that starts with the words outranks one that only mentions them.
        val back = searchSettings("back").map { it.name }
        assertEquals(listOf("Back up now", "Backup", "Backup folder"), back.take(3))
        assertTrue("Weekly backup" in back.drop(3))
        assertTrue(searchSettings("   ").isEmpty())
        assertTrue(searchSettings("zzzz").isEmpty())
        assertEquals("Data", searchSettings("backup").first().where)
    }

    @Test fun theBackupLineSaysWhenAndWhetherItRuns() {
        assertEquals("Weekly · no backup yet", backupSummary(auto = true, lastOn = null, failed = false))
        assertEquals("Off · last 3 Oct", backupSummary(auto = false, lastOn = "3 Oct", failed = false))
        assertEquals("Last backup failed · back up now", backupSummary(auto = true, lastOn = "3 Oct", failed = true))
    }
}
