package com.forge.app.ui.cardio.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.forge.app.domain.cardio.CardioEffort
import com.forge.app.domain.cardio.CardioRestReason
import com.forge.app.domain.cardio.CardioType
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun CardioLogSheet(
    onDismiss: () -> Unit,
    onLog: (
        type: CardioType,
        durationMin: Int,
        distanceKm: Double?,
        effort: CardioEffort?,
        restReason: CardioRestReason?,
        note: String?
    ) -> Unit
) {
    var type by remember { mutableStateOf(CardioType.RUN) }
    var durationText by remember { mutableStateOf("") }
    var distanceText by remember { mutableStateOf("") }
    var effort by remember { mutableStateOf<CardioEffort?>(null) }
    var restReason by remember { mutableStateOf<CardioRestReason?>(null) }
    var note by remember { mutableStateOf("") }

    val durationInt = durationText.toIntOrNull() ?: 0
    val distanceDouble = distanceText.toDoubleOrNull()
    val canSubmit = if (type.isRest) restReason != null else durationInt > 0

    val onBg = MaterialTheme.colorScheme.onBackground
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    val outline = MaterialTheme.colorScheme.outline
    val bg = MaterialTheme.colorScheme.background
    val accent = MaterialTheme.colorScheme.primary

    val dateHeader = remember {
        val now = Date()
        val day = SimpleDateFormat("EEE", Locale.getDefault()).format(now).uppercase().take(3)
        val date = SimpleDateFormat("MMM d", Locale.getDefault()).format(now).uppercase()
        val time = SimpleDateFormat("H:mm", Locale.getDefault()).format(now)
        "$day · $date · $time"
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text("•", style = MaterialTheme.typography.bodyMedium, color = muted)
                        Text("Forge", style = MaterialTheme.typography.bodyMedium, color = onBg, fontStyle = FontStyle.Italic)
                    }
                },
                navigationIcon = {
                    IconButton(onClick = onDismiss) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back", tint = muted)
                    }
                },
                actions = {
                    Text(
                        "NEW ENTRY",
                        style = MaterialTheme.typography.labelSmall,
                        letterSpacing = 2.sp,
                        color = muted,
                        modifier = Modifier.padding(end = 16.dp)
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        containerColor = Color.Transparent
    ) { inner ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(inner),
            contentPadding = PaddingValues(bottom = 48.dp)
        ) {
            item("hero") {
                CardioLogHeroItem(dateHeader = dateHeader, muted = muted, onBg = onBg, outline = outline)
            }

            item("type") {
                FormSection(label = "What kind?", optional = false, muted = muted, onBg = onBg, outline = outline) {
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        CardioType.entries.forEach { t ->
                            PillChip(
                                label = t.code.uppercase(),
                                selected = type == t,
                                onClick = { type = t },
                                onBg = onBg, bg = bg, muted = muted, outline = outline
                            )
                        }
                    }
                }
            }

            if (!type.isRest) {
                item("duration") {
                    FormSection(label = "For how long?", optional = false, muted = muted, onBg = onBg, outline = outline) {
                        NumberInputRow(
                            value = durationText,
                            onValueChange = { durationText = it.filter(Char::isDigit).take(4) },
                            placeholder = "30",
                            unit = "minutes",
                            keyboardType = KeyboardType.Number,
                            onBg = onBg, muted = muted, accent = accent, outline = outline
                        )
                    }
                }

                item("distance") {
                    FormSection(label = "How far?", optional = true, muted = muted, onBg = onBg, outline = outline) {
                        NumberInputRow(
                            value = distanceText,
                            onValueChange = { distanceText = sanitizeDecimal(it) },
                            placeholder = "0",
                            unit = "kilometres",
                            keyboardType = KeyboardType.Decimal,
                            onBg = onBg, muted = muted, accent = accent, outline = outline
                        )
                    }
                }

                item("effort") {
                    FormSection(label = "How hard?", optional = true, muted = muted, onBg = onBg, outline = outline) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            CardioEffort.entries.forEach { e ->
                                PillChip(
                                    label = e.displayName.uppercase(),
                                    selected = effort == e,
                                    onClick = { effort = if (effort == e) null else e },
                                    onBg = onBg, bg = bg, muted = muted, outline = outline
                                )
                            }
                        }
                    }
                }
            } else {
                item("rest-reason") {
                    FormSection(label = "What kind of rest?", optional = false, muted = muted, onBg = onBg, outline = outline) {
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            CardioRestReason.entries.forEach { r ->
                                PillChip(
                                    label = r.displayName.uppercase(),
                                    selected = restReason == r,
                                    onClick = { restReason = r },
                                    onBg = onBg, bg = bg, muted = muted, outline = outline
                                )
                            }
                        }
                    }
                }
            }

            item("note") {
                FormSection(label = "How did it feel?", optional = true, muted = muted, onBg = onBg, outline = outline) {
                    BasicTextField(
                        value = note,
                        onValueChange = { note = it.take(300) },
                        textStyle = MaterialTheme.typography.bodyMedium.copy(color = onBg),
                        cursorBrush = SolidColor(accent),
                        minLines = 2,
                        maxLines = 5,
                        decorationBox = { inner ->
                            Box {
                                if (note.isEmpty()) {
                                    Text(
                                        "jot a few words...",
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = muted.copy(alpha = 0.45f),
                                        fontStyle = FontStyle.Italic
                                    )
                                }
                                inner()
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }

            item("actions") {
                Spacer(Modifier.height(8.dp))
                Row(
                    modifier = Modifier.padding(horizontal = 24.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(20.dp)
                ) {
                    OutlinedButton(
                        onClick = {
                            onLog(
                                type,
                                if (type.isRest) 0 else durationInt,
                                if (type.isRest) null else distanceDouble,
                                if (type.isRest) null else effort,
                                if (type.isRest) restReason else null,
                                note.ifBlank { null }
                            )
                        },
                        enabled = canSubmit,
                        shape = RoundedCornerShape(50),
                        border = androidx.compose.foundation.BorderStroke(
                            width = 1.5.dp,
                            color = if (canSubmit) onBg else onBg.copy(alpha = 0.3f)
                        ),
                        colors = ButtonDefaults.outlinedButtonColors(
                            contentColor = onBg,
                            disabledContentColor = onBg.copy(alpha = 0.4f)
                        ),
                        contentPadding = PaddingValues(horizontal = 24.dp, vertical = 14.dp)
                    ) {
                        Text(
                            if (type.isRest) "Save rest day →" else "Save entry →",
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.SemiBold
                        )
                    }
                    TextButton(onClick = onDismiss) {
                        Text("cancel", style = MaterialTheme.typography.bodySmall, color = muted)
                    }
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

