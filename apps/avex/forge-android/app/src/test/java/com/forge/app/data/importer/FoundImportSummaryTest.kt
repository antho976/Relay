package com.forge.app.data.importer

import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * L-01: a found file has to be able to say what it holds.
 *
 * The scanner described every candidate by its workout count alone, so a bodyweight CSV — which
 * returns its data through `parseExtras` by design — had nothing to say for itself and was cached
 * as absent; a cardio- or goals-only Avex JSON went the same way. The same files imported
 * perfectly when picked directly, which is what made the gap quiet.
 *
 * Robolectric only for `Uri`, which the row carries.
 */
@RunWith(RobolectricTestRunner::class)
class FoundImportSummaryTest {

    private fun found(
        source: ImportSource = ImportSource.FORGE_JSON,
        sessions: Int = 0,
        cardio: Int = 0,
        goals: Int = 0,
        bodyweight: Int = 0
    ) = FoundImport(
        uri = Uri.parse("content://docs/1"),
        name = "export.json",
        source = source,
        sessionCount = sessions,
        lastModified = 0L,
        cardioCount = cardio,
        coachGoalCount = goals,
        bodyweightCount = bodyweight
    )

    @Test
    fun aBodyweightOnlyExportSaysSoInsteadOfClaimingNoWorkouts() {
        assertEquals(
            "Avex bodyweight export · 12 weigh-ins",
            foundImportSummary(found(source = ImportSource.FORGE_BODYWEIGHT_CSV, bodyweight = 12))
        )
    }

    @Test
    fun anOrdinaryWorkoutExportReadsAsItAlwaysDid() {
        assertEquals("Strong · 42 workouts", foundImportSummary(found(source = ImportSource.STRONG, sessions = 42)))
        assertEquals("Strong · 1 workout", foundImportSummary(found(source = ImportSource.STRONG, sessions = 1)))
    }

    @Test
    fun everythingAFullExportCarriesIsNamed() {
        assertEquals(
            "Avex export · 3 workouts · 2 cardio entries · 1 weigh-in · 4 goals",
            foundImportSummary(found(sessions = 3, cardio = 2, bodyweight = 1, goals = 4))
        )
    }

    @Test
    fun singularsReadCorrectly() {
        assertEquals("Avex export · 1 cardio entry", foundImportSummary(found(cardio = 1)))
        assertEquals("Avex export · 1 goal", foundImportSummary(found(goals = 1)))
    }

    @Test
    fun aFileWithNothingCountableStillNamesItsSource() {
        assertEquals("Avex export", foundImportSummary(found()))
    }
}
