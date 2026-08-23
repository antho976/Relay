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
 * compositions are authored so frame 0 and the last frame are the SAME finished plan (a built week ·
 * a hand-built day · a caught freestyle log), so the loop restart is seamless (no jump) AND the freeze
 * lands on that plan instead of endlessly rebuilding. A shared [PlanModeSync] gates the videos so they
 * all start (and therefore loop and freeze) on the same frame.
 *
 * Frozen is not final: tapping a card bumps its `replays` counter and the video plays its two loops
 * again from the top. Only the tapped card replays — see [StepPlanMode].
 */

/** 1128×288 — the Remotion canvas (282×72dp at 4x). One aspect for video AND Canvas fallback. */
private const val VIGNETTE_ASPECT = 1128f / 288f

/**
 * How many times the vignette plays before it holds its last frame (the finished plan). [repeatCount]
 * on [AnimatedImageDrawable] counts repeats AFTER the first play (0 = play once), so we pass
 * [PLAN_LOOPS] - 1.
 */
internal const val PLAN_LOOPS = 2

@RawRes
private fun rawFor(mode: String): Int? = when (mode) {
    PLAN_GENERATED -> R.raw.planmode_generated
    PLAN_CUSTOM -> R.raw.planmode_custom
    PLAN_FREESTYLE -> R.raw.planmode_freestyle
    else -> null
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

/**
 * [replays] is a tap counter: every increment plays the video again from frame 0. It is ignored under
 * reduce-motion, where there is no video to replay and the Canvas twin is already a settled still.
 */
@Composable
internal fun PlanModeMedia(mode: String, sync: PlanModeSync, replays: Int = 0) {
    val raw = rawFor(mode)
    val canPlay = raw != null && Build.VERSION.SDK_INT >= 28 && ForgeMotion.durationScale > 0f
    Box(Modifier.fillMaxWidth().aspectRatio(VIGNETTE_ASPECT)) {
        if (canPlay && raw != null) {
            AnimatedWebp(raw, sync, replays, fallback = { PlanModeVignette(mode, Modifier.fillMaxSize(), replays) })
        } else {
            PlanModeVignette(mode, Modifier.fillMaxSize(), replays)
        }
    }
}

@RequiresApi(28)
@Composable
private fun AnimatedWebp(
    @RawRes resId: Int,
    sync: PlanModeSync,
    replays: Int,
    fallback: @Composable () -> Unit
) {
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
    // Replay on tap. Once the drawable has run out its repeats it is stopped, and start() on a stopped
    // AnimatedImageDrawable rewinds to frame 0 — so this restarts a frozen card and re-starts one that
    // is still mid-play. Skipped at 0 so the first composition doesn't fight the initial start() below.
    LaunchedEffect(drawable, replays) {
        if (replays > 0) (drawable as? AnimatedImageDrawable)?.run { stop(); start() }
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
