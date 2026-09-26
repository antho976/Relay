package com.forge.app.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * R1 (audit 2026-09-26 P1 #1): nothing may open a window the app lock can't cover.
 *
 * The lock is an overlay inside the activity's window. A dialog, bottom sheet, dropdown or popup is
 * a window of its own, stacked above the activity, so one left open when the phone went to sleep
 * stayed on screen and tappable over the lock. The fix is the lock-aware versions in
 * `ui/common/window`, which draw nothing while the lock is up. They only help if every caller uses
 * them, and the unguarded ones have the same names, so this checks the imports.
 */
class LockAwareWindowDoctrineTest {

    private val wrappers = "ui/common/window/LockAwareWindows.kt"

    // Everything Material or Compose ships that opens its own window, wrapped or not. A new one
    // needs a lock-aware version in ui/common/window first.
    private val banned = Regex(
        """androidx\.compose\.material3\.(AlertDialog|BasicAlertDialog|ModalBottomSheet|DropdownMenu|""" +
            """ExposedDropdownMenuBox|DatePickerDialog|TimePickerDialog|TooltipBox)\b|""" +
            """androidx\.compose\.ui\.window\.(Dialog|Popup)\b"""
    )

    @Test
    fun everyWindowOwningComposableIsTheLockAwareOne() {
        val root = DesignDoctrine.appSource("")
        val offenders = root.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .filter { it.relativeTo(root).invariantSeparatorsPath != wrappers }
            .filter { banned.containsMatchIn(withoutComments(it.readText())) }
            .map { it.relativeTo(root).invariantSeparatorsPath }
            .sorted()
            .toList()

        assertEquals(
            "These open a window above the app lock. Import the same name from " +
                "com.forge.app.ui.common.window instead, which hides it while the app is locked.",
            emptyList<String>(),
            offenders
        )
    }

    /** Comments describing the ban are not the ban being broken. */
    private fun withoutComments(source: String): String = source
        .replace(Regex("""/\*[\s\S]*?\*/"""), " ")
        .replace(Regex("""//[^\n]*"""), " ")
}
