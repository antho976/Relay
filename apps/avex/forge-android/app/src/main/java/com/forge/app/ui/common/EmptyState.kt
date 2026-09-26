package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp

/**
 * A lightweight one-line empty hint for sub-sections that already sit inside a populated screen
 * (the Overview "recent" block, the trophy case). It keeps the italic/muted styling in one place so
 * the inline hints can't drift apart. [color] is passed in because these call sites already derive
 * their own muted tone from the screen theme.
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
 * Overview welcome, the Profile header, the first session). Distinct from [InlineEmptyHint]
 * (one-liner) — this is the multi-line onboarding nudge. Kept
 * here so the first-touch sites share one treatment instead of each hand-rolling the same card.
 *
 * DEPRECATED by DESIGN §12: empty is data at zero, drawn not written. A boxed card around passive
 * content breaks §1, and a first-run nudge is still a zero-state, so it should be drawn.
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
