package com.tally.app.ui.accounts

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.db.AccountBalance
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.core.AccountType
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class AccountsScreenshotTest {

    @get:Rule val compose = createComposeRule()

    // ── The list ─────────────────────────────────────────────────────────────

    private val balances = Fixtures.accounts +
        AccountBalance(5, "Old Mastercard", AccountType.CREDIT, 0, true, 4, 0, 12)

    private val populated = buildAccountsState(balances, defaultAccountId = 1, entries = 108)

    private val zero = buildAccountsState(emptyList(), defaultAccountId = 0, entries = 0)

    @Test fun accounts() = compose.shoot("accounts") { AccountsScreen(populated, AccountsActions()) }
    @Test fun accounts200() = compose.shoot("accounts-200", fontScale = 2f) { AccountsScreen(populated, AccountsActions()) }
    @Test fun accountsZero() = compose.shoot("accounts-zero") { AccountsScreen(zero, AccountsActions()) }

    // ── The editor ───────────────────────────────────────────────────────────

    private val visa = AccountDraft(
        name = "Visa",
        type = AccountType.CREDIT,
        openingText = "412.80",
        opening = 41_280,
        owe = true,
    )

    private val editing = AccountEditState(
        id = 2,
        draft = visa,
        loaded = true,
        stored = true,
        storedName = "Visa",
        balance = -41_220,
        entryCount = 61,
        share = SharePreview(1f, 41_220),
        problems = accountProblems(visa),
    )

    private val fresh = AccountDraft(isDefault = true)

    private val creating = AccountEditState(
        draft = fresh,
        loaded = true,
        problems = accountProblems(fresh),
        showErrors = true,
    )

    @Test fun accountEdit() = compose.shoot("account-edit") { AccountEditScreen(editing, AccountEditActions()) }
    @Test fun accountEdit200() = compose.shoot("account-edit-200", fontScale = 2f) { AccountEditScreen(editing, AccountEditActions()) }
    @Test fun accountNew() = compose.shoot("account-new") { AccountEditScreen(creating, AccountEditActions()) }
}
