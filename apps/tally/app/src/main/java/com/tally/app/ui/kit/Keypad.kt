package com.tally.app.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tally.app.ui.theme.Tally

/** One cell of [KeyGrid]: its label (spoken too), or the backspace glyph, and what it does. */
data class GridKey(val label: String, val backspace: Boolean = false, val press: () -> Unit)

/**
 * Relay's keypad: slab keys with an edge, Geist figures, 48dp rows; a null cell keeps the grid's
 * columns (a currency without minor units has no separator).
 */
@Composable
fun KeyGrid(rows: List<List<GridKey?>>, modifier: Modifier = Modifier) {
    val c = Tally.colors
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        rows.forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                row.forEach { key ->
                    if (key == null) {
                        Box(Modifier.weight(1f).height(48.dp))
                    } else {
                        Box(
                            Modifier.weight(1f).height(48.dp).clip(Radii.key).background(c.slab).border(1.dp, c.edge, Radii.key)
                                .clickable(role = Role.Button, onClick = key.press)
                                .semantics { contentDescription = if (key.backspace) "Delete" else key.label },
                            contentAlignment = Alignment.Center,
                        ) {
                            if (key.backspace) Glyph("arrow-left", 18.dp, c.ink2) else T(key.label, style = Tally.type.title.copy(fontSize = 20.sp), color = c.ink)
                        }
                    }
                }
            }
        }
    }
}
