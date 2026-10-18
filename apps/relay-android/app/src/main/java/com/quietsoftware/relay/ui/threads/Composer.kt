package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.theme.Relay

/** The models a thread may run on (threads_view.rs `MODELS`): an empty id is Claude's own default. */
internal val MODELS = listOf(
    "" to "Default model",
    "claude-opus-5-5" to "Opus 5.5",
    "claude-sonnet-5-5" to "Sonnet 5.5",
    "claude-haiku-5-5" to "Haiku 5.5",
    "claude-fable-5-1" to "Fable 5.1",
)

/** How hard the agent thinks, as `claude --effort` takes it (threads_view.rs `EFFORTS`). */
internal val EFFORTS = listOf(
    "" to "Default effort",
    "low" to "Low effort",
    "medium" to "Medium effort",
    "high" to "High effort",
    "xhigh" to "Extra high effort",
    "max" to "Max effort",
)

private fun modelChip(id: String) = if (id.isEmpty()) "Default" else MODELS.firstOrNull { it.first == id }?.second ?: id.removePrefix("claude-")

private fun effortChip(id: String) = if (id.isEmpty()) "Effort" else EFFORTS.firstOrNull { it.first == id }?.second?.removeSuffix(" effort") ?: id

/**
 * The message box (threads.css `.threads-composer`): raised, a strong edge that lights when
 * focused, the text growing to about six lines, and under it the model and effort chips, what
 * the agent may do, and the send key, which stops the reply while the agent works.
 */
@Composable
internal fun Composer(
    value: String,
    onValue: (String) -> Unit,
    model: String,
    effort: String,
    onModel: (String) -> Unit,
    onEffort: (String) -> Unit,
    working: Boolean,
    onSend: () -> Unit,
    onStop: () -> Unit,
    modifier: Modifier = Modifier,
    canSend: Boolean = true,
    canPick: Boolean = true,
    onDictate: (() -> Unit)? = null,
) {
    val c = Relay.colors
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val wide = maxWidth >= 440.dp
        Column {
            Column(
                Modifier.fillMaxWidth().clip(Radii.card).background(c.raised)
                    .border(1.dp, if (focused) c.lineFocus else c.strong, Radii.card)
                    .padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 6.dp),
            ) {
                BasicTextField(
                    value = value,
                    onValueChange = onValue,
                    modifier = Modifier.fillMaxWidth().padding(end = 8.dp).heightIn(min = 24.dp),
                    textStyle = Relay.type.body.copy(color = c.ink),
                    cursorBrush = SolidColor(c.ink),
                    minLines = 1,
                    maxLines = 6,
                    interactionSource = source,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    decorationBox = { inner ->
                        Box {
                            if (value.isEmpty()) T("What do you want to know?", style = Relay.type.body, color = c.ink3, maxLines = 1)
                            inner()
                        }
                    },
                )
                Row(Modifier.fillMaxWidth().padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Picker("claude", modelChip(model), MODELS, model, onModel, canPick)
                    Picker("sliders", effortChip(effort), EFFORTS, effort, onEffort, canPick)
                    if (wide) T("Edits with Undo", Modifier.padding(start = 4.dp), Relay.type.caption, c.ink3, maxLines = 1)
                    Box(Modifier.weight(1f))
                    if (onDictate != null && !working) {
                        Box(
                            Modifier.size(TOUCH).clip(Radii.key).clickable(onClickLabel = "Dictate", role = Role.Button, onClick = onDictate),
                            contentAlignment = Alignment.Center,
                        ) { com.quietsoftware.relay.ui.dev.MicGlyph(18.dp, c.ink3) }
                    }
                    SendKey(working, enabled = working || (canSend && value.isNotBlank()), onClick = { if (working) onStop() else onSend() })
                }
            }
            if (!wide) T("Edits with Undo · deleting asks you first", Modifier.fillMaxWidth().padding(top = 6.dp), Relay.type.caption.copy(fontSize = 11.5.sp, textAlign = TextAlign.Center), c.ink3, maxLines = 1)
        }
    }
}

/** A chip that opens its list of choices above it. */
@Composable
private fun Picker(glyph: String, caption: String, options: List<Pair<String, String>>, selected: String, onPick: (String) -> Unit, enabled: Boolean) {
    val c = Relay.colors
    var open by remember { mutableStateOf(false) }
    Box(
        Modifier.heightIn(min = TOUCH).clickable(enabled = enabled, role = Role.Button) { open = true },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier.alpha(if (enabled) 1f else .45f).heightIn(min = 30.dp).clip(Radii.pill).background(c.slab).border(1.dp, c.strong, Radii.pill).padding(start = 8.dp, end = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(5.dp),
        ) {
            Glyph(glyph, 13.dp, c.ink3)
            T(caption, style = Relay.type.caption, color = c.ink2, maxLines = 1, weight = FontWeight.Medium)
            Glyph("chevron-down", 11.dp, c.ink3)
        }
        Choices(open, { open = false }, options, selected, onPick, above = true, anchor = TOUCH)
    }
}

/** The send key: 34dp, the surface's one primary; `stop` while the agent works. */
@Composable
private fun SendKey(working: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val c = Relay.colors
    Box(
        Modifier.size(TOUCH).clip(Radii.key).clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = if (working) "Stop the reply" else "Send" },
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(34.dp).clip(Radii.key).background(if (enabled) c.ink else c.track), contentAlignment = Alignment.Center) {
            Glyph(if (working) "stop" else "arrow-up", 16.dp, if (enabled) c.wall else c.ink3)
        }
    }
}
