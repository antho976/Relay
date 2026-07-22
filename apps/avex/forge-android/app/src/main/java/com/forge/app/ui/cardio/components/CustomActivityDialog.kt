package com.forge.app.ui.cardio.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.forge.app.domain.cardio.CardioGlyphs
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.ui.common.ForgeOutlineCapsule
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.bounceClick

/**
 * Create or edit a custom cardio activity (GYMAP-37) — a name + a glyph from the curated cardio set.
 * A modal (surface fill, rounded corners) with a tiny input, per DESIGN §3. Reused by the log picker's
 * "add custom" flow and the Settings management page, so the two never drift.
 *
 * [initial] null = create (mints a fresh code on save); non-null = edit (keeps the code, so already
 * logged sessions stay attached). Save is disabled until the name is non-blank.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CustomActivityDialog(
    initial: CustomCardioType?,
    onDismiss: () -> Unit,
    onConfirm: (CustomCardioType) -> Unit,
) {
    var name by remember { mutableStateOf(initial?.name ?: "") }
    var glyphKey by remember { mutableStateOf(initial?.glyphKey ?: CardioGlyphs.DEFAULT_KEY) }

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val accent = MaterialTheme.colorScheme.primary

    val trimmed = name.trim()
    val canSave = trimmed.isNotEmpty()

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.surface
        ) {
            Column(Modifier.padding(24.dp)) {
                Text(
                    if (initial == null) "New activity" else "Edit activity",
                    style = MaterialTheme.typography.titleLarge,
                    color = onBg
                )

                Spacer(Modifier.height(18.dp))
                Text("NAME", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it.take(CustomCardioType.MAX_NAME_LEN) },
                    singleLine = true,
                    placeholder = { Text("Padel", color = muted.copy(alpha = 0.45f)) },
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                        capitalization = KeyboardCapitalization.Words,
                        keyboardType = KeyboardType.Text,
                        imeAction = ImeAction.Done
                    ),
                    shape = RoundedCornerShape(12.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = accent,
                        unfocusedBorderColor = outline.copy(alpha = 0.35f),
                        cursorColor = accent,
                        focusedTextColor = onBg,
                        unfocusedTextColor = onBg
                    ),
                    modifier = Modifier.fillMaxWidth()
                )

                Spacer(Modifier.height(18.dp))
                Text("ICON", style = MaterialTheme.typography.labelSmall, color = muted, fontSize = 10.sp, letterSpacing = 1.sp)
                Spacer(Modifier.height(8.dp))
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    CardioGlyphs.catalog.forEach { g ->
                        val selected = g.key == glyphKey
                        Box(
                            modifier = Modifier
                                .size(44.dp)
                                .border(
                                    1.dp,
                                    if (selected) accent else outline.copy(alpha = 0.35f),
                                    RoundedCornerShape(12.dp)
                                )
                                .background(
                                    if (selected) accent.copy(alpha = 0.15f) else Color.Transparent,
                                    RoundedCornerShape(12.dp)
                                )
                                .bounceClick { glyphKey = g.key },
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(g.icon, contentDescription = g.key, tint = onBg, modifier = Modifier.size(20.dp))
                        }
                    }
                }

                Spacer(Modifier.height(24.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    ForgeOutlineCapsule(label = "Cancel", onClick = onDismiss, modifier = Modifier.weight(1f))
                    ForgePrimaryCapsule(
                        label = "Save",
                        onClick = {
                            val result = initial?.copy(name = trimmed, glyphKey = glyphKey)
                                ?: CustomCardioType.create(trimmed, glyphKey)
                            onConfirm(result)
                        },
                        enabled = canSave,
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        }
    }
}
