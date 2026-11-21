package com.forge.app.debug

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.forge.app.data.repo.SampleDataSeeder
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Debug-only: fills an onboarded install with eight weeks of sample training so every screen has
 * something to show on an emulator. Finish onboarding first, then:
 *
 * `adb shell am broadcast -a com.forge.app.debug.SEED -n com.quietsoftware.avex.debug/com.forge.app.debug.SeedSampleDataReceiver`
 */
@AndroidEntryPoint
class SeedSampleDataReceiver : BroadcastReceiver() {

    @Inject lateinit var seeder: SampleDataSeeder

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION_SEED) return
        val pending = goAsync()
        CoroutineScope(Dispatchers.IO).launch {
            try {
                seeder.seed()
                Log.i(TAG, "Sample data seeded")
            } catch (t: Throwable) {
                Log.e(TAG, "Seeding failed", t)
            } finally {
                pending.finish()
            }
        }
    }

    private companion object {
        const val ACTION_SEED = "com.forge.app.debug.SEED"
        const val TAG = "AvexSeed"
    }
}
