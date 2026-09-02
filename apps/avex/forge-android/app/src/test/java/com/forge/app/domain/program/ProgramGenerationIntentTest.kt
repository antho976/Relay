package com.forge.app.domain.program

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * M-06: the deload marker and the program it describes live in two stores that nothing can write
 * together, so an interrupted regeneration has to be finishable afterwards from what it left
 * behind. This is that decision, and the signature it turns on.
 *
 * Robolectric because the record round-trips through `org.json`, which the unit-test stub jar
 * leaves unimplemented.
 */
@RunWith(RobolectricTestRunner::class)
class ProgramGenerationIntentTest {

    private fun day(id: String, vararg slots: ProgramSignatureSlot) =
        ProgramSignatureDay(id, archetype = id, slots = slots.toList())

    private val fullVolume = listOf(
        day("push", ProgramSignatureSlot("bench_press", 4, "8-12"), ProgramSignatureSlot("ohp", 3, "8-12")),
        day("pull", ProgramSignatureSlot("barbell_row", 4, "8-12"))
    )
    private val deloadVolume = listOf(
        day("push", ProgramSignatureSlot("bench_press", 2, "8-12"), ProgramSignatureSlot("ohp", 2, "8-12")),
        day("pull", ProgramSignatureSlot("barbell_row", 2, "8-12"))
    )

    private fun intent(deload: Boolean, before: List<ProgramSignatureDay>) =
        ProgramGenerationIntent(deload = deload, atMs = 1_767_600_000_000L, beforeSignature = programSignature(before))

    @Test
    fun noIntentIsNothingToDo() {
        assertEquals(PendingGeneration.None, resolvePendingGeneration(null, programSignature(fullVolume)))
    }

    @Test
    fun anUnchangedProgramMeansTheTransactionNeverCommitted() {
        // A deload generate that threw. The plan is still the full-volume one, so the marker must
        // NOT be set — this is the case that used to leave a permanent false deload.
        assertEquals(
            PendingGeneration.Discard,
            resolvePendingGeneration(intent(deload = true, before = fullVolume), programSignature(fullVolume))
        )
        assertEquals(
            PendingGeneration.Discard,
            resolvePendingGeneration(intent(deload = false, before = fullVolume), programSignature(fullVolume))
        )
    }

    @Test
    fun aChangedProgramMeansTheRowsLandedAndTheMarkerIsOwed() {
        val pending = resolvePendingGeneration(
            intent(deload = true, before = fullVolume),
            programSignature(deloadVolume)
        )
        assertEquals(PendingGeneration.Apply(1_767_600_000_000L), pending)
    }

    @Test
    fun anOrdinaryRegenerateThatLandedClearsTheMarkerRatherThanSettingOne() {
        // The mirror case: a normal program committed, the process died before the old marker was
        // cleared. Zero is "not in a deload week".
        val pending = resolvePendingGeneration(
            intent(deload = false, before = deloadVolume),
            programSignature(fullVolume)
        )
        assertEquals(PendingGeneration.Apply(0L), pending)
    }

    @Test
    fun theSignatureCoversWhatARegenerationRewritesAndNothingElse() {
        assertEquals(programSignature(fullVolume), programSignature(fullVolume))
        // Set counts are the whole point: a deload is the same movements at less volume.
        assertNotEquals(programSignature(fullVolume), programSignature(deloadVolume))
        // Movements, rep ranges, day identity and day order all count too.
        assertNotEquals(
            programSignature(fullVolume),
            programSignature(listOf(day("push", ProgramSignatureSlot("db_press", 4, "8-12"), ProgramSignatureSlot("ohp", 3, "8-12")), fullVolume[1]))
        )
        assertNotEquals(
            programSignature(fullVolume),
            programSignature(listOf(day("push", ProgramSignatureSlot("bench_press", 4, "5-8"), ProgramSignatureSlot("ohp", 3, "8-12")), fullVolume[1]))
        )
        assertNotEquals(programSignature(fullVolume), programSignature(fullVolume.reversed()))
        assertEquals("an empty plan has an empty signature", "", programSignature(emptyList()))
    }

    @Test
    fun theRecordSurvivesARoundTripAndACorruptOneReadsAsNothingInFlight() {
        val original = intent(deload = true, before = fullVolume)
        assertEquals(original, ProgramGenerationIntent.fromJson(original.toJson()))

        assertNull(ProgramGenerationIntent.fromJson(null))
        assertNull(ProgramGenerationIntent.fromJson(""))
        assertNull(ProgramGenerationIntent.fromJson("{not json"))
        assertNull("no instant is no record", ProgramGenerationIntent.fromJson("""{"deload":true}"""))
    }
}
