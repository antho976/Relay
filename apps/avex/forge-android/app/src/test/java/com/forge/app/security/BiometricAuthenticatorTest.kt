package com.forge.app.security

import androidx.biometric.BiometricManager
import org.junit.Assert.assertEquals
import org.junit.Test

class BiometricAuthenticatorTest {

    @Test
    fun onlyMissingCredentialCanFailOpen() {
        assertEquals(
            BiometricAuthenticator.Availability.NO_CREDENTIAL,
            BiometricAuthenticator.availabilityForStatus(BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED),
        )
    }

    @Test
    fun temporaryAndHardwareErrorsStayLocked() {
        assertEquals(
            BiometricAuthenticator.Availability.UNAVAILABLE,
            BiometricAuthenticator.availabilityForStatus(BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE),
        )
        assertEquals(
            BiometricAuthenticator.Availability.UNAVAILABLE,
            BiometricAuthenticator.availabilityForStatus(BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE),
        )
    }

    @Test
    fun successfulCapabilityCheckCanPrompt() {
        assertEquals(
            BiometricAuthenticator.Availability.AVAILABLE,
            BiometricAuthenticator.availabilityForStatus(BiometricManager.BIOMETRIC_SUCCESS),
        )
    }
}
