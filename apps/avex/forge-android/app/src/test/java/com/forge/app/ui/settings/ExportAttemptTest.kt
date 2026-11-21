package com.forge.app.ui.settings

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.io.IOException

/** A full disk used to crash the app from a Settings export (2026-09-26 audit, 04). */
class ExportAttemptTest {

    @Test
    fun `a written file is handed on`() = runTest {
        val file = File("export.csv")
        assertEquals(ExportAttempt.Written(file), attemptExport { file })
    }

    @Test
    fun `an IO failure is reported instead of thrown`() = runTest {
        assertEquals(ExportAttempt.Failed, attemptExport { throw IOException("ENOSPC (No space left on device)") })
    }

    @Test
    fun `nothing to write is its own outcome`() = runTest {
        assertEquals(ExportAttempt.NothingToExport, attemptExport { null })
    }

    @Test
    fun `cancellation is rethrown, never reported as a failure`() = runTest {
        val thrown = runCatching { attemptExport { throw CancellationException("stopped") } }.exceptionOrNull()
        assertTrue(thrown is CancellationException)
    }
}
