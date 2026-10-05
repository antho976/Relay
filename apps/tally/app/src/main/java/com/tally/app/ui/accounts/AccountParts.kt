package com.tally.app.ui.accounts

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.LiveRegionMode
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
import com.tally.app.ui.common.FIGURE_GAP
import com.tally.app.ui.common.GlyphBadge
import com.tally.app.ui.common.PanelHeader
import com.tally.app.ui.common.ROW_PAD
import com.tally.app.ui.common.slab

/*
 * Pieces the Accounts and Categories screens share: the lit panel's opening row, its serif
 * figure, a filled text field that sits in a slab group, the zero-state line and a pair of stat
 * tiles. Internal so the categories package can reach them; nothing outside these two uses them.
 */

/** The hero figure stops growing here, so a 200% font keeps it on one line. */
private const val HERO_MAX_SCALE = 1.3f

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
 * THE serif figure of a screen. Its font scale is capped and it steps down a size as the text
 * grows, so a long balance at 200% stays on one line instead of breaking mid-number.
 */
@Composable
internal fun HeroNumber(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onBackground,
    description: String? = null,
    live: Boolean = false,
) {
    val density = LocalDensity.current
    val scale = density.fontScale.coerceAtMost(HERO_MAX_SCALE)
    val style = heroStyle(text.length, scale)
    CompositionLocalProvider(LocalDensity provides Density(density.density, scale)) {
        Text(
            text,
            style = style,
            color = color,
            modifier = modifier.semantics {
                if (description != null) contentDescription = description
                if (live) liveRegion = LiveRegionMode.Polite
            },
        )
    }
}

@Composable
private fun heroStyle(length: Int, scale: Float): TextStyle {
    val type = MaterialTheme.typography
    val width = length * scale
    return when {
        width <= 9.5f -> type.displayLarge
        width <= 12f -> type.displayMedium
        width <= 15f -> type.headlineLarge
        else -> type.headlineMedium
    }
}

/**
 * A filled text field drawn as one member of a slab group: its glyph, the text, an optional
 * suffix. Done closes the keyboard. [isError] draws the error edge; the reason is the group's
 * footer, never inside the field.
 */
@Composable
internal fun FieldRow(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    icon: ImageVector,
    shape: Shape,
    modifier: Modifier = Modifier,
    keyboardType: KeyboardType = KeyboardType.Text,
    capitalization: KeyboardCapitalization = KeyboardCapitalization.None,
    suffix: String? = null,
    isError: Boolean = false,
) {
    val focus = LocalFocusManager.current
    val edge = MaterialTheme.colorScheme.error
    BasicTextField(
        value = value,
        onValueChange = onChange,
        modifier = modifier
            .fillMaxWidth()
            .semantics { contentDescription = label },
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
                    .slab(shape)
                    .then(if (isError) Modifier.border(1.dp, edge, shape) else Modifier)
                    .heightIn(min = 56.dp)
                    .padding(horizontal = ROW_PAD, vertical = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                GlyphBadge(icon)
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) {
                        Text(label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
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

/** A zero state inside a section: one quiet line. */
@Composable
internal fun EmptyNote(text: String, modifier: Modifier = Modifier) {
    Aside(text, modifier.fillMaxWidth().padding(vertical = 6.dp))
}

/** Two stat tiles side by side; stacked past 1.5x font so their figures never wrap mid-number. */
@Composable
internal fun TilePair(
    modifier: Modifier = Modifier,
    first: @Composable (Modifier) -> Unit,
    second: @Composable (Modifier) -> Unit,
) {
    if (LocalDensity.current.fontScale > 1.5f) {
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
