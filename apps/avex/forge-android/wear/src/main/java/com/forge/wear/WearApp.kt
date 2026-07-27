package com.forge.wear

import android.app.Application
import com.forge.wear.data.SessionOngoing
import com.forge.wear.data.WearDataRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChangedBy
import kotlinx.coroutines.launch

/**
 * Watch app entry (W1). Boots the Data Layer repository and keeps the OngoingActivity chip in
 * lockstep with session presence — the repo is a process singleton, so the activity, the
 * background listener and the tiles all see one state.
 */
class WearApp : Application() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        val repo = WearDataRepository.instance(this)
        scope.launch {
            repo.session.distinctUntilChangedBy { it?.sessionId }.collect { session ->
                if (session == null) {
                    SessionOngoing.clear(this@WearApp)
                    com.forge.wear.service.WearHrService.stop(this@WearApp)
                } else {
                    SessionOngoing.show(this@WearApp, session.dayTitle)
                    // Live HR follows session presence (W3) — only with the sensor permission;
                    // denied = the session works without HR, no nagging.
                    if (checkSelfPermission(android.Manifest.permission.BODY_SENSORS) ==
                        android.content.pm.PackageManager.PERMISSION_GRANTED
                    ) {
                        com.forge.wear.service.WearHrService.start(this@WearApp, session.sessionId)
                    }
                }
            }
        }
    }
}
