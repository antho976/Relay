package com.tally.app.ui.onboarding

import android.content.Context
import android.os.Looper
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.test.core.app.ApplicationProvider
import com.tally.app.data.FixedClock
import com.tally.app.data.RoomTestDb
import com.tally.app.data.db.BudgetEntity
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.ledgerRepository
import com.tally.app.data.planRepository
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.data.repo.DataRepository
import com.tally.core.AccountType
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import java.time.LocalDate

/** First run, end to end over a real database: every account added is written, the first as the default. */
@RunWith(RobolectricTestRunner::class)
class OnboardingViewModelTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val clock = FixedClock(LocalDate.of(2026, 10, 14))
    private lateinit var db: TallyDatabase
    private lateinit var settings: SettingsRepository

    @Before fun open() {
        db = RoomTestDb.create()
        settings = SettingsRepository(context)
        runBlocking {
            settings.resetData()
            settings.setCurrency("CAD")
        }
    }

    @After fun close() { db.close() }

    private fun awaitMain(what: String, done: () -> Boolean) {
        val end = System.currentTimeMillis() + 10_000
        while (true) {
            shadowOf(Looper.getMainLooper()).idle()
            if (done()) return
            check(System.currentTimeMillis() < end) { "Timed out waiting for $what" }
            Thread.sleep(10)
        }
    }

    private fun viewModel(): OnboardingViewModel {
        val ledger = db.ledgerRepository(clock)
        val vm = OnboardingViewModel(
            SavedStateHandle(),
            settings,
            ledger,
            db.planRepository(clock),
            DataRepository(context, db, settings, ledger, clock),
            clock,
        )
        awaitMain("the settings read") { vm.state.value.loaded }
        return vm
    }

    @Test fun everyAccountAddedIsCreatedAndTheFirstIsTheDefault() {
        val vm = viewModel()
        vm.next()
        assertEquals(OnboardingStep.ACCOUNT, vm.state.value.step)

        vm.setBalance("3184.50", 318_450)
        vm.addAccount()
        assertFalse("Adding closes the form", vm.state.value.formOpen)
        assertEquals(listOf("Chequing"), vm.state.value.accounts.map { it.name })

        vm.openNewAccount()
        assertEquals("The next form suggests a card", AccountType.CREDIT, vm.state.value.accountType)
        vm.setName("Visa")
        vm.setBalance("412.20", 41_220)
        // Continue takes the open form's account along; no extra tap for the last one.
        vm.next()
        vm.setBudget("2900", 290_000)
        vm.next()

        awaitMain("the setup written") { runBlocking { settings.current().onboarded } }
        val accounts = runBlocking { db.accounts().all() }
        assertEquals(listOf("Chequing", "Visa"), accounts.map { it.name })
        assertEquals(listOf(318_450L, -41_220L), accounts.map { it.openingBalance })
        assertEquals(accounts.first().id, runBlocking { settings.current().defaultAccountId })
        assertEquals(290_000L, runBlocking { db.budgets().getFor(BudgetEntity.OVERALL)?.amount })
        vm.viewModelScope.cancel()
    }

    @Test fun aClosedFormAddsNothingAndRemovingTheLastAccountReopensIt() {
        val vm = viewModel()
        vm.next()
        vm.addAccount()
        vm.openNewAccount()
        vm.cancelNewAccount()
        assertFalse(vm.state.value.formOpen)
        assertEquals(1, vm.state.value.toCreate.size)

        vm.removeAccount(0)
        assertTrue("With nothing added the form is back", vm.state.value.formOpen)
        assertTrue(vm.state.value.accounts.isEmpty())
        vm.viewModelScope.cancel()
    }
}
