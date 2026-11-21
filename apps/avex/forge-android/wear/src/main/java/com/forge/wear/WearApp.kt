package com.forge.wear

import android.app.Application
import android.content.Context
import com.forge.shared.protocol.SessionLiveDto
import com.forge.wear.data.SessionOngoing
import com.forge.wear.data.WearDataRepository
import com.forge.wear.service.WearHrService
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
                applySession(this@WearApp, session)
            }
        }
    }

    companion object {
        /**
         * Bring the wrist's session surfaces in line with [session]: the OngoingActivity chip, and
         * the live-HR foreground service.
         *
         * Deliberately callable from outside the collector above, because the collector keys on
         * `sessionId` and a PERMISSION GRANT does not change it. MainActivity asks for notifications
         * and body sensors at launch, which resolves after the first session emission has already
         * been processed and refused for want of them — so granting used to buy nothing until the
         * phone happened to publish a different session or the app was restarted. Re-applying once
         * on the grant result closes that, and re-applying is cheap: showing the chip again is
         * idempotent, and the service ignores a start for a session it is already streaming.
         */
        fun applySession(context: Context, session: SessionLiveDto?) {
            if (session == null) {
                SessionOngoing.clear(context)
                WearHrService.stop(context)
                return
            }
            SessionOngoing.show(context, session.dayTitle)
            // Live HR follows session presence (W3) — only with the heart-rate permission for THIS
            // api level (BODY_SENSORS, or its health-permission replacement from 36); denied = the
            // session works without HR, no nagging.
            if (WearHealthPermissions.canStreamHeartRate(context)) {
                WearHrService.start(context, session.sessionId)
            }
        }
    }
}
