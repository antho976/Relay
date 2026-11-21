@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)

package com.forge.app.ui.recipes

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.filled.Search
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import com.forge.app.ui.common.ForgeTopBar
import com.forge.app.ui.common.GROUP_SEAM
import com.forge.app.ui.common.ROW_H
import com.forge.app.ui.common.rowShape
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.InlineEmptyHint
import com.forge.app.ui.common.bounceClick
import com.forge.app.ui.common.forgeItemMotion
import com.forge.app.ui.theme.ForgeTheme

/**
 * RECIPE — List / browser archetype (DESIGN §3).
 *
 * History, Goals, Trophies, pickers. Search-first and trim. The banned things matter more than the
 * present ones: NO charts, NO big serif hero, NO draw-in theatrics. A list is for finding, not for
 * admiring.
 *
 *   top bar (← + ≤1 action; the bell is Home only)
 *   ├─ tiny hero        title + ≤1–2 figures, not a display-size hero        §3
 *   ├─ search           bordered field, leading magnifier + trailing clear   §13
 *   └─ trim rows        light stagger only, whole-row tap                    §3, §9
 */
@Composable
fun ListRecipe(
    onBack: () -> Unit = {},
    all: List<String> = listOf(
        "Pull B · 24 Jul", "Push A · 22 Jul", "Legs · 20 Jul",
        "Pull A · 18 Jul", "Push B · 16 Jul", "Legs · 14 Jul",
    ),
) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val outline = MaterialTheme.colorScheme.outline

    var query by remember { mutableStateOf("") }
    val rows = remember(query, all) {
        if (query.isBlank()) all else all.filter { it.contains(query, ignoreCase = true) }
    }

    Scaffold(
        topBar = {
            // The shared bar: a filled back capsule, never the screen's name.
            ForgeTopBar(onBack = onBack)
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            Modifier.fillMaxSize().padding(inner),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 24.dp)
        ) {
            item {
                // §3: a TINY hero here. A list is not an overview — one line plus a count, no
                // display-size serif, no figure row.
                Spacer(Modifier.height(8.dp))
                Text("History", style = MaterialTheme.typography.headlineSmall, color = onBg)
                Spacer(Modifier.height(2.dp))
                Text(
                    "${all.size} SESSIONS",
                    style = MaterialTheme.typography.labelMedium,
                    color = muted
                )
                Spacer(Modifier.height(16.dp))
            }

            item {
                // Search is a filled rounded field on the group surface, never an outlined box
                // (grouped-surface kit, 2026-09-27). The placeholder may dim below the muted floor:
                // it is a ghost affordance, not content.
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 48.dp)
                        .clip(RoundedCornerShape(50))
                        .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                        .padding(horizontal = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    Icon(Icons.Filled.Search, contentDescription = null, tint = muted, modifier = Modifier.size(20.dp))
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyLarge.copy(color = onBg),
                        cursorBrush = SolidColor(accent),
                        modifier = Modifier.weight(1f).semantics { contentDescription = "Search" },
                        decorationBox = { field ->
                            Box {
                                if (query.isEmpty()) {
                                    Text("Search", style = MaterialTheme.typography.bodyLarge, color = muted.copy(alpha = 0.6f))
                                }
                                field()
                            }
                        }
                    )
                }
                Spacer(Modifier.height(16.dp))
            }

            if (rows.isEmpty()) {
                item {
                    // §12: a search with no hits has no zero-SHAPE to draw — there is no mark for
                    // "nothing matched". This is exactly the last-resort case InlineEmptyHint
                    // exists for, ≤1 per lens, replacing the caption rather than joining it.
                    Spacer(Modifier.height(16.dp))
                    InlineEmptyHint("No sessions match that", muted.copy(alpha = 0.65f))
                }
            } else {
                itemsIndexed(rows, key = { _, it -> it }) { i, row ->
                    // Lists get a light stagger only, never the overview's entrance cascade or a
                    // chart draw-in. `forgeItemMotion()` is an extension on LazyItemScope that
                    // RETURNS a Modifier. Each row is one member of a connected group: its own
                    // fill, 2dp seam, and the group's outer corners only at the ends.
                    SessionRow(
                        row,
                        forgeItemMotion()
                            .padding(bottom = GROUP_SEAM)
                            .clip(rowShape(i, rows.size))
                            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    )
                }
            }

            item { Spacer(Modifier.height(32.dp)) }
        }
    }
}

@Composable
private fun SessionRow(text: String, modifier: Modifier = Modifier) {
    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val (title, date) = text.split(" · ").let { it[0] to it.getOrElse(1) { "" } }
    Row(
        modifier
            .fillMaxWidth()
            // §2③: the WHOLE row is the tap target. Adding a button inside it would be a nested
            // tap, which is why "log again" lives on the detail page rather than on this row.
            .bounceClick { }
            .padding(horizontal = ROW_H, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // §14: no maxLines = 1 on user content — a long session name must wrap, not truncate.
        Text(title, style = MaterialTheme.typography.bodyMedium, color = onBg, modifier = Modifier.weight(1f))
        // §8: right meta is a reading or a count. Never a state word a dot already carries.
        Text(date, style = MaterialTheme.typography.labelSmall, color = muted.copy(alpha = 0.65f))
    }
}

@Preview(name = "List", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun ListRecipePreview() {
    ForgeTheme { ListRecipe() }
}

@Preview(name = "List · no results", showBackground = true, backgroundColor = 0xFF0E0E11)
@Composable
private fun ListRecipeEmptyPreview() {
    ForgeTheme { ListRecipe(all = emptyList()) }
}

@Preview(name = "List · 200% font", showBackground = true, backgroundColor = 0xFF0E0E11, fontScale = 2.0f)
@Composable
private fun ListRecipeLargeFontPreview() {
    ForgeTheme { ListRecipe() }
}
