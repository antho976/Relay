package com.forge.app.data.prefs

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.domain.units.WeightUnit
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * A section's "Reset this section" restores EVERY control the page draws and nothing else, against
 * a real DataStore.
 *
 * Prompted by the Format page: its Distance and Length chip rows kept their explicit picks through
 * a reset because their keys were never in [SettingsSection.FORMAT]. The reset row promises the
 * page; a reset that quietly does less than that is worse than none, because the user stops
 * looking at the control they already "reset".
 */
@RunWith(RobolectricTestRunner::class)
class SettingsSectionResetTest {

    private lateinit var repo: SettingsRepository

    @Before
    fun setUp() {
        val context: Context = ApplicationProvider.getApplicationContext()
        repo = SettingsRepository(context, Clock { 0L })
    }

    @Test
    fun formatResetRestoresEveryControlThePageOwns() = runTest {
        // Every control on the Format page, set away from its default.
        repo.setWeightUnit(WeightUnit.KG)
        repo.setUseMiles(true)   // kg would derive km; an explicit "mi" is the case that stuck
        repo.setUseCm(false)     // kg would derive cm; an explicit "in" is the other one
        repo.setDateFormat("dd/MM/yyyy")
        repo.setTimeFormat24h(true)
        repo.setFirstDayMonday(false)
        repo.setTimezone("Pacific/Auckland")
        repo.setUserSex("female")

        assertTrue("precondition: explicit miles held", repo.useMiles.first())
        assertFalse("precondition: explicit inches held", repo.useCm.first())

        repo.resetSection(SettingsSection.FORMAT)

        assertEquals(WeightUnit.LB, repo.weightUnit.first())
        // With no explicit pick, distance and length derive from the (now default, lb) weight unit.
        assertTrue("distance follows lb again", repo.useMiles.first())
        assertFalse("length follows lb again", repo.useCm.first())
        assertEquals("MMM d, yyyy", repo.dateFormat.first())
        assertFalse(repo.timeFormat24h.first())
        assertTrue(repo.firstDayMonday.first())
        assertEquals(java.util.TimeZone.getDefault().id, repo.timezone.first())
        assertEquals("", repo.userSex.first())
    }

    @Test
    fun formatResetLeavesUnrelatedKeysAlone() = runTest {
        repo.toggleFavoriteTimezone("Europe/Lisbon") // data the page shows, not a format default
        repo.setHapticStrength("light")              // another section entirely
        repo.setUseMiles(false)
        repo.setUseCm(true)

        repo.resetSection(SettingsSection.FORMAT)

        assertTrue("starred timezones survive", "Europe/Lisbon" in repo.favoriteTimezones.first())
        assertEquals("Session's haptics survive", "light", repo.hapticStrength.first())
    }

    /** The explicit distance/length picks come back as derivations, whichever way they pointed. */
    @Test
    fun formatResetDropsExplicitDistanceAndLengthPicksInBothDirections() = runTest {
        repo.setWeightUnit(WeightUnit.LB)
        repo.setUseMiles(false)
        repo.setUseCm(true)
        assertFalse(repo.useMiles.first())
        assertTrue(repo.useCm.first())

        repo.resetSection(SettingsSection.FORMAT)

        assertTrue("lb derives miles once the explicit km pick is gone", repo.useMiles.first())
        assertFalse("lb derives inches once the explicit cm pick is gone", repo.useCm.first())
    }
}
