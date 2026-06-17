package com.forge.app.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Generates `app/src/main/baseline-prof.txt`. Run on a device/emulator (P2):
 *
 *   ./gradlew :baselineprofile:generateBaselineProfile
 *
 * The profile precompiles the cold-start + first-frame hot paths so ART doesn't JIT-compile the
 * whole Compose layout/draw stack on launch — the single biggest startup win on a mid-range phone.
 */
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun generate() = rule.collect(packageName = "com.forge.app") {
        pressHome()
        startActivityAndWait()
        // Let the Overview settle so first-frame composition/draw paths are captured.
        device.waitForIdle()
    }
}
