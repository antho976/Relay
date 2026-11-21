package com.forge.wear.glance

import com.forge.shared.protocol.loadAdjustment
import com.forge.shared.protocol.loadAdjustmentText

import android.content.Context
import androidx.wear.protolayout.ColorBuilders.argb
import androidx.wear.protolayout.DimensionBuilders.dp
import androidx.wear.protolayout.LayoutElementBuilders
import androidx.wear.protolayout.ResourceBuilders
import androidx.wear.protolayout.TimelineBuilders
import androidx.wear.protolayout.material.Text
import androidx.wear.protolayout.material.Typography
import androidx.wear.tiles.RequestBuilders
import androidx.wear.tiles.TileBuilders
import androidx.wear.tiles.TileService
import com.forge.shared.protocol.ConfigDto
import com.forge.shared.protocol.GlanceTodayDto
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.SettableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The swipe-right glances (W4), rendered from /glance/today — always stamped with data age,
 * degrade-never-blank (readiness below its gates → the next day carries the tile). Refresh is
 * DataItem-driven: WearDataListenerService requests a tile update whenever the phone republishes.
 */
abstract class AvexTileService : TileService() {

    abstract fun layout(context: Context, glance: GlanceTodayDto?, accent: Int): LayoutElementBuilders.LayoutElement

    /** Every job here is bounded by [READ_TIMEOUT_MS] plus a layout build, so nothing outlives a render. */
    private val tileScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onTileRequest(requestParams: RequestBuilders.TileRequest): ListenableFuture<TileBuilders.Tile> {
        // TileService calls this on the MAIN thread (the tiles binary marks it @MainThread). It used
        // to runBlocking two sequential Data Layer IPC reads right here, holding the watch's UI
        // thread for as long as Play services took to answer. The future is returned at once and
        // completed off the main thread, and the reads are bounded: a stalled Data Layer renders the
        // "open Avex" placeholder rather than a tile that never answers.
        val future = SettableFuture.create<TileBuilders.Tile>()
        tileScope.launch {
            try {
                val (glance, config) = withTimeoutOrNull(READ_TIMEOUT_MS) {
                    WearGlanceStore.glance(this@AvexTileService) to WearGlanceStore.config(this@AvexTileService)
                } ?: (null to ConfigDto())
                val accent = accentArgb(config.accentHex, config.accentEnabled)
                future.set(
                    TileBuilders.Tile.Builder()
                        .setResourcesVersion(RESOURCES_VERSION)
                        .setTileTimeline(
                            TimelineBuilders.Timeline.fromLayoutElement(layout(this@AvexTileService, glance, accent))
                        )
                        .setFreshnessIntervalMillis(30 * 60_000L)
                        .build()
                )
            } catch (t: Throwable) {
                future.setException(t)
            }
        }
        return future
    }

    override fun onTileResourcesRequest(
        requestParams: RequestBuilders.ResourcesRequest
    ): ListenableFuture<ResourceBuilders.Resources> =
        Futures.immediateFuture(ResourceBuilders.Resources.Builder().setVersion(RESOURCES_VERSION).build())

    protected fun column(vararg children: LayoutElementBuilders.LayoutElement): LayoutElementBuilders.LayoutElement =
        LayoutElementBuilders.Box.Builder()
            .setWidth(androidx.wear.protolayout.DimensionBuilders.expand())
            .setHeight(androidx.wear.protolayout.DimensionBuilders.expand())
            .addContent(
                LayoutElementBuilders.Column.Builder()
                    .setHorizontalAlignment(LayoutElementBuilders.HORIZONTAL_ALIGN_CENTER)
                    .apply { children.forEach { addContent(it) } }
                    .build()
            )
            .build()

    protected fun label(context: Context, text: String, colorArgb: Int): LayoutElementBuilders.LayoutElement =
        Text.Builder(context, text)
            .setTypography(Typography.TYPOGRAPHY_CAPTION3)
            .setColor(argb(colorArgb))
            .build()

    protected fun figure(context: Context, text: String, colorArgb: Int): LayoutElementBuilders.LayoutElement =
        Text.Builder(context, text)
            .setTypography(Typography.TYPOGRAPHY_DISPLAY3)
            .setColor(argb(colorArgb))
            .build()

    protected fun gap(heightDp: Float): LayoutElementBuilders.LayoutElement =
        LayoutElementBuilders.Spacer.Builder().setHeight(dp(heightDp)).build()

    protected companion object {
        const val RESOURCES_VERSION = "1"
        /** Upper bound on the Data Layer reads behind one tile render. */
        const val READ_TIMEOUT_MS = 5_000L
        const val ON_BG = 0xFFF2EFEA.toInt()
        const val MUTED = 0xFFBFB6AA.toInt()

        fun accentArgb(hex: String, enabled: Boolean): Int {
            if (!enabled) return ON_BG
            val h = hex.removePrefix("#")
            return h.takeIf { it.length == 6 }?.toLongOrNull(16)?.let { (0xFF000000L or it).toInt() }
                ?: 0xFFE23D3D.toInt()   // AccentRed — the phone's default
        }

        fun age(computedAtMs: Long): String {
            val min = ((System.currentTimeMillis() - computedAtMs) / 60_000L).coerceAtLeast(0)
            return when {
                min < 1 -> "JUST NOW"
                min < 60 -> "$min MIN AGO"
                else -> "${min / 60}H AGO"
            }
        }
    }
}

/** Today: readiness (or the next day below the gates) + one week line + the age stamp. */
class TodayTileService : AvexTileService() {
    override fun layout(context: Context, glance: GlanceTodayDto?, accent: Int): LayoutElementBuilders.LayoutElement {
        if (glance == null) {
            return column(
                label(context, "AVEX", MUTED),
                gap(4f),
                label(context, "OPEN AVEX ON YOUR PHONE", ON_BG)
            )
        }
        // The phone's directive owns the answer when it has one (Coach v3 B2) — the wrist shows
        // the same words, not its own guess. Older phone builds send no directive, so the readiness
        // figure and next-day fallback still stand.
        val directive = glance.directiveHeadline?.takeIf { it.isNotBlank() }
        val headline = directive?.uppercase()
            ?: glance.loadAdjustment?.let { loadAdjustmentText(it) }
            ?: (glance.nextDayTitle ?: "REST")
        val headlineCaption = when {
            directive != null -> "TODAY"
            glance.loadAdjustment != null -> "LOAD"
            else -> "NEXT"
        }
        val nextLine = glance.nextDayTitle?.takeIf { directive == null && glance.loadAdjustment != null }
        return column(
            label(context, "AVEX · TODAY", MUTED),
            gap(4f),
            figure(context, headline, accent),
            label(context, headlineCaption, MUTED),
            gap(6f),
            *(nextLine?.let { arrayOf(label(context, "NEXT · $it".uppercase(), ON_BG), gap(3f)) } ?: emptyArray()),
            label(context, age(glance.computedAtMs), MUTED)
        )
    }
}

/** Week: sessions done (of planned) + volume + the age stamp. */
class WeekTileService : AvexTileService() {
    override fun layout(context: Context, glance: GlanceTodayDto?, accent: Int): LayoutElementBuilders.LayoutElement {
        if (glance == null) {
            return column(
                label(context, "AVEX", MUTED),
                gap(4f),
                label(context, "OPEN AVEX ON YOUR PHONE", ON_BG)
            )
        }
        val done = "${glance.weekSessionsDone}" + (glance.weekSessionsPlanned?.let { " of $it" } ?: "")
        return column(
            label(context, "AVEX · THIS WEEK", MUTED),
            gap(4f),
            figure(context, done, accent),
            label(context, "SESSIONS", MUTED),
            gap(6f),
            *(glance.weekVolumeText?.let { arrayOf(label(context, it.uppercase(), ON_BG), gap(3f)) } ?: emptyArray()),
            label(context, age(glance.computedAtMs), MUTED)
        )
    }
}
