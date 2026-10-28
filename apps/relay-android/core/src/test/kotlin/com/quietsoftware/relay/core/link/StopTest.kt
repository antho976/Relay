package com.quietsoftware.relay.core.link

import com.quietsoftware.relay.core.wire.Greeting
import com.quietsoftware.relay.core.wire.Route
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** A stop is the last word: a dial loop already under way never writes over it. */
class StopTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `a loop past its last check when the link stops leaves it stopped`() = runBlocking {
        // The loop is reading the profile, past where a cancel reaches it, when the person's
        // Disconnect lands: the app's start-up kick against the saved "disconnected".
        val reading = CountDownLatch(1)
        val release = CountDownLatch(1)
        val profiles = object : Link.ProfileSource {
            override suspend fun current(): PcProfile? {
                reading.countDown()
                release.await(5, TimeUnit.SECONDS)
                return null
            }

            override suspend fun connected(profile: PcProfile, route: Route, greeting: Greeting) {}
        }
        val link = Link(scope, { _, _ -> error("nothing is paired") }, profiles)
        link.kick()
        assertEquals(true, reading.await(5, TimeUnit.SECONDS))
        link.stop()
        release.countDown()
        delay(300)
        assertEquals(LinkState.Stopped, link.state.value)
    }
}
