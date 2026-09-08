package com.forge.app.security

import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.flow.first
import javax.inject.Inject

/** Recheck protection at the write boundary, including callers outside the Settings composables. */
class ProtectedSettingsActions @Inject constructor(
    private val settings: SettingsRepository,
    private val lock: AppLockManager
) {
    suspend fun setAppLock(enabled: Boolean) {
        if (enabled || !settings.appLockEnabled.first() || lock.isAuthenticated) settings.setAppLockEnabled(enabled)
    }
    suspend fun setGalleryLock(enabled: Boolean) {
        if (enabled || !settings.galleryLockEnabled.first() || lock.isAuthenticated) settings.setGalleryLockEnabled(enabled)
    }
    suspend fun canExportPhotos(): Boolean = !settings.galleryLockEnabled.first() || lock.isAuthenticated
}
