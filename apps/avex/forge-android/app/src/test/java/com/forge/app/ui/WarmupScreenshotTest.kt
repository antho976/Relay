package com.forge.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import com.forge.app.domain.warmup.WarmupEngine
import com.forge.app.domain.warmup.WarmupExercise
import com.forge.app.program.ExerciseUnit
import com.forge.app.program.MuscleGroup
import com.forge.app.ui.gym.train.components.WarmupFlow
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Goldens for the pre-session warmup.
 *
 * The warmup is a one-screen, one-button surface, and the thing most likely to break it silently is
 * length. The ramp section was removed on 2026-08-24, so the remaining variable is the prep drills
 * themselves: a lower-body day draws a different (and longer-named) set than a push day, and at 200%
 * font scale that is where a capsule would clip or two rows would collide. These pin both ends.
 *
 * The lb/kg and no-history cases went with the ramp — with no loads on screen, both rendered
 * identically to [warmup] and asserted nothing.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class WarmupScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    /** The everyday case: a moderate dumbbell session with history. */
    private val typical = WarmupEngine.build(
        listOf(
            WarmupExercise(
                "a", "DB Bench Press", MuscleGroup.CHEST, ExerciseUnit.DUMBBELL,
                isCompound = true, workingLoad = 70.0, targetReps = 8
            ),
            WarmupExercise(
                "b", "DB Lateral Raise", MuscleGroup.SHOULDERS, ExerciseUnit.DUMBBELL,
                isCompound = false, workingLoad = 20.0, targetReps = 14
            )
        )
    )

    /** The longest realistic case: a heavy barbell triple, which earns the full five-rung ramp. */
    private val heaviest = WarmupEngine.build(
        listOf(
            WarmupExercise(
                "a", "Barbell Back Squat", MuscleGroup.QUADS, ExerciseUnit.WEIGHT,
                isCompound = true, workingLoad = 315.0, targetReps = 3
            ),
            WarmupExercise(
                "b", "Romanian Deadlift", MuscleGroup.HAMSTRINGS, ExerciseUnit.WEIGHT,
                isCompound = true, workingLoad = 225.0, targetReps = 8
            )
        )
    )

    @Composable
    private fun flow(
        protocol: com.forge.app.domain.warmup.WarmupProtocol,
        checked: Set<String> = emptySet()
    ) {
        WarmupFlow(
            protocol = protocol,
            checked = checked,
            onToggle = {},
            onStart = {},
            onDisableToday = {},
            onDisableWeek = {}
        )
    }

    private fun shoot(name: String, content: @Composable () -> Unit) {
        compose.setContent { ForgeTheme { content() } }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    private fun shootLarge(name: String, content: @Composable () -> Unit) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(density = base.density, fontScale = 2f)
            ) {
                ForgeTheme { content() }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name-200.png", options)
    }

    @Test fun warmup() = shoot("warmup") { flow(typical) }
    @Test fun warmupHeavy() = shoot("warmup-heavy") { flow(heaviest) }
    @Test fun warmupLarge() = shootLarge("warmup") { flow(typical) }
    @Test fun warmupHeavyLarge() = shootLarge("warmup-heavy") { flow(heaviest) }

    /** Part-ticked: the state the user is actually looking at halfway through. */
    @Test fun warmupPartlyTicked() = shoot("warmup-ticked") {
        flow(typical, checked = typical.steps.take(2).map { it.id }.toSet())
    }

}
