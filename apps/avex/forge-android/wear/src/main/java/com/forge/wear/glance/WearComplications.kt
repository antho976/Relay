package com.forge.wear.glance

import com.forge.shared.protocol.loadAdjustment
import com.forge.shared.protocol.loadAdjustmentText

import androidx.wear.watchface.complications.data.ComplicationData
import androidx.wear.watchface.complications.data.ComplicationType
import androidx.wear.watchface.complications.data.CountDownTimeReference
import androidx.wear.watchface.complications.data.PlainComplicationText
import androidx.wear.watchface.complications.data.RangedValueComplicationData
import androidx.wear.watchface.complications.data.ShortTextComplicationData
import androidx.wear.watchface.complications.data.TimeDifferenceComplicationText
import androidx.wear.watchface.complications.data.TimeDifferenceStyle
import androidx.wear.watchface.complications.datasource.ComplicationRequest
import androidx.wear.watchface.complications.datasource.SuspendingComplicationDataSourceService
import com.forge.wear.data.WearClockSkew
import java.time.Instant

/**
 * Watch-face complications (W4): readiness (ranged/short-text), next session (short-text) and the
 * active rest timer (a locally-rendering countdown via TimeDifferenceComplicationText — no update
 * budget burned on ticking). All read the mirrored DataItems on demand and degrade to honest
 * placeholders; WearDataListenerService requests updates when the phone republishes.
 */

private fun text(value: String) = PlainComplicationText.Builder(value).build()

class ReadinessComplicationService : SuspendingComplicationDataSourceService() {

    override fun getPreviewData(type: ComplicationType): ComplicationData? = when (type) {
        ComplicationType.RANGED_VALUE -> RangedValueComplicationData.Builder(
            value = 3f, min = 0f, max = 10f, contentDescription = text("Suggested load adjustment: -2%")
        ).setText(text("-2%")).build()
        ComplicationType.SHORT_TEXT -> ShortTextComplicationData.Builder(text("-2%"), text("Suggested load adjustment"))
            .setTitle(text("LOAD")).build()
        else -> null
    }

    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationData? {
        val readiness = WearGlanceStore.glance(this)?.loadAdjustment
        return when (request.complicationType) {
            ComplicationType.RANGED_VALUE -> RangedValueComplicationData.Builder(
                value = ((readiness ?: 0) + 5).toFloat(), min = 0f, max = 10f,
                contentDescription = text("Suggested load adjustment")
            ).setText(text(loadAdjustmentText(readiness))).build()
            ComplicationType.SHORT_TEXT -> ShortTextComplicationData.Builder(
                text(loadAdjustmentText(readiness)), text("Suggested load adjustment")
            ).setTitle(text("LOAD")).build()
            else -> null
        }
    }
}

class NextSessionComplicationService : SuspendingComplicationDataSourceService() {

    override fun getPreviewData(type: ComplicationType): ComplicationData? =
        ShortTextComplicationData.Builder(text("Pull B"), text("Next session"))
            .setTitle(text("NEXT")).build()

    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationData? {
        if (request.complicationType != ComplicationType.SHORT_TEXT) return null
        val next = WearGlanceStore.glance(this)?.nextDayTitle
        return ShortTextComplicationData.Builder(text(next ?: "—"), text("Next session"))
            .setTitle(text("NEXT")).build()
    }
}

class RestTimerComplicationService : SuspendingComplicationDataSourceService() {

    override fun getPreviewData(type: ComplicationType): ComplicationData? =
        ShortTextComplicationData.Builder(text("2:30"), text("Rest timer"))
            .setTitle(text("REST")).build()

    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationData? {
        if (request.complicationType != ComplicationType.SHORT_TEXT) return null
        val now = System.currentTimeMillis()
        // endAtMs is an instant on the PHONE's clock and the watch face counts down on its OWN, so
        // comparing the two directly turned every millisecond of skew between the devices into a
        // millisecond of error — and in the opposite direction from the same countdown inside the
        // app, which corrects for it (RestCountdown). With no measurement available the phone
        // instant is returned unchanged, which is exactly the previous behaviour.
        val endOnThisWatch = WearGlanceStore.timer(this)
            ?.takeIf { !it.paused }
            ?.let { WearClockSkew.toWatchInstant(this, it.endAtMs, now) }
            ?.takeIf { it > now }
        if (endOnThisWatch == null) {
            return ShortTextComplicationData.Builder(text("—"), text("Rest timer"))
                .setTitle(text("REST")).build()
        }
        // TimeDifference text renders the countdown locally on the watch face — zero updates.
        val countdown = TimeDifferenceComplicationText.Builder(
            TimeDifferenceStyle.SHORT_DUAL_UNIT,
            CountDownTimeReference(Instant.ofEpochMilli(endOnThisWatch))
        ).build()
        return ShortTextComplicationData.Builder(countdown, text("Rest timer"))
            .setTitle(text("REST")).build()
    }
}
