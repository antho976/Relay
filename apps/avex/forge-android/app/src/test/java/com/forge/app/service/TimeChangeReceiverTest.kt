package com.forge.app.service

import android.content.Intent
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import com.forge.app.ForgeApp
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * The 2026-09-26 audit (11) read the missing `super.onReceive` in [TimeChangeReceiver] as "Hilt
 * never injects it". The Hilt Gradle plugin inserts that call itself; this pins that the manifest
 * receiver really is injected when a zone change arrives, so the zone and reminder handling runs.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = ForgeApp::class, sdk = [34])
class TimeChangeReceiverTest {

    @Test
    fun `a timezone change reaches an injected receiver`() = runBlocking {
        val app = ApplicationProvider.getApplicationContext<ForgeApp>()
        withTimeout(10_000) { app.awaitStorageReady() }
        val intent = Intent(Intent.ACTION_TIMEZONE_CHANGED)
        val receivers = shadowOf(app).getReceiversForIntent(intent)
            .filterIsInstance<TimeChangeReceiver>()
        assertEquals("the manifest registers exactly one TimeChangeReceiver", 1, receivers.size)

        app.sendBroadcast(intent)
        shadowOf(Looper.getMainLooper()).idle()

        // lateinit: reading an uninjected field throws UninitializedPropertyAccessException.
        assertNotNull(receivers.single().timeSignals)
        assertNotNull(receivers.single().reminderScheduler)
    }
}
