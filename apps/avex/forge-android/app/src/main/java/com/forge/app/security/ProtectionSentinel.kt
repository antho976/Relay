package com.forge.app.security

import android.content.Context

/**
 * A tiny, independent record of which protections the user turned on.
 *
 * Everything the app knows about privacy mode and the two locks lives in the DataStore, and reading
 * it can fail: an IOException on an unreadable file, a corruption the store's
 * `ReplaceFileCorruptionHandler` resets, a partially-written blob left by an interrupted restore.
 * Every one of those paths degrades to `emptyPreferences()`, and empty preferences say privacy mode
 * is off, the app lock is off and the gallery lock is off — so a transient read failure at launch
 * cleared FLAG_SECURE, primed [AppLockManager] as unlocked, and put the photo gallery on screen
 * behind no gate at all. A protection that disappears when a file is briefly unreadable is not a
 * protection.
 *
 * Failing CLOSED on nothing but a default is not the answer either: priming the app lock on for
 * someone who never enabled it, on a device with no enrolled biometric, locks them out of their own
 * training history. So the fallback is not a guess — it is what the user last actually chose, kept
 * in a separate SharedPreferences file that is written on every successful read and consulted only
 * when a read fails.
 *
 * Two independent stores being unreadable at the same moment is a different situation: the app's
 * data is gone, and there is nothing left to protect.
 */
object ProtectionSentinel {

    private const val PREFS = "avex_protection_sentinel"
    private const val KEY_KNOWN = "known"
    private const val KEY_PRIVACY = "privacy_mode"
    private const val KEY_APP_LOCK = "app_lock"
    private const val KEY_GALLERY_LOCK = "gallery_lock"

    /** What the user last chose, as far as this file knows. */
    data class Protections(
        val privacyMode: Boolean,
        val appLockEnabled: Boolean,
        val galleryLockEnabled: Boolean
    )

    private fun prefs(context: Context) =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** Record the settings that were just read successfully. Cheap enough to call on every emission. */
    fun remember(context: Context, protections: Protections) {
        runCatching {
            prefs(context).edit()
                .putBoolean(KEY_KNOWN, true)
                .putBoolean(KEY_PRIVACY, protections.privacyMode)
                .putBoolean(KEY_APP_LOCK, protections.appLockEnabled)
                .putBoolean(KEY_GALLERY_LOCK, protections.galleryLockEnabled)
                .apply()
        }
    }

    /** The last recorded protections, or null when none were ever recorded. */
    fun lastKnown(context: Context): Protections? = runCatching {
        val p = prefs(context)
        if (!p.getBoolean(KEY_KNOWN, false)) return null
        Protections(
            privacyMode = p.getBoolean(KEY_PRIVACY, false),
            appLockEnabled = p.getBoolean(KEY_APP_LOCK, false),
            galleryLockEnabled = p.getBoolean(KEY_GALLERY_LOCK, false)
        )
    }.getOrNull()

    /**
     * What to apply when the settings read failed: the last known choice, or — with nothing
     * recorded — privacy mode ON and both locks off.
     *
     * That asymmetry is deliberate. FLAG_SECURE costs a user who never asked for it one screenshot
     * they can retry after the next successful read; a primed app lock costs a user who never
     * enabled it access to their own data, possibly permanently. When the honest answer is unknown,
     * take the protection that cannot lock anyone out and leave the one that can.
     */
    fun fallback(context: Context): Protections =
        lastKnown(context) ?: Protections(
            privacyMode = true,
            appLockEnabled = false,
            galleryLockEnabled = false
        )
}
