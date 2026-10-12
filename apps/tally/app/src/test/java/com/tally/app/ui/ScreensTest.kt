package com.tally.app.ui

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.CategoryEntity
import com.tally.app.data.sync.Connect
import com.tally.app.data.sync.PairedPc
import com.tally.app.data.sync.SyncPrefs
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.app.ui.accounts.AccountsScreen
import com.tally.app.ui.accounts.buildAccountsState
import com.tally.app.ui.activity.ActivityState
import com.tally.app.ui.activity.EntriesActions
import com.tally.app.ui.activity.EntriesScreen
import com.tally.app.ui.activity.groupByDay
import com.tally.app.ui.activity.summarize
import com.tally.app.ui.entry.EntryActions
import com.tally.app.ui.entry.EntryDraft
import com.tally.app.ui.entry.EntryScreen
import com.tally.app.ui.entry.EntryState
import com.tally.app.ui.home.Envelope
import com.tally.app.ui.home.HomeState
import com.tally.app.ui.home.OverviewActions
import com.tally.app.ui.home.OverviewScreen
import com.tally.app.ui.home.UpcomingBill
import com.tally.app.ui.plan.BudgetLine
import com.tally.app.ui.plan.BudgetsReading
import com.tally.app.ui.plan.BudgetsScreen
import com.tally.app.ui.plan.PlanState
import com.tally.app.ui.settings.SettingsState
import com.tally.app.ui.settings.SyncScreen
import com.tally.core.AmountInput
import com.tally.core.CategoryKind
import com.tally.core.PaceReading
import com.tally.core.TxType
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The main screens drawn from state alone, as the app draws them, at 100% and 200% font: each must
 * compose without throwing and show its reading. The captures become the goldens CI compares.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class ScreensTest {

    @get:Rule val compose = createComposeRule()

    private val period = Fixtures.PERIOD
    private val elapsed = period.elapsedDays(Fixtures.TODAY)

    private fun home() = HomeState(
        today = Fixtures.TODAY,
        period = period,
        income = 215_000,
        spent = 141_220,
        reading = PaceReading(300_000, 141_220, period.days, elapsed),
        envelopes = listOf(
            Envelope(1, "Groceries", "cart", 0, PaceReading(60_000, 41_800, period.days, elapsed)),
            Envelope(2, "Dining", "dining", 6, PaceReading(20_000, 23_100, period.days, elapsed)),
        ),
        upcoming = listOf(UpcomingBill(1, "Rent", 145_000, TxType.EXPENSE, Fixtures.TODAY.plusDays(2), 2, "home", 1, "Chequing", true)),
        recent = Fixtures.rows,
        accounts = Fixtures.accounts,
        hasEntries = true,
        loaded = true,
        week = listOf(4_200, 0, 9_400, 6_842, 0, 0, 0),
        weekStart = Fixtures.TODAY.minusDays(3),
        lastPeriodSameDay = 120_000,
        lastPeriodIncomeSameDay = 215_000,
    )

    @Test fun overview() {
        compose.shoot("overview") { OverviewScreen(home(), OverviewActions()) }
        compose.onNodeWithText("Left this month").assertExists()
        compose.onNodeWithText("Rent").assertExists()
    }

    @Test fun overviewAtTwiceTheFont() {
        compose.shoot("overview-200", fontScale = 2f) { OverviewScreen(home(), OverviewActions()) }
    }

    @Test fun overviewEmpty() {
        compose.shoot("overview-empty") { OverviewScreen(HomeState(today = Fixtures.TODAY, period = period, loaded = true), OverviewActions()) }
        compose.onNodeWithText("Nothing logged yet").assertExists()
    }

    @Test fun entries() {
        val state = ActivityState(
            today = Fixtures.TODAY,
            period = period,
            days = groupByDay(Fixtures.rows),
            summary = summarize(Fixtures.rows),
            perDay = 141_220L / elapsed,
            loaded = true,
        )
        compose.shoot("entries") { EntriesScreen(state, EntriesActions()) }
        compose.onNodeWithText("Metro").assertExists()
    }

    @Test fun entryEditor() {
        val groceries = CategoryEntity(id = 1, name = "Groceries", kind = CategoryKind.EXPENSE, color = 0, icon = "cart")
        val dining = CategoryEntity(id = 2, name = "Dining", kind = CategoryKind.EXPENSE, color = 6, icon = "dining")
        val state = EntryState(
            draft = EntryDraft(date = Fixtures.TODAY, amount = AmountInput("12.5"), accountId = 2, categoryId = 1, categoryChosen = true),
            today = Fixtures.TODAY,
            period = period,
            loaded = true,
            categories = listOf(groceries, dining),
            accounts = Fixtures.accounts,
            category = groceries,
            account = Fixtures.accounts[1],
            leftAfter = 157_530,
        )
        compose.shoot("entry") { EntryScreen(state, EntryActions()) }
        compose.onNodeWithText("Groceries").assertExists()
    }

    @Test fun budgets() {
        val state = PlanState(
            today = Fixtures.TODAY,
            period = period,
            loaded = true,
            budgets = BudgetsReading(
                spent = 141_220,
                lastPeriodSpent = 180_000,
                overall = PaceReading(300_000, 141_220, period.days, elapsed),
                envelopes = listOf(BudgetLine(1, "Groceries", "cart", 0, PaceReading(60_000, 41_800, period.days, elapsed))),
                budgeted = 60_000,
                projected = 290_000,
            ),
        )
        compose.shoot("budgets") { BudgetsScreen(state, edit = {}, applySuggestions = {}, add = {}) }
        compose.onNodeWithText("Monthly budget").assertExists()
    }

    @Test fun accounts() {
        compose.shoot("accounts") { AccountsScreen(buildAccountsState(Fixtures.accounts, 1, 120), open = {}, add = {}, transfer = {}) }
        compose.onNodeWithText("Net worth").assertExists()
    }

    @Test fun syncWaitingOnDiscord() {
        val state = SettingsState(
            today = Fixtures.TODAY,
            connect = Connect.Waiting("7F3A", notified = true, expiresAt = Long.MAX_VALUE, server = "https://anthos-bot.onrender.com"),
            server = "https://anthos-bot.onrender.com",
            loaded = true,
        )
        compose.shoot("sync-waiting") { SyncScreen(state, {}, {}, {}, {}, {}, {}, {}, {}) }
        compose.onNodeWithText("7F3A").assertExists()
    }

    @Test fun syncConnected() {
        val pc = PairedPc("vm", "h1", "stable", listOf("wss://anthos-bot.onrender.com/relay/join/r"), "d1", "t1", 1L)
        val now = System.currentTimeMillis()
        val state = SettingsState(
            today = Fixtures.TODAY,
            pc = SyncPrefs(pc = pc, replaced = true, lastAttemptAt = now - 60_000, lastSuccessAt = now - 60_000, lastSent = 3, lastReceived = 12),
            connect = Connect.Connected,
            server = "https://anthos-bot.onrender.com",
            loaded = true,
        )
        compose.shoot("sync-connected") { SyncScreen(state, {}, {}, {}, {}, {}, {}, {}, {}) }
        compose.onNodeWithText("Sync now").assertExists()
    }
}
