package com.forge.app.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.forge.app.core.time.TimeSignals
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Reacts to the device's day, clock or timezone changing.
 *
 * Nothing in the app used to. Every day and week bucket is derived from `ZoneId.systemDefault()` at
 * read time, so a user who flies Auckland → London with the process alive kept seeing the old
 * zone's week — day dots, streak and "next up" all shifted — until the app was killed, and any
 * export taken in that session stamped New Zealand calendar days while the UI showed London ones.
 *
 * Three things happen here, none of them expensive:
 *  - [TimeSignals] re-emits, so every flow anchored on "today" or "this week" rebuilds.
 *  - The widget refreshes; it otherwise updates at most hourly and never on a date change.
 *  - The daily training reminder is re-armed. It is a 24-hour PERIODIC, which is elapsed time, not
 *    wall-clock time: without this an 18:00 reminder becomes 17:00 at every spring-forward and
 *    stays there for seven months.
 *
 * `TIMEZONE_CHANGED` and `TIME_SET` are implicit broadcasts that still reach a manifest receiver;
 * `DATE_CHANGED` is registered here for the midnight roll on devices that send it.
 */
@AndroidEntryPoint
class TimeChangeReceiver : BroadcastReceiver() {

    @Inject lateinit var timeSignals: TimeSignals
    @Inject lateinit var reminderScheduler: ReminderScheduler

    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_TIMEZONE_CHANGED,
            Intent.ACTION_TIME_CHANGED,
            Intent.ACTION_DATE_CHANGED -> Unit
            else -> return
        }
        runCatching { timeSignals.onSystemTimeChanged() }
        val pending = goAsync()
        CoroutineScope(Dispatchers.Default).launch {
            try {
                com.forge.app.widget.refreshForgeWidgets(context)
                // Re-anchor the reminder to the wall-clock hour the user chose, in the CURRENT zone.
                runCatching { reminderScheduler.reanchor() }
            } finally {
                pending.finish()
            }
        }
    }
}
