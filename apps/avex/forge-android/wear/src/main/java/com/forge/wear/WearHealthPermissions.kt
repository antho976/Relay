package com.forge.wear

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build

/**
 * The runtime permissions the live-HR exercise needs, resolved for the API level it is running on.
 *
 * Health Services enforces its data types through the platform's permission model, and that model
 * changed underneath this module: `BODY_SENSORS` is what grants `HEART_RATE_BPM` up to API 35, and
 * from API 36 it is replaced by the granular health permission below. The module targets 36 and
 * declared only the legacy one, so on a current watch the exercise started, reported no heart rate,
 * and reported no error either — Health Services simply delivers an empty metric.
 *
 * Everything here is a plain string comparison against [Build.VERSION.SDK_INT], with no framework
 * calls, so the version split is unit-testable rather than something only an API 36 emulator can
 * answer.
 */
object WearHealthPermissions {

    /**
     * API 36's replacement for `BODY_SENSORS` on heart rate. A string literal rather than a
     * constant: it belongs to the health permission group, which has no framework constant on the
     * classpath of a module that does not depend on the Health Connect client.
     */
    const val READ_HEART_RATE = "android.permission.health.READ_HEART_RATE"

    /** The first API level on which [READ_HEART_RATE] is the permission that grants heart rate. */
    const val HEALTH_PERMISSIONS_SDK = 36

    /** Whichever permission grants `HEART_RATE_BPM` on [sdkInt]. */
    fun heartRatePermission(sdkInt: Int = Build.VERSION.SDK_INT): String =
        if (sdkInt >= HEALTH_PERMISSIONS_SDK) READ_HEART_RATE else Manifest.permission.BODY_SENSORS

    /**
     * Everything the exercise wants, most important first.
     *
     * Calories are second because they are genuinely optional: `CALORIES_TOTAL` needs
     * `ACTIVITY_RECOGNITION`, and a session without it still streams heart rate, which is the part
     * the wrist renders.
     */
    fun exercisePermissions(sdkInt: Int = Build.VERSION.SDK_INT): List<String> =
        listOf(heartRatePermission(sdkInt), Manifest.permission.ACTIVITY_RECOGNITION)

    /** Those of [exercisePermissions] not yet granted — what an ask should actually contain. */
    fun missing(context: Context, sdkInt: Int = Build.VERSION.SDK_INT): List<String> =
        exercisePermissions(sdkInt).filter {
            context.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED
        }

    /**
     * Whether the health foreground service may start at all.
     *
     * Heart rate alone is the bar: an exercise that streams HR without calories is the documented
     * degraded state, and the phone falls back to its own estimate. From API 34 a `health`-typed
     * foreground service also requires the app to hold one of the health permissions before
     * `startForeground`, so this is not only a question of what the exercise will report — calling
     * it wrong throws.
     */
    fun canStreamHeartRate(context: Context, sdkInt: Int = Build.VERSION.SDK_INT): Boolean =
        context.checkSelfPermission(heartRatePermission(sdkInt)) == PackageManager.PERMISSION_GRANTED
}
