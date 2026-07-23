package com.forge.wear.glance

import android.content.Context
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.shared.protocol.SessionLiveDto
import com.forge.shared.protocol.TimerStateDto
import com.forge.shared.protocol.WearCodec
import com.forge.shared.protocol.WearProtocol
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.tasks.await

/**
 * Direct DataItem reads for the glanceable surfaces (W4). Tiles and complications render in
 * short-lived binder callbacks where the app's live repository may not be seeded yet, so they
 * fetch the current item on demand — the DataItem IS the cache (latest-wins, survives both apps
 * restarting). Fail-soft to null: a tile degrades, never errors.
 */
object WearGlanceStore {

    suspend fun glance(context: Context): GlanceTodayDto? = fetch(context, WearProtocol.PATH_GLANCE_TODAY)

    suspend fun config(context: Context): ConfigDto = fetch(context, WearProtocol.PATH_CONFIG) ?: ConfigDto()

    suspend fun timer(context: Context): TimerStateDto? = fetch(context, WearProtocol.PATH_TIMER_STATE)

    suspend fun session(context: Context): SessionLiveDto? = fetch(context, WearProtocol.PATH_SESSION_LIVE)

    private suspend inline fun <reified T> fetch(context: Context, path: String): T? = try {
        val buffer = Wearable.getDataClient(context).dataItems.await()
        try {
            buffer.firstOrNull { it.uri.path == path }?.data?.let { bytes ->
                when (val d = WearCodec.decode<T>(bytes)) {
                    is WearCodec.DecodeResult.Ok -> d.value
                    else -> null
                }
            }
        } finally {
            buffer.release()
        }
    } catch (t: Throwable) {
        null
    }
}
