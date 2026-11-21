package com.forge.app.domain.health

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * The clientRecordId scheme is a wire contract with records already sitting in Health Connect
 * (M-02): a mirror can only be updated or deleted later if the very same string can be rebuilt
 * from the local row. These pin the spellings earlier builds wrote under, and that the deleters
 * derive exactly what the writers wrote.
 */
class HcRecordKeysTest {

    @Test
    fun sessionKeysKeepTheSpellingEarlierBuildsWroteUnder() {
        assertEquals("avex-session-42", HcRecordKeys.session(42L))
        assertEquals("avex-session-kcal-42", HcRecordKeys.sessionCalories(42L))
        assertEquals("avex-session-hr-42", HcRecordKeys.sessionHeartRate(42L))
        assertEquals("avex-cardio-7", HcRecordKeys.cardio(7L))
    }

    @Test
    fun bodyKeysAreKeyedOnTheDayNotTheRowId() {
        // The weight / body-fat tables upsert by day with INSERT OR REPLACE, which re-ids a re-saved
        // day; the date_key is the identity that survives, so the mirror must be addressed by it.
        assertEquals("avex-weight-2026-09-01", HcRecordKeys.weight("2026-09-01"))
        assertEquals("avex-bodyfat-2026-09-01", HcRecordKeys.bodyFat("2026-09-01"))
    }

    @Test
    fun keysNeverCollideAcrossTypesForTheSameId() {
        val keys = listOf(
            HcRecordKeys.session(1L),
            HcRecordKeys.sessionCalories(1L),
            HcRecordKeys.sessionHeartRate(1L),
            HcRecordKeys.cardio(1L)
        )
        assertEquals(keys.size, keys.toSet().size)
        assertNotEquals(HcRecordKeys.weight("1"), HcRecordKeys.bodyFat("1"))
    }

    @Test
    fun sessionMirrorsFanOutEveryTypeForEveryId() {
        val keys = HcRecordKeys.sessionMirrors(listOf(3L, 5L))
        assertEquals(listOf("avex-session-3", "avex-session-5"), keys.sessions)
        assertEquals(listOf("avex-session-kcal-3", "avex-session-kcal-5"), keys.calories)
        assertEquals(listOf("avex-session-hr-3", "avex-session-hr-5"), keys.heartRate)
    }

    @Test
    fun sessionMirrorsOfNothingAreEmpty() {
        val keys = HcRecordKeys.sessionMirrors(emptyList())
        assertEquals(emptyList<String>(), keys.sessions)
        assertEquals(emptyList<String>(), keys.calories)
        assertEquals(emptyList<String>(), keys.heartRate)
    }
}
