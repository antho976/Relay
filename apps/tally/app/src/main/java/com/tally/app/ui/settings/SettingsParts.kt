package com.tally.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.GUTTER
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.GroupRow
import com.tally.app.ui.common.PageTitle
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.TopBar

/*
 * Pieces the Settings pages and onboarding share: the page head, the hero's label row and figure,
 * the accent-lit row badge, and the currency row.
 */

/** The hero figure stops growing here, so a 200% font keeps a long amount on one line. */
private const val FIGURE_MAX_SCALE = 1.3f

/** Titles stop growing here, as Android 14's own non-linear font scale does, so "Appearance" never breaks mid-word. */
private const val TITLE_MAX_SCALE = 1.5f

/**
 * The back capsule, then the serif title with its context line under it. The title's scale is
 * capped; the context line under it scales in full.
 */
@Composable
internal fun PageHead(title: String, onBack: () -> Unit, context: String? = null) {
    val density = LocalDensity.current
    Column {
        TopBar(onBack = onBack)
        Column(
            Modifier.padding(horizontal = GUTTER).padding(top = 4.dp, bottom = 6.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            CompositionLocalProvider(LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(TITLE_MAX_SCALE))) {
                PageTitle(title)
            }
            if (context != null) {
                Text(context, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/**
 * Two stat tiles side by side at matching heights; past 1.3x font they stack, so a label like
 * "NEWEST ENTRY" never has to break in a half-width tile.
 */
@Composable
internal fun TilePair(
    modifier: Modifier = Modifier,
    first: @Composable (Modifier) -> Unit,
    second: @Composable (Modifier) -> Unit,
) {
    if (LocalDensity.current.fontScale > 1.3f) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.fillMaxWidth())
            second(Modifier.fillMaxWidth())
        }
    } else {
        Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(FIGURE_GAP)) {
            first(Modifier.weight(1f).fillMaxHeight())
            second(Modifier.weight(1f).fillMaxHeight())
        }
    }
}

/**
 * The lit panel's opening row: glyph tile, the mono label, an optional reading at the end. It is
 * the kit's [PanelHeader], so a long reading drops under the label instead of breaking it.
 */
@Composable
internal fun HeroHead(
    label: String,
    modifier: Modifier = Modifier,
    end: String? = null,
    tint: Color = MaterialTheme.colorScheme.primary,
) {
    PanelHeader(label, modifier, meta = end, tint = tint)
}

/**
 * THE serif figure of a page. Its font scale is capped and it steps down a size as the text grows,
 * so "R$1,234.56" at 200% stays on one line instead of breaking mid-number.
 */
@Composable
internal fun HeroNumber(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onBackground,
    description: String? = null,
) {
    val density = LocalDensity.current
    val scale = density.fontScale.coerceAtMost(FIGURE_MAX_SCALE)
    CompositionLocalProvider(LocalDensity provides Density(density.density, scale)) {
        Text(
            text,
            style = figureStyle(text.length, scale),
            color = color,
            modifier = modifier.semantics { if (description != null) contentDescription = description },
        )
    }
}

@Composable
private fun figureStyle(length: Int, scale: Float): TextStyle {
    val type = MaterialTheme.typography
    val width = length * scale
    return when {
        width <= 9.5f -> type.displayLarge
        width <= 12f -> type.displayMedium
        width <= 15f -> type.headlineLarge
        else -> type.headlineMedium
    }
}

/** A settings row's glyph on an accent wash, so the page reads as one lit family of rows. */
@Composable
internal fun RowBadge(icon: ImageVector, tint: Color = MaterialTheme.colorScheme.primary) {
    GlyphBadge(icon, tint = tint, fill = tint.copy(alpha = 0.14f))
}

/** The currency's code on its own tile, lit when it is the pick. Grows with the font, never clips. */
@Composable
internal fun CurrencyMark(code: String, selected: Boolean) {
    val tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onBackground
    val fill = if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.14f) else MaterialTheme.colorScheme.surfaceContainerHighest
    Box(
        Modifier
            .sizeIn(minWidth = 44.dp, minHeight = 44.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(fill)
            .padding(horizontal = 6.dp, vertical = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(code, style = MaterialTheme.typography.labelLarge, color = tint)
    }
}

/** One currency as a slab row: code tile, its name, symbol and a sample, a check when picked. */
@Composable
internal fun CurrencyRow(option: CurrencyOption, selected: Boolean, shape: Shape, onClick: () -> Unit) {
    GroupRow(
        option.name,
        shape,
        modifier = Modifier.semantics { this.selected = selected },
        subtitle = option.subtitle,
        leading = { CurrencyMark(option.code, selected) },
        trailing = {
            if (selected) {
                Icon(Icons.Rounded.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            }
        },
        chevron = false,
        onClick = onClick,
    )
}
