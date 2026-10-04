package com.tally.app.ui.common

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.Density
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.tally.app.ui.theme.TallyTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** What TalkBack hears from the kit's single-choice controls. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class ChoiceSemanticsTest {

    @get:Rule val compose = createComposeRule()

    private fun hasRole(role: Role) = SemanticsMatcher.expectValue(SemanticsProperties.Role, role)

    private val isSelectableGroup = SemanticsMatcher.keyIsDefined(SemanticsProperties.SelectableGroup)

    @Test fun aChoiceChipIsOneRadioButtonOfAGroup() {
        compose.setContent {
            TallyTheme {
                Row(Modifier.selectableGroup().testTag("accounts")) {
                    ChoiceChip("Chequing", selected = true) {}
                    ChoiceChip("Visa", selected = false) {}
                }
            }
        }
        compose.onNodeWithText("Chequing").assert(hasRole(Role.RadioButton)).assertIsSelected()
        compose.onNodeWithText("Visa").assert(hasRole(Role.RadioButton)).assertIsNotSelected()
        compose.onNodeWithTag("accounts").assert(isSelectableGroup)
    }

    @Test fun aChipThatActsIsAButtonWithNoSelection() {
        compose.setContent {
            TallyTheme { ChoiceChip("Metro", selected = false, role = Role.Button) {} }
        }
        compose.onNodeWithText("Metro")
            .assert(hasRole(Role.Button))
            .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Selected))
    }

    @Test fun segmentsWrappedAtLargeTextStillReadAsTabsOfOneGroup() {
        compose.setContent {
            TallyTheme {
                val density = LocalDensity.current
                CompositionLocalProvider(LocalDensity provides Density(density.density, 2f)) {
                    SlidingSegments(listOf("Budgets", "Bills", "Goals"), 2, {}, Modifier.testTag("lens"))
                }
            }
        }
        compose.onNodeWithText("Goals").assert(hasRole(Role.Tab)).assertIsSelected()
        compose.onNodeWithText("Bills").assert(hasRole(Role.Tab)).assertIsNotSelected()
        compose.onNodeWithTag("lens").assert(isSelectableGroup)
    }

    @Test fun segmentsAtNormalTextAreTabsOfOneGroup() {
        compose.setContent {
            TallyTheme { SlidingSegments(listOf("Month", "Trend", "Calendar"), 0, {}, Modifier.testTag("lens")) }
        }
        compose.onNodeWithText("Month").assert(hasRole(Role.Tab)).assertIsSelected()
        compose.onNodeWithTag("lens").assert(isSelectableGroup)
    }
}
