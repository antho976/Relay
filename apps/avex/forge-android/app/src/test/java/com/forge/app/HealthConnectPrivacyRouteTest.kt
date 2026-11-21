package com.forge.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HealthConnectPrivacyRouteTest {
    @Test
    fun bothHealthConnectPrivacyActionsOpenThePolicy() {
        assertTrue(opensHealthConnectPrivacyPolicy("androidx.health.connect.action.SHOW_PERMISSIONS_RATIONALE"))
        assertTrue(opensHealthConnectPrivacyPolicy("android.intent.action.VIEW_PERMISSION_USAGE"))
        assertFalse(opensHealthConnectPrivacyPolicy(null))
        assertFalse(opensHealthConnectPrivacyPolicy("android.intent.action.VIEW"))
    }
}
