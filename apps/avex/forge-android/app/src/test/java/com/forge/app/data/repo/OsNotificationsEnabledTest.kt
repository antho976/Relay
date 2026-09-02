package com.forge.app.data.repo

import android.app.NotificationManager
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * M-28: an OS-level block on Android 8 to 12 must be reported, so the feed's "Turn on
 * notifications" row can appear. The old read hardcoded `true` below Android 13.
 */
@RunWith(RobolectricTestRunner::class)
class OsNotificationsEnabledTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val manager: NotificationManager =
        context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

    @Test
    @Config(sdk = [28])
    fun aBlockOnAndroidNineIsReported() {
        shadowOf(manager).setNotificationsEnabled(false)
        assertFalse("blocked in OS settings", NotificationFeed.osNotificationsEnabled(context))

        shadowOf(manager).setNotificationsEnabled(true)
        assertTrue("allowed again", NotificationFeed.osNotificationsEnabled(context))
    }

    @Test
    @Config(sdk = [31])
    fun aBlockOnAndroidTwelveIsReported() {
        shadowOf(manager).setNotificationsEnabled(false)
        assertFalse(NotificationFeed.osNotificationsEnabled(context))
    }

    @Test
    @Config(sdk = [28])
    fun theDefaultIsAllowed() {
        assertTrue(NotificationFeed.osNotificationsEnabled(context))
    }
}
