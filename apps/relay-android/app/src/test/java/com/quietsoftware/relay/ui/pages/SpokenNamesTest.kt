package com.quietsoftware.relay.ui.pages

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.ui.code.FindBar
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.RelayTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Keys a screen reader names for what they do, at a finger's size. */
@RunWith(RobolectricTestRunner::class)
class SpokenNamesTest {
    @get:Rule val compose = createComposeRule()

    private val isSwitch = SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Switch)

    @Test
    fun aSkillsSwitchIsNamedForTheSkill() {
        var on = false
        compose.setContent { RelayTheme(Palette.Matte) { ExtensionSwitch("Audit", "A user-authored audit skill", on, true, onOpen = {}, onChange = { on = it }) } }
        val switch = compose.onNode(hasContentDescription("Audit") and isSwitch)
        switch.assertIsOff().assertHeightIsAtLeast(TOUCH)
        switch.performClick()
        assertTrue(on)
    }

    @Test
    fun theFindBarsOptionsSayWhatTheyAre() {
        compose.setContent {
            RelayTheme(Palette.Matte) {
                FindBar("x", {}, "", matchCase = true, onCase = {}, regex = false, onRegex = {}, canStep = false, onPrevious = {}, onNext = {}, onClose = {})
            }
        }
        compose.onNodeWithContentDescription("Match case").assertHeightIsAtLeast(28.dp)
        compose.onNodeWithContentDescription("Regular expression").assertHeightIsAtLeast(28.dp)
    }
}
