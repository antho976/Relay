package com.forge.wear.service

import androidx.wear.tiles.TileService
import androidx.wear.watchface.complications.datasource.ComplicationDataSourceUpdateRequester
import android.content.ComponentName
import com.forge.shared.protocol.WearProtocol
import com.forge.wear.data.WearDataRepository
import com.forge.wear.glance.NextSessionComplicationService
import com.forge.wear.glance.ReadinessComplicationService
import com.forge.wear.glance.RestTimerComplicationService
import com.forge.wear.glance.TodayTileService
import com.forge.wear.glance.WeekTileService
import com.google.android.gms.wearable.DataEvent
import com.google.android.gms.wearable.DataEventBuffer
import com.google.android.gms.wearable.WearableListenerService

/**
 * Background ear (W1/W4): the Data Layer wakes this when the phone publishes while the watch app
 * isn't running. Booting [WearDataRepository] absorbs the change (session chip, HR service via the
 * Application collector); tiles and complications don't refresh themselves, so the relevant paths
 * explicitly request their update here — the W4 refresh mechanics.
 */
class WearDataListenerService : WearableListenerService() {

    override fun onDataChanged(events: DataEventBuffer) {
        WearDataRepository.instance(this)
        var glanceChanged = false
        var timerChanged = false
        for (event in events) {
            if (event.type != DataEvent.TYPE_CHANGED && event.type != DataEvent.TYPE_DELETED) continue
            when (event.dataItem.uri.path) {
                WearProtocol.PATH_GLANCE_TODAY -> glanceChanged = true
                WearProtocol.PATH_TIMER_STATE -> timerChanged = true
            }
        }
        if (glanceChanged) {
            runCatching { TileService.getUpdater(this).requestUpdate(TodayTileService::class.java) }
            runCatching { TileService.getUpdater(this).requestUpdate(WeekTileService::class.java) }
            requestComplication(ReadinessComplicationService::class.java)
            requestComplication(NextSessionComplicationService::class.java)
        }
        if (timerChanged) requestComplication(RestTimerComplicationService::class.java)
        super.onDataChanged(events)
    }

    private fun requestComplication(service: Class<*>) {
        runCatching {
            ComplicationDataSourceUpdateRequester
                .create(this, ComponentName(this, service))
                .requestUpdateAll()
        }
    }
}
