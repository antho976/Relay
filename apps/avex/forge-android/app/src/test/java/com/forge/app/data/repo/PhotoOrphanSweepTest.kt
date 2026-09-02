package com.forge.app.data.repo

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * M-05: which files in the progress-photo folder are orphans.
 *
 * An import that died between copying its bytes and committing the index used to leave a
 * `pp_*.jpg` no gallery entry pointed at: invisible in the app, with no delete route, and zipped
 * into every backup. The sweep removes exactly those, plus the staging files an interrupted import
 * leaves behind — and nothing else: not the index, not the album list, not a copy still in flight.
 */
class PhotoOrphanSweepTest {

    private val indexedName = "pp_0123456789ab.jpg"
    private val orphanName = "pp_abcdef012345.jpg"

    private fun sweep(
        present: List<String>,
        indexed: Set<String> = setOf(indexedName),
        inFlight: Set<String> = emptySet()
    ) = ProgressPhotoRepository.orphanFileNames(present, indexed, inFlight)

    @Test
    fun anIndexedPhotoIsNeverAnOrphan() {
        assertEquals(emptyList<String>(), sweep(listOf(indexedName)))
    }

    @Test
    fun aPhotoNamedFileTheIndexDoesNotListIsAnOrphan() {
        assertEquals(listOf(orphanName), sweep(listOf(indexedName, orphanName)))
    }

    @Test
    fun anInterruptedImportsStagingFileIsAnOrphan() {
        val staging = "$orphanName${ProgressPhotoRepository.STAGING_SUFFIX}"
        assertEquals(listOf(staging), sweep(listOf(indexedName, staging)))
    }

    @Test
    fun aStagingFileOfAnIndexedPhotoIsStillAnOrphan() {
        // The rename published the photo; the leftover staging copy is dead weight either way.
        val staging = "$indexedName${ProgressPhotoRepository.STAGING_SUFFIX}"
        assertEquals(listOf(staging), sweep(listOf(indexedName, staging)))
    }

    @Test
    fun aCopyStillInFlightIsLeftAlone() {
        val staging = "$orphanName${ProgressPhotoRepository.STAGING_SUFFIX}"
        assertEquals(emptyList<String>(), sweep(listOf(staging), inFlight = setOf(staging)))
    }

    @Test
    fun theLibrarysOwnMetadataAndForeignNamesSurvive() {
        assertEquals(
            emptyList<String>(),
            sweep(listOf("index.json", "albums.json", "avatar.jpg", "notes.txt", "pp_.jpg"))
        )
    }

    @Test
    fun anEmptyIndexMakesEveryPhotoNamedFileAnOrphan() {
        assertEquals(
            listOf(indexedName, orphanName),
            sweep(listOf(indexedName, orphanName, "index.json"), indexed = emptySet())
        )
    }
}
