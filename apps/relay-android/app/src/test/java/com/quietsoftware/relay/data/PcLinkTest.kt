package com.quietsoftware.relay.data

import android.Manifest
import android.app.Application
import android.app.NotificationManager
import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.model.Notification
import com.quietsoftware.relay.data.db.RelayDb
import com.quietsoftware.relay.data.link.PcStore
import com.quietsoftware.relay.service.Notifier
import com.quietsoftware.relay.service.forgetNotified
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

/** The PC link as the person left it, and what is announced after another PC is paired. */
@RunWith(RobolectricTestRunner::class)
class PcLinkTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    private fun relay() = Relay(context, Room.inMemoryDatabaseBuilder(context, RelayDb::class.java).allowMainThreadQueries().build(), PcStore(context), scope)

    @Test
    fun `a disconnect outlasts the background starts and a new process, until the person connects`() = runBlocking {
        val first = relay()
        first.disconnect()
        withTimeout(5_000) { while (!PcStore(context).disconnected.first()) delay(20) }

        // A new process: the app, the service and the half-hourly worker all call start().
        val again = relay()
        again.start()
        again.kick()
        delay(500)
        assertEquals(LinkState.Stopped, again.state.value)

        again.connect()
        // No PC is paired in this test, so wanting the link shows as "Unpaired" rather than "Stopped".
        withTimeout(5_000) { again.state.first { it == LinkState.Unpaired } }
        assertTrue(!PcStore(context).disconnected.first())
    }

    @Test
    fun `after another PC is paired its notifications are announced from its own first`() = runBlocking {
        shadowOf(context as Application).grantPermissions(Manifest.permission.POST_NOTIFICATIONS)
        val notifier = Notifier(context, relay(), Settings(context))
        val nm = context.getSystemService(NotificationManager::class.java)
        fun done(id: Long) = Notification(id, projectId = 1, category = "agent_done", title = "calm-heron finished")
        // The old PC: a first look records what is there, then one past it is announced.
        notifier.post(emptyList(), listOf(done(4_000)), quiet = false)
        notifier.post(emptyList(), listOf(done(4_000), done(4_001)), quiet = false)
        assertEquals(1, shadowOf(nm).allNotifications.size)

        // A new PC counts its notifications from 1.
        forgetNotified(context)
        notifier.post(emptyList(), emptyList(), quiet = false)
        notifier.post(emptyList(), listOf(done(3)), quiet = false)
        assertEquals("the new PC's first is not silenced by the old one's 4001", 2, shadowOf(nm).allNotifications.size)
    }
}
