package com.forge.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
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
import com.forge.app.program.ExerciseDef
import com.forge.app.program.ExercisePlan
import com.forge.app.program.ExerciseLibrary
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.train.components.SwapPickerContent
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
 * Goldens of the swap picker (DESIGN §3 Modal over §3 List).
 *
 * The sheet it replaced carried two capsules on every candidate plus three prose fields, which is
 * the button wall and the prose wall named in `design/FAILURES.md`. Both are the kind of regression
 * that grows back one row at a time, so the layout is pinned here at 100% and 200% font scale, on
 * AMOLED, and in the state where the user's equipment leaves nothing to swap to.
 *
 * Record or re-record:  gradle -p forge-android :app:recordRoborazziDebug
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class SwapPickerScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(
        name: String,
        amoled: Boolean = false,
        fontScale: Float = 1f,
        candidates: List<ExerciseDef> = chestSwaps(),
        currentSwapName: String? = null,
        hasPersistentSwap: Boolean = false
    ) {
        compose.setContent {
            ForgeTheme(amoledMode = amoled) {
                CompositionLocalProvider(
                    LocalDensity provides Density(LocalDensity.current.density, fontScale)
                ) {
                    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface)) {
                        SwapPickerContent(
                            forExercise = DB_FLY_SLOT,
                            candidates = candidates,
                            hasPersistentSwap = hasPersistentSwap,
                            currentSwapName = currentSwapName
                        )
                    }
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    @Test
    fun picker() = shoot("swap-picker")

    /** With a swap already live: the sheet opens armed on it, at the reach it was taken with. */
    @Test
    fun pickerWithACurrentSwap() = shoot(
        "swap-picker-current",
        currentSwapName = "Incline DB Bench Press",
        hasPersistentSwap = true
    )

    /** §14: the sheet must survive the biggest font without clipping or lost content. */
    @Test
    fun pickerAtLargeFontScale() = shoot("swap-picker-200", fontScale = 2f)

    /** §12 zero: no alternative the user's equipment can reach. */
    @Test
    fun pickerWithNothingToSwapTo() = shoot("swap-picker-zero", candidates = emptyList())

    /** AMOLED is a shipped ground, not a variant. */
    @Test
    fun pickerAmoled() = shoot("swap-picker-amoled", amoled = true)
}

/** The slot being swapped out of, as `DayScreen` hands it over. */
private val DB_FLY_SLOT = ExercisePlan(
    id = "chest-2",
    name = "DB Fly",
    sets = 3,
    reps = "12-15",
    unit = ExerciseUnit.DUMBBELL,
    muscle = MuscleGroup.CHEST,
    difficulty = Difficulty.BEGINNER,
    note = "",
    equipment = listOf(Equipment.DUMBBELLS, Equipment.BENCH)
)

/** Real library rows, so the goldens pin the copy that actually ships. */
private fun chestSwaps(): List<ExerciseDef> =
    ExerciseLibrary.all.filter { it.muscle == MuscleGroup.CHEST && it.id != "db-fly" }
