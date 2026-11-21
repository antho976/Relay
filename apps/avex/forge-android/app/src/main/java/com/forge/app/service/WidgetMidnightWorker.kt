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
        arm(applicationContext, successorMidnightMs(System.currentTimeMillis(), ZoneId.systemDefault()))
        refreshForgeWidgets(applicationContext)
        return Result.success()
    }

    companion object {
        private const val TAG = "forge_widget_midnight"

        /**
         * Arm the redraw for the next local midnight, keeping one already armed for it.
         *
         * This used to REPLACE one unique name, and [doWork] arms before its own redraw, so the
         * run that armed its successor was the unfinished work REPLACE cancels: the midnight redraw
         * could be cut off inside `refreshForgeWidgets` (2026-09-26 audit, 11 / domain D3). Each
         * midnight now has its own name, enqueued with KEEP: a launch in the same zone is a no-op,
         * and a flight or a clock change arms its new midnight beside the old one, which then costs
         * one spare redraw and re-arms onto the same name the new chain already holds.
         */
        fun schedule(context: Context) {
            arm(context, nextMidnightMs(System.currentTimeMillis(), ZoneId.systemDefault()))
        }

        private fun arm(context: Context, midnightMs: Long) {
            val request = OneTimeWorkRequestBuilder<WidgetMidnightWorker>()
                .addTag(TAG)
                .setInitialDelay(delayUntilMs(midnightMs, System.currentTimeMillis()), TimeUnit.MILLISECONDS)
                .build()
            runCatching {
                WorkManager.getInstance(context)
                    .enqueueUniqueWork(workNameFor(midnightMs), ExistingWorkPolicy.KEEP, request)
            }
        }

        /** Epoch-ms of the first local midnight strictly after [nowMs] in [zone]. */
        internal fun nextMidnightMs(nowMs: Long, zone: ZoneId): Long =
            Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
                .plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli()

        /**
         * The midnight a run at [nowMs] arms: the one after its OWN.
         *
         * A run can fire a little before the midnight it was armed for — the wall clock corrected
         * backwards since. The nearest midnight is then this run's own, whose unique name is this
         * RUNNING work's, and KEEP silently drops the successor: the chain ends until the next app
         * launch. Looking [EARLY_RUN_SLACK_MS] ahead steps past it.
         */
        internal fun successorMidnightMs(nowMs: Long, zone: ZoneId): Long =
            nextMidnightMs(nowMs + EARLY_RUN_SLACK_MS, zone)

        /**
         * Delay from [nowMs] to [midnightMs]. The floor keeps a wrong-clock device from queueing a
         * zero/negative delay in a tight loop.
         *
         * The ceiling has to admit a 25-hour day. It used to be exactly one day, so on a DST
         * fall-back day the redraw ran at 23:00 instead of midnight, and the "next midnight" it
         * re-armed from there was its own name — KEEP dropped it and the chain ended.
         */
        internal fun delayUntilMs(midnightMs: Long, nowMs: Long): Long =
            (midnightMs - nowMs).coerceIn(MIN_DELAY_MS, MAX_DELAY_MS)

        private const val EARLY_RUN_SLACK_MS = 5 * 60_000L
        private const val MIN_DELAY_MS = 60_000L
        /** A 25-hour fall-back day plus [EARLY_RUN_SLACK_MS], with room to spare. */
        private const val MAX_DELAY_MS = 26 * 60 * 60_000L

        /** One unique name per midnight instant. */
        internal fun workNameFor(midnightMs: Long): String = "$TAG@$midnightMs"
    }
}
