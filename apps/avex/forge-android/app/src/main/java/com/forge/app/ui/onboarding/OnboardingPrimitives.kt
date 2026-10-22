package com.forge.app.ui.onboarding

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.ForgeSwitch
import com.forge.app.ui.common.OpenLabel
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.touchSlop
import com.forge.app.ui.theme.ForgeMotion
import com.forge.app.ui.theme.OnboardingType
import com.forge.app.ui.theme.OpenColors
import com.forge.app.ui.theme.OpenType
import com.forge.app.ui.theme.openOnAccentLarge
import kotlin.math.roundToInt

/**
 * The first run's building blocks, redrawn 2026-10-21 to the "Open panel" designs: a serif question,
 * one caption, choices as outlined tiles (the picked one in a red outline over a light red wash),
 * value answers as a big serif figure over a red-filled slider, and a footer that carries the plan
 * so far over one red action.
 */

private val TileCorner = RoundedCornerShape(4.dp)

/** The step's question: serif 36, the page's only headline. */
@Composable
internal fun StepTitle(text: String) {
    Text(text, style = OnboardingType.question, color = MaterialTheme.colorScheme.onBackground)
}

/** One quiet caption under the question. */
@Composable
internal fun StepCaption(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier = modifier, style = OpenType.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/** The question and its caption, with the design's 10dp between them. */
@Composable
internal fun StepHeader(title: String, caption: String) {
    Column {
        StepTitle(title)
        Spacer(Modifier.height(10.dp))
        StepCaption(caption)
    }
}

/** A mono section head inside a step (ABOUT YOU, YOUR WEEK) with an optional reading on the right. */
@Composable
internal fun StepSectionLabel(text: String, meta: String? = null) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OpenLabel(text, Modifier.weight(1f))
        if (meta != null) OpenLabel(meta)
    }
}

/**
 * The step rail: one 3dp segment per step of the path being walked. Done steps are red, the current
 * one ink, the rest a dark rung, so the short custom / freestyle path visibly drops what it skips.
 */
@Composable
internal fun StepRail(step: Int, total: Int, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    val ink = MaterialTheme.colorScheme.onBackground
    Row(
        modifier.semantics { contentDescription = "Step ${step + 1} of $total" },
        horizontalArrangement = Arrangement.spacedBy(3.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        repeat(total) { i ->
            val color by animateColorAsState(
                when {
                    i < step -> accent
                    i == step -> ink
                    else -> OpenColors.Unreached
                },
                ForgeMotion.standardTween(),
                label = "rail_cell"
            )
            Box(
                Modifier
                    .weight(1f)
                    .height(3.dp)
                    .clip(RoundedCornerShape(1.dp))
                    .background(color)
            )
        }
    }
}

/**
 * A choice: an outlined 4dp tile, the picked one outlined in red over a light red wash. Pick-one
 * tiles announce as radio buttons, pick-many ([multi]) as checkboxes.
 */
@Composable
internal fun OnbTile(
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    multi: Boolean = false,
    contentPadding: PaddingValues = PaddingValues(16.dp),
    content: @Composable ColumnScope.() -> Unit
) {
    val accent = MaterialTheme.colorScheme.primary
    val border by animateColorAsState(
        if (selected) accent else MaterialTheme.colorScheme.outline, ForgeMotion.standardTween(), label = "tile_edge"
    )
    val wash by animateColorAsState(
        if (selected) accent.copy(alpha = 0.15f) else accent.copy(alpha = 0f), ForgeMotion.standardTween(), label = "tile_wash"
    )
    Column(
        modifier
            .clip(TileCorner)
            .background(wash)
            .border(1.dp, border, TileCorner)
            .then(
                if (multi) Modifier.toggleable(value = selected, role = Role.Checkbox, onValueChange = { onClick() })
                else Modifier.selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            )
            .padding(contentPadding),
        content = content
    )
}

/** A tile's mono reading (8–12 REPS, 4 PIECES): ink on the picked tile, muted otherwise. */
@Composable
internal fun TileMeta(text: String, selected: Boolean, modifier: Modifier = Modifier) {
    OpenLabel(
        text, modifier,
        color = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
        style = OpenType.monoXS
    )
}

/** The tile body every list choice shares: name and what it changes on the left, a reading right. */
@Composable
internal fun TileRow(
    title: String,
    subtitle: String?,
    meta: String?,
    selected: Boolean,
    metaInk: Boolean = selected,
    icon: androidx.compose.ui.graphics.vector.ImageVector? = null,
) {
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (icon != null) {
            androidx.compose.material3.Icon(
                icon, contentDescription = null,
                tint = if (selected) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(24.dp)
            )
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(title, style = OnboardingType.tileTitle, color = MaterialTheme.colorScheme.onBackground)
            if (subtitle != null) {
                Text(subtitle, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (meta != null) TileMeta(meta, metaInk)
    }
}

/** A small one-of-few answer (lb, Male, 15): 36dp outlined, picked in red over the wash. */
@Composable
internal fun OnbChip(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier
            .touchSlop(vertical = 6.dp)
            .padding(vertical = 6.dp)
            .heightIn(min = 36.dp)
            .clip(TileCorner)
            .background(if (selected) accent.copy(alpha = 0.15f) else accent.copy(alpha = 0f))
            .border(1.dp, if (selected) accent else OpenColors.Rule, TileCorner)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(label, style = OnboardingType.chip, color = MaterialTheme.colorScheme.onBackground)
    }
}

/**
 * A value answer's slider: a 4dp track filled red to the knob, a dot on every stop, a white knob
 * ringed in red, and the stops named underneath. Tap a stop or drag along it. [selectedIndex] -1
 * draws no knob and no fill: the question is still unanswered.
 */
@Composable
internal fun OnbSlider(
    labels: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    valueDescription: (Int) -> String,
    modifier: Modifier = Modifier
) {
    val count = labels.size
    val accent = MaterialTheme.colorScheme.primary
    val ink = MaterialTheme.colorScheme.onBackground
    val latestSelect by rememberUpdatedState(onSelect)
    val latestIndex by rememberUpdatedState(selectedIndex)
    val knobAt by animateFloatAsState(
        selectedIndex.coerceAtLeast(0).toFloat(), ForgeMotion.standardTween(), label = "slider_knob"
    )
    Column(modifier.fillMaxWidth()) {
        Canvas(
            Modifier
                .fillMaxWidth()
                .height(26.dp)
                .touchSlop(vertical = 11.dp)
                .semantics {
                    stateDescription = if (selectedIndex >= 0) valueDescription(selectedIndex) else "Not set"
                    progressBarRangeInfo = ProgressBarRangeInfo(
                        selectedIndex.coerceAtLeast(0).toFloat(), 0f..(count - 1).toFloat(), steps = (count - 2).coerceAtLeast(0)
                    )
                    setProgress { target ->
                        val i = target.roundToInt().coerceIn(0, count - 1)
                        if (i != latestIndex) latestSelect(i)
                        true
                    }
                }
                .pointerInput(count) {
                    fun indexAt(x: Float): Int {
                        val inset = 11.dp.toPx()
                        val span = (size.width - 2 * inset).coerceAtLeast(1f)
                        return (((x - inset) / span) * (count - 1)).roundToInt().coerceIn(0, count - 1)
                    }
                    detectTapGestures { latestSelect(indexAt(it.x)) }
                }
                .pointerInput(count) {
                    fun indexAt(x: Float): Int {
                        val inset = 11.dp.toPx()
                        val span = (size.width - 2 * inset).coerceAtLeast(1f)
                        return (((x - inset) / span) * (count - 1)).roundToInt().coerceIn(0, count - 1)
                    }
                    detectHorizontalDragGestures { change, _ ->
                        val i = indexAt(change.position.x)
                        if (i != latestIndex) latestSelect(i)
                    }
                }
                // The slop above makes the whole 48dp band the target; the drawing keeps to 26dp.
                .padding(vertical = 11.dp)
        ) {
            val inset = 11.dp.toPx()
            val span = size.width - 2 * inset
            val cy = size.height / 2f
            fun xOf(i: Float) = inset + span * (i / (count - 1).coerceAtLeast(1))
            val trackH = 4.dp.toPx()
            drawRoundRect(
                OpenColors.Unreached, Offset(inset, cy - trackH / 2), Size(span, trackH), CornerRadius(trackH / 2)
            )
            val answered = selectedIndex >= 0
            if (answered) {
                drawRoundRect(
                    accent, Offset(inset, cy - trackH / 2), Size(xOf(knobAt) - inset, trackH), CornerRadius(trackH / 2)
                )
            }
            for (i in 0 until count) {
                val lit = answered && i <= selectedIndex
                drawCircle(if (lit) accent else OpenColors.Rule, 3.dp.toPx(), Offset(xOf(i.toFloat()), cy))
            }
            if (answered) {
                val c = Offset(xOf(knobAt), cy)
                drawCircle(ink, 11.dp.toPx(), c)
                drawCircle(accent, 9.5.dp.toPx(), c, style = Stroke(3.dp.toPx()))
            }
        }
        Spacer(Modifier.height(4.dp))
        SliderAxis(labels)
    }
}

/** The stops' names, each centred under its own stop and held inside the row at the two ends. */
@Composable
private fun SliderAxis(labels: List<String>) {
    Layout(
        content = {
            labels.forEach {
                Text(
                    it.uppercase(), style = OpenType.monoXS, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            }
        },
        modifier = Modifier.fillMaxWidth()
    ) { measurables, constraints ->
        val placeables = measurables.map { it.measure(constraints.copy(minWidth = 0, minHeight = 0)) }
        val width = constraints.maxWidth
        val inset = 11.dp.roundToPx()
        val span = width - 2 * inset
        val height = placeables.maxOfOrNull { it.height } ?: 0
        layout(width, height) {
            placeables.forEachIndexed { i, p ->
                val cx = inset + span * i / (labels.size - 1).coerceAtLeast(1)
                val x = (cx - p.width / 2).coerceIn(0, (width - p.width).coerceAtLeast(0))
                p.place(x, 0)
            }
        }
    }
}

/** A row that owns a switch: name and what it does on the left, the switch on the right. */
@Composable
internal fun OnbSwitchRow(
    label: String,
    description: String,
    checked: Boolean,
    onToggle: (Boolean) -> Unit,
    enabled: Boolean = true
) {
    Row(
        Modifier
            .fillMaxWidth()
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onToggle)
            .padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                label, style = OnboardingType.tileTitle,
                color = if (enabled) MaterialTheme.colorScheme.onBackground else MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(description, style = OpenType.bodyS, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        ForgeSwitch(
            checked = checked,
            onCheckedChange = null,
            enabled = enabled,
            uncheckedTrackColor = MaterialTheme.colorScheme.primary.copy(alpha = 0f),
            uncheckedThumbColor = OpenColors.Glyph,
            uncheckedBorderColor = OpenColors.Rule
        )
    }
}

/** The footer's "plan so far": a red dot and one mono line that grows with every answer. */
@Composable
internal fun PlanSoFar(text: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        Box(
            Modifier
                .padding(top = 4.dp)
                .size(6.dp)
                .background(MaterialTheme.colorScheme.primary, CircleShape)
        )
        // Wraps only between answers ("60 MIN · FULL GYM"), never inside one ("1 / SPOT FLAGGED").
        OpenLabel(
            text.split(" · ").joinToString("\u00A0· ") { it.replace(' ', '\u00A0') },
            color = MaterialTheme.colorScheme.onBackground,
            style = OpenType.monoXS
        )
    }
}

/**
 * The flow's one action: a 56dp red bar, the label centred in white. Held (not hidden) while the
 * step still needs an answer, drawn on the dark rung so it reads as waiting rather than broken.
 */
@Composable
internal fun OnbPrimaryButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) {
    Box(
        modifier
            .heightIn(min = 56.dp)
            .bounceClick(enabled = enabled, role = Role.Button, onClick = onClick)
            .clip(TileCorner)
            .background(if (enabled) MaterialTheme.colorScheme.primary else OpenColors.Unreached)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label, style = OpenType.button, textAlign = TextAlign.Center,
            color = if (enabled) openOnAccentLarge() else MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

/** The outlined secondary action beside the primary one (Re-roll). */
@Composable
internal fun OnbSecondaryButton(label: String, icon: ImageVector, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier
            .heightIn(min = 56.dp)
            .bounceClick(role = Role.Button, onClick = onClick)
            .clip(TileCorner)
            .border(1.dp, OpenColors.Rule, TileCorner)
            .padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onBackground, modifier = Modifier.size(16.dp))
        Text(label, style = OnboardingType.secondaryButton, color = MaterialTheme.colorScheme.onBackground)
    }
}
