package com.forge.app.data.prefs

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.security.ProtectionSentinel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * What the three protection settings read as when the settings file cannot be trusted.
 *
 * The failure this pins down: `allPreferences` catches an IOException and emits
 * `emptyPreferences()`, and empty preferences say "privacy off, app lock off, gallery lock off" in
 * exactly the same words a user who wants none of them says it. So a briefly unreadable file — or a
 * corruption the store's handler had just reset to empty — cleared FLAG_SECURE, primed the app lock
 * as unlocked and put the photo grid on screen behind no gate. `ProtectionSentinel` was written to
 * cover that and could never fire, because the catch had already turned the failure into a success.
 *
 * There are three cases and they are genuinely different, so each is asserted separately.
 */
@RunWith(RobolectricTestRunner::class)
class ProtectionFallbackTest {

    private lateinit var context: Context
    private lateinit var repo: SettingsRepository

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        ProtectionSentinel.forget(context)
        repo = SettingsRepository(context, Clock { 0L })
    }

    // ── Case 1: a fresh install really does default to off ────────────────────

    @Test
    fun aFreshInstallLeavesEveryProtectionOff() = runTest {
        assertNull("no sentinel record on a fresh install", ProtectionSentinel.lastKnown(context))
        assertFalse("privacy mode", repo.privacyMode.first())
        assertFalse("app lock", repo.appLockEnabled.first())
        assertFalse("gallery lock", repo.galleryLockEnabled.first())
    }

    // ── Case 2: an emptied store is not a fresh install ───────────────────────

    /**
     * The corruption handler replaces an unparseable file with `emptyPreferences()`, and a restore
     * can leave a partially-written blob behind. Either way the keys are gone while the user's
     * choices are not — the sentinel is the only surviving record of them, and an absent key with a
     * record present means the store lost something rather than that nothing was ever chosen.
     */
    @Test
    fun aWipedStoreFallsBackToWhatTheUserLastChose() = runTest {
        repo.setPrivacyMode(true)
        repo.setGalleryLockEnabled(true)
        ProtectionSentinel.remember(
            context,
            ProtectionSentinel.Protections(
                privacyMode = true, appLockEnabled = false, galleryLockEnabled = true
            )
        )

        // The store loses its contents WITHOUT the user asking — what the corruption handler's
        // `emptyPreferences()` replacement leaves behind. Deliberately not `resetAll()`, which is a
        // wipe the user asked for and clears the sentinel along with everything else.
        context.forgePreferences.edit { it.clear() }

        assertTrue("privacy mode survives the wipe", repo.privacyMode.first())
        assertTrue("gallery lock survives the wipe", repo.galleryLockEnabled.first())
        assertFalse("a lock they never enabled stays off", repo.appLockEnabled.first())
    }

    /**
     * The counterpart, and the reason [ProtectionSentinel.forget] exists: a wipe the user ASKED for
     * has to clear the sentinel too. Without that, the case above would resurrect every protection
     * after a factory reset and the reset would look broken.
     */
    @Test
    fun aFactoryResetClearsTheSentinelSoProtectionsDoNotComeBack() = runTest {
        repo.setPrivacyMode(true)
        repo.setGalleryLockEnabled(true)
        ProtectionSentinel.remember(
            context,
            ProtectionSentinel.Protections(
                privacyMode = true, appLockEnabled = true, galleryLockEnabled = true
            )
        )

        repo.resetAll()

        assertNull("the record is gone", ProtectionSentinel.lastKnown(context))
        assertFalse("privacy mode stays off", repo.privacyMode.first())
        assertFalse("app lock stays off", repo.appLockEnabled.first())
        assertFalse("gallery lock stays off", repo.galleryLockEnabled.first())
    }

    @Test
    fun resettingSettingsOnlyAlsoClearsTheSentinel() = runTest {
        repo.setPrivacyMode(true)
        ProtectionSentinel.remember(
            context,
            ProtectionSentinel.Protections(
                privacyMode = true, appLockEnabled = false, galleryLockEnabled = false
            )
        )

        repo.resetSettingsOnly()

        assertNull(ProtectionSentinel.lastKnown(context))
        assertFalse("privacy mode is not preserved by this reset, so it must not return", repo.privacyMode.first())
    }

    // ── Case 3: an explicit choice always beats the sentinel ──────────────────

    /**
     * Turning a protection OFF writes `false`; it does not remove the key. The stored value has to
     * win, or a user could never switch privacy mode off again once the sentinel had seen it on.
     */
    @Test
    fun anExplicitOffIsHonouredOverARememberedOn() = runTest {
        ProtectionSentinel.remember(
            context,
            ProtectionSentinel.Protections(
                privacyMode = true, appLockEnabled = true, galleryLockEnabled = true
            )
        )
        repo.setPrivacyMode(false)
        repo.setAppLockEnabled(false)
        repo.setGalleryLockEnabled(false)

        assertFalse("privacy mode", repo.privacyMode.first())
        assertFalse("app lock", repo.appLockEnabled.first())
        assertFalse("gallery lock", repo.galleryLockEnabled.first())
    }

    // ── The two sentinel accessors differ, and the difference is the point ────

    /**
     * `lastKnown` is for an absent key on a GOOD read: no record means a fresh install, which
     * defaults off. `fallback` is for a read that FAILED: no record means we cannot tell, so it
     * secures the window and primes no lock.
     *
     * The asymmetry is deliberate. FLAG_SECURE costs a user who never asked for it one screenshot
     * they can retry after the next good read; a primed app lock costs a user who never enabled it
     * access to their own training history, on a device that may have no enrolled biometric.
     */
    @Test
    fun anUnknownFailedReadSecuresTheWindowWithoutPrimingALock() {
        assertNull(ProtectionSentinel.lastKnown(context))

        val f = ProtectionSentinel.fallback(context)
        assertTrue("privacy mode is the protection that cannot lock anyone out", f.privacyMode)
        assertFalse("never prime an app lock nobody enabled", f.appLockEnabled)
        assertFalse("never prime a gallery lock nobody enabled", f.galleryLockEnabled)
    }

    @Test
    fun aFailedReadPrefersTheRememberedChoiceOverThatDefault() {
        ProtectionSentinel.remember(
            context,
            ProtectionSentinel.Protections(
                privacyMode = false, appLockEnabled = true, galleryLockEnabled = false
            )
        )
        val f = ProtectionSentinel.fallback(context)
        assertFalse("a deliberate privacy-off is not overridden", f.privacyMode)
        assertTrue("their app lock is restored", f.appLockEnabled)
        assertFalse(f.galleryLockEnabled)
    }

    // ── The startup read reports failure instead of reporting defaults ────────

    /**
     * `MainActivity` wraps `startupPreferences()` in `runCatching` and applies the sentinel in
     * `getOrElse`. That branch was unreachable: the catch on `allPreferences` had already made the
     * failure a success, so the `onSuccess` branch ran instead and wrote every protection off into
     * the sentinel — destroying the record it existed to keep. A good read must still return
     * normally, which is what this asserts; the failing read now throws.
     */
    @Test
    fun aGoodStartupReadStillReturnsTheStoredValues() = runTest {
        repo.setPrivacyMode(true)
        repo.setAppLockEnabled(true)

        val startup = repo.startupPreferences()
        assertTrue(startup.privacyMode)
        assertTrue(startup.appLockEnabled)
        assertFalse(startup.galleryLockEnabled)
    }

    @Test
    fun rememberRoundTripsThroughItsOwnFile() {
        val p = ProtectionSentinel.Protections(
            privacyMode = true, appLockEnabled = false, galleryLockEnabled = true
        )
        ProtectionSentinel.remember(context, p)
        assertEquals(p, ProtectionSentinel.lastKnown(context))
    }
}
