package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.TextButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.gym.train.state.DayListItem
import com.forge.app.ui.theme.toAccentColor
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun DayCard(
    item: DayListItem,
    onClick: () -> Unit,
    onQuickStart: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    if (item.isNextUp) {
        NextUpCard(item = item, onClick = onClick, onQuickStart = onQuickStart, onLongPress = onLongPress, modifier = modifier)
    } else {
        CompactCard(item = item, onClick = onClick, onLongPress = onLongPress, modifier = modifier)
    }
}

// ── Next-up variant ──────────────────────────────────────────────────────────

@Composable
private fun NextUpCard(
    item: DayListItem,
    onClick: () -> Unit,
    onQuickStart: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val accent = (item.customAccentHex ?: item.plan.accentHex).toAccentColor()
    val surface = MaterialTheme.colorScheme.surface

    val bloom = remember(accent) {
        Brush.radialGradient(
            colorStops = arrayOf(
                0.0f to accent.copy(alpha = 0.55f),
                0.55f to accent.copy(alpha = 0.22f),
                1.0f to Color.Transparent
            ),
            center = Offset.Unspecified,
            radius = Float.MAX_VALUE,
            tileMode = TileMode.Clamp
        )
    }

    @OptIn(ExperimentalFoundationApi::class)
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .combinedClickable(onClick = onClick, onLongClick = onLongPress)
            .bounceClick { onClick() },
        color = Color.Transparent,
        tonalElevation = 2.dp
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .background(surface)
                .drawBehind {
                    val radius = size.width * 0.72f
                    drawRect(
                        brush = Brush.radialGradient(
                            colorStops = arrayOf(
                                0.0f to accent.copy(alpha = 0.50f),
                                0.50f to accent.copy(alpha = 0.20f),
                                1.0f to Color.Transparent
                            ),
                            center = Offset(x = size.width * 0.92f, y = size.height * 0.5f),
                            radius = radius,
                            tileMode = TileMode.Clamp
                        )
                    )
                }
        ) {
            Row(Modifier.fillMaxWidth()) {
                SpineStrip(accent = accent, word = item.plan.word)
                Column(
                    Modifier
                        .weight(1f)
                        .padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    // "NEXT UP" pill
                    NextUpPill(accent = accent)

                    Text(
                        item.displayName,
                        style = MaterialTheme.typography.headlineMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        "${item.plan.subtitle} · ${item.exerciseCount} exercises",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )

                    Spacer(Modifier.weight(1f))

                    Button(
                        onClick = onClick,
                        modifier = Modifier.fillMaxWidth(),
                        colors = ButtonDefaults.buttonColors(containerColor = accent)
                    ) {
                        Text(
                            if (item.isActive) "Continue →" else "Start →",
                            fontWeight = FontWeight.Bold
                        )
                    }
                    if (!item.isActive && onQuickStart != null) {
                        TextButton(
                            onClick = onQuickStart,
                            modifier = Modifier.fillMaxWidth(),
                            colors = ButtonDefaults.textButtonColors(
                                contentColor = accent.copy(alpha = 0.75f)
                            )
                        ) {
                            Text(
                                "Quick start (skip warmup)",
                                style = MaterialTheme.typography.labelMedium
                            )
                        }
                    }
                }
            }
        }
    }
}

// ── Compact variant ──────────────────────────────────────────────────────────

@Composable
private fun CompactCard(
    item: DayListItem,
    onClick: () -> Unit,
    onLongPress: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    val accent = (item.customAccentHex ?: item.plan.accentHex).toAccentColor()
    val surface = MaterialTheme.colorScheme.surface

    @OptIn(ExperimentalFoundationApi::class)
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .height(96.dp)
            .clip(RoundedCornerShape(16.dp))
            .combinedClickable(onClick = onClick, onLongClick = onLongPress)
            .bounceClick { onClick() },
        color = Color.Transparent,
        tonalElevation = 2.dp
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .background(surface)
                .drawBehind {
                    val radius = size.width * 0.55f
                    drawRect(
                        brush = Brush.radialGradient(
                            colorStops = arrayOf(
                                0.0f to accent.copy(alpha = 0.38f),
                                0.50f to accent.copy(alpha = 0.12f),
                                1.0f to Color.Transparent
                            ),
                            center = Offset(x = size.width * 0.95f, y = size.height * 0.5f),
                            radius = radius,
                            tileMode = TileMode.Clamp
                        )
                    )
                }
        ) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .fillMaxHeight(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                SpineStrip(accent = accent, word = item.plan.word)
                Column(
                    Modifier
                        .weight(1f)
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(3.dp)
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            item.displayName,
                            style = MaterialTheme.typography.titleLarge,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                        if (item.isActive) {
                            ActiveDot(accent)
                        }
                    }
                    Text(
                        buildString {
                            append(
                                item.lastFinishedAt
                                    ?.let { formatRelative(it) }
                                    ?: "Never trained"
                            )
                            append(" · ${item.exerciseCount} exercises")
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                // Arrow
                Text(
                    "→",
                    modifier = Modifier.padding(end = 16.dp),
                    style = MaterialTheme.typography.titleMedium,
                    color = accent.copy(alpha = 0.70f)
                )
            }
        }
    }
}

// ── Shared sub-composables ────────────────────────────────────────────────────

@Composable
private fun NextUpPill(accent: Color) {
    Surface(
        shape = RoundedCornerShape(6.dp),
        color = accent.copy(alpha = 0.18f)
    ) {
        Text(
            "NEXT UP",
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
            color = accent,
            fontWeight = FontWeight.Black,
            fontSize = 10.sp,
            letterSpacing = 1.5.sp
        )
    }
}

@Composable
private fun SpineStrip(accent: Color, word: String) {
    Box(
        Modifier
            .width(44.dp)
            .fillMaxHeight()
            .background(accent.copy(alpha = 0.18f)),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = word,
            color = accent,
            fontWeight = FontWeight.Black,
            fontSize = 12.sp,
            letterSpacing = 3.sp,
            modifier = Modifier.graphicsLayer { rotationZ = -90f }
        )
    }
}

@Composable
private fun ActiveDot(color: Color) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Box(
            Modifier
                .width(6.dp)
                .height(6.dp)
                .clip(RoundedCornerShape(50))
                .background(color)
        )
        Text(
            "ACTIVE",
            color = color,
            fontSize = 9.sp,
            fontWeight = FontWeight.Black,
            letterSpacing = 1.sp
        )
    }
}

private val dateFormat = SimpleDateFormat("MMM d", Locale.getDefault())

private fun formatRelative(epochMs: Long): String {
    val nowMs = System.currentTimeMillis()
    val deltaMs = nowMs - epochMs
    val day = 24L * 60 * 60 * 1000
    return when {
        deltaMs < day -> "Today"
        deltaMs < 2 * day -> "Yesterday"
        deltaMs < 7 * day -> "${deltaMs / day} days ago"
        else -> dateFormat.format(Date(epochMs))
    }
}
