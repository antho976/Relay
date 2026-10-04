package com.tally.app.ui.settings

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.prefs.Accent
import com.tally.app.data.prefs.Settings
import com.tally.app.data.repo.DataResult
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.BudgetPeriod
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

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

    // ── Backup & data ────────────────────────────────────────────────────────

    private val data = DataState(
        today = today,
        entries = 1_284,
        accounts = 4,
        categories = 17,
        budgets = 5,
        bills = 3,
        goals = 2,
        newest = Fixtures.rows.first(),
        results = mapOf(DataGroup.BACKUP to DataResult.Done("Backup saved")),
        loaded = true,
    )

    private val dataZero = DataState(today = today, categories = 17, loaded = true)

    @Test fun data() = compose.shoot("data") { DataScreen(data, DataActions()) }
    @Test fun data200() = compose.shoot("data-200", fontScale = 2f) { DataScreen(data, DataActions()) }
    @Test fun dataZero() = compose.shoot("data-zero") { DataScreen(dataZero, DataActions()) }
    @Test fun dataSampleBusy() = compose.shoot("data-sample") {
        DataScreen(
            data.copy(
                sampleLoaded = true,
                busy = DataOp.EXPORT_CSV,
                results = mapOf(DataGroup.BACKUP to DataResult.Failed("This file is not a Tally backup.")),
            ),
            DataActions(),
        )
    }
}
