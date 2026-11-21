package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import com.forge.app.ui.profile.state.ProfileUiState
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
 * Golden screenshots of the Profile page (2026-10-21 redesign): Overview, Body and Photos, the
 * Photos tab under the gallery lock, and a brand-new account. Each is a tall frame so the whole tab
 * reads in one image. Drawn at 412dp (the design's width) and again at 384dp (Antho's phone), and
 * once at 384dp with 1.3 font scale.
 *
 * Record or re-record:  gradle -p forge-android :app:recordRoborazziDebug --tests '*ProfileScreenshotTest*'
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w412dp-h1500dp-xhdpi")
class ProfileScreenshotTest {

    @get:Rule
    val compose = createComposeRule()

    private val options = RoborazziOptions(
        compareOptions = RoborazziOptions.CompareOptions(changeThreshold = 0.001f)
    )

    private fun shoot(name: String, fontScale: Float = 1f, content: @Composable () -> Unit) {
        compose.setContent {
            ForgeTheme {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                    CompositionLocalProvider(
                        LocalDensity provides Density(LocalDensity.current.density, fontScale = fontScale)
                    ) { content() }
                }
            }
        }
        compose.onRoot().captureRoboImage("src/test/screenshots/$name.png", options)
    }

    private fun page(
        tab: ProfileTab,
        locked: Boolean = false,
        state: ProfileUiState = ProfileFixtures.state(),
        body: ProfileBody = ProfileFixtures.body(),
    ) = @Composable {
        ProfilePage(
            state = state,
            body = body,
            weightUnit = ProfileFixtures.unit,
            galleryLocked = locked,
            today = ProfileFixtures.TODAY,
            nowMs = ProfileFixtures.NOW,
            planOn = ProfileFixtures.planOn,
            onVacation = ProfileFixtures.onVacation,
            actions = ProfileActions(onBack = {}),
            cover = null,
            photo = if (locked) { _, _ -> error("A locked gallery must not draw a photo") } else ProfileFixtures.photo,
            initialTab = tab,
        )
    }

    @Test fun overview() = shoot("profile-overview", content = page(ProfileTab.OVERVIEW))
    @Test fun body() = shoot("profile-body", content = page(ProfileTab.BODY))
    @Test fun photos() = shoot("profile-photos", content = page(ProfileTab.PHOTOS))
    @Test fun photosLocked() = shoot("profile-photos-locked", content = page(ProfileTab.PHOTOS, locked = true))

    @Test fun newAccountOverview() = shoot(
        "profile-overview-empty",
        content = page(ProfileTab.OVERVIEW, state = ProfileUiState(loading = false, name = "Antho"), body = ProfileBody())
    )
    @Test fun newAccountBody() = shoot(
        "profile-body-empty",
        content = page(ProfileTab.BODY, state = ProfileUiState(loading = false, name = "Antho"), body = ProfileBody())
    )
    @Test fun newAccountPhotos() = shoot(
        "profile-photos-empty",
        content = page(ProfileTab.PHOTOS, state = ProfileUiState(loading = false, name = "Antho"), body = ProfileBody())
    )

    @Config(qualifiers = "w384dp-h1500dp-xhdpi")
    @Test fun overview384() = shoot("profile-overview-384", content = page(ProfileTab.OVERVIEW))

    @Config(qualifiers = "w384dp-h1500dp-xhdpi")
    @Test fun body384() = shoot("profile-body-384", content = page(ProfileTab.BODY))

    @Config(qualifiers = "w384dp-h1500dp-xhdpi")
    @Test fun photos384() = shoot("profile-photos-384", content = page(ProfileTab.PHOTOS))

    @Config(qualifiers = "w384dp-h1500dp-xhdpi")
    @Test fun photosLocked384() = shoot("profile-photos-locked-384", content = page(ProfileTab.PHOTOS, locked = true))

    @Config(qualifiers = "w384dp-h1700dp-xhdpi")
    @Test fun overview384Large() = shoot("profile-overview-384-font130", fontScale = 1.3f, content = page(ProfileTab.OVERVIEW))

    @Config(qualifiers = "w384dp-h1700dp-xhdpi")
    @Test fun body384Large() = shoot("profile-body-384-font130", fontScale = 1.3f, content = page(ProfileTab.BODY))
}
