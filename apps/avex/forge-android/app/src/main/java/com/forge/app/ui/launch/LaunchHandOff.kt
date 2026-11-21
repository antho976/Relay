package com.forge.app.ui.launch

import android.os.Build
import android.view.View
import android.view.animation.DecelerateInterpolator
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.core.splashscreen.SplashScreenViewProvider
import com.forge.app.appicon.AppIcon
import com.forge.app.ui.theme.ForgeMotion
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope

/**
 * The launch, made low-key on 2026-10-21 (Relay "Launch" designs): the splash is your app icon on the
 * app's own dark, nothing else, and it stays only until Home's first frame is ready. Then the icon
 * fades and settles a touch smaller as Home fades in. "Remove animations" makes it a cut.
 *
 * Where the icon comes from, by platform:
 *
 * - **Android 12+ (API 31).** The system draws the splash before the process starts, from the
 *   launching activity's theme. `Theme.Forge.Splash` (values-v31) names no splash icon, so the
 *   platform draws the launching component's own icon — and a launcher tap starts the enabled
 *   `.icon.*` activity-alias, whose icon IS the pick (an alias's ActivityInfo carries the alias's
 *   icon, not the target's). An activity-alias cannot carry a theme of its own, so naming the icon in
 *   the theme would have pinned every pick to one drawable. [playSplashExit] then animates the
 *   system's own splash views out.
 * - **Android 8–11 (API 26–30).** core-splashscreen's pre-12 splash is the theme's static
 *   `windowBackground`, drawn by the system before any app code runs, so it can only be one drawable
 *   for everyone: it is the dark ground alone (`splash_blank`). The icon is drawn by [LaunchCover]
 *   from the first frame the app draws, and the same fade-and-settle plays when Home is ready.
 */

/** How much smaller the icon settles as it fades (the design's 128 → 112). */
internal const val SETTLE_SCALE = 0.875f

/**
 * The icon a launch started from: the `.icon.<Name>` alias the launcher tapped, or null when the app
 * was opened another way (a widget, a notification), which the caller resolves from the enabled
 * alias instead. Pure, so it is pinned by `LaunchHandOffTest`.
 */
internal fun launchIconFromComponent(className: String?): AppIcon? {
    val suffix = className?.substringAfter(".icon.", missingDelimiterValue = "").orEmpty()
    return if (suffix.isEmpty()) null else AppIcon.fromKey(suffix)
}

/** True where the platform splash draws the icon itself (see the file comment). */
internal val systemSplashDrawsIcon: Boolean get() = Build.VERSION.SDK_INT >= 31

/**
 * Android 12+: fade the system splash's icon while it settles to [SETTLE_SCALE], and fade the splash
 * itself so Home shows through, over ForgeMotion's standard duration. With animations removed the
 * splash is simply taken away.
 */
fun playSplashExit(provider: SplashScreenViewProvider, onDone: () -> Unit = {}) {
    val duration = ForgeMotion.nominalDuration(ForgeMotion.DurationStandard).toLong()
    val splash: View = provider.view
    if (duration <= 0L) {
        provider.remove()
        onDone()
        return
    }
    val icon: View? = runCatching { provider.iconView }.getOrNull()
    // The icon is a child of the splash, so fading the splash fades it too; the icon only settles.
    icon?.animate()
        ?.scaleX(SETTLE_SCALE)?.scaleY(SETTLE_SCALE)
        ?.setDuration(duration)?.setInterpolator(DecelerateInterpolator())
        ?.start()
    splash.animate()
        .alpha(0f)
        .setDuration(duration)
        .setInterpolator(DecelerateInterpolator())
        .withEndAction {
            provider.remove()
            onDone()
        }
        .start()
}

/**
 * Android 8–11: the icon on the app's dark, drawn by the app from its first frame, until [ready]
 * (Home's first frame is drawn beneath it). Then the icon fades and settles to [SETTLE_SCALE] while
 * the ground fades to show Home, and the cover leaves composition ([onGone]).
 */
@Composable
fun LaunchCover(icon: AppIcon, ready: Boolean, onGone: () -> Unit = {}) {
    val fade = remember { Animatable(1f) }
    val scale = remember { Animatable(1f) }
    var gone by remember { mutableStateOf(false) }
    LaunchedEffect(ready) {
        if (!ready) return@LaunchedEffect
        if (ForgeMotion.animationsOff) {
            fade.snapTo(0f)
        } else coroutineScope {
            val a = async { fade.animateTo(0f, ForgeMotion.exitTween()) }
            val b = async { scale.animateTo(SETTLE_SCALE, ForgeMotion.exitTween()) }
            a.await(); b.await()
        }
        gone = true
        onGone()
    }
    if (gone) return
    Box(
        Modifier
            .fillMaxSize()
            .clearAndSetSemantics {}
            .graphicsLayer { alpha = fade.value }
            .background(MaterialTheme.colorScheme.background),
        contentAlignment = Alignment.Center
    ) {
        Image(
            painter = painterResource(icon.previewRes),
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = Modifier
                .size(128.dp)
                .graphicsLayer { scaleX = scale.value; scaleY = scale.value }
                .clip(CircleShape)
        )
    }
}
