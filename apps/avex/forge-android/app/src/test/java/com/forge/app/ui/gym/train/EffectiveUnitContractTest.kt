package com.forge.app.ui.gym.train

import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

/**
 * Nothing under the day-training UI may read `plan.unit`. It must read `effectiveUnit`.
 *
 * `ExerciseUiState` carries two units. `plan.unit` is the unit of the exercise the slot was
 * PRESCRIBED as; `effectiveUnit` is the unit of the one the athlete is actually doing, which
 * differs the moment a swap crosses unit families. The card has always rendered the effective one,
 * and everything downstream of the tap — parsing, editing, the plate-jump warning, the kg→lb
 * conversion, the suggestion outcome's unit code — read the prescribed one. So the field said
 * WEIGHT and the write stored a plate count as pounds, or said PLATES and ran the count through a
 * kilogram conversion, or showed a weight box on a swapped-in bodyweight slot and stored null.
 *
 * A unit test of any one of those call sites would not have caught it, because each of them was
 * individually consistent; what was wrong was that they disagreed with the screen. This asserts the
 * property that actually holds: in this package there is one unit, and it is the effective one.
 *
 * Two definitions are exempt, being the places `effectiveUnit` is derived FROM. Everything else is
 * a bug in waiting — if a new one is genuinely correct, say why here rather than widening the rule.
 */
class EffectiveUnitContractTest {

    private val allowed = setOf(
        // ExerciseUiState.effectiveUnit itself: swap unit, else the plan's.
        "state/DayUiState.kt",
        // DayViewModelBuilders' copy of the same precedence, applied before the state exists.
        "DayViewModelBuilders.kt",
    )

    @Test
    fun `the day-training UI reads effectiveUnit, never plan unit`() {
        val root = generateSequence(File(".").canonicalFile) { it.parentFile }
            .firstOrNull { File(it, "src/main/java/com/forge/app/ui").isDirectory }
            ?: error("Could not locate the :app module from ${File(".").canonicalPath}")
        val train = File(root, "src/main/java/com/forge/app/ui/gym/train")
        check(train.isDirectory) { "Expected the day-training package at $train" }

        val offenders = train.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .filter { it.relativeTo(train).path.replace(File.separatorChar, '/') !in allowed }
            .flatMap { file ->
                file.readLines().withIndex()
                    .filter { (_, line) ->
                        // Code only: the fix's own explanatory comments name the old expression.
                        val code = line.substringBefore("//")
                        Regex("""\bplan\.unit\b""").containsMatchIn(code)
                    }
                    .map { (i, line) -> "${file.name}:${i + 1}  ${line.trim()}" }
            }
            .toList()

        assertEquals(
            "These read the PRESCRIBED unit where the screen renders the EFFECTIVE one. Use " +
                "ExerciseUiState.effectiveUnit:\n" + offenders.joinToString("\n"),
            emptyList<String>(),
            offenders
        )
    }
}
