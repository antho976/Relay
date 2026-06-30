package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick

/**
 * The profile's shared building blocks: a rounded surface card, a light section header, and the
 * stat tile + grid that replace the old hairline-divided rows. The redo trades the flat divider
 * rhythm for cards/tiles so individual figures actually stand out.
 */

/** The card shell every profile block sits on — a [surfaceVariant] slab, optionally tappable. */
@Composable
internal fun ProfileCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    padding: Dp = 16.dp,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .then(if (onClick != null) Modifier.bounceClick { onClick() } else Modifier)
            .padding(padding),
        content = content
    )
}

/**
 * A quiet small-caps section anchor (label + optional accent action). Replaces [ProfileBlock]'s
 * heavy divider — spacing and the cards themselves now carry the structure, so this is just a label.
 */
@Composable
internal fun SectionHeader(
    label: String,
    muted: Color,
    accent: Color,
    action: String? = null,
    onAction: (() -> Unit)? = null
) {
    Row(
        Modifier.fillMaxWidth().then(if (onAction != null) Modifier.bounceClick { onAction() } else Modifier)
            .padding(start = 2.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = muted)
        if (action != null) Text(action, style = MaterialTheme.typography.labelSmall, color = accent)
    }
}

/** The data behind one [StatTile] — a big figure, its label, and an optional sparkline or caption. */
internal data class StatTileSpec(
    val value: String,
    val label: String,
    val caption: String? = null,
    val sparkline: List<Double>? = null
)

/**
 * A single dashboard tile: a big serif figure + small-caps label, with an optional sparkline
 * (drawn along the bottom) or accent caption. Fixed height so a grid of them lines up.
 */
@Composable
internal fun StatTile(
    value: String,
    label: String,
    accent: Color,
    muted: Color,
    onBg: Color,
    modifier: Modifier = Modifier,
    caption: String? = null,
    sparkline: List<Double>? = null,
    onClick: (() -> Unit)? = null
) {
    ProfileCard(modifier = modifier.height(104.dp), onClick = onClick, padding = 14.dp) {
        Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.SpaceBetween) {
            Column {
                Text(
                    value,
                    style = MaterialTheme.typography.headlineMedium,
                    color = onBg,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Spacer(Modifier.height(2.dp))
                Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
            }
            when {
                sparkline != null && sparkline.size >= 2 ->
                    ProfileSparkline(sparkline, accent, Modifier.fillMaxWidth().height(26.dp))
                caption != null ->
                    Text(caption, style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 9.sp)
            }
        }
    }
}

/** Lay a list of [StatTileSpec]s out two-up; a trailing odd tile keeps its half-width. */
@Composable
internal fun StatTileGrid(
    specs: List<StatTileSpec>,
    accent: Color,
    muted: Color,
    onBg: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        specs.chunked(2).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                row.forEach { spec ->
                    StatTile(
                        value = spec.value,
                        label = spec.label,
                        accent = accent,
                        muted = muted,
                        onBg = onBg,
                        modifier = Modifier.weight(1f),
                        caption = spec.caption,
                        sparkline = spec.sparkline
                    )
                }
                if (row.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}
