package com.forge.app.ui.settings

import org.junit.Assert.assertEquals
import org.junit.Test

/** The Settings root's derived lines (Relay design "st"): the split, the plan, locks, Health Connect. */
class RootLinesTest {

    @Test fun `a split folds its lettered days`() {
        assertEquals("Upper · Lower", splitLabel(listOf("Upper A", "Lower A", "Upper B", "Lower B")))
        assertEquals("Push · Pull · Legs", splitLabel(listOf("Push", "Pull", "Legs", "Push 2")))
        assertEquals("Full Body", splitLabel(listOf("Full Body")))
        assertEquals("", splitLabel(emptyList()))
    }

    @Test fun `the plan line names the split and the days`() {
        assertEquals("Push · Pull · Legs, 5 days", planLine("Push · Pull · Legs", 5))
        assertEquals("3 days a week", planLine("", 3))
    }

    @Test fun `the lock line names what locks`() {
        assertEquals("Off", securityReading(SettingsUiState()))
        assertEquals("App lock on", securityReading(SettingsUiState(appLockEnabled = true)))
        assertEquals("Photo lock on", securityReading(SettingsUiState(galleryLockEnabled = true)))
        assertEquals("App and photo locks on", securityReading(SettingsUiState(appLockEnabled = true, galleryLockEnabled = true)))
        assertEquals("Privacy mode on", securityReading(SettingsUiState(privacyMode = true)))
    }

    @Test fun `health connect reads how many readings are on`() {
        assertEquals("Health Connect · 6 readings", watchReading(available = true, needsUpdate = false, on = 6))
        assertEquals("Health Connect · 1 reading", watchReading(available = true, needsUpdate = false, on = 1))
        assertEquals("Health Connect · not connected", watchReading(available = true, needsUpdate = false, on = 0))
        assertEquals("Health Connect not installed", watchReading(available = false, needsUpdate = false, on = 0))
        assertEquals("Health Connect needs an update", watchReading(available = false, needsUpdate = true, on = 0))
    }

    @Test fun `connected readings count every grant once`() {
        val none = HealthConnectViewModel.UiState(loading = false, available = true)
        assertEquals(0, connectedReadings(none))
        val some = none.copy(granted = true, weightGranted = true, stepsGranted = true)
        assertEquals(3, connectedReadings(some))
        // Sleep and heart rate need the provider as well as the grant.
        assertEquals(2, connectedReadings(some.copy(available = false)))
    }

    @Test fun `notifications count the phone alerts and the in-app kinds that are on`() {
        val kinds = com.forge.app.data.repo.NoticeKind.entries
        val all = SettingsUiState(trainingReminderEnabled = true)
        assertEquals(3 + kinds.size, notificationsOn(all))
        val off = SettingsUiState(
            weeklyRecapEnabled = false,
            restTimerAlertEnabled = false,
            disabledNoticeKinds = kinds.map { it.key }.toSet()
        )
        assertEquals(0, notificationsOn(off))
    }
}
