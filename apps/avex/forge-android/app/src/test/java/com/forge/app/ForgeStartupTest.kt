package com.forge.app

import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Exercises the real Hilt constructor graph, including early timer and preference consumers. */
@RunWith(RobolectricTestRunner::class)
@Config(application = ForgeApp::class, sdk = [34])
class ForgeStartupTest {
    @Test fun `injected application opens its storage gate without a constructor deadlock`() = runBlocking {
        val app = ApplicationProvider.getApplicationContext<ForgeApp>()
        withTimeout(10_000) {
            app.awaitStorageReady()
            assertNotNull(app.settingsRepository.startupPreferences())
        }
    }
}
