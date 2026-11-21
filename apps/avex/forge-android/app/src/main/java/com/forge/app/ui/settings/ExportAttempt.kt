package com.forge.app.ui.settings

import kotlinx.coroutines.CancellationException
import java.io.File

/** What one Settings export produced. */
internal sealed interface ExportAttempt {
    data class Written(val file: File) : ExportAttempt
    /** The exporter had nothing to write (the last-session PDF with no finished session). */
    data object NothingToExport : ExportAttempt
    data object Failed : ExportAttempt
}

/**
 * Run one export and classify it, so a failure reaches the snackbar instead of the process.
 *
 * The export launches caught nothing, and `writeText` / `outputStream` throw `IOException` on a full
 * disk: uncaught in `viewModelScope`, that killed the app (2026-09-26 audit, 04). Cancellation is
 * rethrown, never reported: a stopped export is the user's own choice, not a failure.
 */
internal suspend fun attemptExport(export: suspend () -> File?): ExportAttempt =
    try {
        export()?.let { ExportAttempt.Written(it) } ?: ExportAttempt.NothingToExport
    } catch (e: CancellationException) {
        throw e
    } catch (_: Exception) {
        ExportAttempt.Failed
    }
