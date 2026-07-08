package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.stats.components.statsEntrance

/** Theme colors bundled so the per-tier section builders don't each take a fistful of color params. */
data class StatsColors(
    val onBg: Color,
    val muted: Color,
    val accent: Color,
    val outline: Color
)

// ── One spacing/size scale so every tier lines up and nothing reads as "too big / too small" ──
/** The screen's horizontal gutter — one place so every section lines up. */
internal val STATS_GUTTER = 16.dp
/** The standard full-width chart height — used by every line/scatter/gauge so they're visually peers. */
internal val STATS_CHART_H = 132.dp
/** A taller chart for the headline/hero visual. */
internal val STATS_HERO_CHART_H = 116.dp

/**
 * Open editorial section — content sits directly on the near-black page (no card shell, no
 * background fill). Air + the small-caps [title] eyebrow (kept as a TalkBack heading) anchor the
 * section (§1/§7 — the old hairline anchor was a §14 defect), then the optional [caption] and the
 * body. Plays the one-shot staggered [statsEntrance] keyed by [index] so sections settle in on
 * first open.
 */
@Composable
internal fun StatsCard(
    c: StatsColors,
    modifier: Modifier = Modifier,
    title: String? = null,
    caption: String? = null,
    index: Int = 0,
    /** Makes the whole section tappable (content should carry its own "→" affordance). */
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier
            .fillMaxWidth()
            .statsEntrance(index)
    ) {
        // §7 air rhythm: the previous section's trailing 18dp + this 10dp ≈ the 28dp section gap.
        Spacer(Modifier.height(10.dp))
        Column(
            Modifier
                .padding(horizontal = STATS_GUTTER)
                .then(if (onClick != null) Modifier.bounceClick { onClick() } else Modifier)
        ) {
            if (title != null) {
                Text(
                    title.uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = c.muted,
                    letterSpacing = 1.sp,
                    modifier = Modifier.semantics { heading() }
                )
            }
            if (caption != null) {
                if (title != null) Spacer(Modifier.height(2.dp))
                Text(caption, style = MaterialTheme.typography.bodySmall, color = c.muted)
            }
            if (title != null || caption != null) Spacer(Modifier.height(14.dp))
            content()
            Spacer(Modifier.height(18.dp))
        }
    }
}
