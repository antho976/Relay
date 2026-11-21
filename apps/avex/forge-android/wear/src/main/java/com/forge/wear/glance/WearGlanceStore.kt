package com.forge.wear.glance

import android.content.Context
import android.net.Uri
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.forge.shared.protocol.TimerStateDto
import com.forge.shared.protocol.WearCodec
import com.forge.shared.protocol.WearProtocol
import com.google.android.gms.wearable.PutDataRequest
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

    /**
     * Path-scoped: the one item asked for, not every DataItem on the node. The full read also
     * carried the per-command acks (up to ten live at once) on every tile and complication render.
     * A `wear:` URI with no host matches that path from any node.
     */
    private suspend inline fun <reified T> fetch(context: Context, path: String): T? = try {
        val uri = Uri.Builder().scheme(PutDataRequest.WEAR_URI_SCHEME).path(path).build()
        val buffer = Wearable.getDataClient(context).getDataItems(uri).await()
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
