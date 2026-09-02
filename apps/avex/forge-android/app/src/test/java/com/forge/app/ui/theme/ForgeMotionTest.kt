package com.forge.app.ui.theme

import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.SnapSpec
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.TweenSpec
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * ForgeMotion and the system animator duration scale (audit H-14). Compose applies that scale
 * itself, once, through MotionDurationScale — so every spec ForgeMotion builds for Compose must
 * carry its NOMINAL duration whatever the scale is (multiplying here too squared the setting:
 * 2x ran 4x, 10x ran 100x). Only [ForgeMotion.scaledDuration], for raw delays and other motion
 * Compose does not clock, follows the scale; and a 0 scale still collapses everything.
 */
class ForgeMotionTest {

    private var saved = 1f

    @Before
    fun rememberScale() {
        saved = ForgeMotion.durationScale
    }

    @After
    fun restoreScale() {
        ForgeMotion.durationScale = saved
    }

    private fun millis(spec: FiniteAnimationSpec<Float>): Int = (spec as TweenSpec<Float>).durationMillis

    @Test
    fun composeTweensKeepTheirNominalDurationAtEveryAnimatorScale() {
        for (scale in listOf(0.5f, 1f, 1.5f, 2f, 5f, 10f)) {
            ForgeMotion.durationScale = scale
            val at = "at ${scale}x"
            assertFalse(at, ForgeMotion.animationsOff)
            assertEquals(at, ForgeMotion.DurationEmphasized, millis(ForgeMotion.enterTween(ForgeMotion.DurationEmphasized)))
            assertEquals(at, ForgeMotion.DurationStandard, millis(ForgeMotion.exitTween()))
            assertEquals(at, ForgeMotion.DurationFast, millis(ForgeMotion.standardTween(ForgeMotion.DurationFast)))
            assertEquals(at, ForgeMotion.DurationDraw, millis(ForgeMotion.drawTween()))
            assertEquals(at, 950, ForgeMotion.nominalDuration(950))
            assertTrue(at, ForgeMotion.bouncy<Float>() is SpringSpec<*>)
            assertTrue(at, ForgeMotion.snappy<Float>() is SpringSpec<*>)
        }
    }

    @Test
    fun tweenEasingsAreUntouchedByTheScale() {
        ForgeMotion.durationScale = 2f
        assertEquals(ForgeMotion.Decelerate, (ForgeMotion.enterTween<Float>() as TweenSpec<Float>).easing)
        assertEquals(ForgeMotion.Accelerate, (ForgeMotion.exitTween<Float>() as TweenSpec<Float>).easing)
        assertEquals(ForgeMotion.Standard, (ForgeMotion.standardTween<Float>() as TweenSpec<Float>).easing)
        assertEquals(ForgeMotion.DrawDecelerate, (ForgeMotion.drawTween<Float>() as TweenSpec<Float>).easing)
    }

    @Test
    fun animationsOffStillCollapsesEverySpecAndDelay() {
        ForgeMotion.durationScale = 0f
        assertTrue(ForgeMotion.animationsOff)
        assertEquals(0, millis(ForgeMotion.enterTween(ForgeMotion.DurationEmphasized)))
        assertEquals(0, millis(ForgeMotion.exitTween()))
        assertEquals(0, millis(ForgeMotion.standardTween(ForgeMotion.DurationFast)))
        assertEquals(0, millis(ForgeMotion.drawTween()))
        assertEquals(0, ForgeMotion.nominalDuration(950))
        assertEquals(0, ForgeMotion.scaledDuration(700))
        assertTrue(ForgeMotion.bouncy<Float>() is SnapSpec<*>)
        assertTrue(ForgeMotion.snappy<Float>() is SnapSpec<*>)
    }

    @Test
    fun rawDelaysFollowTheAnimatorScaleOnce() {
        ForgeMotion.durationScale = 0.5f
        assertEquals(350, ForgeMotion.scaledDuration(700))
        ForgeMotion.durationScale = 1f
        assertEquals(700, ForgeMotion.scaledDuration(700))
        ForgeMotion.durationScale = 2f
        assertEquals(1400, ForgeMotion.scaledDuration(700))
        ForgeMotion.durationScale = 10f
        assertEquals(450, ForgeMotion.scaledDuration(45))
    }
}
