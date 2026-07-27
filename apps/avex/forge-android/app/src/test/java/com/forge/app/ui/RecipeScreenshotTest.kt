package com.forge.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import com.forge.app.ui.recipes.DetailRecipe
import com.forge.app.ui.recipes.ListRecipe
import com.forge.app.ui.recipes.LiveRecipe
import com.forge.app.ui.recipes.ModalRecipeContent
import com.forge.app.ui.recipes.OverviewRecipe
import com.forge.app.ui.recipes.SettingsRecipe
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Golden screenshots of the six archetype recipes (DESIGN §3).
 *
 * This is the half of the doctrine a regex cannot reach. `DesignDoctrineTest` can tell you an alpha
 * is off-ladder; only a rendered pixel can tell you a capsule clips its label at 200% font scale,
 * that two elements overlap, or that a section's rhythm drifted. §14 requires every screen to
 * survive 200%, and the recipes are the templates every screen is copied from, so they are the
 * right things to pin.
 *
 * Record or re-record goldens:
 *
 *     gradle -p forge-android :app:recordRoborazziDebug
 *
 * Verify against them (what CI runs):
 *
 *     gradle -p forge-android :app:verifyRoborazziDebug
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = RobolectricDeviceQualifiers.Pixel7)
class RecipeScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    /**
     * A 0.1% pixel tolerance. Font rasterisation differs slightly between machines and JDKs, so a
     * zero-tolerance golden recorded on a dev box tends to go red the first time CI renders it, for
     * reasons that have nothing to do with the design. 0.1% absorbs that jitter while staying far
     * below any real layout change: a shifted capsule or a clipped label moves thousands of pixels,
     * not tens.
     *
     * If CI ever fails purely on rendering, download the diff artifact and LOOK before widening it.
     */
    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(name: String, content: @Composable () -> Unit) {
        compose.setContent { ForgeTheme { content() } }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    @Test fun overview() = shoot("overview") { OverviewRecipe() }
    @Test fun overviewAtZero() = shoot("overview-zero") { OverviewRecipe(weekSessions = List(7) { 0 }, prCount = 0) }
    @Test fun detail() = shoot("detail") { DetailRecipe() }
    @Test fun list() = shoot("list") { ListRecipe() }
    @Test fun listNoResults() = shoot("list-empty") { ListRecipe(all = emptyList()) }
    @Test fun settings() = shoot("settings") { SettingsRecipe() }
    @Test fun live() = shoot("live") { LiveRecipe() }
    @Test fun modal() = shoot("modal") { ModalRecipeContent() }

    /** AMOLED is a shipped ground, not a variant: it needs its own goldens. */
    @Test fun overviewAmoled() {
        compose.setContent { ForgeTheme(amoledMode = true) { OverviewRecipe() } }
        compose.onRoot().captureRoboImage("src/test/screenshots/overview-amoled.png", options)
    }

    /** Monochrome drops the accent to a neutral (§5); highlights must stay legible without colour. */
    @Test fun overviewMonochrome() {
        compose.setContent { ForgeTheme(accentEnabled = false) { OverviewRecipe() } }
        compose.onRoot().captureRoboImage("src/test/screenshots/overview-mono.png", options)
    }

    // ── 200% font scale ────────────────────────────────────────────────────────────────────────
    // §14: "the app must survive 200%". This is the reason the screenshot suite exists. Clipping,
    // overlap and truncation at large scale are invisible to every static check in the repo, and
    // invisible at 100% too. If one of these goldens changes, LOOK at the diff before re-recording.

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

    // ── Longest realistic content ──────────────────────────────────────────────────────────────
    // §12/§14: "design against the longest realistic string, not the demo one." The short fixtures
    // above cannot catch a truncation or clipping bug — verified by adding `maxLines = 1` to a list
    // row and watching every golden stay identical while the static rule caught it. Fixtures that
    // only ever show "Pull B" are the visual gate agreeing with itself.

    private val longNames = listOf(
        "Bulgarian Split Squat (Rear Foot Elevated) · 24 Jul",
        "Single-Arm Half-Kneeling Landmine Press · 22 Jul",
        "Romanian Deadlift, Snatch Grip, Deficit · 20 Jul",
    )

    @Test fun listLongContent() = shoot("list-long") { ListRecipe(all = longNames) }
    @Test fun listLongContentLargeFont() = shootLarge("list-long") { ListRecipe(all = longNames) }
    @Test fun detailLongContent() =
        shoot("detail-long") { DetailRecipe(exercise = "Single-Arm Half-Kneeling Landmine Press") }
    @Test fun detailLongContentLargeFont() =
        shootLarge("detail-long") { DetailRecipe(exercise = "Single-Arm Half-Kneeling Landmine Press") }

    @Test fun overviewLargeFont() = shootLarge("overview") { OverviewRecipe() }
    @Test fun detailLargeFont() = shootLarge("detail") { DetailRecipe() }
    @Test fun listLargeFont() = shootLarge("list") { ListRecipe() }
    @Test fun settingsLargeFont() = shootLarge("settings") { SettingsRecipe() }
    @Test fun liveLargeFont() = shootLarge("live") { LiveRecipe() }
    @Test fun modalLargeFont() = shootLarge("modal") { ModalRecipeContent() }
}
