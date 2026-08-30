package com.forge.wear

import android.Manifest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The api-level split for health permissions, which is the whole of finding 1: the module targets
 * API 36 and declared only `BODY_SENSORS`, so on a current watch the exercise started and reported
 * no heart rate — Health Services delivers an empty metric rather than an error, and nothing in the
 * app was in a position to notice.
 *
 * Pure integer comparisons on purpose. The alternative is an API 36 Wear emulator, which the
 * pipeline does not have and which an API 34 one would answer wrongly.
 */
class WearHealthPermissionsTest {

    @Test
    fun `below api 36 heart rate is the legacy body sensor permission`() {
        for (sdk in 30..35) {
            assertEquals(
                "sdk $sdk",
                Manifest.permission.BODY_SENSORS,
                WearHealthPermissions.heartRatePermission(sdk)
            )
        }
    }

    @Test
    fun `from api 36 heart rate is the health permission`() {
        assertEquals(
            WearHealthPermissions.READ_HEART_RATE,
            WearHealthPermissions.heartRatePermission(36)
        )
        assertEquals(
            WearHealthPermissions.READ_HEART_RATE,
            WearHealthPermissions.heartRatePermission(40)
        )
    }

    @Test
    fun `calories always need activity recognition`() {
        // CALORIES_TOTAL is in the exercise config on every api level, and it is the permission the
        // module never asked for at all — so the phone received null calories and fell back to its
        // MET estimate believing the watch simply had none to report.
        for (sdk in listOf(30, 33, 34, 35, 36)) {
            assertTrue(
                "sdk $sdk",
                Manifest.permission.ACTIVITY_RECOGNITION in WearHealthPermissions.exercisePermissions(sdk)
            )
        }
    }

    @Test
    fun `an ask never mixes the two eras`() {
        // Asking for BODY_SENSORS on 36 is an immediate silent denial, and asking for the health
        // permission below 36 is the same in reverse. Exactly one heart-rate permission per ask.
        for (sdk in listOf(30, 35, 36)) {
            val heartRate = WearHealthPermissions.exercisePermissions(sdk)
                .filter { it == Manifest.permission.BODY_SENSORS || it == WearHealthPermissions.READ_HEART_RATE }
            assertEquals("sdk $sdk", 1, heartRate.size)
        }
    }

    @Test
    fun `heart rate leads the ask`() {
        // HR is what the wrist renders; calories are a bonus the phone can estimate without.
        for (sdk in listOf(30, 36)) {
            assertEquals(
                WearHealthPermissions.heartRatePermission(sdk),
                WearHealthPermissions.exercisePermissions(sdk).first()
            )
        }
    }
}
