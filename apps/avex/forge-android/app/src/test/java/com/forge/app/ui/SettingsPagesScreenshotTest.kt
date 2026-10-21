package com.forge.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.test.core.app.ApplicationProvider
import com.forge.app.appicon.AppIcon
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.ui.security.AppLockContent
import com.forge.app.ui.settings.AppIconPreviews
import com.forge.app.ui.settings.AppearanceActions
import com.forge.app.ui.settings.AppearanceContent
import com.forge.app.ui.settings.HealthConnectViewModel
import com.forge.app.ui.settings.ProgramMenuActions
import com.forge.app.ui.settings.ProgramMenuContent
import com.forge.app.ui.settings.RecoveryActions
import com.forge.app.ui.settings.RecoveryContent
import com.forge.app.ui.settings.SecurityContent
import com.forge.app.ui.settings.SettingsScaffold
import com.forge.app.ui.settings.SettingsTheme
import com.forge.app.ui.settings.SettingsUiState
import com.forge.app.ui.theme.ForgeTheme
import com.forge.app.ui.theme.forgeBackgroundGradient
import com.github.takahirom.roborazzi.RoborazziOptions
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Goldens of the Settings pages drawn in the redesign (Relay designs sa, sp, sw, lk5) and the App
 * lock screen (lk1, lk3), each at the design's 412dp, at Antho's 384dp, and at 384dp with the font
 * at 130%, so a collision at the narrow width or a larger font shows.
 *
 *     gradle -p forge-android :app:recordRoborazziDebug --tests '*SettingsPagesScreenshotTest*'
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w412dp-h1200dp-xxhdpi")
class SettingsPagesScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(name: String, fontScale: Float = 1f, content: @Composable () -> Unit) {
        // The icon previews decode off the main thread in the app; warm the cache so they draw.
        val res = ApplicationProvider.getApplicationContext<android.content.Context>().resources
        AppIcon.entries.forEach { runCatching { AppIconPreviews.load(res, it.previewRes) } }
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(base.density, fontScale)) {
                ForgeTheme { SettingsTheme { content() } }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    // ── Appearance ──

    @Composable private fun Appearance() = AppearanceContent(SettingsUiState(), AppearanceActions(), onBack = {})

    @Test fun appearance() = shoot("settings-appearance") { Appearance() }
    @Config(qualifiers = "w384dp-h1200dp-xxhdpi")
    @Test fun appearance384() = shoot("settings-appearance-384") { Appearance() }
    @Config(qualifiers = "w384dp-h1400dp-xxhdpi")
    @Test fun appearance384Large() = shoot("settings-appearance-384-130", fontScale = 1.3f) { Appearance() }

    // ── Program & equipment ──

    private val programState = SettingsUiState(daysPerWeek = 5, sessionMinutes = 60, priorityMuscles = setOf("chest", "back"))

    @Composable private fun Program() = SettingsScaffold("Program & equipment", onBack = {}) {
        ProgramMenuContent(programState, "sequence", "Push · Pull · Legs", ProgramMenuActions())
    }

    @Config(qualifiers = "w412dp-h1400dp-xxhdpi")
    @Test fun program() = shoot("settings-program") { Program() }
    @Config(qualifiers = "w384dp-h1400dp-xxhdpi")
    @Test fun program384() = shoot("settings-program-384") { Program() }
    @Config(qualifiers = "w384dp-h1700dp-xxhdpi")
    @Test fun program384Large() = shoot("settings-program-384-130", fontScale = 1.3f) { Program() }

    // ── Watch & health data ──

    private val healthState = HealthConnectViewModel.UiState(
        loading = false, available = true, granted = true,
        weightGranted = true, weightWriteGranted = true, writeBodyweight = true,
        stepsGranted = true, sessionGranted = true, writeSessions = true,
        watchWorkoutGranted = true, calorieGranted = true, writeCalories = false,
        signalFlow = HealthConnectManager.SignalFlow(sleepOrHr = true, weight = true, steps = false, route = false)
    )

    @Composable private fun Watch() = RecoveryContent(healthState, watchAppPaired = false, a = RecoveryActions(), onBack = {})

    @Config(qualifiers = "w412dp-h2000dp-xxhdpi")
    @Test fun watch() = shoot("settings-watch") { Watch() }
    @Config(qualifiers = "w384dp-h2100dp-xxhdpi")
    @Test fun watch384() = shoot("settings-watch-384") { Watch() }
    @Config(qualifiers = "w384dp-h2500dp-xxhdpi")
    @Test fun watch384Large() = shoot("settings-watch-384-130", fontScale = 1.3f) { Watch() }
    @Config(qualifiers = "w412dp-h1200dp-xxhdpi")
    @Test fun watchNotConnected() = shoot("settings-watch-empty") {
        RecoveryContent(HealthConnectViewModel.UiState(loading = false, available = true), false, RecoveryActions(), onBack = {})
    }

    // ── Privacy & security ──

    @Composable private fun Security(state: SettingsUiState = SettingsUiState(appLockEnabled = true, galleryLockEnabled = true, appLockTimeoutSec = 60)) =
        SecurityContent(state, noCredentialNote = false, onAppLock = {}, onGalleryLock = {}, onTimeout = {}, onPrivacyMode = {}, onBack = {})

    @Config(qualifiers = "w412dp-h915dp-xxhdpi")
    @Test fun security() = shoot("settings-security") { Security() }
    @Config(qualifiers = "w384dp-h915dp-xxhdpi")
    @Test fun security384() = shoot("settings-security-384") { Security() }
    @Config(qualifiers = "w384dp-h1100dp-xxhdpi")
    @Test fun security384Large() = shoot("settings-security-384-130", fontScale = 1.3f) { Security() }
    @Config(qualifiers = "w412dp-h915dp-xxhdpi")
    @Test fun securityNoLock() = shoot("settings-security-off") { Security(SettingsUiState()) }

    // ── App lock ──

    @Composable private fun Lock(note: String? = null) {
        val (top, bottom) = forgeBackgroundGradient(false)
        AppLockContent(
            icon = AppIcon.Default,
            caption = "Unlock to open Avex",
            note = note,
            gradient = Brush.verticalGradient(listOf(top, bottom)),
            onUnlock = {}
        )
    }

    @Config(qualifiers = "w412dp-h915dp-xxhdpi")
    @Test fun lock() = shoot("lock-locked") { Lock() }
    @Config(qualifiers = "w412dp-h915dp-xxhdpi")
    @Test fun lockTooManyTries() = shoot("lock-too-many-tries") { Lock("Too many attempts. Try again in a moment.") }
    @Config(qualifiers = "w384dp-h800dp-xxhdpi")
    @Test fun lock384Large() = shoot("lock-too-many-tries-384-130", fontScale = 1.3f) { Lock("Too many attempts. Try again in a moment.") }
}
