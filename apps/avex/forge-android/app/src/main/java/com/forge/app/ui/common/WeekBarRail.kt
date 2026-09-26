package com.forge.app.ui.common

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import com.forge.app.ui.theme.ForgeMotion

/**
 * The week as one bar per training day, each carrying that day's sets against the heaviest day.
 * Extracted from onboarding's `PlanLedger` on its second screen (2026-09-25) so the plan the user
 * approved in onboarding and the plan they later open under Your program are drawn by ONE
 * implementation and can't drift into reading as two different weeks.
 *
 * - Passive (no [onSelect]): every bar reads as data; the caller supplies the whole-week readout.
 * - Navigating ([selectedIndex] + [onSelect]): accent marks the day being read, the rest step down
 *   to muted, and each bar is the tap target for its own day.
 * - Reorderable ([keys] + [onMove]): hold a bar and drag it sideways; it swaps with its neighbour
 *   each time it crosses half a slot, so the week re-sorts under the finger.
 */
@Composable
fun WeekBarRail(
    names: List<String>,
    sets: List<Int>,
    modifier: Modifier = Modifier,
    trackHeight: Dp = 72.dp,
    selectedIndex: Int? = null,
    onSelect: ((Int) -> Unit)? = null,
    keys: List<Any>? = null,
    onMove: ((from: Int, to: Int) -> Unit)? = null
) {
    if (names.isEmpty()) return
    val peak = (sets.maxOrNull() ?: 0).coerceAtLeast(1)
    val rowKeys = keys ?: names.indices.toList()
    val reorderable = onMove != null && keys != null && names.size > 1

    val gapPx = with(LocalDensity.current) { BAR_GAP.toPx() }
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    var rowWidth by remember { mutableIntStateOf(0) }
    var dragKey by remember { mutableStateOf<Any?>(null) }
    var dragOffset by remember { mutableFloatStateOf(0f) }
    val currentKeys by rememberUpdatedState(rowKeys)
    val currentMove by rememberUpdatedState(onMove)
    val currentSelect by rememberUpdatedState(onSelect)

    Row(
        modifier
            .fillMaxWidth()
            .onSizeChanged { rowWidth = it.width },
        horizontalArrangement = Arrangement.spacedBy(BAR_GAP),
        // Top, not bottom: every track is the same height, so aligning their TOPS keeps the whole
        // row of bars on one baseline even when one day's name wraps to two lines.
        verticalAlignment = Alignment.Top
    ) {
        names.forEachIndexed { i, name ->
            val barKey = rowKeys[i]
            key(barKey) {
                val dragging = dragKey == barKey
                DayBar(
                    name = name,
                    // A day that trains at all keeps a visible stub, so a light day reads as light
                    // rather than as missing.
                    fraction = if (sets.getOrElse(i) { 0 } <= 0) 0f
                    else (sets[i].toFloat() / peak).coerceAtLeast(0.15f),
                    lit = sets.getOrElse(i) { 0 } > 0,
                    // No selection = every bar reads as data. With one, accent marks the day being
                    // read and the rest step down to muted, so the accent means "you are here".
                    selected = selectedIndex == null || selectedIndex == i || dragging,
                    onClick = onSelect?.let { select -> { select(i) } },
                    readout = "$name, ${sets.getOrElse(i) { 0 }} sets",
                    // Dragging is invisible to TalkBack, so a reorderable bar also carries the
                    // move as named actions.
                    moveActions = if (!reorderable) emptyList() else buildList {
                        if (i > 0) add(CustomAccessibilityAction("Move earlier") { onMove?.invoke(i, i - 1); true })
                        if (i < names.size - 1) add(CustomAccessibilityAction("Move later") { onMove?.invoke(i, i + 1); true })
                    },
                    trackHeight = trackHeight,
                    modifier = Modifier
                        .weight(1f)
                        .then(
                            if (dragging) Modifier.zIndex(1f).graphicsLayer {
                                translationX = dragOffset
                                scaleX = 1.06f
                                scaleY = 1.06f
                            } else Modifier
                        )
                        .then(
                            if (!reorderable) Modifier
                            else Modifier.pointerInput(barKey) {
                                var at = -1
                                fun slot(): Float {
                                    val n = currentKeys.size.coerceAtLeast(1)
                                    return (rowWidth + gapPx) / n
                                }
                                detectDragGesturesAfterLongPress(
                                    onDragStart = {
                                        at = currentKeys.indexOf(barKey)
                                        dragKey = barKey
                                        dragOffset = 0f
                                        if (at >= 0) currentSelect?.invoke(at)
                                    },
                                    onDrag = { change, amount ->
                                        change.consume()
                                        dragOffset += amount.x
                                        val s = slot()
                                        // Logical offset: in RTL, moving right means moving earlier.
                                        val logical = if (rtl) -dragOffset else dragOffset
                                        val step = when {
                                            logical > s / 2 && at < currentKeys.size - 1 -> 1
                                            logical < -s / 2 && at > 0 -> -1
                                            else -> 0
                                        }
                                        if (step != 0 && at >= 0) {
                                            currentMove?.invoke(at, at + step)
                                            at += step
                                            // The bar's slot moved one step; carry the finger's
                                            // position over so the bar doesn't jump.
                                            dragOffset -= s * step * (if (rtl) -1 else 1)
                                        }
                                    },
                                    onDragEnd = { dragKey = null; dragOffset = 0f },
                                    onDragCancel = { dragKey = null; dragOffset = 0f }
                                )
                            }
                        )
                )
            }
        }
    }
}

private val BAR_GAP = 8.dp

/**
 * One day's bar in its track, with the day's mono name beneath. The WHOLE column is the tap target
 * when it has one (bar plus label, never a nested tap), which also gets it near 48dp at seven days
 * across a phone gutter.
 */
@Composable
private fun DayBar(
    name: String,
    fraction: Float,
    lit: Boolean,
    selected: Boolean,
    onClick: (() -> Unit)?,
    readout: String,
    moveActions: List<CustomAccessibilityAction>,
    trackHeight: Dp,
    modifier: Modifier = Modifier
) {
    val animated by animateFloatAsState(fraction, ForgeMotion.standardTween(), label = "day_sets")
    // Muted at the 0.7 rung, not at full: six near-white slabs beside one accent bar out-shout the
    // accent. 0.7 measures 5.18:1 on Pearl, clear of the 3:1 floor for a mark that carries meaning.
    val fill by animateColorAsState(
        if (selected) MaterialTheme.colorScheme.primary
        else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
        ForgeMotion.standardTween(ForgeMotion.DurationFast),
        label = "day_fill"
    )
    Column(
        modifier
            .then(
                if (onClick == null) Modifier
                else Modifier
                    .minimumInteractiveComponentSize()
                    .bounceClick(onClick = onClick)
                    .semantics {
                        contentDescription = readout
                        this.selected = selected
                        if (moveActions.isNotEmpty()) customActions = moveActions
                    }
            ),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Box(
            Modifier
                // Capped so four days read as bars rather than as slabs of accent, and seven still
                // fit the gutter. No text inside, so a fixed height is safe at any font scale.
                .widthIn(max = 28.dp)
                .fillMaxWidth()
                .height(trackHeight)
                .clip(RoundedCornerShape(4.dp))
                .background(MaterialTheme.colorScheme.outline.copy(alpha = 0.25f)),
            contentAlignment = Alignment.BottomCenter
        ) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .fillMaxHeight(animated.coerceIn(0f, 1f))
                    .clip(RoundedCornerShape(4.dp))
                    .background(fill)
            )
        }
        Text(
            name.uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = if (lit && selected) MaterialTheme.colorScheme.onBackground
            else MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis
        )
    }
}
