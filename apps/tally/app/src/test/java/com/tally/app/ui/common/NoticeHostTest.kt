package com.tally.app.ui.common

import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The root snackbar as the app hosts it: the newest notice takes the snackbar, its Undo is the one
 * that runs, and a host rebuilt mid-notice (a rotation) shows the same notice again.
 */
@RunWith(RobolectricTestRunner::class)
class NoticeHostTest {

    @get:Rule val compose = createComposeRule()

    private val notices = Notices(CoroutineScope(Dispatchers.Unconfined))

    /** Bumped to rebuild the host from scratch, as a recreated Activity does. */
    private var host by mutableIntStateOf(0)

    private fun showHost() = compose.setContent {
        key(host) { NoticeHost(notices, remember { SnackbarHostState() }) }
    }

    @Test fun aNewerNoticeTakesTheSnackbarAndItsUndoIsTheOneThatRuns() {
        val restored = mutableListOf<String>()
        showHost()
        compose.runOnIdle { notices.showUndo("Coffee entry deleted") { restored += "Coffee" } }
        compose.onNodeWithText("Coffee entry deleted").assertExists()

        compose.runOnIdle { notices.showUndo("Lunch entry deleted") { restored += "Lunch" } }
        compose.onNodeWithText("Lunch entry deleted").assertExists()
        compose.onNodeWithText("Coffee entry deleted").assertDoesNotExist()

        compose.onNodeWithText("Undo").performClick()
        compose.waitForIdle()
        assertEquals(listOf("Lunch"), restored)
        compose.onNodeWithText("Lunch entry deleted").assertDoesNotExist()
    }

    @Test fun aRebuiltHostShowsTheSameNoticeAndItsUndoStillRuns() {
        val restored = mutableListOf<String>()
        notices.showUndo("Rent deleted") { restored += "Rent" }
        showHost()
        compose.onNodeWithText("Rent deleted").assertExists()

        compose.runOnIdle { host++ }
        compose.onNodeWithText("Rent deleted").assertExists()

        compose.onNodeWithText("Undo").performClick()
        compose.waitForIdle()
        assertEquals(listOf("Rent"), restored)
    }
}
