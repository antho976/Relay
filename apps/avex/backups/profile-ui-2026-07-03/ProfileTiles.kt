package com.forge.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.ui.common.bounceClick

/**
 * The profile's shared building blocks, open-editorial edition: a quiet section header, big serif
 * stat figures set directly on the page, and a small chart legend. The bordered "grey box" cards are
 * gone on purpose — the cover photo up top dissolves into the background, so everything below it
 * sits on the same open page instead of fighting it with slabs.
 */

/**
 * A quiet small-caps section anchor (label + optional accent action). Spacing and the big figures
 * themselves carry the page structure — this is just the label.
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
            .padding(start = 2.dp, bottom = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = muted)
        if (action != null) Text(action, style = MaterialTheme.typography.labelSmall, color = accent)
    }
}

/** The data behind one [StatCell] — a big figure, its label, and an optional week-over-week delta. */
internal data class StatCellSpec(
    val value: String,
    val label: String,
    /** Signed week-over-week change shown as a small ↑/↓ badge beside the figure (0/null = none). */
    val delta: Int? = null
)

/**
 * One open dashboard figure: a large serif number set straight on the background with its small-caps
 * label underneath — no card, no border. The optional delta rides just after the figure's cap line.
 */
@Composable
internal fun StatCell(
    value: String,
    label: String,
    accent: Color,
    muted: Color,
    onBg: Color,
    modifier: Modifier = Modifier,
    delta: Int? = null
) {
    Column(modifier) {
        Row(verticalAlignment = Alignment.Top) {
            Text(
                value,
                style = MaterialTheme.typography.headlineLarge,
                color = onBg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            if (delta != null && delta != 0) {
                Spacer(Modifier.width(6.dp))
                DeltaBadge(delta, accent, onBg, Modifier.padding(top = 8.dp))
            }
        }
        Spacer(Modifier.height(2.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 9.sp)
    }
}

/**
 * A "↑2 / ↓1" week-over-week badge. The arrow is always accent-tinted so it stays legible on the
 * dark page (a muted down-arrow was getting lost), and the count sits in the bright on-background
 * ink beside it; both a touch larger than the small-caps label so the change actually reads.
 */
@Composable
private fun DeltaBadge(delta: Int, accent: Color, onBg: Color, modifier: Modifier = Modifier) {
    val up = delta > 0
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text(
            if (up) "↑" else "↓",
            style = MaterialTheme.typography.labelSmall,
            color = accent,
            fontSize = 13.sp
        )
        Text(
            "${kotlin.math.abs(delta)}",
            style = MaterialTheme.typography.labelSmall,
            color = onBg,
            fontSize = 13.sp
        )
    }
}

/** Lay a list of [StatCellSpec]s out two-up with an airy vertical rhythm; an odd trailing cell keeps its half-width. */
@Composable
internal fun StatCellGrid(
    specs: List<StatCellSpec>,
    accent: Color,
    muted: Color,
    onBg: Color,
    modifier: Modifier = Modifier
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(18.dp)) {
        specs.chunked(2).forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                row.forEach { spec ->
                    StatCell(
                        value = spec.value,
                        label = spec.label,
                        accent = accent,
                        muted = muted,
                        onBg = onBg,
                        modifier = Modifier.weight(1f),
                        delta = spec.delta
                    )
                }
                if (row.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

/** A dot-plus-caption legend line for the open full-width charts. */
@Composable
internal fun ChartCaption(color: Color, label: String, muted: Color) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(6.dp).clip(RoundedCornerShape(50)).background(color))
        Spacer(Modifier.width(5.dp))
        Text(label, style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 8.sp)
    }
}
