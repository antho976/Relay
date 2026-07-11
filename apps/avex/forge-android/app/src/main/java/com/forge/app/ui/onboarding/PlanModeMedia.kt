package com.forge.app.ui.onboarding

import android.graphics.ImageDecoder
import android.graphics.drawable.AnimatedImageDrawable
import android.graphics.drawable.Drawable
import android.os.Build
import android.widget.ImageView
import androidx.annotation.RawRes
import androidx.annotation.RequiresApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import com.forge.app.R
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The plan-mode card illustration: a pre-rendered looping video (alpha-transparent animated WebP,
 * authored in `remotion-vignettes/` and rendered into res/raw) when the platform can play it,
 * else the live Canvas vignette. The Canvas twin also covers reduce-motion (frozen final frame)
 * and the first frames while the WebP decodes off the main thread.
 *
 * The videos don't loop forever: each loops [PLAN_LOOPS] times then FREEZES on its last frame. The
 * compositions are authored so frame 0 and the last frame are the SAME finished plan (a built week /
 * a hand-built day), so the loop restart is seamless (no jump) AND the freeze lands on that plan
 * instead of endlessly rebuilding. A shared [PlanModeSync] gates the videos so both start (and
 * therefore loop and freeze) on the same frame.
 */

/** 1128×288 — the Remotion canvas (282×72dp at 4x). One aspect for video AND Canvas fallback. */
private const val VIGNETTE_ASPECT = 1128f / 288f

/**
 * How many times the vignette plays before it holds its last frame (the finished plan). [repeatCount]
 * on [AnimatedImageDrawable] counts repeats AFTER the first play (0 = play once), so we pass
 * [PLAN_LOOPS] - 1.
 */
private const val PLAN_LOOPS = 2

@RawRes
private fun rawFor(mode: String): Int? = when (mode) {
    PLAN_GENERATED -> R.raw.planmode_generated
    PLAN_CUSTOM -> R.raw.planmode_custom
    else -> null // freestyle keeps its live Canvas vignette (the "log whenever" scatter suits a loop)
}

/** True for the modes backed by a rendered video — [StepPlanMode] counts these to size [PlanModeSync]. */
internal fun planModeHasVideo(mode: String): Boolean = rawFor(mode) != null

/**
 * Keeps the plan-mode videos in lockstep. Each card reports [markReady] once its WebP has finished
 * decoding (success or failure); [started] flips true only when every video has reported, and both
 * cards mount + start their ImageView on that same recomposition — so they replay and freeze together.
 */
@Stable
internal class PlanModeSync(private val expected: Int) {
    private var readyCount by mutableIntStateOf(0)
    val started: Boolean get() = expected > 0 && readyCount >= expected
    fun markReady() { readyCount++ }
}

@Composable
internal fun PlanModeMedia(mode: String, sync: PlanModeSync) {
    val raw = rawFor(mode)
    val canPlay = raw != null && Build.VERSION.SDK_INT >= 28 && ForgeMotion.durationScale > 0f
    Box(Modifier.fillMaxWidth().aspectRatio(VIGNETTE_ASPECT)) {
        if (canPlay && raw != null) {
            AnimatedWebp(raw, sync, fallback = { PlanModeVignette(mode, Modifier.fillMaxSize()) })
        } else {
            PlanModeVignette(mode, Modifier.fillMaxSize())
        }
    }
}

@RequiresApi(28)
@Composable
private fun AnimatedWebp(@RawRes resId: Int, sync: PlanModeSync, fallback: @Composable () -> Unit) {
    val context = LocalContext.current
    // Decode off the main thread. A null drawable inside a resolved [Decoded] means decode failed —
    // distinct from "still decoding" (result == null), so a failed twin doesn't hang the other card.
    val result by produceState<Decoded?>(null, resId) {
        val d = withContext(Dispatchers.IO) {
            runCatching {
                ImageDecoder.decodeDrawable(ImageDecoder.createSource(context.resources, resId))
            }.getOrNull()
        }
        value = Decoded(d)
    }
    // Release this card's gate once its decode attempt has resolved, whatever the outcome.
    val resolved = result != null
    LaunchedEffect(resolved) { if (resolved) sync.markReady() }

    val drawable = result?.drawable
    // Hold the animating Canvas until BOTH videos are ready (so they start together), and forever if
    // this one failed to decode.
    if (!sync.started || drawable == null) {
        fallback()
        return
    }
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { ctx -> ImageView(ctx).apply { scaleType = ImageView.ScaleType.FIT_XY } },
        update = { view ->
            if (view.drawable !== drawable) {
                view.setImageDrawable(drawable)
                (drawable as? AnimatedImageDrawable)?.apply {
                    repeatCount = PLAN_LOOPS - 1 // loops PLAN_LOOPS times, then holds the last frame (the plan)
                    start()
                }
            }
        }
    )
}

/** Wraps the decode result so a failed decode (null drawable) is distinguishable from "still decoding". */
private class Decoded(val drawable: Drawable?)
