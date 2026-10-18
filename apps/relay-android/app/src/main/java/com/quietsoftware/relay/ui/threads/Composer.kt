package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.dev.Attachment
import com.quietsoftware.relay.ui.dev.AttachmentStrip
import com.quietsoftware.relay.ui.dev.Chip
import com.quietsoftware.relay.ui.dev.MAX_PICTURES
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

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

/** What a thread's model is called on its chip: a model on the PC says so while it loads. */
private fun modelChip(id: String, local: LocalModels): String {
    if (id.isEmpty()) return "Default"
    MODELS.firstOrNull { it.first == id }?.let { return it.second }
    val name = local.name(id) ?: return id.removePrefix("claude-")
    return if (local.loading(id)) "$name · loading" else name
}

private fun effortName(id: String) = if (id.isEmpty()) "Default" else EFFORTS.firstOrNull { it.first == id }?.second?.removeSuffix(" effort") ?: id

private fun effortChip(id: String) = if (id.isEmpty()) "Effort" else effortName(id)

/** What the model and effort chips say while a reply is written (threads_view.rs `WAITS`). */
private const val WAITS = "Waits for this reply to finish"

/** Why a thread on a model on the PC takes no pictures (threads_pictures.rs `LOCAL_CANNOT`). */
internal const val LOCAL_CANNOT_READ = "This model on your PC cannot read pictures: choose a Claude model"

/** What a thread's agent may do, under the message box. */
internal const val DATA_LIMITS = "Edits with Undo · asks first"

/**
 * The models on the PC itself (`thread.models`, "On this PC" in the picker; docs/THREADS.md "Local
 * models"): those whose file is there, by id and name; every id it knows, since a thread may name
 * one whose file has gone; the one running, and whether it answers yet. [read] is false until the
 * PC has said, or on a PC too old to say.
 */
internal class LocalModels(val present: List<Pair<String, String>>, private val known: Set<String>, val running: String?, val ready: Boolean, val read: Boolean) {
    fun has(model: String?): Boolean = !model.isNullOrEmpty() && model in known

    fun name(model: String): String? = present.firstOrNull { it.first == model }?.second

    /** Picked, and not answering yet: the picker says "· loading" until it does. */
    fun loading(model: String?): Boolean = !model.isNullOrEmpty() && model == running && !ready

    /** Whether a thread on [model] can be shown pictures: Claude's read them, these do not. */
    fun readsPictures(model: String?): Boolean = if (read) !has(model) else seesPictures(model)

    companion object {
        val None = LocalModels(emptyList(), emptySet(), null, ready = false, read = false)

        fun of(o: JsonObject?): LocalModels {
            if (o == null) return None
            val list = o.objs("models")
            return LocalModels(
                list.filter { it.b("present") == true }.mapNotNull { m -> m.s("id")?.let { id -> id to (m.s("name") ?: id) } },
                list.mapNotNull { it.s("id") }.toSet(),
                o.s("running"),
                ready = o.b("ready") == true,
                read = true,
            )
        }
    }
}

/**
 * The PC's own models, read again on `thread.local` (one loading, ready, stopped). One that did not
 * load says why, as the desktop's picker does.
 */
@Composable
internal fun rememberLocalModels(): LocalModels {
    val nav = LocalNav.current
    val live by remember { nav.relay.live("thread.models") }.collectAsStateWithLifecycle(RelayData.Live())
    LaunchedEffect(Unit) {
        nav.relay.hub.session.collectLatest { bus ->
            bus?.events?.collect { e ->
                if (e.ev == "thread.local" && e.payload.s("state") == "failed") {
                    nav.shell.toast("${e.payload.s("name") ?: "The model"} did not load: ${e.payload.s("error")?.ifBlank { null } ?: "no reason given"}")
                }
            }
        }
    }
    return remember(live.result) { LocalModels.of(live.result as? JsonObject) }
}

/**
 * A model picked (threads_view.rs `choose`): one on the PC starts loading now (`thread.pick`), so
 * the next message waits less; any other stops the one running, which frees the graphics card.
 */
internal fun Nav.pickModel(model: String, local: LocalModels) {
    when {
        local.has(model) -> shell.act("thread.pick", buildJsonObject { put("model", model) })
        local.running != null -> shell.act("thread.pick", JsonObject(emptyMap()))
    }
}

/**
 * The message box (threads.css `.threads-composer`): raised, a strong edge that lights when
 * focused, the text growing to about six lines, and under it the model and effort chips, what
 * the agent may do, and the send key, which stops the reply while the agent works. [hint] is the
 * placeholder: an open question on a new thread, what the thread is about in an open one.
 *
 * With [onPick], a picture key leads the chips, as in an agent's conversation: the [pictures]
 * picked show over the text, each sent to the PC as it is picked, so the key needs the PC
 * ([online]). A model on the PC itself ([local]) cannot read them, so on one the key steps aside
 * and the box says why; it has no effort either.
 *
 * [limits] is the line on what the agent may do, under the box.
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
    focus: FocusRequester? = null,
    hint: String = "What do you want to know?",
    pictures: List<Attachment<*>> = emptyList(),
    onPick: (() -> Unit)? = null,
    onUnpick: (Long) -> Unit = {},
    online: Boolean = true,
    local: LocalModels = LocalModels.None,
    limits: String = DATA_LIMITS,
) {
    val c = Relay.colors
    val nav = LocalNav.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    // The text with its cursor: a change from outside (Edit, dictation) puts the cursor at its end.
    var field by remember { mutableStateOf(TextFieldValue(value, TextRange(value.length))) }
    val shown = if (field.text == value) field else TextFieldValue(value, TextRange(value.length))
    val reads = local.readsPictures(model)
    val onPc = local.has(model)
    // The models a thread may run on, the PC's own under their heading.
    val models = remember(local) { MODELS + local.present }
    val groups = remember(local) { local.present.firstOrNull()?.let { mapOf(it.first to "On this PC") }.orEmpty() }
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val wide = maxWidth >= 440.dp
        Column {
            Column(
                Modifier.fillMaxWidth().clip(Radii.card).background(c.raised)
                    .border(1.dp, if (focused) c.lineFocus else c.strong, Radii.card)
                    .padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 6.dp),
            ) {
                if (pictures.isNotEmpty()) AttachmentStrip(pictures, onUnpick)
                BasicTextField(
                    value = shown,
                    onValueChange = {
                        field = it
                        if (it.text != value) onValue(it.text)
                    },
                    modifier = Modifier.fillMaxWidth().padding(end = 8.dp).heightIn(min = 24.dp).then(if (focus != null) Modifier.focusRequester(focus) else Modifier),
                    textStyle = Relay.type.body.copy(color = c.ink),
                    cursorBrush = SolidColor(c.ink),
                    minLines = 1,
                    maxLines = 6,
                    interactionSource = source,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    decorationBox = { inner ->
                        Box {
                            if (value.isEmpty()) T(hint, style = Relay.type.body, color = c.ink3, maxLines = 1)
                            inner()
                        }
                    },
                )
                Row(Modifier.fillMaxWidth().padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    // The chips scroll on a narrow phone, as the agent chat's do; the keys stay put.
                    Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (onPick != null && reads) Chip(
                            "",
                            if (online) "Attach a photo or a screenshot" else "Attach a photo or a screenshot: needs the PC",
                            enabled = online && pictures.size < MAX_PICTURES,
                            glyph = "file-image",
                            whyOff = { nav.shell.toast(if (!online) "Pictures go to the PC as they are picked, and it is out of reach" else "Up to $MAX_PICTURES pictures in one message") },
                            onClick = { onPick() },
                        )
                        // `thread.set` closes a running agent: a choice waits for the reply to end, and says so.
                        val modelSpoken = "Model: ${modelChip(model, local)}" + if (onPick != null && !reads) ". $PICTURES_NEED_CLAUDE" else ""
                        Picker("claude", modelChip(model, local), modelSpoken, models, model, onModel, canPick, whyOff = WAITS.takeIf { working }, groups = groups)
                        Picker(
                            "sliders", effortChip(effort), "Effort: ${effortName(effort)}", EFFORTS, effort, onEffort, canPick && !onPc,
                            whyOff = if (onPc) "A model on this PC has no effort setting" else WAITS.takeIf { working },
                        )
                        if (wide) T(limits.substringBefore(" · "), Modifier.padding(start = 4.dp), Relay.type.caption, c.ink3, maxLines = 1)
                    }
                    if (onDictate != null && !working) {
                        Box(
                            Modifier.size(TOUCH).clip(Radii.key).clickable(onClickLabel = "Dictate", role = Role.Button, onClick = onDictate)
                                .semantics { contentDescription = "Dictate a message" },
                            contentAlignment = Alignment.Center,
                        ) { com.quietsoftware.relay.ui.dev.MicGlyph(18.dp, c.ink3) }
                    }
                    SendKey(working, enabled = working || (canSend && (value.isNotBlank() || pictures.isNotEmpty())), onClick = { if (working) onStop() else onSend() })
                }
            }
            val caption = Relay.type.caption.copy(fontSize = 11.5.sp, textAlign = TextAlign.Center)
            if (!wide) T(limits, Modifier.fillMaxWidth().padding(top = 6.dp), caption, c.ink3, maxLines = 1)
            if (onPick != null && !reads) {
                T(if (onPc) LOCAL_CANNOT_READ else "$PICTURES_NEED_CLAUDE: this one runs on the PC itself", Modifier.fillMaxWidth().padding(top = if (wide) 6.dp else 2.dp), caption, c.ink3, maxLines = 2)
            }
        }
    }
}

/**
 * A chip that opens its list of choices above it; [spoken] names it and its choice for a screen
 * reader. Off, a tap says [whyOff] (a reply to wait for) instead of nothing. [groups] heads the
 * options they name: the PC's own models.
 */
@Composable
private fun Picker(
    glyph: String,
    caption: String,
    spoken: String,
    options: List<Pair<String, String>>,
    selected: String,
    onPick: (String) -> Unit,
    enabled: Boolean,
    whyOff: String?,
    groups: Map<String, String> = emptyMap(),
) {
    val c = Relay.colors
    val nav = LocalNav.current
    var open by remember { mutableStateOf(false) }
    Box(
        Modifier.heightIn(min = TOUCH).clickable(enabled = enabled || whyOff != null, role = Role.Button) { if (enabled) open = true else whyOff?.let { nav.shell.toast(it) } }
            .semantics { contentDescription = spoken },
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
        Choices(open, { open = false }, options, selected, onPick, above = true, anchor = TOUCH, groups = groups)
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
