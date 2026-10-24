package com.quietsoftware.relay.ui.dev

import android.content.Context
import android.view.View
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Moving between the conversation and the terminal keeps the screen awake. */
@RunWith(RobolectricTestRunner::class)
class KeepScreenOnTest {
    @Test fun `the screen stays on while any screen holds it`() {
        val view = View(ApplicationProvider.getApplicationContext<Context>())
        holdScreen(view, +1) // the conversation
        holdScreen(view, +1) // the terminal fades in over it
        holdScreen(view, -1) // the conversation's effect lets go when the fade ends
        assertTrue(view.keepScreenOn)
        holdScreen(view, -1)
        assertFalse(view.keepScreenOn)
        holdScreen(view, -1)
        holdScreen(view, +1)
        assertTrue("a stray release does not leave the count behind", view.keepScreenOn)
    }
}
