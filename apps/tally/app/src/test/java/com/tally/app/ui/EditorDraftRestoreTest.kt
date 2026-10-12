package com.tally.app.ui

import android.content.Context
import android.os.Looper
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.test.core.app.ApplicationProvider
import com.tally.app.data.FixedClock
import com.tally.app.data.RoomTestDb
import com.tally.app.data.addAccount
import com.tally.app.data.db.TallyDatabase
import com.tally.app.data.investRepository
import com.tally.app.data.ledgerRepository
import com.tally.app.data.planRepository
import com.tally.app.data.prefs.SettingsRepository
import com.tally.app.ui.accounts.AccountDraft
import com.tally.app.ui.accounts.AccountEditViewModel
import com.tally.app.ui.common.Notices
import com.tally.app.ui.common.keepDraft
import com.tally.app.ui.common.savedDraft
import com.tally.app.ui.nav.Args
import com.tally.app.ui.plan.GoalDraft
import com.tally.app.ui.plan.GoalEditViewModel
import com.tally.app.ui.plan.SavedGoal
import com.tally.core.AccountType
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import java.time.LocalDate

/**
 * Process death, end to end: an editor built over a SavedStateHandle that already holds a draft
 * (what Navigation hands a recreated screen) shows that draft, not a blank form or the stored
 * row, and every later change is written back to the handle.
 */
@RunWith(RobolectricTestRunner::class)
class EditorDraftRestoreTest {

    private val today = LocalDate.of(2026, 10, 4)
    private val clock = FixedClock(today)
    private val notices = Notices(CoroutineScope(Job()))
    private lateinit var db: TallyDatabase
    private lateinit var settings: SettingsRepository

    @Before fun open() {
        db = RoomTestDb.create()
        settings = SettingsRepository(ApplicationProvider.getApplicationContext<Context>())
        runBlocking {
            settings.resetData()
            settings.setCurrency("CAD")
        }
    }

    @After fun close() { db.close() }

    /** Runs the main looper (where viewModelScope lives) until [done] holds, or fails after a while. */
    private fun awaitMain(what: String, done: () -> Boolean) {
        val end = System.currentTimeMillis() + 10_000
        while (true) {
            shadowOf(Looper.getMainLooper()).idle()
            if (done()) return
            check(System.currentTimeMillis() < end) { "Timed out waiting for $what" }
            Thread.sleep(10)
        }
    }

    /** The first state that is [loaded], read while something collects it, as the screen would. */
    private fun <S> StateFlow<S>.awaitState(loaded: (S) -> Boolean): S {
        var latest: S? = null
        val job = CoroutineScope(Dispatchers.Main.immediate).launch { collect { latest = it } }
        try {
            awaitMain("a loaded state") { latest?.let(loaded) == true }
        } finally {
            job.cancel()
        }
        return checkNotNull(latest)
    }

    @Test fun aNewGoalComesBackAsTyped() {
        val handle = SavedStateHandle(mapOf(Args.ID to 0L))
        val typed = GoalDraft(name = "Lisbon in May", targetText = "3200", target = 320_000, targetDate = LocalDate.of(2027, 5, 1), color = 4)
        handle.keepDraft(SavedGoal.serializer(), SavedGoal.of(typed))

        val vm = GoalEditViewModel(handle, db.planRepository(clock), db.ledgerRepository(clock), settings, notices, clock)
        val state = vm.state.awaitState { it.loaded }

        assertEquals(typed, state.draft)
        assertTrue(state.isNew)
        vm.viewModelScope.cancel()
    }

    @Test fun anEditedAccountComesBackAsTypedOverItsStoredRow() {
        val visa = runBlocking { db.addAccount("Visa", AccountType.CREDIT, opening = -40_000) }
        val handle = SavedStateHandle(mapOf(Args.ID to visa))
        val typed = AccountDraft(name = "Visa Infinite", type = AccountType.CREDIT, openingText = "512.30", opening = 51_230, owe = true)
        handle.keepDraft(AccountDraft.serializer(), typed)

        val vm = AccountEditViewModel(handle, db.ledgerRepository(clock), db.investRepository(clock, settings), settings, notices, clock)
        val state = vm.state.awaitState { it.loaded }

        assertEquals(typed, state.draft)
        assertTrue("Still editing the stored account", state.stored)
        assertEquals("Visa", state.storedName)
        vm.viewModelScope.cancel()
    }

    @Test fun everyChangeIsKeptForTheNextProcess() {
        val handle = SavedStateHandle(mapOf(Args.ID to 0L))
        val vm = AccountEditViewModel(handle, db.ledgerRepository(clock), db.investRepository(clock, settings), settings, notices, clock)
        vm.state.awaitState { it.loaded }

        vm.setName("Joint chequing")
        vm.setType(AccountType.SAVINGS)

        awaitMain("the draft in the handle") {
            handle.savedDraft(AccountDraft.serializer())?.let { it.name == "Joint chequing" && it.type == AccountType.SAVINGS } == true
        }
        vm.viewModelScope.cancel()
    }
}
