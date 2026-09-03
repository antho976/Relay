package com.forge.app.ui

import org.junit.Assert.assertEquals
import java.io.File
import org.junit.Test

/**
 * M-15: a composition may not own the calendar.
 *
 * `remember { ZoneId.systemDefault() }` and `LocalDate.now()` inside a composable are wall-clock
 * reads with nothing to recompose them. A remembered zone survives recomposition by definition, so
 * after a flight the screen keeps interpreting new-zone epochs with the old zone's offset — a
 * day-boundary error, on screens whose entire subject is which day or week something happened in.
 * The unkeyed `remember` is the worse of the two: `LocalDate.now()` at least re-reads whenever
 * anything else recomposes, while the remembered zone is wrong until the screen is left.
 *
 * The rule the fixes settled on: the anchor (and the zone it was computed in) travels with the
 * state, emitted by a ViewModel that observes `TimeSignals`. This is the mechanical half of that —
 * prose can state it, only a test keeps it true, exactly as [DesignDoctrine] argues for the design
 * rules next door.
 *
 * Deliberately zero allowlist. Both call sites this was written for are fixed, so the ban costs
 * nothing today and exists so it cannot quietly come back.
 */
class ComposedClockDoctrineTest {

    private val banned = Regex("""remember\s*\{\s*ZoneId\.systemDefault\(\)\s*}""")

    @Test
    fun noComposableRemembersItsOwnTimeZone() {
        val offenders = uiSources()
            .filter { banned.containsMatchIn(withoutComments(it.readText())) }
            .map { it.name }
            .sorted()

        assertEquals(
            "A remembered zone never updates. Carry it on the state beside the day anchor " +
                "(see CardioWeeksState.zone) so the anchor and the arithmetic on it agree.",
            emptyList<String>(),
            offenders
        )
    }

    private fun uiSources(): List<File> =
        DesignDoctrine.appSource("ui").walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()

    /** Comments describing the ban are not the ban being broken — a rule that flags its own
     *  explanation teaches people to stop explaining. */
    private fun withoutComments(source: String): String = source
        .replace(Regex("""/\*[\s\S]*?\*/"""), " ")
        .replace(Regex("""//[^\n]*"""), " ")
}
