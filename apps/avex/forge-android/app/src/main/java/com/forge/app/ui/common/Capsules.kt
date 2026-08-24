package com.forge.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.heightIn
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

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
    enabled: Boolean = true,
    /**
     * Fill with the ACCENT instead of the light `onBackground` (2026-08-24). For a modal's one
     * commit, where the capsule has to out-rank a list of accent-bordered selection pills the light
     * fill sat quieter than. The label rides `onPrimary`, which `ForgeTheme` flips to the background
     * tone above luminance 0.18 — so a mid-tone warm accent gets dark text and a monochrome accent
     * still reads, exactly as [ForgeHeroAction] does. A page's do-it-now action stays light; a hub
     * tab's stays [ForgeHeroAction].
     */
    accent: Boolean = false
) {
    val cs = MaterialTheme.colorScheme
    val fill = if (accent) cs.primary else cs.onBackground
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .background(fill.copy(alpha = if (enabled) 1f else 0.35f))
            .bounceClick(enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.titleSmall,
            color = if (accent) cs.onPrimary else cs.background
        )
    }
}

/** §8 level ② — the outlined sidekick capsule. Disabled = dimmed and inert (§4.5). Pass
 *  [contentColor] only for a true state (§5 — e.g. error on a destructive action); it tints the
 *  label full-strength and the border at the outline rung. */
@Composable
fun ForgeOutlineCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    contentColor: Color? = null
) {
    val alpha = if (enabled) 1f else 0.35f
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .border(
                1.dp,
                (contentColor ?: MaterialTheme.colorScheme.outline).copy(alpha = 0.35f * alpha),
                RoundedCornerShape(50)
            )
            .bounceClick(enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.titleSmall,
            color = (contentColor ?: MaterialTheme.colorScheme.onBackground).copy(alpha = alpha)
        )
    }
}

/**
 * §8's per-row action: a COMPACT outlined pill, right-aligned in a list row whose WHOLE surface is
 * the tap target — so the pill is drawn, never independently clickable (no nested tap). Promoted out
 * of `settings/SettingsPrimitives.ConnectPill` on its third screen (Coach · Recovery · the Profile's
 * BODY rows, 2026-07-24). Deliberately border-only: a filled capsule per row stacks into a button
 * wall, and a bare mono accent link reads too dim against a muted accent.
 */
@Composable
fun ForgeRowPill(label: String, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .border(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.35f), RoundedCornerShape(50))
            .padding(horizontal = 14.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onBackground,
            letterSpacing = 0.3.sp
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

/**
 * The HERO action — a page's one do-it-now button at full weight: accent-filled, 56dp minimum, bold
 * mono label on `onPrimary` (which flips to the background tone above luminance 0.18, so a mid-tone
 * warm accent still gets dark text and a monochrome accent still reads).
 *
 * Promoted out of `OverviewScreen.HomePrimaryAction` on its second screen (Home · Cardio,
 * 2026-08-23) rather than copied: Antho's note was that Cardio's white [ForgePrimaryCapsule] beside
 * Home's orange one "doesn't make sense", and two lookalikes drawn from two definitions drift apart
 * the moment one is touched. A hub tab's primary action is this; a section's is still level ①.
 *
 * [onLongClick] carries the optional hold gesture (Home's start-skipping-warmup).
 */
@Composable
fun ForgeHeroAction(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    label: String = text,
    onLongClick: (() -> Unit)? = null,
    onLongClickLabel: String? = null
) {
    Box(
        modifier = modifier
            // Height from a minimum, never a fixed one — the label has to survive 200% (§14).
            .heightIn(min = 56.dp)
            .clip(HeroActionShape)
            .background(MaterialTheme.colorScheme.primary)
            .bounceCombinedClick(
                onClickLabel = label,
                onLongClickLabel = onLongClickLabel,
                onLongClick = onLongClick,
                onClick = onClick
            )
            .padding(horizontal = 20.dp, vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onPrimary
        )
    }
}

/** The hero action's corner — the 12dp tile rung (§7), not the capsule's pill. */
private val HeroActionShape = RoundedCornerShape(12.dp)
