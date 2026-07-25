package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * Reusable empty-state card. [title] is punchy; [subtitle] adds context.
 * Used on first launch before any data exists (#29 #66).
 *
 * DEPRECATED by DESIGN §12: empty is data at zero, DRAWN not written. A boxed card around passive
 * content also breaks §1 (surfaces are earned by interactivity). Redraw the section's own visual at
 * zero — a ghost sparkline, a hollow dot rail, an empty meter track — and fall back to
 * [InlineEmptyHint] only where there is genuinely no zero-shape.
 */
@Deprecated(
    "DESIGN §12: draw empty as the section's own mark at zero, not a boxed card. " +
        "Use InlineEmptyHint only as a last resort.",
    ReplaceWith("InlineEmptyHint(title, color)"),
    DeprecationLevel.WARNING
)
@Composable
fun EmptyState(
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = MaterialTheme.shapes.large
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = TextAlign.Center
            )
            Text(
                subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center
            )
        }
    }
}

/**
 * A lightweight one-line empty hint for sub-sections that already sit inside a populated screen
 * (the Overview "recent" block, the trophy case). The big [EmptyState] card is for full empty
 * surfaces; this shares the italic/muted styling so the inline hints can't drift apart. [color] is
 * passed in because these call sites already derive their own muted tone from the screen theme.
 */
@Composable
fun InlineEmptyHint(
    text: String,
    color: Color,
    modifier: Modifier = Modifier
) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.bodySmall,
        color = color,
        fontStyle = FontStyle.Italic
    )
}

/**
 * Left-aligned [title] + [body] card shown to a brand-new user on an otherwise-empty surface (the
 * Overview welcome, the Profile header, the first session). Distinct from [EmptyState] (centered,
 * emoji-anchored) and [InlineEmptyHint] (one-liner) — this is the multi-line onboarding nudge. Kept
 * here so the first-touch sites share one treatment instead of each hand-rolling the same card.
 *
 * DEPRECATED by DESIGN §12 for the same reason as [EmptyState]: a boxed card around passive content
 * breaks §1, and a first-run nudge is still a zero-state, so it should be drawn.
 */
@Deprecated(
    "DESIGN §12: draw the zero-state as the section's own mark. Use InlineEmptyHint as a last resort.",
    ReplaceWith("InlineEmptyHint(title, color)"),
    DeprecationLevel.WARNING
)
@Composable
fun FirstTouchTip(
    title: String,
    body: String,
    modifier: Modifier = Modifier
) {
    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
            .padding(16.dp)
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onBackground
        )
        Spacer(Modifier.height(4.dp))
        Text(
            body,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}
