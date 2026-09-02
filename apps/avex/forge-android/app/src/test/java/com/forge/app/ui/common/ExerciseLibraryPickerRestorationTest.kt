package com.forge.app.ui.common

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyDescendant
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * H-13: the Program Builder's in-flight edit has to survive a recreation, and the dialog is only
 * half of it.
 *
 * The ViewModel restores its draft and WHICH dialog was open through `SavedStateHandle`, so a
 * rotation mid-swap reopened the right picker. The picker's own query and selection lived in plain
 * `remember`, so it reopened blank: the search the user had typed gone, every row they had ticked
 * cleared. That is the part they actually see, and it is the part the H-13 fix did not cover.
 *
 * [StateRestorationTester] is the deterministic form of "rotate": it saves the composition's
 * saveable state, throws the composition away and rebuilds it — which is exactly what an Activity
 * recreation does to `remember` and does not do to `rememberSaveable`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ExerciseLibraryPickerRestorationTest {

    @get:Rule
    val compose = createComposeRule()

    /** A query with at least two library matches, so the fixture proves a MULTI-row selection. */
    private val query = "press"
    private val matches = filterLibrary(query, exclude = emptySet())

    private fun row(name: String) = compose.onNode(hasClickAction() and hasAnyDescendant(hasText(name)))

    @Test
    fun theTypedQueryAndTheTickedRowsSurviveRecreation() {
        assertTrue("the fixture needs at least two matches to be worth running", matches.size >= 2)
        val restorer = StateRestorationTester(compose)
        restorer.setContent {
            ForgeTheme {
                ExerciseLibraryPicker(exclude = emptySet(), onDismiss = {}, onConfirm = {})
            }
        }

        compose.onAllNodes(hasSetTextAction()).onFirst().performTextInput(query)
        row(matches[0].name).performClick()
        row(matches[1].name).performClick()
        // The confirm verb carries the running count, which is the selection made observable.
        compose.onNode(hasText("Add 2")).assertIsDisplayed()

        restorer.emulateSavedInstanceStateRestore()

        compose.onNode(hasSetTextAction() and hasText(query))
            .assertIsDisplayed()
        compose.onNode(hasText("Add 2"))
            .assertIsDisplayed()
        // ...and the restored selection is still the same two rows, not merely a count.
        row(matches[0].name).assertIsDisplayed()
        row(matches[1].name).assertIsDisplayed()
    }
}
