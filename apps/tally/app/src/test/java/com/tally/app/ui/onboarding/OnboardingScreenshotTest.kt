package com.tally.app.ui.onboarding

import androidx.compose.ui.test.junit4.createComposeRule
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
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
class OnboardingScreenshotTest {

    @get:Rule val compose = createComposeRule()

    private val base = OnboardingState(
        today = Fixtures.TODAY,
        period = Fixtures.PERIOD,
        currency = "CAD",
        deviceCurrency = "CAD",
        loaded = true,
    )

    private val account = base.copy(step = OnboardingStep.ACCOUNT, balanceText = "3184.50", balance = 318_450)
    private val budget = base.copy(step = OnboardingStep.BUDGET, budgetText = "2900", budget = 290_000)

    @Test fun currency() = compose.shoot("onboarding-currency") { OnboardingScreen(base, OnboardingActions()) }
    @Test fun currencyMore() = compose.shoot("onboarding-currency-more") {
        OnboardingScreen(base.copy(deviceCurrency = "USD", moreCurrencies = true), OnboardingActions())
    }
    @Test fun currency200() = compose.shoot("onboarding-200", fontScale = 2f) { OnboardingScreen(base, OnboardingActions()) }

    @Test fun account() = compose.shoot("onboarding-account") { OnboardingScreen(account, OnboardingActions()) }
    @Test fun accountCredit() = compose.shoot("onboarding-account-credit") {
        OnboardingScreen(
            account.copy(accountType = AccountType.CREDIT, accountName = "Credit card", balanceText = "412.2", balance = 41_220),
            OnboardingActions(),
        )
    }
    @Test fun accountError() = compose.shoot("onboarding-account-error") {
        OnboardingScreen(account.copy(balanceText = "about a grand", balance = null), OnboardingActions())
    }
    @Test fun account200() = compose.shoot("onboarding-account-200", fontScale = 2f) { OnboardingScreen(account, OnboardingActions()) }

    private val added = listOf(
        OnboardAccount("Chequing", AccountType.CHEQUING, "3184.50", 318_450),
        OnboardAccount("Visa", AccountType.CREDIT, "412.20", 41_220),
    )

    @Test fun accountsAdded() = compose.shoot("onboarding-accounts") {
        OnboardingScreen(
            account.copy(accounts = added, editing = false, accountType = AccountType.SAVINGS, accountName = "Savings", balanceText = "", balance = null),
            OnboardingActions(),
        )
    }
    @Test fun accountsNext() = compose.shoot("onboarding-accounts-next") {
        OnboardingScreen(
            account.copy(accounts = added, editing = true, accountType = AccountType.SAVINGS, accountName = "Savings", balanceText = "7250", balance = 725_000),
            OnboardingActions(),
        )
    }

    @Test fun budget() = compose.shoot("onboarding-budget") { OnboardingScreen(budget, OnboardingActions()) }
    @Test fun budgetZero() = compose.shoot("onboarding-budget-zero") {
        OnboardingScreen(base.copy(step = OnboardingStep.BUDGET), OnboardingActions())
    }
    @Test fun budget200() = compose.shoot("onboarding-budget-200", fontScale = 2f) { OnboardingScreen(budget, OnboardingActions()) }
}
