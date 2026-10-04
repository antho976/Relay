package com.tally.app.ui.entry

import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Backspace
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.tally.app.ui.common.bounceClick

/** The seam between keys, the grouped kit's 2dp gap. */
private val KEYPAD_GAP = 2.dp

/**
 * The amount keypad: twelve raised slab keys in a 3 by 4 grid, digits in the sans title voice,
 * the locale's own decimal separator, and a drawn backspace. It is the hot path's first touch, so
 * every key is a full-cell target that grows with the font instead of a fixed square.
 */
@Composable
internal fun Keypad(
    showDecimal: Boolean,
    separator: Char,
    onKey: (KeypadKey) -> Unit,
    modifier: Modifier = Modifier,
) {
    val rows = remember(showDecimal) { keypadRows(showDecimal) }
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(KEYPAD_GAP)) {
        rows.forEach { row ->
            Row(
                Modifier.fillMaxWidth().height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(KEYPAD_GAP),
            ) {
                row.forEach { key ->
                    if (key == null) {
                        Spacer(Modifier.weight(1f))
                    } else {
                        KeypadButton(key, separator, onKey, Modifier.weight(1f).fillMaxHeight())
                    }
                }
            }
        }
    }
}

@Composable
private fun KeypadButton(
    key: KeypadKey,
    separator: Char,
    onKey: (KeypadKey) -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(12.dp)
    Box(
        modifier
            .heightIn(min = 52.dp)
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .bounceClick(role = Role.Button) { onKey(key) }
            .then(if (key == KeypadKey.Decimal) Modifier.semantics { contentDescription = "Decimal point" } else Modifier)
            .padding(vertical = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        when (key) {
            is KeypadKey.Digit -> Text(
                key.value.toString(),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            KeypadKey.Decimal -> Text(
                separator.toString(),
                modifier = Modifier.clearAndSetSemantics { },
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onBackground,
            )
            KeypadKey.Backspace -> Icon(
                Icons.AutoMirrored.Rounded.Backspace,
                contentDescription = "Delete digit",
                tint = MaterialTheme.colorScheme.onBackground,
            )
        }
    }
}
