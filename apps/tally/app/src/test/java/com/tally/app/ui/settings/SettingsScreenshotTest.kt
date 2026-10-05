package com.tally.app.ui.settings

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.backup.BackupCopy
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.prefs.Accent
import com.tally.app.data.prefs.BackupPrefs
import com.tally.app.data.prefs.Settings
import com.tally.app.data.repo.DataResult
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.BudgetPeriod
import com.tally.core.CategoryKind
import com.tally.core.Statement
import com.tally.core.StatementFormat
import com.tally.core.StatementRow
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.time.ZoneId

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class SettingsScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val today = Fixtures.TODAY

    // ── Settings ─────────────────────────────────────────────────────────────

    private val settings = SettingsState(
        today = today,
        settings = Settings(currency = "CAD"),
        period = Fixtures.PERIOD,
        version = "1.0",
        entries = 1_284,
        accounts = 4,
        net = 1_009_580,
        spendingCategories = 13,
        incomeCategories = 4,
        budgets = 5,
        bills = 3,
        goals = 2,
        loaded = true,
    )

    private val settingsZero = SettingsState(
        today = today,
        settings = Settings(currency = "CAD"),
        period = Fixtures.PERIOD,
        version = "1.0",
        loaded = true,
    )

    @Test fun settings() = compose.shoot("settings") { SettingsScreen(settings, SettingsActions()) }
    @Test fun settings200() = compose.shoot("settings-200", fontScale = 2f) { SettingsScreen(settings, SettingsActions()) }
    @Test fun settingsZero() = compose.shoot("settings-zero") { SettingsScreen(settingsZero, SettingsActions()) }

    // ── Appearance ───────────────────────────────────────────────────────────

    private val appearance = AppearanceState(accent = Accent.EMBER, accentEnabled = true, amoled = false, loaded = true)

    @Test fun appearance() = compose.shoot("appearance") { AppearanceScreen(appearance, AppearanceActions()) }
    @Test fun appearance200() = compose.shoot("appearance-200", fontScale = 2f) { AppearanceScreen(appearance, AppearanceActions()) }
    @Test fun appearanceMono() = compose.shoot("appearance-mono", accentEnabled = false) {
        AppearanceScreen(appearance.copy(accentEnabled = false), AppearanceActions())
    }
    @Test fun appearanceBlack() = compose.shoot("appearance-black", amoled = true) {
        AppearanceScreen(appearance.copy(amoled = true), AppearanceActions())
    }

    // ── Currency & dates ─────────────────────────────────────────────────────

    private val format = FormatState(
        today = today,
        currency = "CAD",
        monthStartDay = 1,
        weekStartsMonday = true,
        period = Fixtures.PERIOD,
        loaded = true,
    )

    @Test fun format() = compose.shoot("format") { FormatScreen(format, FormatActions()) }
    @Test fun format200() = compose.shoot("format-200", fontScale = 2f) { FormatScreen(format, FormatActions()) }
    @Test fun formatPayday() = compose.shoot("format-payday") {
        FormatScreen(
            format.copy(currency = "EUR", monthStartDay = 15, weekStartsMonday = false, period = BudgetPeriod.containing(today, 15)),
            FormatActions(),
        )
    }

    // ── Data: backup, export, import, about ─────────────────────────────────

    private val data = DataState(today = today, period = Fixtures.PERIOD, entries = 1_284, loaded = true)

    @Test fun settingsSearch() = compose.shoot("settings-search") { SettingsScreen(settings, SettingsActions(), data, query = "back") }

    private val savedAt = today.atTime(9, 30).atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()

    private val backup = BackupState(
        today = today,
        prefs = BackupPrefs(auto = true, folderUri = null, lastAt = savedAt),
        copies = listOf(
            BackupCopy("tally-auto-2026-10-14-093000.json", savedAt, 412_000),
            BackupCopy("tally-auto-2026-10-07-093000.json", savedAt - 7 * 86_400_000L, 398_500),
        ),
        entries = 1_284,
        result = DataResult.Done("Backed up"),
        loaded = true,
    )

    @Test fun backup() = compose.shoot("backup") { BackupScreen(backup, BackupActions()) }
    @Test fun backup200() = compose.shoot("backup-200", fontScale = 2f) { BackupScreen(backup, BackupActions()) }
    @Test fun backupZero() = compose.shoot("backup-zero") {
        BackupScreen(BackupState(today = today, prefs = BackupPrefs(auto = false), loaded = true), BackupActions())
    }

    @Test fun export() = compose.shoot("export") { ExportScreen(data, ExportActions()) }
    @Test fun export200() = compose.shoot("export-200", fontScale = 2f) { ExportScreen(data, ExportActions()) }

    @Test fun about() = compose.shoot("about") { AboutScreen(settings) }

    private val groceries = CategoryEntity(1, "Groceries", CategoryKind.EXPENSE, 0, "cart")
    private val dining = CategoryEntity(2, "Dining", CategoryKind.EXPENSE, 6, "dining")
    private val salary = CategoryEntity(3, "Salary", CategoryKind.INCOME, 0, "work")

    private val statement = Statement(
        StatementFormat.DESJARDINS,
        listOf(
            StatementRow(today.minusDays(9), "PAIE EMPLOYEUR INC", 215_000, balance = 455_000),
            StatementRow(today.minusDays(8), "Achat - METRO PLUS #12", -6_842, balance = 448_158),
            StatementRow(today.minusDays(6), "Paiement carte VISA", -90_000, balance = 358_158),
            StatementRow(today.minusDays(4), "Achat - PHO LIEN", -2_310, balance = 355_848),
            StatementRow(today.minusDays(2), "Achat - CAFE OLIMPICO", -465, balance = 355_383),
        ),
    )

    private val importChoose = ImportState(today = today, accounts = Fixtures.accounts)

    private val importPreview = ImportState(
        today = today,
        stage = ImportStage.PREVIEW,
        source = BankSource.DESJARDINS,
        statement = statement,
        plan = planImport(statement, ImportChoices(), listOf(groceries, dining, salary), emptyMap(), emptyList()),
        accounts = Fixtures.accounts,
        targetId = 1,
    )

    @Test fun importChoose() = compose.shoot("import") { ImportScreen(importChoose, ImportActions()) }
    @Test fun importPreview() = compose.shoot("import-preview") { ImportScreen(importPreview, ImportActions()) }
    @Test fun importPreview200() = compose.shoot("import-preview-200", fontScale = 2f) { ImportScreen(importPreview, ImportActions()) }
}
