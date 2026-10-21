package com.forge.app.ui.common

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenIcons
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.openOnAccentLarge

/*
 * The "Open panel" kit (2026-10-21): the pieces every redesigned screen is built from.
 *
 * The style in one paragraph, as Antho settled it over the Home, Stats and Coach rounds: no boxes
 * around passive content (sections hold by the gutter, a grid and ~52dp of air); only actions are
 * filled, and a red 4dp-corner button is the primary one; each reading is mono label → serif figure →
 * small mark → mono change; red is spent on today, the action and what moved, nothing else.
 * `OpenType` and `OpenColors` (ui/theme/OpenPanel.kt) hold the rungs these draw with.
 */

private val OpenCorner = RoundedCornerShape(4.dp)

/**
 * Grows a control's touch target past the box it lays out in, so a 15dp-tall label can be a 48dp
 * target (§14) without pushing the design's spacing apart. The content is measured with [vertical]
 * and [horizontal] slop on each side, laid out at its own size, and drawn back in place.
 */
fun Modifier.touchSlop(vertical: Dp, horizontal: Dp = 0.dp): Modifier = layout { measurable, constraints ->
    val v = vertical.roundToPx()
    val h = horizontal.roundToPx()
    val placeable = measurable.measure(constraints.offset(2 * h, 2 * v))
    layout(placeable.width - 2 * h, placeable.height - 2 * v) { placeable.place(-h, -v) }
}

private fun androidx.compose.ui.unit.Constraints.offset(h: Int, v: Int) = copy(
    maxWidth = if (hasBoundedWidth) maxWidth + h else maxWidth,
    maxHeight = if (hasBoundedHeight) maxHeight + v else maxHeight,
    minWidth = minWidth + h,
    minHeight = minHeight + v,
)

/** The label voice: mono, upper case, tracked. Muted unless it is the thing that changed. */
@Composable
fun OpenLabel(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onSurfaceVariant,
    style: TextStyle = OpenType.monoS,
) {
    Text(text.uppercase(), modifier = modifier, style = style, color = color)
}

/**
 * A section's head: its mono label on the left and, when the section has a page of its own, a mono
 * "View all ›" on the right. No rule under it: the air above is the separator.
 */
@Composable
fun OpenSectionHead(
    label: String,
    modifier: Modifier = Modifier,
    action: String? = null,
    actionLabel: String? = null,
    onAction: () -> Unit = {},
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel(label, Modifier.semantics { heading() })
        if (action != null) {
            Row(
                modifier = Modifier
                    .touchSlop(vertical = 16.dp, horizontal = 8.dp)
                    .clickableLabeled(actionLabel ?: action, onClick = onAction)
                    .padding(vertical = 16.dp, horizontal = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                OpenLabel(action)
                Icon(OpenIcons.ChevronRight, contentDescription = null, tint = muted, modifier = Modifier.size(14.dp))
            }
        }
    }
}

/**
 * A reading's label when the reading can be swapped for another (Home's two slots): the label with a
 * ▾ after it, and the pair is the tap target that opens the picker.
 */
@Composable
fun OpenSlotLabel(label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = modifier
            .touchSlop(vertical = 16.dp, horizontal = 8.dp)
            .clickableLabeled("Change ${label.lowercase()}", onClick = onClick)
            .padding(vertical = 16.dp, horizontal = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel(label)
        Icon(OpenIcons.ChevronDown, contentDescription = null, tint = muted, modifier = Modifier.size(14.dp))
    }
}

/** A 3dp progress track: the outline grey, filled red to [fraction]. */
@Composable
fun OpenMeter(fraction: Float, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    Canvas(modifier.fillMaxWidth().height(3.dp)) {
        val r = CornerRadius(1.dp.toPx())
        drawRoundRect(OpenColors.Track, cornerRadius = r)
        val f = fraction.coerceIn(0f, 1f)
        if (f > 0f) drawRoundRect(accent, size = Size(size.width * f, size.height), cornerRadius = r)
    }
}

/**
 * The primary action: a full-width red bar, 56dp, 4dp corners, the label in white 19 bold on the left
 * and an arrow on the right. [emphasized] false draws it outlined (a rest day's "Train anyway": the
 * page should not tell you to do what its headline just said not to).
 */
@Composable
fun OpenPrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    emphasized: Boolean = true,
    icon: ImageVector? = OpenIcons.ArrowRight,
    onLongClick: (() -> Unit)? = null,
    onLongClickLabel: String? = null,
) {
    val accent = MaterialTheme.colorScheme.primary
    val ink = if (emphasized) openOnAccentLarge() else MaterialTheme.colorScheme.onBackground
    Row(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .bounceCombinedClick(
                onClickLabel = text,
                onLongClick = onLongClick,
                onLongClickLabel = onLongClickLabel,
                role = Role.Button,
                onClick = onClick
            )
            .clip(OpenCorner)
            .then(
                if (emphasized) Modifier.background(accent)
                else Modifier.border(1.5.dp, OpenColors.Rule, OpenCorner)
            )
            .padding(start = 20.dp, end = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(text, style = OpenType.button, color = ink, modifier = Modifier.padding(vertical = 14.dp))
        if (icon != null) {
            Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
                Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(22.dp))
            }
        } else {
            Spacer(Modifier.width(12.dp))
        }
    }
}

/**
 * A small action (Apply, Skip, Undo, a photo-lock Unlock): 36dp tall, hugging its label. Filled red
 * with dark text, or outlined. Its 48dp target comes from [touchSlop], not from its drawn size.
 */
@Composable
fun OpenSmallButton(
    text: String,
    filled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
) {
    val accent = MaterialTheme.colorScheme.primary
    val ink = if (filled) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onBackground
    Row(
        modifier = modifier
            .touchSlop(vertical = 6.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .padding(vertical = 6.dp)
            .heightIn(min = 36.dp)
            .clip(OpenCorner)
            .then(
                if (filled) Modifier.background(accent)
                else Modifier.border(1.dp, OpenColors.Rule, OpenCorner)
            )
            .padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (icon != null) Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(18.dp))
        Text(text, style = OpenType.buttonS, color = ink)
    }
}

/** The personal-record tag: red, 2dp corners, dark "PR". */
@Composable
fun OpenPrTag(modifier: Modifier = Modifier) {
    Box(
        modifier
            .clip(RoundedCornerShape(2.dp))
            .background(MaterialTheme.colorScheme.primary)
            .padding(horizontal = 5.dp, vertical = 3.dp)
            .semantics { contentDescription = "Personal record" },
        contentAlignment = Alignment.Center
    ) {
        Text("PR", style = OpenType.tag, color = MaterialTheme.colorScheme.onPrimary)
    }
}

/**
 * "Each lift vs last time": one bar per exercise on a baseline two thirds of the way down. A gain
 * rises above it, a loss hangs below it, a lift that matched last time is a 2dp stub, and the biggest
 * gain is the one red bar (other gains grey, losses dimmer still). A null entry (no previous time to
 * compare) keeps its place but draws nothing: a row of stubs read as a dashed line, not as data. An
 * empty list draws the bare baseline, which an empty reading uses as its mark.
 */
@Composable
fun OpenDeltaBars(
    deltas: List<Float?>,
    modifier: Modifier = Modifier,
    barWidth: Dp = 8.dp,
    description: String? = null,
) {
    val accent = MaterialTheme.colorScheme.primary
    val maxAbs = deltas.maxOfOrNull { kotlin.math.abs(it ?: 0f) }?.takeIf { it > 0f } ?: 1f
    val lit = deltas.withIndex().filter { (it.value ?: 0f) > 0f }.maxByOrNull { it.value!! }?.index
    Canvas(
        modifier.then(
            if (description != null) Modifier.clearAndSetSemantics { contentDescription = description }
            else Modifier
        )
    ) {
        val rule = 1.dp.toPx()
        val baseY = (size.height * 0.66f).let { kotlin.math.round(it) }
        drawRect(OpenColors.Rule, topLeft = Offset(0f, baseY), size = Size(size.width, rule))
        if (deltas.isEmpty()) return@Canvas
        val n = deltas.size
        // Many lifts in a narrow mark: the bars thin out rather than run past the edge.
        val minGap = 2.dp.toPx()
        val bw = minOf(barWidth.toPx(), if (n > 1) (size.width - (n - 1) * minGap) / n else size.width)
        val gap = if (n > 1) ((size.width - n * bw) / (n - 1)).coerceAtLeast(minGap) else 0f
        val upRoom = baseY - 2.dp.toPx()
        val downRoom = size.height - baseY - rule
        val perUnit = upRoom / maxAbs
        val stub = 2.dp.toPx()
        val r = CornerRadius(1.dp.toPx())
        deltas.forEachIndexed { i, d ->
            val v = d ?: return@forEachIndexed
            val x = i * (bw + gap)
            val color = when {
                i == lit -> accent
                v < 0f -> OpenColors.BarDown
                else -> OpenColors.Bar
            }
            if (v < 0f) {
                val h = (-v * perUnit).coerceIn(stub, downRoom)
                drawRoundRect(color, topLeft = Offset(x, baseY + rule), size = Size(bw, h), cornerRadius = r)
            } else {
                val h = if (v == 0f) stub else (v * perUnit).coerceIn(stub, upRoom)
                drawRoundRect(color, topLeft = Offset(x, baseY - h), size = Size(bw, h), cornerRadius = r)
            }
        }
    }
}

/**
 * A reading's sparkline: the red line in the top two thirds over a faint red area down to the
 * bottom edge, with a dot on the latest value ringed in the page colour so it reads as "now".
 */
@Composable
fun OpenSparkline(
    values: List<Float>,
    modifier: Modifier = Modifier,
    description: String? = null,
    /** Just the line: no area under it and no dot on the latest value (Stats' Body readings). */
    plain: Boolean = false,
) {
    val accent = MaterialTheme.colorScheme.primary
    val ground = MaterialTheme.colorScheme.background
    Canvas(
        modifier.then(
            if (description != null) Modifier.clearAndSetSemantics { contentDescription = description }
            else Modifier
        )
    ) {
        if (values.size < 2) return@Canvas
        val dotR = 5.dp.toPx()
        val w = size.width - dotR
        val top = 2.dp.toPx()
        val h = size.height * 2f / 3f
        val lo = values.min()
        val hi = values.max()
        val span = (hi - lo).takeIf { it > 0f } ?: 1f
        val pts = values.mapIndexed { i, v ->
            Offset(i * w / (values.size - 1), top + h - (v - lo) / span * h)
        }
        val line = Path().apply { pts.forEachIndexed { i, p -> if (i == 0) moveTo(p.x, p.y) else lineTo(p.x, p.y) } }
        val area = Path().apply {
            addPath(line)
            lineTo(pts.last().x, size.height)
            lineTo(pts.first().x, size.height)
            close()
        }
        if (!plain) drawPath(area, accent.copy(alpha = 0.10f))
        drawPath(line, accent, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        if (!plain) {
            drawCircle(ground, radius = dotR + 2.dp.toPx() / 2f, center = pts.last())
            drawCircle(accent, radius = dotR - 2.dp.toPx() / 2f, center = pts.last())
        }
    }
}

/**
 * Home's bell in the redesign's icon family: a 22dp outline glyph with the unread count on a 16dp red
 * disc at its shoulder. Behaves as [NotificationBell] does (tap opens the feed, hold goes Home, it
 * publishes where it sits so an arrival banner can fly into it); only the drawing differs.
 */
@Composable
fun OpenNotificationBell(modifier: Modifier = Modifier) {
    val unread = LocalUnreadNotifications.current
    val onOpen = LocalOpenNotifications.current
    val onHome = LocalGoHome.current
    val anchor = LocalBellAnchor.current
    val label = if (unread > 0) "Notifications, $unread waiting" else "Notifications"
    DisposableEffect(anchor) { onDispose { anchor.position = null } }
    Box(
        modifier = modifier
            .touchSlop(vertical = 13.dp, horizontal = 13.dp)
            .onGloballyPositioned { anchor.position = it.boundsInRoot().center }
            .bounceCombinedClick(
                onClickLabel = label,
                onLongClickLabel = "Go to Home",
                onLongClick = onHome,
                onClick = onOpen
            )
            .padding(13.dp)
            .size(22.dp)
    ) {
        Icon(
            OpenIcons.Bell,
            contentDescription = label,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(22.dp)
        )
        if (unread > 0) {
            Box(
                Modifier
                    .offset(x = 12.dp, y = (-6).dp)
                    .sizeIn(minWidth = 16.dp, minHeight = 16.dp)
                    .background(MaterialTheme.colorScheme.primary, CircleShape)
                    .padding(horizontal = 3.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    if (unread > 9) "9+" else "$unread",
                    style = OpenType.tag,
                    color = MaterialTheme.colorScheme.onPrimary
                )
            }
        }
    }
}

/** A top-bar glyph (Profile, Settings): 22dp drawn, 48dp to the finger. */
@Composable
fun OpenTopBarIcon(icon: ImageVector, label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Box(
        modifier
            .touchSlop(vertical = 13.dp, horizontal = 13.dp)
            .clickableLabeled(label, onClick = onClick)
            .padding(13.dp)
    ) {
        Icon(icon, contentDescription = label, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(22.dp))
    }
}

/**
 * A page's tabs: mono labels with the selected one in ink over a 2dp red underline as wide as its
 * word (Stats, Coach, Profile). Each label is a 48dp target.
 */
@Composable
fun <T> OpenTabs(tabs: List<T>, selected: T, label: (T) -> String, onSelect: (T) -> Unit, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(26.dp), verticalAlignment = Alignment.Top) {
        tabs.forEach { t ->
            val on = t == selected
            androidx.compose.foundation.layout.Column(
                Modifier
                    .touchSlop(vertical = 10.dp, horizontal = 6.dp)
                    .selectable(selected = on, role = Role.Tab) { onSelect(t) }
                    .padding(vertical = 10.dp, horizontal = 6.dp)
                    .width(androidx.compose.foundation.layout.IntrinsicSize.Max),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    label(t).uppercase(),
                    style = OpenType.monoM,
                    color = if (on) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
                )
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(2.dp)
                        .then(if (on) Modifier.background(MaterialTheme.colorScheme.primary) else Modifier)
                )
            }
        }
    }
}

/**
 * The page's one range (4W / 12W / 6M / All): mono labels, the chosen one in ink inside a quiet 4dp
 * outline. Small on purpose: it sets the window, it is not the content.
 */
@Composable
fun <T> OpenRangeSwitch(options: List<T>, selected: T, label: (T) -> String, onSelect: (T) -> Unit, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
        options.forEach { o ->
            val on = o == selected
            Box(
                Modifier
                    .touchSlop(vertical = 11.dp, horizontal = 2.dp)
                    .selectable(selected = on, role = Role.RadioButton) { onSelect(o) }
                    .padding(vertical = 11.dp, horizontal = 2.dp)
                    .heightIn(min = 26.dp)
                    .then(if (on) Modifier.border(1.dp, OpenColors.Rule, OpenCorner) else Modifier)
                    .padding(horizontal = 10.dp),
                contentAlignment = Alignment.Center
            ) {
                Text(label(o).uppercase(), style = OpenType.monoS, color = if (on) MaterialTheme.colorScheme.onBackground else OpenColors.Glyph)
            }
        }
    }
}

/**
 * One goal on a scoreboard (Home, Coach's plan): its name and target as a mono label, where it
 * stands as a serif figure (a count keeps its denominator full size, muted), the meter, and what
 * moved. Three share a row.
 */
@Composable
fun OpenGoalColumn(
    label: String,
    figure: String,
    denominator: String,
    fraction: Float,
    change: String?,
    changeMoved: Boolean,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val ink = MaterialTheme.colorScheme.onBackground
    androidx.compose.foundation.layout.Column(
        modifier
            .clickableLabeled(description, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = description },
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        OpenLabel(label, style = OpenType.monoSTight, modifier = Modifier.clearAndSetSemantics { })
        Text(
            androidx.compose.ui.text.buildAnnotatedString {
                append(figure)
                if (denominator.isNotEmpty()) {
                    pushStyle(OpenType.figureMDenominator.toSpanStyle().copy(color = muted)); append(denominator); pop()
                }
            },
            style = OpenType.figureM,
            color = ink,
            modifier = Modifier.clearAndSetSemantics { }
        )
        OpenMeter(fraction)
        if (change != null) {
            OpenLabel(change, style = OpenType.monoSTight, color = if (changeMoved) ink else muted, modifier = Modifier.clearAndSetSemantics { })
        }
    }
}
