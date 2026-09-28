package com.forge.app.baselineprofile

import androidx.benchmark.macro.MacrobenchmarkScope
import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.Direction
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Generates the app's baseline profile. Run on a device/emulator (P2):
 *
 *   ./gradlew :baselineprofile:generateBaselineProfile
 *
 * then replace `app/src/main/baseline-prof.txt` (a wildcard stand-in until then) with the output.
 *
 * The profile precompiles what this journey touches, so it has to walk what users actually do, not
 * just launch: cold start alone captured the first frame and nothing a first visit to any other
 * screen runs. It swipes every hub page both ways (Home, Coach, Academy, Stats, Cardio), which is
 * where most first-frame jank lived. Run it on a device that has finished onboarding with some
 * history logged — on a fresh install the launch lands on onboarding, and the profile describes that
 * instead of Home.
 */
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun generate() = rule.collect(packageName = APP_PACKAGE) {
        pressHome()
        startActivityAndWait()
        // Let the Overview settle (and the launch intro play out) so first-frame paths are captured.
        device.waitForIdle()
        swipeHubs()
    }

    /** Walk the hub pager to its far edge and back, pausing on each page so it composes and loads. */
    private fun MacrobenchmarkScope.swipeHubs() {
        repeat(HUB_PAGES) { swipe(Direction.LEFT) }
        repeat(HUB_PAGES * 2) { swipe(Direction.RIGHT) }
        repeat(HUB_PAGES) { swipe(Direction.LEFT) }
    }

    private fun MacrobenchmarkScope.swipe(direction: Direction) {
        val w = device.displayWidth
        val h = device.displayHeight
        val y = h / 2
        // A horizontal fling across the middle of the screen; the pager sits under it on every hub.
        if (direction == Direction.LEFT) device.swipe(w * 4 / 5, y, w / 5, y, SWIPE_STEPS)
        else device.swipe(w / 5, y, w * 4 / 5, y, SWIPE_STEPS)
        device.waitForIdle()
    }

    private companion object {
        /** Hub pages either side of Home, at most (five hubs, Home in the middle). */
        const val HUB_PAGES = 2
        const val SWIPE_STEPS = 20
    }
}
