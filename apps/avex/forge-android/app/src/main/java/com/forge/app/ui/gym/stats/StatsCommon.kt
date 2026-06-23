package com.forge.app.ui.gym.stats

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Theme colors bundled so the per-tier section builders don't each take a fistful of color params. */
data class StatsColors(
    val onBg: Color,
    val muted: Color,
    val accent: Color,
    val outline: Color
)

/** The screen's horizontal gutter — one place so every tier lines up. */
internal val STATS_GUTTER = 24.dp

/**
 * A small-caps section eyebrow — the app's quiet section-label convention (cf. "THIS WEEK" / "COACH"
 * on the home screen), not a big headline. Marked as a TalkBack heading so a user can still jump
 * between sections on this data-dense screen instead of swiping through every row.
 */
internal fun LazyListScope.statsSection(key: String, title: String, c: StatsColors) {
    item(key) {
        Text(
            title.uppercase(),
            style = MaterialTheme.typography.labelMedium,
            color = c.muted,
            letterSpacing = 1.sp,
            modifier = Modifier
                .padding(horizontal = STATS_GUTTER, vertical = 4.dp)
                .semantics { heading() }
        )
    }
}

/** A muted one-line caption under a section header. */
internal fun LazyListScope.statsCaption(key: String, text: String, c: StatsColors) {
    item(key) {
        Text(
            text,
            style = MaterialTheme.typography.bodySmall,
            color = c.muted,
            modifier = Modifier.padding(horizontal = STATS_GUTTER).padding(bottom = 8.dp)
        )
    }
}
