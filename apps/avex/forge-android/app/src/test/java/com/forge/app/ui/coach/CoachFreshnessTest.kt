package com.forge.app.ui.coach

import androidx.lifecycle.ViewModelProvider
import androidx.test.core.app.ApplicationProvider
import com.forge.app.ForgeApp
import com.forge.app.MainActivity
import com.forge.app.data.db.session
import com.forge.app.widget.WidgetEntryPoint
import dagger.hilt.android.EntryPointAccessors
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.*
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Real Hilt ViewModel retained across stop/return, reading actual Room revisions. */
@RunWith(RobolectricTestRunner::class)
@Config(application=ForgeApp::class, sdk=[34])
@OptIn(ExperimentalCoroutinesApi::class)
class CoachFreshnessTest {
 @Test fun `retained coach reloads finished training on return`() = runBlocking {
  val app = ApplicationProvider.getApplicationContext<ForgeApp>()
  withTimeout(10000) { app.awaitStorageReady() }
  app.settingsRepository.setFreestyleMode(false)
  Dispatchers.setMain(Dispatchers.Unconfined)
  val activity = Robolectric.buildActivity(MainActivity::class.java).create()
  try {
   val vm = ViewModelProvider(activity.get())[CoachViewModel::class.java]
   val dao = EntryPointAccessors.fromApplication(app, WidgetEntryPoint::class.java).sessionDao()
   var visible = launch { vm.refreshWhileVisible() }
   withTimeout(10000) { vm.state.first { !it.loading } }
   val before = vm.state.value.brief?.sessionsLogged ?: 0
   visible.cancelAndJoin()
   val now = System.currentTimeMillis()
   dao.insert(session(startedAt=now-100000,finishedAt=now-1))
   visible = launch { vm.refreshWhileVisible() }
   withTimeout(10000) { vm.state.first { (it.brief?.sessionsLogged ?: 0) > before } }
   visible.cancelAndJoin()
   assertFalse(vm.state.value.loading)
  } finally { activity.destroy(); Dispatchers.resetMain() }
 }
}
