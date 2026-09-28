package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.forge.app.ui.common.window.Dialog
import com.forge.app.domain.cardio.CardioGlyphs
import com.forge.app.domain.cardio.CustomCardioType
import com.forge.app.ui.common.ForgeFieldRow
import com.forge.app.ui.common.ForgeGroupSection
import com.forge.app.ui.common.ForgePrimaryCapsule
import com.forge.app.ui.common.ForgeRowGroup
import com.forge.app.ui.common.ForgeSecondaryCapsule
import com.forge.app.ui.common.ForgeTileGrid

/**
 * Create or edit a custom cardio activity (GYMAP-37) — a name + a glyph from the curated cardio set.
 * A modal (surface fill, rounded corners) with a tiny input, per DESIGN §3: the name typed inline
 * on one filled row, the glyphs as one connected pick-one grid. Reused by the log picker's
 * "add custom" flow and the Settings management page, so the two never drift.
 *
 * [initial] null = create (mints a fresh code on save); non-null = edit (keeps the code, so already
 * logged sessions stay attached). Save is disabled until the name is non-blank.
 */
@Composable
fun CustomActivityDialog(
    initial: CustomCardioType?,
    onDismiss: () -> Unit,
    onConfirm: (CustomCardioType) -> Unit,
) {
    // Saveable, not remembered: this dialog is opened from the cardio log sheet, whose Activity is
    // recreated on rotation, and a half-typed activity name coming back blank is the same lost
    // draft as the sheet behind it (M-12). Both values are plain strings, so the bundle carries them.
    var name by rememberSaveable(initial?.code) { mutableStateOf(initial?.name ?: "") }
    var glyphKey by rememberSaveable(initial?.code) { mutableStateOf(initial?.glyphKey ?: CardioGlyphs.DEFAULT_KEY) }

    val trimmed = name.trim()
    val canSave = trimmed.isNotEmpty()
    val focus = LocalFocusManager.current

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.surface
        ) {
            Column(
                Modifier.verticalScroll(rememberScrollState()).padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp)
            ) {
                Text(
                    if (initial == null) "New activity" else "Edit activity",
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onBackground
                )

                ForgeRowGroup({
                    ForgeFieldRow(
                        label = "Name",
                        value = name,
                        onValueChange = { name = it.take(CustomCardioType.MAX_NAME_LEN) },
                        placeholder = "Padel",
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Words,
                            keyboardType = KeyboardType.Text,
                            imeAction = ImeAction.Done
                        ),
                        keyboardActions = KeyboardActions(onDone = { focus.clearFocus() })
                    )
                })

                ForgeGroupSection("Icon") {
                    ForgeTileGrid(CardioGlyphs.catalog, cols = 4) { g, corners, modifier ->
                        CardioGlyphTile(
                            icon = g.icon,
                            label = g.key,
                            selected = g.key == glyphKey,
                            corners = corners,
                            onClick = { glyphKey = g.key },
                            modifier = modifier
                        )
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    ForgeSecondaryCapsule(label = "Cancel", onClick = onDismiss, modifier = Modifier.weight(1f))
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
