package com.tally.app.ui.plan

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Backspace
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.Aside
import com.tally.app.ui.common.EndsRow
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.TextAction
import com.tally.app.ui.common.bounceClick
import com.tally.core.AmountInput
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/*
 * Pieces the Plan screens share: the hero's label row and figure, a row of small figures, the
 * zero-state line, the filled text field, the keypad, the date picker and the stepper button.
 */

/** The hero figure stops growing here, as Avex's hero does, so a 200% font keeps it on one line. */
private const val HERO_MAX_SCALE = 1.3f

/** The lit panel's opening row: glyph tile, the mono label, an optional reading at the end. */
@Composable
internal fun HeroLabel(
    label: String,
    modifier: Modifier = Modifier,
    end: String? = null,
    tint: Color = MaterialTheme.colorScheme.primary,
) {
    PanelHeader(label, modifier, meta = end, tint = tint)
}

/**
 * THE serif figure of a screen. Its font scale is capped and it steps down a size as the text
 * grows, so a long amount at 200% stays on one line instead of breaking mid-number.
 */
@Composable
internal fun HeroFigure(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onBackground,
    description: String? = null,
    live: Boolean = false,
) {
    val density = LocalDensity.current
    val scale = density.fontScale.coerceAtMost(HERO_MAX_SCALE)
    CompositionLocalProvider(LocalDensity provides Density(density.density, scale)) {
        Text(
            text,
            style = figureStyle(text.length, scale),
            color = color,
            modifier = modifier.semantics {
                if (description != null) contentDescription = description
                if (live) liveRegion = LiveRegionMode.Polite
            },
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

/** One reading in a [FigureRow]: the serif value over its mono label. */
@Immutable
internal data class FigureCell(
    val label: String,
    val value: String,
    val color: Color = Color.Unspecified,
)

/**
 * Two to four small figures side by side, serif values over mono labels. Past 1.3x font they
 * stack into label-and-value lines so no amount wraps; a value too wide to share its line with
 * the label's longest word drops under the label.
 */
@Composable
internal fun FigureRow(cells: List<FigureCell>, modifier: Modifier = Modifier) {
    val stacked = LocalDensity.current.fontScale > 1.3f
    val base = MaterialTheme.colorScheme.onBackground
    if (stacked) {
        Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            cells.forEach { cell ->
                // A long amount on a narrow phone takes the line; its label then sits over it
                // instead of breaking ("MONTH / LY").
                EndsRow(
                    start = {
                        Text(
                            cell.label.uppercase(),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    },
                    end = {
                        Text(
                            cell.value,
                            style = MaterialTheme.typography.headlineSmall,
                            color = if (cell.color == Color.Unspecified) base else cell.color,
                        )
                    },
                    modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) { },
                )
            }
        }
    } else {
        Row(
            modifier.fillMaxWidth().height(IntrinsicSize.Min),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            cells.forEach { cell ->
                Column(
                    Modifier.weight(1f).fillMaxHeight().semantics(mergeDescendants = true) { },
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Text(
                        cell.value,
                        style = MaterialTheme.typography.headlineSmall,
                        color = if (cell.color == Color.Unspecified) base else cell.color,
                    )
                    Text(
                        cell.label.uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

/** A zero state: one quiet line, and optionally the act that fills it. */
@Composable
internal fun EmptyLine(
    text: String,
    modifier: Modifier = Modifier,
    action: String? = null,
    onAction: (() -> Unit)? = null,
) {
    // The act drops under the line before a word of the line would break (200% font).
    EndsRow(
        start = { Aside(text) },
        end = if (action != null && onAction != null) {
            { TextAction(action, onAction, color = MaterialTheme.colorScheme.primary) }
        } else {
            null
        },
        modifier = modifier.fillMaxWidth(),
    )
}

/** A mono side label over a run of chips ("FROM", "TO"). */
@Composable
internal fun SideLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(),
        modifier = modifier,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/**
 * A filled rounded field with its glyph, the search-field look from Avex. Done closes the
 * keyboard. [isError] draws the error edge; the reason is a line under it, never inside.
 */
@Composable
internal fun PlanField(
    value: String,
    onChange: (String) -> Unit,
    placeholder: String,
    icon: ImageVector,
    modifier: Modifier = Modifier,
    keyboardType: KeyboardType = KeyboardType.Text,
    capitalization: KeyboardCapitalization = KeyboardCapitalization.None,
    suffix: String? = null,
    isError: Boolean = false,
) {
    val focus = LocalFocusManager.current
    val shape = RoundedCornerShape(16.dp)
    val edge = MaterialTheme.colorScheme.error
    BasicTextField(
        value = value,
        onValueChange = onChange,
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = placeholder },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onBackground),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = KeyboardOptions(
            capitalization = capitalization,
            keyboardType = keyboardType,
            imeAction = ImeAction.Done,
        ),
        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
        decorationBox = { inner ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .then(if (isError) Modifier.border(1.dp, edge, shape) else Modifier)
                    .heightIn(min = 56.dp)
                    .padding(horizontal = ROW_PAD, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                GlyphBadge(icon)
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) {
                        Text(
                            placeholder,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    inner()
                }
                if (suffix != null) {
                    Text(suffix, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
    )
}

/** A round step control (minus, plus) on the raised rung, so it reads on a slab. 44dp drawn, 48dp touch. */
@Composable
internal fun StepButton(icon: ImageVector, label: String, enabled: Boolean, onClick: () -> Unit) {
    val tint = MaterialTheme.colorScheme.onBackground.copy(alpha = if (enabled) 1f else 0.35f)
    Box(
        Modifier
            .minimumInteractiveComponentSize()
            .size(44.dp)
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            .bounceClick(enabled = enabled, label = label, role = Role.Button, focusShape = RoundedCornerShape(50), onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(22.dp))
    }
}

// ── Keypad ───────────────────────────────────────────────────────────────────

/** One key of the budget keypad. */
sealed interface PadKey {
    data class Digit(val value: Int) : PadKey
    data object Decimal : PadKey
    data object Backspace : PadKey
}

/** 1 2 3 / 4 5 6 / 7 8 9 / separator 0 backspace; no separator cell for a currency without minor units. */
internal fun padRows(showDecimal: Boolean): List<List<PadKey?>> = listOf(
    listOf(PadKey.Digit(1), PadKey.Digit(2), PadKey.Digit(3)),
    listOf(PadKey.Digit(4), PadKey.Digit(5), PadKey.Digit(6)),
    listOf(PadKey.Digit(7), PadKey.Digit(8), PadKey.Digit(9)),
    listOf(if (showDecimal) PadKey.Decimal else null, PadKey.Digit(0), PadKey.Backspace),
)

/** What one key press does to the typed amount. */
internal fun AmountInput.pressed(key: PadKey): AmountInput = when (key) {
    is PadKey.Digit -> digit(key.value)
    PadKey.Decimal -> decimal()
    PadKey.Backspace -> backspace()
}

private val PAD_GAP = 2.dp

/** The amount keypad, the entry screen's style: raised slab keys with 2dp seams, full-cell targets. */
@Composable
internal fun PlanKeypad(
    showDecimal: Boolean,
    separator: Char,
    onKey: (PadKey) -> Unit,
    modifier: Modifier = Modifier,
) {
    val rows = remember(showDecimal) { padRows(showDecimal) }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(PAD_GAP)) {
        rows.forEach { row ->
            Row(
                Modifier.fillMaxWidth().height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(PAD_GAP),
            ) {
                row.forEach { key ->
                    if (key == null) {
                        Spacer(Modifier.weight(1f))
                    } else {
                        PadButton(key, separator, onKey, Modifier.weight(1f).fillMaxHeight())
                    }
                }
            }
        }
    }
}

@Composable
private fun PadButton(key: PadKey, separator: Char, onKey: (PadKey) -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(12.dp)
    Box(
        modifier
            .heightIn(min = 52.dp)
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .bounceClick(role = Role.Button) { onKey(key) }
            .then(if (key == PadKey.Decimal) Modifier.semantics { contentDescription = "Decimal point" } else Modifier)
            .padding(vertical = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        when (key) {
            is PadKey.Digit -> Text(
                key.value.toString(),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            PadKey.Decimal -> Text(
                separator.toString(),
                modifier = Modifier.clearAndSetSemantics { },
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            PadKey.Backspace -> Icon(
                Icons.AutoMirrored.Rounded.Backspace,
                contentDescription = "Delete digit",
                tint = MaterialTheme.colorScheme.onBackground,
            )
        }
    }
}

// ── Date picker ──────────────────────────────────────────────────────────────

/**
 * The Material 3 calendar on the warm ladder. The picker speaks UTC midnight; the day is read back
 * in UTC so the picked calendar day never shifts by the zone offset.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun PlanDatePicker(
    date: LocalDate,
    onPick: (LocalDate) -> Unit,
    onDismiss: () -> Unit,
    confirmLabel: String = "Set date",
) {
    val pickerState = rememberDatePickerState(
        initialSelectedDateMillis = date.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
    )
    val colors = DatePickerDefaults.colors(
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        headlineContentColor = MaterialTheme.colorScheme.onBackground,
        weekdayContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
        dividerColor = MaterialTheme.colorScheme.outlineVariant,
    )
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(
                onClick = {
                    pickerState.selectedDateMillis?.let { picked ->
                        onPick(Instant.ofEpochMilli(picked).atZone(ZoneOffset.UTC).toLocalDate())
                    }
                    onDismiss()
                },
            ) { Text(confirmLabel) }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurfaceVariant),
            ) { Text("Cancel") }
        },
        colors = colors,
    ) {
        DatePicker(state = pickerState, colors = colors)
    }
}
