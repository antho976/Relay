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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
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
 */

/** 1128×288 — the Remotion canvas (282×72dp at 4x). One aspect for video AND Canvas fallback. */
private const val VIGNETTE_ASPECT = 1128f / 288f

@RawRes
private fun rawFor(mode: String): Int? = when (mode) {
    PLAN_GENERATED -> R.raw.planmode_generated
    else -> null // custom/freestyle still run their Canvas vignettes; videos land one at a time
}

@Composable
internal fun PlanModeMedia(mode: String) {
    val raw = rawFor(mode)
    val canPlay = raw != null && Build.VERSION.SDK_INT >= 28 && ForgeMotion.durationScale > 0f
    Box(Modifier.fillMaxWidth().aspectRatio(VIGNETTE_ASPECT)) {
        if (canPlay && raw != null) {
            AnimatedWebp(raw, fallback = { PlanModeVignette(mode, Modifier.fillMaxSize()) })
        } else {
            PlanModeVignette(mode, Modifier.fillMaxSize())
        }
    }
}

@RequiresApi(28)
@Composable
private fun AnimatedWebp(@RawRes resId: Int, fallback: @Composable () -> Unit) {
    val context = LocalContext.current
    // Decode off the main thread; the Canvas vignette holds the slot so the card never blanks.
    val drawable by produceState<Drawable?>(null, resId) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                ImageDecoder.decodeDrawable(ImageDecoder.createSource(context.resources, resId))
            }.getOrNull()
        }
    }
    val d = drawable
    if (d == null) {
        fallback()
        return
    }
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { ctx -> ImageView(ctx).apply { scaleType = ImageView.ScaleType.FIT_XY } },
        update = { view ->
            if (view.drawable !== d) {
                view.setImageDrawable(d)
                (d as? AnimatedImageDrawable)?.apply {
                    repeatCount = AnimatedImageDrawable.REPEAT_INFINITE
                    start()
                }
            }
        }
    )
}
