package com.forge.app.service

import android.content.Context
import androidx.work.ExistingPeriodicWorkPolicy
import com.forge.app.data.prefs.SettingsRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Thin seam between the UI / app boot and [TrainingReminderWorker]'s WorkManager scheduling, so a
 * ViewModel never has to hold WorkManager logic and ForgeApp/Settings share one entry point.
 */
@Singleton
class ReminderScheduler @Inject constructor(
    @ApplicationContext private val context: Context,
    private val settingsRepo: SettingsRepository
) {
    /** React to a user change: re-arm with a fresh fire time (REPLACE) or cancel when turned off. */
    fun apply(enabled: Boolean, hour: Int) {
        if (enabled) TrainingReminderWorker.schedule(context, hour, ExistingPeriodicWorkPolicy.REPLACE)
        else TrainingReminderWorker.cancel(context)
    }

    /**
     * Boot-time: keep an enabled reminder on its wall-clock hour, or ensure it's cancelled.
     *
     * Neither plain policy is right here. KEEP left a 24-hour PERIODIC on elapsed time, so an 18:00
     * reminder moved an hour at each DST change and stayed there for months. REPLACE, run
     * at every cold start, cancelled the very run that had just woken a dead process at 18:00, and
     * the next occurrence of 18:00 is tomorrow (2026-09-26 audit, 11 / W09). So the queued run is
     * kept when it is running or still lands on the chosen hour, and replaced only when it has
     * drifted off it: see [TrainingReminderWorker.rearm].
     */
    suspend fun ensureScheduled(enabled: Boolean, hour: Int) {
        if (enabled) TrainingReminderWorker.rearm(context, hour)
        else TrainingReminderWorker.cancel(context)
    }

    /** Re-anchor from the stored setting — for a timezone or clock change, where the hour the user
     *  chose hasn't moved but the elapsed-time schedule underneath it now points somewhere else.
     *  The same drift check as boot: a zone change moves the queued run off the chosen hour, so it
     *  is replaced, while a midnight `DATE_CHANGED` no longer cancels a reminder set for 00:00. */
    suspend fun reanchor() {
        val enabled = settingsRepo.trainingReminderEnabled.first()
        ensureScheduled(enabled, settingsRepo.trainingReminderHour.first())
    }
}
