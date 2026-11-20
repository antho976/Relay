package com.tally.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.rememberNavController
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.data.sync.Connect
import com.tally.app.testing.Fixtures
import com.tally.app.testing.shoot
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Group
import com.tally.app.ui.kit.PalettePreview
import com.tally.app.ui.kit.SettingRow
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.Routes
import com.tally.app.ui.onboarding.ConnectStep
import com.tally.app.ui.settings.ImportScreen
import com.tally.app.ui.settings.ImportState
import com.tally.app.ui.settings.SettingsState
import com.tally.app.ui.shell.ShellState
import com.tally.app.ui.shell.Sidebar
import com.tally.app.ui.shell.SyncLine
import com.tally.app.ui.shell.Tone
import com.tally.app.ui.theme.Hue
import com.tally.app.ui.theme.Palette
import com.tally.app.ui.theme.Tally
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Direction C's setup and settings pages, drawn from state: each composes and shows its words. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class SetupScreensTest {

    @get:Rule val compose = createComposeRule()

    @Test fun onboardingWaiting() {
        val state = SettingsState(
            today = Fixtures.TODAY,
            connect = Connect.Waiting("K74Q9D", notified = true, expiresAt = Long.MAX_VALUE, server = "https://antho-s-bot-8dve.onrender.com"),
            server = "https://antho-s-bot-8dve.onrender.com",
            loaded = true,
        )
        compose.shoot("onboarding-waiting") { ConnectStep(state, {}, {}, {}, {}) }
        compose.onNodeWithText("Your budget, the same on this phone, your server and your PC.").assertExists()
        compose.onNodeWithText("K7 · 4Q · 9D").assertExists()
    }

    @Test fun onboardingIdle() {
        val state = SettingsState(today = Fixtures.TODAY, server = "https://antho-s-bot-8dve.onrender.com", loaded = true)
        compose.shoot("onboarding-idle") { ConnectStep(state, {}, {}, {}, {}) }
        compose.onNodeWithText("Set up on this phone").assertExists()
    }

    @Test fun settingsGroups() {
        compose.shoot("settings") {
            Column(Modifier.statusBarsPadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Group("Sync") {
                    SettingRow("Your server", glyph = "globe", glyphTint = Hue.mint, subtitle = "Synced · 4m ago", onClick = {}) { Dot(Tally.colors.live, 8.dp) }
                    SettingRow("Connected banks", glyph = "link", glyphTint = Hue.violet, subtitle = "Desjardins and Wealthsimple through Plaid, read only", onClick = {})
                }
                Group("Look and money") {
                    SettingRow("Appearance", glyph = "palette", glyphTint = Hue.pink, subtitle = "Sky", onClick = {})
                    SettingRow("Currency and dates", glyph = "calendar", glyphTint = Hue.sky, subtitle = "CAD · month starts on 1st · week from Monday", onClick = {})
                }
                Group("Data") {
                    SettingRow("Erase everything", glyph = "trash", glyphTint = Tally.colors.held, titleColor = Tally.colors.heldText, subtitle = "1,284 entries on this phone", onClick = {})
                }
            }
        }
        compose.onNodeWithText("Connected banks").assertExists()
    }

    @Test fun appearance() {
        compose.shoot("appearance") {
            Column(Modifier.statusBarsPadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Palette.ALL.chunked(2).forEach { pair ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        pair.forEach { p -> PalettePreview(p, p.name, "A ground", p == Palette.Sky, {}, Modifier.weight(1f)) }
                    }
                }
            }
        }
    }

    @Test fun importChoose() {
        compose.shoot("import-choose") {
            ImportScreen(ImportState(today = Fixtures.TODAY), {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, { _, _ -> }, {}, {}, {}, {}, connectBank = {})
        }
        compose.onNodeWithText("Skip the CSV: connect your bank").assertExists()
    }

    @Test fun drawer() {
        compose.shoot("drawer") {
            val nav = AppNav(rememberNavController())
            Box { Sidebar(Routes.OVERVIEW, ShellState(sync = SyncLine("Synced · 4m ago", Tone.LIVE), paired = true), {}, {}, nav, {}) }
        }
        compose.onNodeWithText("Connected banks").assertExists()
    }
}
