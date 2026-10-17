package com.quietsoftware.relay.ui.term

import android.content.Context
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.TerminalFeed
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.PcStore
import com.quietsoftware.relay.ui.theme.RelayTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * A pinch grows the terminal's text by about as much as the fingers spread, not a sliver of it;
 * a flick scrolls on past where the finger let go, as a list does.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class TerminalZoomTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun `a pinch outwards doubles the text`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val feed = TerminalFeed(relay, "brisk-otter", CoroutineScope(SupervisorJob() + Dispatchers.Default))
        var font by mutableFloatStateOf(8f)
        compose.setContent {
            RelayTheme {
                TerminalView(feed, font, onFontSp = { font = it }, onFit = { _, _ -> }, modifier = Modifier.fillMaxSize())
            }
        }
        compose.onRoot().performTouchInput {
            // Fingers 100px apart spread to 200px over many small steps, as a real pinch reports them.
            pinch(
                start0 = center - Offset(50f, 0f), end0 = center - Offset(100f, 0f),
                start1 = center + Offset(50f, 0f), end1 = center + Offset(100f, 0f),
                durationMillis = 600,
            )
        }
        compose.waitForIdle()
        assertTrue("font after pinch: $font", font in 14f..17f)
    }

    @Test
    fun `a flick scrolls further than a slow drag of the same length`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val feed = TerminalFeed(relay, "brisk-otter", CoroutineScope(SupervisorJob() + Dispatchers.Default))
        synchronized(feed.lock) { feed.terminal.feed((1..3000).joinToString("") { "line $it\r\n" }) }
        compose.setContent {
            RelayTheme {
                TerminalView(feed, 10f, onFontSp = {}, onFit = { _, _ -> }, modifier = Modifier.fillMaxSize())
            }
        }
        fun swipe(durationMillis: Long): Int {
            compose.onRoot().performTouchInput { swipeDown(startY = centerY - 200f, endY = centerY + 200f, durationMillis = durationMillis) }
            compose.mainClock.advanceTimeBy(3_000)
            compose.waitForIdle()
            val said = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsProperties.StateDescription)).fetchSemanticsNodes().single().config[SemanticsProperties.StateDescription]
            return said.substringBefore(' ').toIntOrNull() ?: 0
        }
        val slow = swipe(2_000)
        val flicked = swipe(60) - slow
        assertTrue("slow drag: $slow lines back", slow > 0)
        assertTrue("flick: $flicked lines further, slow drag: $slow", flicked > slow * 2)
    }
}
