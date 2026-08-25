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
        // over for the life of the install with nothing to show that it did.
        schedule(applicationContext)
        refreshForgeWidgets(applicationContext)
        return Result.success()
    }

    companion object {
        private const val TAG = "forge_widget_midnight"

        /** (Re)arm the next-midnight redraw, replacing any already-queued one. */
        fun schedule(context: Context) {
            val zone = ZoneId.systemDefault()
            val now = System.currentTimeMillis()
            val nextMidnight = Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
                .plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()
            // A floor of one minute keeps a wrong-clock device from queueing a zero/negative delay
            // in a tight loop; the ceiling of a day bounds a clock that reads far in the past.
            val delayMs = (nextMidnight - now).coerceIn(60_000L, TimeUnit.DAYS.toMillis(1))
            val request = OneTimeWorkRequestBuilder<WidgetMidnightWorker>()
                .addTag(TAG)
                .setInitialDelay(delayMs, TimeUnit.MILLISECONDS)
                .build()
            runCatching {
                WorkManager.getInstance(context)
                    .enqueueUniqueWork(TAG, ExistingWorkPolicy.REPLACE, request)
            }
        }
    }
}
