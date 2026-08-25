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
     * Boot-time: re-anchor an enabled reminder to the next occurrence of its wall-clock hour, or
     * ensure it's cancelled.
     *
     * This used to KEEP the existing schedule, which sounds conservative and isn't: the work is a
     * 24-hour PERIODIC, and 24 hours is ELAPSED time, not wall-clock time. Only the initial delay
     * was ever zone-aware. So an 18:00 reminder became 17:00 at the spring-forward and stayed there
     * for the seven months until the clocks went back — a nudge set for after work arriving while
     * the user was still at their desk, for half the year, with nothing but toggling the setting off
     * and on to fix it.
     *
     * REPLACE is safe to run at every launch because [TrainingReminderWorker.schedule] always
     * targets the NEXT occurrence of the chosen hour: re-arming can move the fire time to the right
     * moment, never past it.
     */
    fun ensureScheduled(enabled: Boolean, hour: Int) {
        if (enabled) TrainingReminderWorker.schedule(context, hour, ExistingPeriodicWorkPolicy.REPLACE)
        else TrainingReminderWorker.cancel(context)
    }

    /** Re-anchor from the stored setting — for a timezone or clock change, where the hour the user
     *  chose hasn't moved but the elapsed-time schedule underneath it now points somewhere else. */
    suspend fun reanchor() {
        val enabled = settingsRepo.trainingReminderEnabled.first()
        ensureScheduled(enabled, settingsRepo.trainingReminderHour.first())
    }
}
