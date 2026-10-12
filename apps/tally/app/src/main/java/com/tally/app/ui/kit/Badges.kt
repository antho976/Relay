package com.tally.app.ui.kit

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.CategoryIcons
import com.tally.app.ui.theme.Tally
import com.tally.app.ui.theme.categoryColor
import com.tally.core.AccountType

/**
 * A category's badge: its icon in its own hue on a soft wash of that hue, in Relay's 9dp square.
 * The hue is data (which category), so it is spent here and in charts, never on text.
 */
@Composable
fun CategoryBadge(icon: String?, color: Int?, modifier: Modifier = Modifier, size: Dp = 34.dp) {
    val hue = categoryColor(color)
    VectorBadge(CategoryIcons.of(icon), hue, hue.copy(alpha = .16f), modifier, size)
}

/** A transfer has no category: Relay's transfer glyph in ink-2. */
@Composable
fun TransferBadge(modifier: Modifier = Modifier, size: Dp = 34.dp) {
    val c = Tally.colors
    GlyphBadge("transfer", c.ink2, c.wash, modifier, size)
}

@Composable
fun AccountBadge(type: AccountType, modifier: Modifier = Modifier, size: Dp = 34.dp) {
    val c = Tally.colors
    GlyphBadge(accountGlyph(type), c.ink2, c.wash, modifier, size)
}

/** Relay's glyph for each kind of account. */
fun accountGlyph(type: AccountType): String = when (type) {
    AccountType.CASH -> "coins"
    AccountType.CHEQUING -> "bank"
    AccountType.SAVINGS -> "wallet"
    AccountType.CREDIT -> "card"
    AccountType.INVESTMENT -> "trend"
}

@Composable
fun GlyphBadge(glyph: String, tint: Color, fill: Color, modifier: Modifier = Modifier, size: Dp = 34.dp) {
    Box(
        modifier.size(size).clip(Radii.badge).background(fill).border(1.dp, Tally.colors.edge.copy(alpha = .6f), Radii.badge),
        contentAlignment = Alignment.Center,
    ) {
        Glyph(glyph, size * .5f, tint)
    }
}

@Composable
private fun VectorBadge(vector: ImageVector, tint: Color, fill: Color, modifier: Modifier, size: Dp) {
    Box(modifier.size(size).clip(Radii.badge).background(fill), contentAlignment = Alignment.Center) {
        Image(vector, contentDescription = null, modifier = Modifier.size(size * .54f), colorFilter = ColorFilter.tint(tint))
    }
}

/** A category's icon alone, in its hue: for pickers and chips. */
@Composable
fun CategoryGlyph(icon: String?, color: Int?, size: Dp = 16.dp, modifier: Modifier = Modifier) {
    Image(CategoryIcons.of(icon), contentDescription = null, modifier = modifier.size(size), colorFilter = ColorFilter.tint(categoryColor(color)))
}
