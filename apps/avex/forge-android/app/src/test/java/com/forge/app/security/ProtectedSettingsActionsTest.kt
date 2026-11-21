package com.forge.app.security
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.prefs.SettingsRepository
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
@RunWith(RobolectricTestRunner::class)
@Config(application=android.app.Application::class)
@OptIn(ExperimentalCoroutinesApi::class)
class ProtectedSettingsActionsTest {
 @Test fun `gallery protection gates settings and photo export until authenticated`() = runBlocking {
  Dispatchers.setMain(Dispatchers.Unconfined)
  try {
   val settings=SettingsRepository(ApplicationProvider.getApplicationContext<Context>(),Clock { 1000L })
   settings.setAppLockEnabled(false);settings.setGalleryLockEnabled(true)
   val manager=AppLockManager(settings)
   withTimeout(5000) { manager.galleryLocked.first { it } }
   assertFalse(manager.appLocked.value)
   val actions = ProtectedSettingsActions(settings, manager)
   actions.setGalleryLock(false)
   assertTrue(settings.galleryLockEnabled.first())
   assertFalse(actions.canExportPhotos())
   settings.resetSettingsOnly()
   assertTrue(settings.galleryLockEnabled.first())
   assertFalse(actions.canExportPhotos())
   manager.markAuthenticated()
   assertTrue(actions.canExportPhotos())
   actions.setGalleryLock(false)
   withTimeout(5000) { manager.galleryLocked.first { !it } }
   assertFalse(manager.galleryLocked.value)
  } finally { Dispatchers.resetMain() }
 }
}