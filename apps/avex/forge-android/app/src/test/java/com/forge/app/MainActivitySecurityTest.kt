package com.forge.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MainActivitySecurityTest {

    @Test
    fun pendingImportWaitsForUnlockAndIntro() {
        assertFalse(shouldShowImportConfirmation(true, appLocked = true, showIntro = false, hasPendingImport = true))
        assertFalse(shouldShowImportConfirmation(true, appLocked = false, showIntro = true, hasPendingImport = true))
        assertFalse(shouldShowImportConfirmation(null, appLocked = false, showIntro = false, hasPendingImport = true))
        assertTrue(shouldShowImportConfirmation(true, appLocked = false, showIntro = false, hasPendingImport = true))
    }

    @Test
    fun noPendingImportNeverShowsConfirmation() {
        assertFalse(shouldShowImportConfirmation(true, appLocked = false, showIntro = false, hasPendingImport = false))
    }
}
