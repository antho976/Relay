package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import com.forge.app.program.Difficulty
import com.forge.app.program.Equipment
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.train.components.SwapPickerContent
import com.forge.app.ui.gym.train.components.SwapScope
import com.forge.app.ui.theme.ForgeTheme
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Goldens for the swap picker (DESIGN §14 — the half of the doctrine a regex cannot reach).
 *
 * The sheet is the one surface in the live session that shows a whole LIST inside a modal, so the
 * things worth pinning are the ones that only a rendered pixel reports: that the scope pills and the
 * restore capsule survive a 200% font scale without clipping, and that the zero pool still draws
 * something. [SwapPickerContent] is captured rather than `SwapPickerSheet` because Robolectric can't
 * settle a `ModalBottomSheet`'s animation, and the sheet frame adds nothing to the design anyway.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class SwapPickerScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    /** Matches `RecipeScreenshotTest` — 0.1% absorbs font-rasterisation jitter, nothing structural. */
    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private val chestPlan = ExercisePlan(
        id = "d1-chest-1",
        name = "DB Bench Press",
        sets = 3,
        reps = "8-10",
        unit = ExerciseUnit.DUMBBELL,
        muscle = MuscleGroup.CHEST,
        difficulty = Difficulty.BEGINNER,
        note = "",
        equipment = listOf(Equipment.DUMBBELLS, Equipment.BENCH)
    )

    private val chestCandidates = ExerciseLibrary.all.filter { it.muscle == MuscleGroup.CHEST }
    private val coreCandidates = ExerciseLibrary.all.filter { it.muscle == MuscleGroup.CORE }

    private fun shoot(
        name: String,
        fontScale: Float = 1f,
        content: @Composable () -> Unit
    ) {
        compose.setContent {
            ForgeTheme {
                CompositionLocalProvider(
                    LocalDensity provides Density(
                        density = LocalDensity.current.density,
                        fontScale = fontScale
                    )
                ) {
                    androidx.compose.foundation.layout.Box(
                        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface)
                    ) { content() }
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    @Composable
    private fun sheet(
        candidates: List<com.forge.app.program.ExerciseDef> = chestCandidates,
        current: String? = null,
        persistent: Boolean = false,
        plan: ExercisePlan = chestPlan,
        armed: Pair<String, SwapScope>? = null
    ) = SwapPickerContent(
        forExercise = plan,
        candidates = candidates,
        hasPersistentSwap = persistent,
        currentSwapName = current,
        onPickForSession = {},
        onPickPersistent = {},
        onClearPersistent = {},
        initialArmed = armed
    )

    @Test fun swapPicker() = shoot("swap-picker") { sheet() }

    /** The restore capsule is the only way back out of a persistent swap. */
    @Test fun swapPickerSwapped() = shoot("swap-picker-swapped") {
        sheet(current = "DB Fly", persistent = true)
    }

    /** Armed: the row carries the wash and the confirm capsule names the move AND the scope. */
    @Test fun swapPickerArmed() = shoot("swap-picker-armed") {
        sheet(armed = "db-fly" to SwapScope.EVERY_WEEK)
    }

    /** Armed on top of a persistent swap: filled confirm above its outlined sidekick (§8). */
    @Test fun swapPickerArmedWithRestore() = shoot("swap-picker-armed-restore") {
        sheet(current = "DB Fly", persistent = true, armed = "push-up" to SwapScope.TODAY)
    }

    /** §12 zero: an equipment set that can't reach the muscle still draws its hint. */
    @Test fun swapPickerEmpty() = shoot("swap-picker-empty") { sheet(candidates = emptyList()) }

    /** A timed hold in the pool — the one exception the right meta flags (§8). */
    @Test fun swapPickerTimed() = shoot("swap-picker-timed") {
        sheet(
            candidates = coreCandidates,
            plan = chestPlan.copy(name = "Cable Crunch", muscle = MuscleGroup.CORE)
        )
    }

    /** §14: the sheet must survive the biggest font without clipping a pill or a capsule. */
    @Test fun swapPickerLargeFont() = shoot("swap-picker-200", fontScale = 2f) {
        sheet(current = "DB Fly", persistent = true, armed = "db-fly" to SwapScope.EVERY_WEEK)
    }

    @Test fun swapPickerEmptyLargeFont() = shoot("swap-picker-empty-200", fontScale = 2f) {
        sheet(candidates = emptyList())
    }
}
