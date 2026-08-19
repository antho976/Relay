package com.forge.app.ui.overview.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.forge.app.ui.experiment.SurfaceCta

/**
 * Home's hero: what to do now.
 *
 * ## It is not a card any more (2026-08-16)
 *
 * > "The hero is nice, while I do think it would be better with no background box." — Antho
 *
 * The eyebrow, the headline, the whisper and the CTA sit directly on the page, over the accent bloom
 * `HeroGlow` paints behind them. Removing the fill also removed the on-card contrast penalty: every
 * caption in here can drop back to the page's own muted rung instead of being forced a step brighter
 * to clear AA on a lifted surface.
 *
 * ## The shape is identical in all three modes
 *
 * Planned day, freestyle and no-plan differ only in their words and their CTA target — never in
 * their structure. The page must not restructure itself between users, and the whisper line always
 * has something to say because "what you last trained" exists in every mode.
 *
 * The CTA is deliberately the loudest thing on Home and the largest piece of accent in the app. It
 * was already the best element on the old page; it is the same control, warmed.
 */
@Composable
fun HomeHero(
    eyebrow: String,
    dateLabel: String,
    headline: String,
    /** The lifts today holds, or what you last trained. Null renders nothing rather than filler. */
    whisper: String?,
    ctaText: String,
    ctaLabel: String,
    ctaHint: String?,
    onBg: Color,
    muted: Color,
    modifier: Modifier = Modifier,
    /** False on a rest day: the action exists, but it is not what the coach is telling you to do. */
    ctaFilled: Boolean = true,
    onLongCta: (() -> Unit)? = null,
    longCtaLabel: String? = null,
    onCta: () -> Unit
) {
    Column(modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                eyebrow.uppercase(),
                style = MaterialTheme.typography.labelMedium,
                color = muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false)
            )
            Spacer(Modifier.width(8.dp))
            Text(
                dateLabel,
                style = MaterialTheme.typography.labelSmall,
                color = muted.copy(alpha = 0.65f),
                maxLines = 1
            )
        }

        Spacer(Modifier.height(14.dp))
        HeroHeadline(headline, onBg)

        if (!whisper.isNullOrBlank()) {
            Spacer(Modifier.height(12.dp))
            Text(whisper, style = MaterialTheme.typography.bodyMedium, color = muted)
        }

        Spacer(Modifier.height(24.dp))
        SurfaceCta(
            text = ctaText,
            label = ctaLabel,
            filled = ctaFilled,
            onLongClick = onLongCta,
            longClickLabel = longCtaLabel,
            onClick = onCta
        )
        if (ctaHint != null) {
            Spacer(Modifier.height(10.dp))
            Text(
                ctaHint,
                style = MaterialTheme.typography.bodySmall,
                color = muted.copy(alpha = 0.65f),
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }
}

/**
 * The one serif line on Home, clamped at 1.3× font scale.
 *
 * Clamping a HERO is allowed where clamping content is not: a 52sp display line at 200% would be
 * 104sp and would push the CTA off the fold, which is the one thing this page cannot afford. Off the
 * card it has the full page measure, so it wraps later than it used to.
 */
@Composable
private fun HeroHeadline(text: String, onBg: Color) {
    val density = LocalDensity.current
    CompositionLocalProvider(
        LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.3f))
    ) {
        Text(
            text,
            style = MaterialTheme.typography.displayLarge,
            color = onBg,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
    }
}
