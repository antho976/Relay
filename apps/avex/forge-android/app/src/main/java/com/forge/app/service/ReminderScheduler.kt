package com.forge.app.service

import android.content.Context
import androidx.work.ExistingPeriodicWorkPolicy
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Thin seam between the UI / app boot and [TrainingReminderWorker]'s WorkManager scheduling, so a
 * ViewModel never has to hold WorkManager logic and ForgeApp/Settings share one entry point.
 */
@Singleton
class ReminderScheduler @Inject constructor(
    @ApplicationContext private val context: Context
) {
    /** React to a user change: re-arm with a fresh fire time (REPLACE) or cancel when turned off. */
    fun apply(enabled: Boolean, hour: Int) {
        if (enabled) TrainingReminderWorker.schedule(context, hour, ExistingPeriodicWorkPolicy.REPLACE)
        else TrainingReminderWorker.cancel(context)
    }

    /** Boot-time: keep an existing schedule (KEEP), arm it if enabled, or ensure it's cancelled. */
    fun ensureScheduled(enabled: Boolean, hour: Int) {
        if (enabled) TrainingReminderWorker.schedule(context, hour, ExistingPeriodicWorkPolicy.KEEP)
        else TrainingReminderWorker.cancel(context)
    }
}
