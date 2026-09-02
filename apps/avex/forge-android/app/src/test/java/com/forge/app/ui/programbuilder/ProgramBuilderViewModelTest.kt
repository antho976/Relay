package com.forge.app.ui.programbuilder

import androidx.lifecycle.SavedStateHandle
import com.forge.app.data.db.entities.ProgramDay
import com.forge.app.data.db.entities.ProgramSlot
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Process recreation of the Program Builder (H-13). Android kills the process behind a retained
 * task; navigation restores the route and Hilt builds a NEW ViewModel on the SAME SavedStateHandle.
 * Before, that new ViewModel held an empty, non-dirty plan and reloaded the saved program, so every
 * unsaved day, exercise and sets × reps edit was gone with no discard warning. Now the second
 * ViewModel must find the whole draft, the dirty flag and where the user was in the editor, and
 * only Save or an explicit Discard may clear it.
 *
 * "Recreation" here is literally constructing a second ViewModel over the first one's handle; the
 * data layer is a fake behind [ProgramBuilderStore], as the real repositories need Room and
 * DataStore. Robolectric because the draft round-trips through `org.json`.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class ProgramBuilderViewModelTest {

    private class FakeStore : ProgramBuilderStore {
        var dayRows: List<ProgramDay> = emptyList()
        var slotRows: List<ProgramSlot> = emptyList()
        var saved: Pair<List<ProgramDay>, List<ProgramSlot>>? = null
        override val freestyleMode: Flow<Boolean> = MutableStateFlow(false)
        override suspend fun currentDayRows(): List<ProgramDay> = dayRows
        override suspend fun slotRowsForDay(dayId: String): List<ProgramSlot> = slotRows.filter { it.dayId == dayId }
        override suspend fun saveCustomProgram(days: List<ProgramDay>, slots: List<ProgramSlot>) {
            saved = days to slots
            dayRows = days
            slotRows = slots
        }
        override suspend fun setFreestyleMode(v: Boolean) {}
        override suspend fun guardProgramChange(action: suspend () -> Unit) = action()
    }

    @Before
    fun setUp() {
        // viewModelScope runs on Main.immediate; Unconfined so launches complete inline and the
        // assertions read settled state, the way the screen does after the load lands.
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun savedDraft(handle: SavedStateHandle): String? = handle.get<String>(ProgramBuilderViewModel.KEY_DRAFT)

    /** A blank builder with one day, two exercises (one re-set), open in the day editor's sheet. */
    private fun ProgramBuilderViewModel.buildDraft(): Pair<String, String> {
        loadIfNeeded(blank = true)
        addDay()
        val dayUid = days.single().uid
        addExercises(dayUid, listOf("db-bench-press", "pec-deck"))
        val exUid = days.single().exercises[1].uid
        setExercise(dayUid, exUid, 5, "6-8")
        renameDay(dayUid, "Push A")
        openDay(dayUid)
        updateDayDialog(DayDialog.SetsReps(exUid))
        return dayUid to exUid
    }

    @Test
    fun `a recreated ViewModel restores the draft the dirty flag and the open editor`() {
        val store = FakeStore()
        val handle = SavedStateHandle()
        val first = ProgramBuilderViewModel(store, handle)
        val (dayUid, exUid) = first.buildDraft()
        assertTrue(first.dirty)
        assertNotNull("every edit writes the draft into the handle", savedDraft(handle))

        // Process killed; navigation restores the route; Hilt builds a new ViewModel on the same handle.
        val second = ProgramBuilderViewModel(store, handle)
        // The screen's LaunchedEffect fires again — it must NOT reload the (empty) saved program over the draft.
        second.loadIfNeeded(blank = false)

        assertEquals(first.days, second.days)
        assertEquals("Push A", second.days.single().name)
        assertEquals(5, second.days.single().exercises[1].sets)
        assertEquals("6-8", second.days.single().exercises[1].reps)
        assertTrue("the discard warning still has to appear", second.dirty)
        assertTrue(second.loadComplete)
        assertEquals(dayUid, second.openDayUid)
        assertEquals(DayDialog.SetsReps(exUid), second.dayDialog)
    }

    @Test
    fun `an editor move alone is enough to persist a non dirty snapshot`() {
        val store = FakeStore().apply {
            dayRows = listOf(ProgramDay(id = "day-a", position = 0, name = "Push", word = "PUSH", accentHex = "#E85D4A", archetype = "push"))
            slotRows = listOf(ProgramSlot(id = "day-a-0", dayId = "day-a", position = 0, exerciseLibId = "db-bench-press", sets = 3, reps = "8-12"))
        }
        val handle = SavedStateHandle()
        val first = ProgramBuilderViewModel(store, handle)
        first.loadIfNeeded(blank = false)
        assertNull("no interaction yet, nothing to restore", savedDraft(handle))
        val dayUid = first.days.single().uid
        first.openDay(dayUid)
        first.updateDayDialog(DayDialog.Rename)

        val second = ProgramBuilderViewModel(store, handle)
        second.loadIfNeeded(blank = false)
        // Restored uids match the restored days, so the open day still resolves.
        assertEquals(dayUid, second.openDayUid)
        assertNotNull(second.day(dayUid))
        assertEquals(DayDialog.Rename, second.dayDialog)
        assertFalse(second.dirty)
    }

    @Test
    fun `nothing is persisted before the initial load has landed`() {
        val handle = SavedStateHandle()
        val vm = ProgramBuilderViewModel(FakeStore(), handle)
        // No loadIfNeeded yet: a snapshot of the empty pre-load list would restore as an empty, loaded plan.
        vm.closeDay()
        assertNull(savedDraft(handle))
    }

    @Test
    fun `save clears the draft and a recreated ViewModel loads the saved program`() {
        val store = FakeStore()
        val handle = SavedStateHandle()
        val first = ProgramBuilderViewModel(store, handle)
        first.buildDraft()
        first.closeDay()
        var saved = false
        first.save { saved = true }

        assertTrue(saved)
        assertNotNull(store.saved)
        assertFalse(first.dirty)
        assertNull("the saved program is the document now", savedDraft(handle))

        val second = ProgramBuilderViewModel(store, handle)
        assertFalse("no draft, so the normal load path runs", second.loadComplete)
        second.loadIfNeeded(blank = false)
        assertEquals(listOf("Push A"), second.days.map { it.name })
        assertEquals(listOf(3, 5), second.days.single().exercises.map { it.sets })
        assertFalse(second.dirty)
    }

    @Test
    fun `discard clears the draft and the editor position`() {
        val store = FakeStore()
        val handle = SavedStateHandle()
        val first = ProgramBuilderViewModel(store, handle)
        first.buildDraft()
        first.closeDay()
        first.discardEdits()

        assertNull(savedDraft(handle))
        assertFalse(first.dirty)
        assertNull(first.openDayUid)
        assertEquals(DayDialog.None, first.dayDialog)
        assertTrue(first.days.isEmpty())   // the fake store holds no saved program

        val second = ProgramBuilderViewModel(store, handle)
        assertFalse(second.loadComplete)
        assertTrue(second.days.isEmpty())
    }

    @Test
    fun `an unreadable draft is ignored and the saved program loads`() {
        val store = FakeStore().apply {
            dayRows = listOf(ProgramDay(id = "day-a", position = 0, name = "Push", word = "PUSH", accentHex = "#E85D4A", archetype = "push"))
        }
        val handle = SavedStateHandle(mapOf(ProgramBuilderViewModel.KEY_DRAFT to "not a draft"))
        val vm = ProgramBuilderViewModel(store, handle)
        assertFalse(vm.loadComplete)
        vm.loadIfNeeded(blank = false)
        assertTrue(vm.loadComplete)
        assertEquals(listOf("Push"), vm.days.map { it.name })
        assertFalse(vm.dirty)
    }
}
