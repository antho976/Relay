package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp

/**
 * The two capsule button levels of DESIGN §8, promoted on their third re-implementation
 * (onboarding / settings / sheets). ① [ForgePrimaryCapsule] = the do-it-now action, filled
 * light, ≤1 per section and grouped at the END of a form page; ② [ForgeOutlineCapsule] = its
 * sidekick. Both press with the shared bounce, no ripple; theme-aware (onBackground fill /
 * background text) so they survive a monochrome accent. Standard trim height ≈44dp
 * (titleSmall + 13dp pads); pass a `fillMaxWidth()` modifier for a full-width CTA.
 */
@Composable
fun ForgePrimaryCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .background(onBg.copy(alpha = if (enabled) 1f else 0.35f))
            .bounceClick(enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.background)
    }
}

/** §8 level ② — the outlined sidekick capsule. Disabled = dimmed and inert (§4.5). */
@Composable
fun ForgeOutlineCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) {
    val alpha = if (enabled) 1f else 0.35f
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .border(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.35f * alpha), RoundedCornerShape(50))
            .bounceClick(enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onBackground.copy(alpha = alpha)
        )
    }
}

/**
 * A tappable text-glyph action — the §8 idiom of a plain glyph ("×" to remove, etc.) over a stock
 * M3 [androidx.compose.material3.IconButton]. Centralises what a bare Text + padding kept getting
 * wrong: it guarantees the ≥48dp interactive target (via [minimumInteractiveComponentSize], which a
 * padded glyph fell short of) and the TalkBack label, so every remove-glyph is reachable and
 * announced the same way. Disabled = dimmed to the shared §4.5 inert level and non-clickable.
 */
@Composable
fun GlyphButton(
    glyph: String,
    label: String,
    tint: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    style: TextStyle = MaterialTheme.typography.titleMedium,
) {
    Box(
        modifier = modifier
            .minimumInteractiveComponentSize()
            .then(if (enabled) Modifier.clickableLabeled(label, onClick = onClick) else Modifier),
        contentAlignment = Alignment.Center
    ) {
        Text(glyph, style = style, color = if (enabled) tint else tint.copy(alpha = 0.35f))
    }
}
