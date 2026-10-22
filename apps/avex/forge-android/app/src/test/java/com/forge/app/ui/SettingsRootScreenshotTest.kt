package com.forge.app.ui

import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import com.forge.app.ui.settings.MainList
import com.forge.app.ui.settings.SettingsTheme
import com.forge.app.ui.settings.SettingsUiState
import com.forge.app.ui.theme.ForgeTheme
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Goldens of the Settings root, the one Settings page not built from a recipe. The canvas is a
 * Pixel 7's width and tall enough for the whole index, so a group dropped or reordered shows.
 *
 *     gradle -p forge-android :app:recordRoborazziDebug
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w412dp-h1600dp-xxhdpi")
class SettingsRootScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    @Composable
    private fun Root(state: SettingsUiState = designState, query: String = "", searchOpen: Boolean = false) {
        SettingsTheme {
            MainList(
                state = state,
                searchQuery = query,
                listState = rememberLazyListState(),
                onBack = {},
                onSearchChange = {},
                onOpenPage = {},
                onOpenCoachBrief = {},
                onOpenDataDialog = {},
                onImportData = {},
                onResetTarget = {},
                onOpenResetMenu = {},
                backupStatus = "Weekly · last copy Oct 12, 2026",
                customCardioCount = 2,
                holidayStatus = "Next from Nov 3",
                storageUsed = "48 MB",
                watchStatus = "Health Connect · 6 readings",
                split = "Push · Pull · Legs",
                searchOpen = searchOpen
            )
        }
    }

    /** Close to the design's account: five days, a few likes, the app lock on. */
    private val designState = SettingsUiState(
        daysPerWeek = 5,
        liked = (1..12).map { "e$it" }.toSet(),
        disliked = (1..3).map { "d$it" }.toSet(),
        appLockEnabled = true
    )

    private fun shoot(name: String, fontScale: Float = 1f, content: @Composable () -> Unit) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(base.density, fontScale)) {
                ForgeTheme { content() }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    @Test fun root() = shoot("settings-root") { Root() }
    @Test fun rootLargeFont() = shoot("settings-root-200", fontScale = 2f) { Root() }
    @Test fun rootFreestyle() = shoot("settings-root-freestyle") { Root(SettingsUiState(freestyleMode = true)) }
    @Test fun rootSearch() = shoot("settings-root-search") { Root(query = "lock") }
    @Test fun rootSearchOpen() = shoot("settings-root-search-open") { Root(searchOpen = true) }
    @Config(qualifiers = "w384dp-h1600dp-xxhdpi")
    @Test fun root384() = shoot("settings-root-384") { Root() }
    @Config(qualifiers = "w384dp-h1900dp-xxhdpi")
    @Test fun root384Large() = shoot("settings-root-384-130", fontScale = 1.3f) { Root() }

    @Test fun rootAmoled() {
        compose.setContent { ForgeTheme(amoledMode = true) { Root() } }
        compose.onRoot().captureRoboImage("src/test/screenshots/settings-root-amoled.png", options)
    }
}
