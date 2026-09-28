package com.forge.app.ui.gym.train.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.GROUP_OUTER
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.clickableLabeled
import com.forge.app.ui.common.rowShape

/**
 * Gym-local helpers for the grouped-surface look (`ui/common/ForgeGroups.kt`), shared by History,
 * Notes, Freestyle and Train. Kept here rather than in `ui/common` until a third feature needs them.
 */

/**
 * Member [index] of a [count]-row group drawn one item at a time (a lazy list can't hand its rows
 * to `ForgeRowGroup`): the raised fill, clipped to the group's outer or inner corners. The caller
 * spaces items by `GROUP_SEAM`.
 */
@Composable
internal fun Modifier.groupMember(index: Int, count: Int): Modifier =
    this
        .fillMaxWidth()
        .clip(rowShape(index, count))
        .background(MaterialTheme.colorScheme.surfaceContainerHigh)

/**
 * The filled rounded search row: magnifier, the field, and a clear capsule once there is text. No
 * outline; the raised fill is the affordance.
 */
@Composable
internal fun GymSearchRow(
    query: String,
    onQueryChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .clip(RoundedCornerShape(GROUP_OUTER))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .padding(start = ROW_H, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Icon(Icons.Filled.Search, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
        BasicTextField(
            value = query,
            onValueChange = onQueryChange,
            textStyle = MaterialTheme.typography.bodyLarge.copy(color = onBg),
            cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
            singleLine = true,
            modifier = Modifier
                .weight(1f)
                .padding(vertical = 14.dp)
                .semantics { contentDescription = placeholder },
            decorationBox = { inner ->
                Box {
                    if (query.isEmpty()) {
                        // A placeholder is the one text allowed under the muted floor (§5).
                        Text(placeholder, style = MaterialTheme.typography.bodyLarge, color = muted.copy(alpha = 0.6f))
                    }
                    inner()
                }
            }
        )
        if (query.isNotEmpty()) {
            Box(
                Modifier
                    .minimumInteractiveComponentSize()
                    .clip(RoundedCornerShape(50))
                    .clickableLabeled("Clear search") { onQueryChange("") },
                contentAlignment = Alignment.Center
            ) {
                Box(
                    Modifier
                        .size(28.dp)
                        .clip(RoundedCornerShape(50))
                        .background(MaterialTheme.colorScheme.surfaceContainerHighest),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(Icons.Filled.Close, contentDescription = null, tint = muted, modifier = Modifier.size(16.dp))
                }
            }
        } else {
            Box(Modifier.size(12.dp))
        }
    }
}

/**
 * A filled capsule with free content, for the live surfaces that need a glyph or a running time
 * inside (steppers, the stopwatch, the date). Same fill as `ForgeSecondaryCapsule`; the 48dp target
 * comes from [minHeight] and the padding, never a fixed height, so it grows with font scale.
 */
@Composable
internal fun GymFilledPill(
    onClick: (() -> Unit)?,
    label: String?,
    modifier: Modifier = Modifier,
    minHeight: Dp = 44.dp,
    fill: Color = MaterialTheme.colorScheme.surfaceContainerHigh,
    content: @Composable RowScope.() -> Unit
) {
    Row(
        modifier = modifier
            .heightIn(min = minHeight)
            .clip(RoundedCornerShape(50))
            .background(fill)
            .then(if (onClick != null) Modifier.bounceClick(onClick = onClick) else Modifier)
            .then(if (label != null) Modifier.semantics { contentDescription = label } else Modifier)
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
        content = content
    )
}

/**
 * `ForgeSecondaryCapsule` for a capsule that sits ON a raised surface (an entry slab, a card), where
 * the standard fill would vanish into its container: one rung up the surface ladder by default.
 */
@Composable
internal fun GymSecondaryCapsule(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
    enabled: Boolean = true,
    fill: Color = MaterialTheme.colorScheme.surfaceContainerHighest
) {
    val alpha = if (enabled) 1f else 0.35f
    Box(
        modifier = modifier
            .heightIn(min = 44.dp)
            .clip(RoundedCornerShape(50))
            .background(fill)
            .bounceClick(enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 13.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            label,
            style = MaterialTheme.typography.titleSmall,
            color = (if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onBackground)
                .copy(alpha = alpha),
            textAlign = androidx.compose.ui.text.style.TextAlign.Center
        )
    }
}

/**
 * A multi-line notes well: the one text input allowed to stay boxed (a big note), drawn with the
 * filled look instead of an outline. [fill] steps up a rung when the well sits on a raised surface
 * (a dialog, a slab).
 */
@Composable
internal fun GymNoteBox(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier,
    minLines: Int = 3,
    fill: Color = MaterialTheme.colorScheme.surfaceContainerHigh,
    textStyle: androidx.compose.ui.text.TextStyle = MaterialTheme.typography.bodyMedium,
    keyboardOptions: androidx.compose.foundation.text.KeyboardOptions = androidx.compose.foundation.text.KeyboardOptions.Default,
    keyboardActions: androidx.compose.foundation.text.KeyboardActions = androidx.compose.foundation.text.KeyboardActions.Default
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        minLines = minLines,
        textStyle = textStyle.copy(color = onBg),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        modifier = modifier.fillMaxWidth().semantics { contentDescription = placeholder },
        decorationBox = { inner ->
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(GROUP_OUTER))
                    .background(fill)
                    .padding(horizontal = ROW_H, vertical = 14.dp)
            ) {
                if (value.isEmpty()) {
                    Text(placeholder, style = textStyle, color = muted.copy(alpha = 0.6f))
                }
                inner()
            }
        }
    )
}
