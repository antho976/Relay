package com.forge.app.service

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.forge.app.widget.refreshForgeWidgets
import java.time.Instant
import java.time.ZoneId
import java.util.concurrent.TimeUnit

/**
 * Redraws the home-screen widget at each local midnight, then re-arms itself for the next one.
 *
 * The widget's content is calendar-derived — `LocalDate.now(zone)`, `today.with(MONDAY)` — but
 * nothing rolled it over at midnight. `updatePeriodMillis` is a one-hour FLOOR that does not wake a
 * sleeping device, so the real staleness on a phone asleep from 23:00 to 06:00 is "until you pick it
 * up": a 06:00 Monday gym-goer left the house looking at Sunday's next-up day and last week's
 * Mon-Sun dot row. `ACTION_DATE_CHANGED` covers the same boundary, but not every device sends it,
 * and it doesn't fire while dozing either.
 *
 * Deliberately a one-shot that re-schedules rather than a 24-hour periodic: a periodic measures
 * ELAPSED time, so it drifts off the wall clock at every DST change and never lands on midnight
 * again. Each run recomputes the next boundary in the CURRENT zone, so a flight or a spring-forward
 * re-anchors it on the following run. No constraints and no network — it is a redraw.
 */
class WidgetMidnightWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        // Re-arm FIRST: a redraw that throws must not end the chain, or the widget stops rolling
        // over for the life of the install with nothing to show that it did. The successor is named
        // for the NEXT midnight, so arming it no longer cancels this run mid-redraw.
        schedule(applicationContext)
        refreshForgeWidgets(applicationContext)
        return Result.success()
    }

    companion object {
        private const val TAG = "forge_widget_midnight"

        /**
         * Arm the redraw for the next local midnight, keeping one already armed for it.
         *
         * This used to REPLACE one unique name, and [doWork] calls it before its own redraw, so the
         * run that armed its successor was the unfinished work REPLACE cancels: the midnight redraw
         * could be cut off inside `refreshForgeWidgets` (2026-09-26 audit, 11 / domain D3). Each
         * midnight now has its own name, enqueued with KEEP: a launch in the same zone is a no-op,
         * and a flight or a clock change arms its new midnight beside the old one, which then costs
         * one spare redraw and re-arms onto the same name the new chain already holds.
         */
        fun schedule(context: Context) {
            val now = System.currentTimeMillis()
            val nextMidnight = nextMidnightMs(now, ZoneId.systemDefault())
            // A floor of one minute keeps a wrong-clock device from queueing a zero/negative delay
            // in a tight loop; the ceiling of a day bounds a clock that reads far in the past.
            val delayMs = (nextMidnight - now).coerceIn(60_000L, TimeUnit.DAYS.toMillis(1))
            val request = OneTimeWorkRequestBuilder<WidgetMidnightWorker>()
                .addTag(TAG)
                .setInitialDelay(delayMs, TimeUnit.MILLISECONDS)
                .build()
            runCatching {
                WorkManager.getInstance(context)
                    .enqueueUniqueWork(workNameFor(nextMidnight), ExistingWorkPolicy.KEEP, request)
            }
        }

        /** Epoch-ms of the first local midnight strictly after [nowMs] in [zone]. */
        internal fun nextMidnightMs(nowMs: Long, zone: ZoneId): Long =
            Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
                .plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()

        /** One unique name per midnight instant. */
        internal fun workNameFor(midnightMs: Long): String = "$TAG@$midnightMs"
    }
}
