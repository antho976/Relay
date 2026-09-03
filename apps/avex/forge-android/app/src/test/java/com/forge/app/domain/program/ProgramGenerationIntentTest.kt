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

    /** A LEGACY intent: recorded before `afterSignature` existed, so it can only use the old rule. */
    private fun intent(deload: Boolean, before: List<ProgramSignatureDay>) =
        ProgramGenerationIntent(deload = deload, atMs = 1_767_600_000_000L, beforeSignature = programSignature(before))

    /** A current intent: it knows both what it is replacing AND what it is about to write. */
    private fun intent(
        deload: Boolean,
        before: List<ProgramSignatureDay>,
        after: List<ProgramSignatureDay>
    ) = ProgramGenerationIntent(
        deload = deload,
        atMs = 1_767_600_000_000L,
        beforeSignature = programSignature(before),
        afterSignature = programSignature(after),
        opId = "op-1"
    )

    /** A third program, produced by neither side of an attempt — a custom save or a later reroll. */
    private val handBuilt = listOf(
        day("push", ProgramSignatureSlot("machine_press", 5, "10-15")),
        day("pull", ProgramSignatureSlot("lat_pulldown", 5, "10-15"))
    )

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

    // ── Telling a commit from somebody else's write ──────────────────────────

    /**
     * The hole `beforeSignature` alone left. It can only say "something changed", and something
     * else changing is not this attempt committing: a deload generate that FAILED, followed by a
     * custom save or a reroll before the next boot, left a program matching neither — and the boot
     * set a deload week over a plan the attempt had never touched.
     */
    @Test
    fun anUnrelatedWriteIsNotThisAttemptCommitting() {
        val attempt = intent(deload = true, before = fullVolume, after = deloadVolume)

        assertEquals(
            "the program is neither what it replaced nor what it would have written",
            PendingGeneration.Superseded,
            resolvePendingGeneration(attempt, programSignature(handBuilt))
        )
    }

    @Test
    fun theAttemptsOwnResultIsStillRecognisedAsACommit() {
        val attempt = intent(deload = true, before = fullVolume, after = deloadVolume)
        assertEquals(
            PendingGeneration.Apply(1_767_600_000_000L),
            resolvePendingGeneration(attempt, programSignature(deloadVolume))
        )
        assertEquals(
            PendingGeneration.Discard,
            resolvePendingGeneration(attempt, programSignature(fullVolume))
        )
    }

    /**
     * The inverse hole. A committed ordinary regeneration can produce exactly the program it
     * replaced — deterministic picks, or constrained equipment leaving one legal answer — and under
     * the old rule that was indistinguishable from a failure, so a marker the user had asked to be
     * CLEARED stayed set. Checking the attempt's own result first settles it.
     */
    @Test
    fun aRegenerationThatProducedTheSameProgramStillCounts() {
        val attempt = intent(deload = false, before = fullVolume, after = fullVolume)
        assertEquals(
            PendingGeneration.Apply(0L),
            resolvePendingGeneration(attempt, programSignature(fullVolume))
        )
    }

    @Test
    fun anIntentFromAnOlderBuildKeepsTheOnlyRuleItCanSupport() {
        // No `afterSignature` to compare against: anything that is not the before-program reads as
        // a commit, exactly as it did before. Better than refusing to finish an interrupted saga.
        val legacy = intent(deload = true, before = fullVolume)
        assertEquals(
            PendingGeneration.Apply(1_767_600_000_000L),
            resolvePendingGeneration(legacy, programSignature(handBuilt))
        )
    }

    @Test
    fun bothSignaturesAndTheOperationIdSurviveTheRoundTrip() {
        val attempt = intent(deload = true, before = fullVolume, after = deloadVolume)
        val restored = ProgramGenerationIntent.fromJson(attempt.toJson())
        assertEquals(attempt, restored)
        assertEquals(programSignature(deloadVolume), restored!!.afterSignature)
        assertEquals("op-1", restored.opId)
    }
}
