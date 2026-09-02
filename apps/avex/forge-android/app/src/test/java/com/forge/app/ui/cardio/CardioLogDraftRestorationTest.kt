package com.forge.app.ui.cardio

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import com.forge.app.data.db.entities.CardioEntry
import com.forge.app.ui.cardio.components.CardioLogSheet
import com.forge.app.ui.theme.ForgeTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * M-12: the cardio log is a primary logging flow whose Activity has no `configChanges`, so a
 * rotation or a resize recreates it. Every field lived in a plain `remember`, so a half-filled new
 * log came back blank and an edit silently reverted to the stored row.
 *
 * [StateRestorationTester] is the deterministic form of "rotate": it saves the composition's
 * saveable state, throws the composition away and rebuilds it, which is exactly what an Activity
 * recreation does to `remember` and does NOT do to `rememberSaveable`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class CardioLogDraftRestorationTest {

    @get:Rule
    val compose = createComposeRule()

    /** The duration field is the first editable text in the sheet — the hero above it has none. */
    private fun durationField() = compose.onAllNodes(hasSetTextAction()).onFirst()

    private fun sheet(editing: CardioEntry?): @Composable () -> Unit = {
        ForgeTheme {
            Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
                CardioLogSheet(
                    onDismiss = {},
                    onSave = { _, _, _, _, _, _, _, _, _, _, _, _, _ -> },
                    editing = editing
                )
            }
        }
    }

    @Test
    fun aHalfFilledNewLogSurvivesRecreation() {
        val restorer = StateRestorationTester(compose)
        restorer.setContent(sheet(editing = null))

        durationField().performTextInput("45")
        compose.onNode(hasSetTextAction() and hasText("45")).assertExists()

        restorer.emulateSavedInstanceStateRestore()

        compose.onNode(hasSetTextAction() and hasText("45"))
            .assertExists()
    }

    @Test
    fun anEditedValueSurvivesRecreationRatherThanRevertingToTheStoredRow() {
        val stored = CardioEntry(
            id = 7L,
            date = 1_700_000_000_000L,
            type = "run",
            durationMin = 30,
            note = "tempo"
        )
        val restorer = StateRestorationTester(compose)
        restorer.setContent(sheet(editing = stored))

        durationField().performTextClearance()
        durationField().performTextInput("52")
        compose.onNode(hasSetTextAction() and hasText("52")).assertExists()

        restorer.emulateSavedInstanceStateRestore()

        // The edit, not the row it was opened from.
        compose.onNode(hasSetTextAction() and hasText("52")).assertExists()
        compose.onAllNodes(hasSetTextAction() and hasText("30")).assertCountEquals(0)
    }
}
