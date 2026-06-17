package com.forge.app.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import com.forge.app.data.repo.TrophyCell
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick

/** Fixed popup width so the card centers exactly over the tapped trophy (and the caret aligns). */
private val POPUP_WIDTH = 200.dp

/**
 * TROPHY CASE — icon grid; unlocked carry a check, in-progress show a ring. Tapping a trophy
 * opens a small popup above it (name · what it is · progress / what's left) and the badge plays its
 * signature animation while open. Only one popup is open at a time; tapping outside dismisses.
 */
@Composable
internal fun TrophyCaseSection(
    grid: List<TrophyCell>,
    unlocked: Int,
    total: Int,
    closestTrophy: String?,
    onOpenTrophies: () -> Unit,
    onBg: Color,
    muted: Color,
    accent: Color,
    outline: Color
) {
    var selected by remember { mutableStateOf<Int?>(null) }
    ProfileBlock("TROPHY CASE", muted, accent, outline, action = "$unlocked / $total  →", onAction = onOpenTrophies) {
        grid.chunked(6).forEachIndexed { rowIdx, row ->
            Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEachIndexed { colIdx, cell ->
                    val index = rowIdx * 6 + colIdx
                    Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                        TrophyGridCell(
                            cell = cell,
                            open = selected == index,
                            accent = accent,
                            outline = outline,
                            muted = muted,
                            onBg = onBg,
                            onToggle = { selected = if (selected == index) null else index },
                            onDismiss = { selected = null }
                        )
                    }
                }
                repeat(6 - row.size) { Box(Modifier.weight(1f)) }
            }
        }
        if (unlocked == 0) {
            Spacer(Modifier.height(10.dp))
            InlineEmptyHint("No trophies yet — finish your first workout to start unlocking them.", color = muted)
        }
        if (closestTrophy != null) {
            Spacer(Modifier.height(10.dp))
            Text("Closest: $closestTrophy", style = MaterialTheme.typography.bodySmall, color = onBg, fontStyle = FontStyle.Italic)
        }
    }
}

@Composable
private fun TrophyGridCell(
    cell: TrophyCell,
    open: Boolean,
    accent: Color,
    outline: Color,
    muted: Color,
    onBg: Color,
    onToggle: () -> Unit,
    onDismiss: () -> Unit
) {
    val bg = MaterialTheme.colorScheme.background
    val onBackground = MaterialTheme.colorScheme.onBackground

    // The cell is a tappable Canvas/icon with no text — TalkBack would land on a silent button.
    val a11yLabel = "${cell.name}, " + when {
        cell.unlocked -> "earned"
        cell.progress > 0f -> "${(cell.progress * 100).toInt()} percent progress"
        else -> "locked"
    }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .bounceClick { onToggle() }
                .semantics { contentDescription = a11yLabel; role = Role.Button }
        ) {
            if (!cell.unlocked && cell.progress > 0f) {
                Canvas(Modifier.size(42.dp)) {
                    drawArc(
                        color = accent.copy(alpha = 0.85f),
                        startAngle = -90f,
                        sweepAngle = cell.progress * 360f,
                        useCenter = false,
                        style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round)
                    )
                }
            }
            Box(contentAlignment = Alignment.BottomEnd) {
                AnimatedTrophyBadge(icon = cell.icon, unlocked = cell.unlocked, active = open, size = 34.dp, accent = accent, variant = cell.variant)
                if (cell.unlocked) {
                    Box(Modifier.size(14.dp).clip(CircleShape).background(bg), contentAlignment = Alignment.Center) {
                        Box(Modifier.size(11.dp).clip(CircleShape).background(onBackground), contentAlignment = Alignment.Center) {
                            Icon(Icons.Filled.Check, contentDescription = null, tint = bg, modifier = Modifier.size(7.dp))
                        }
                    }
                }
            }

            if (open) {
                TrophyPopup(cell = cell, accent = accent, outline = outline, muted = muted, onBg = onBg, onDismiss = onDismiss)
            }
        }
        if (!cell.unlocked && cell.progress > 0f) {
            Spacer(Modifier.height(3.dp))
            Text("${(cell.progress * 100).toInt()}%", style = MaterialTheme.typography.labelSmall, color = outline, fontSize = 8.sp)
        }
    }
}

/** The inspect card, anchored centered above the tapped trophy with a caret pointing down at it. */
@Composable
private fun TrophyPopup(
    cell: TrophyCell,
    accent: Color,
    outline: Color,
    muted: Color,
    onBg: Color,
    onDismiss: () -> Unit
) {
    val surface = MaterialTheme.colorScheme.surface
    val density = LocalDensity.current
    val gapPx = with(density) { 8.dp.roundToPx() }
    val edgePx = with(density) { 12.dp.roundToPx() }
    var caretX by remember { mutableIntStateOf(0) }
    val provider = remember(gapPx, edgePx) { AboveAnchorPositionProvider(gapPx, edgePx) { caretX = it } }

    Popup(
        popupPositionProvider = provider,
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true)
    ) {
        Column {
            Column(
                Modifier
                    .width(POPUP_WIDTH)
                    .clip(RoundedCornerShape(12.dp))
                    .background(surface)
                    .border(1.dp, outline.copy(alpha = 0.4f), RoundedCornerShape(12.dp))
                    .padding(horizontal = 12.dp, vertical = 10.dp)
            ) {
                Text(cell.name, style = MaterialTheme.typography.labelLarge, color = accent, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.height(3.dp))
                Text(cell.description, style = MaterialTheme.typography.bodySmall, color = muted, fontSize = 11.sp)
                Spacer(Modifier.height(8.dp))
                if (cell.unlocked) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Filled.Check, contentDescription = null, tint = accent, modifier = Modifier.size(12.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Unlocked", style = MaterialTheme.typography.labelSmall, color = accent, fontSize = 10.sp)
                    }
                } else {
                    Box(Modifier.fillMaxWidth().height(4.dp).clip(RoundedCornerShape(50)).background(outline.copy(alpha = 0.25f))) {
                        Box(Modifier.fillMaxWidth(cell.progress.coerceIn(0f, 1f)).fillMaxHeight().clip(RoundedCornerShape(50)).background(accent))
                    }
                    Spacer(Modifier.height(5.dp))
                    if (cell.progressLabel != null) {
                        Text(cell.progressLabel, style = MaterialTheme.typography.labelSmall, color = onBg, fontSize = 10.sp)
                    } else {
                        Text("Locked", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, fontStyle = FontStyle.Italic)
                    }
                }
            }
            // Downward caret pointing at the trophy. caretX (px from the popup's left) is reported by the provider.
            Canvas(Modifier.width(POPUP_WIDTH).height(7.dp)) {
                val w = 14.dp.toPx()
                val cx = caretX.toFloat().coerceIn(w / 2, (size.width - w / 2).coerceAtLeast(w / 2))
                val path = Path().apply {
                    moveTo(cx - w / 2, 0f)
                    lineTo(cx + w / 2, 0f)
                    lineTo(cx, size.height)
                    close()
                }
                drawPath(path, color = surface)
            }
        }
    }
}

/**
 * Centers the popup over the anchor, clamped to the window edges, and reports back the caret's x
 * (relative to the popup's left edge) so the caret keeps pointing at the anchor even when the popup
 * is clamped near a screen edge (the first/last columns).
 */
private class AboveAnchorPositionProvider(
    private val gapPx: Int,
    private val edgePx: Int,
    private val onCaretX: (Int) -> Unit
) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize
    ): IntOffset {
        val anchorCenterX = anchorBounds.left + anchorBounds.width / 2
        val maxX = (windowSize.width - popupContentSize.width - edgePx).coerceAtLeast(edgePx)
        val x = (anchorCenterX - popupContentSize.width / 2).coerceIn(edgePx, maxX)
        val y = (anchorBounds.top - popupContentSize.height - gapPx).coerceAtLeast(edgePx)
        onCaretX(anchorCenterX - x)
        return IntOffset(x, y)
    }
}
