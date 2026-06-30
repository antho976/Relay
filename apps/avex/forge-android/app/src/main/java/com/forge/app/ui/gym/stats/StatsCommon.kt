package com.forge.app.ui.gym.stats

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.gym.stats.components.statsEntrance

/** Theme colors bundled so the per-tier section builders don't each take a fistful of color params. */
data class StatsColors(
    val onBg: Color,
    val muted: Color,
    val accent: Color,
    val outline: Color,
    /** The quiet card fill — every Stats section sits on one of these so the screen reads as grouped
     *  surfaces, not a flat wall of rows. */
    val surface: Color
)

// ── One spacing/size scale so every tier lines up and nothing reads as "too big / too small" ──
/** The screen's horizontal gutter — one place so every card lines up. */
internal val STATS_GUTTER = 16.dp
/** Gap between stacked cards. */
internal val STATS_CARD_GAP = 6.dp
/** Inner padding of a card. */
internal val STATS_CARD_PAD = 16.dp
internal val STATS_CARD_RADIUS = 20.dp
/** The standard full-width chart height — used by every line/scatter/gauge so they're visually peers. */
internal val STATS_CHART_H = 132.dp
/** A taller chart for the headline/hero visual. */
internal val STATS_HERO_CHART_H = 116.dp

/**
 * A quiet card shell — the chosen "grouped surfaces" Stats style. Each section's content is rendered
 * inside one of these with an optional small-caps [title] eyebrow (TalkBack heading) and [caption].
 * Plays the one-shot staggered [statsEntrance] keyed by [index] so cards settle in on first open.
 */
@Composable
internal fun StatsCard(
    c: StatsColors,
    modifier: Modifier = Modifier,
    title: String? = null,
    caption: String? = null,
    index: Int = 0,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier
            .fillMaxWidth()
            .padding(horizontal = STATS_GUTTER, vertical = STATS_CARD_GAP)
            .statsEntrance(index)
            .clip(RoundedCornerShape(STATS_CARD_RADIUS))
            .background(c.surface)
            .padding(STATS_CARD_PAD)
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
        if (title != null || caption != null) Spacer(Modifier.height(12.dp))
        content()
    }
}
