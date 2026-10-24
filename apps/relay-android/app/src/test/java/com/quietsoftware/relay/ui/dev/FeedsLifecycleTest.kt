package com.quietsoftware.relay.ui.dev

import android.content.Context
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.PcStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowLooper
import java.util.concurrent.TimeUnit

/** An agent's terminal stream and its conversation's reading stop while the app is in the background. */
@RunWith(RobolectricTestRunner::class)
class FeedsLifecycleTest {
    @get:Rule val compose = createComposeRule()

    private class Owner : LifecycleOwner {
        val registry = LifecycleRegistry(this)
        override val lifecycle: Lifecycle get() = registry
    }

    @Test fun `a screen in the background lets go of its feed and its conversation, and takes them up again`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val db = Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build()
        val relay = Relay(context, db, PcStore(context), CoroutineScope(SupervisorJob() + Dispatchers.Default))
        val owner = Owner()
        compose.runOnIdle { owner.registry.currentState = Lifecycle.State.RESUMED }
        compose.setContent {
            CompositionLocalProvider(LocalLifecycleOwner provides owner) {
                rememberFeed(relay, "quiet-heron")
                rememberChat(relay, "quiet-heron")
            }
        }
        compose.runOnIdle {
            assertEquals(1, Feeds.users("quiet-heron"))
            assertTrue(Chats.reading("quiet-heron"))
            // Home, or the phone locked: the activity stops.
            owner.registry.currentState = Lifecycle.State.CREATED
        }
        compose.runOnIdle {
            assertEquals(0, Feeds.users("quiet-heron"))
        }
        // The grace period lets the conversation go a moment later.
        ShadowLooper.idleMainLooper(3, TimeUnit.SECONDS)
        compose.runOnIdle {
            assertFalse(Chats.reading("quiet-heron"))
            owner.registry.currentState = Lifecycle.State.RESUMED
        }
        compose.runOnIdle {
            assertEquals(1, Feeds.users("quiet-heron"))
            assertTrue(Chats.reading("quiet-heron"))
        }
    }
}
