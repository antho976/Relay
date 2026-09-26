package com.forge.app.ui.overview.components

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density

/**
 * The one serif line on Home, clamped at 1.3× font scale.
 *
 * Clamping a HERO is allowed where clamping content is not: a 52sp display line at 200% would be
 * 104sp and would push the CTA off the fold, which is the one thing this page cannot afford. Off the
 * card it has the full page measure, so it wraps later than it used to.
 *
 * [style] exists so the ONE implementation serves both rungs. `OverviewScreen` sets `headlineLarge`
 * (36sp) rather than `displayLarge`; it used to reach that size by copying `headlineLarge` and
 * overriding it to `FontFamily.SansSerif` with a hand-set `letterSpacing` at the call site — which
 * dropped the page's biggest element out of the serif voice (§6) AND out of the scale clamp, so at
 * 200% the headline pushed the CTA below the fold. Pass a style; never re-declare the family.
 */
@Composable
internal fun HeroHeadline(
    text: String,
    onBg: Color,
    style: TextStyle = MaterialTheme.typography.displayLarge
) {
    val density = LocalDensity.current
    CompositionLocalProvider(
        LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.3f))
    ) {
        Text(
            text,
            style = style,
            color = onBg,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
    }
}
