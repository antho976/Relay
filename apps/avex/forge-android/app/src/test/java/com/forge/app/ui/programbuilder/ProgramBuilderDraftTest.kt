package com.forge.app.ui.programbuilder

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The builder draft's JSON round-trip (H-13). Everything the screen keys on — day and exercise uids,
 * the open day, the open dialog and its exercise — must come back byte-for-byte, and a blob this
 * build cannot read must read as "no draft" rather than as a corrupt plan.
 *
 * Robolectric because the draft round-trips through `org.json`, which the Android unit-test stub
 * jar leaves unimplemented (see [com.forge.app.ui.gym.freestyle.FreestyleDraftUnitTest]).
 */
@RunWith(RobolectricTestRunner::class)
class ProgramBuilderDraftTest {

    private fun ex(id: String, sets: Int = 3, reps: String = "8-12") =
        BuilderExercise(uid = "u-$id", libId = id, name = id, muscle = "Chest", sets = sets, reps = reps)

    private val days = listOf(
        BuilderDay(
            uid = "ud1", key = "day-a", name = "Push", archetype = "push", accentHex = "#E85D4A",
            exercises = listOf(ex("db-bench-press", 4, "6-10"), ex("pec-deck")), word = "PUSH"
        ),
        BuilderDay(
            uid = "ud2", key = "day-b", name = "Legs", archetype = "lower-b", accentHex = "#D4A017",
            exercises = emptyList()
        )
    )

    private fun roundTrip(draft: ProgramBuilderDraft): ProgramBuilderDraft =
        ProgramBuilderDraft.fromJson(draft.toJson()) ?: error("draft did not parse")

    @Test
    fun `document dirty flag open day and sheet survive the round trip`() {
        val draft = ProgramBuilderDraft(
            days = days, dirty = true, openDayUid = "ud1", dialog = DayDialog.SetsReps("u-pec-deck")
        )
        assertEquals(draft, roundTrip(draft))
    }

    @Test
    fun `every dialog kind survives the round trip`() {
        val dialogs = listOf(
            DayDialog.None, DayDialog.Rename, DayDialog.AddExercises,
            DayDialog.SetsReps("u-db-bench-press"), DayDialog.Swap("u-db-bench-press")
        )
        for (dialog in dialogs) {
            val draft = ProgramBuilderDraft(days = days, dirty = false, openDayUid = "ud1", dialog = dialog)
            assertEquals(dialog, roundTrip(draft).dialog)
        }
    }

    @Test
    fun `a draft on the plan overview reads back with no open day`() {
        val draft = ProgramBuilderDraft(days = days, dirty = false, openDayUid = null, dialog = DayDialog.None)
        val restored = roundTrip(draft)
        assertNull(restored.openDayUid)
        assertEquals(DayDialog.None, restored.dialog)
        assertEquals(false, restored.dirty)
    }

    @Test
    fun `an empty plan round trips`() {
        val draft = ProgramBuilderDraft(days = emptyList(), dirty = true, openDayUid = null, dialog = DayDialog.None)
        assertEquals(draft, roundTrip(draft))
    }

    @Test
    fun `an unreadable blob or a stale schema reads as no draft`() {
        assertNull(ProgramBuilderDraft.fromJson("not json"))
        assertNull(ProgramBuilderDraft.fromJson("""{"schema":99,"days":[]}"""))
        assertNull(ProgramBuilderDraft.fromJson("""{"days":[]}"""))
    }

    @Test
    fun `an exercise scoped dialog without its exercise falls back to none`() {
        val json = """{"schema":1,"dirty":true,"dialog":"sets","days":[]}"""
        assertEquals(DayDialog.None, ProgramBuilderDraft.fromJson(json)?.dialog)
    }
}
