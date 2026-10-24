package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.text.TextRange
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
import com.quietsoftware.relay.ui.dev.MAX_PICTURES
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
 * The message box, as Claude's app draws its bar ([TalkBar]): the words on top, growing to about
 * eight lines, and under them a round key for pictures and one for the effort on the left, the
 * model on the right, and the round key: clay to send, the microphone with nothing typed, a stop
 * square while the agent works. [hint] is the placeholder: an open question on a new thread, what
 * the thread is about in an open one.
 *
 * With [onPick], the picture key leads: the [pictures] picked show over the text, each sent to the
 * PC as it is picked, so the key needs the PC ([online]). A model on the PC itself ([local])
 * cannot read them, so on one the key steps aside and the box says why; it has no effort either.
 * The model and the effort wait while a reply is written (`thread.set` closes a running agent).
 *
 * [limits] is the line on what the agent may do, under the bar.
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
    // The text with its cursor: a change from outside (Edit, dictation) puts the cursor at its end.
    var field by remember { mutableStateOf(TextFieldValue(value, TextRange(value.length))) }
    val shown = if (field.text == value) field else TextFieldValue(value, TextRange(value.length))
    val reads = local.readsPictures(model)
    val onPc = local.has(model)
    // The models a thread may run on, the PC's own under their heading.
    val models = remember(local) { MODELS + local.present }
    val groups = remember(local) { local.present.firstOrNull()?.let { mapOf(it.first to "On this PC") }.orEmpty() }
    val empty = value.isBlank() && pictures.isEmpty()
    val key = when {
        working -> BarKey.Stop
        empty && onDictate != null -> BarKey.Speak
        else -> BarKey.Send
    }
    var choosingModel by remember { mutableStateOf(false) }
    var choosingEffort by remember { mutableStateOf(false) }
    Column(modifier.fillMaxWidth()) {
        TalkBar(
            field = shown,
            onField = {
                field = it
                if (it.text != value) onValue(it.text)
            },
            placeholder = hint,
            key = key,
            keyEnabled = when (key) {
                BarKey.Stop, BarKey.Speak -> true
                else -> canSend && !empty
            },
            onKey = {
                when (key) {
                    BarKey.Stop -> onStop()
                    BarKey.Speak -> onDictate?.invoke()
                    else -> onSend()
                }
            },
            focus = focus,
            above = { if (pictures.isNotEmpty()) AttachmentStrip(pictures, onUnpick) },
            trailing = {
                // `thread.set` closes a running agent: a choice waits for the reply to end, and says so.
                val spoken = "Model: ${modelChip(model, local)}" + if (onPick != null && !reads) ". $PICTURES_NEED_CLAUDE" else ""
                BarLabel(modelChip(model, local), spoken, enabled = canPick, whyOff = if (working) { { nav.shell.toast(WAITS) } } else null, onClick = { choosingModel = true }) {
                    Choices(choosingModel, { choosingModel = false }, models, model, onModel, above = true, anchor = TOUCH, groups = groups)
                }
            },
        ) {
            if (onPick != null && reads) BarButton(
                "plus",
                if (online) "Attach a photo or a screenshot" else "Attach a photo or a screenshot: needs the PC",
                enabled = online && pictures.size < MAX_PICTURES,
                whyOff = { nav.shell.toast(if (!online) "Pictures go to the PC as they are picked, and it is out of reach" else "Up to $MAX_PICTURES pictures in one message") },
                onClick = { onPick() },
            )
            Box {
                BarButton(
                    "sliders",
                    "Effort: ${effortName(effort)}",
                    enabled = canPick && !onPc,
                    whyOff = when {
                        onPc -> { { nav.shell.toast("A model on this PC has no effort setting") } }
                        working -> { { nav.shell.toast(WAITS) } }
                        else -> null
                    },
                    onClick = { choosingEffort = true },
                )
                Choices(choosingEffort, { choosingEffort = false }, EFFORTS, effort, onEffort, above = true, anchor = TOUCH)
            }
            // A chosen effort shows beside its key, as Claude's app names a tool that is on.
            if (effort.isNotEmpty() && !onPc) T(effortChip(effort), Modifier.padding(start = 2.dp), Relay.type.caption, c.ink3, maxLines = 1)
        }
        val caption = Relay.type.caption.copy(fontSize = 11.5.sp, textAlign = TextAlign.Center)
        T(limits, Modifier.fillMaxWidth().padding(top = 6.dp), caption, c.inkDim, maxLines = 1)
        if (onPick != null && !reads) {
            T(if (onPc) LOCAL_CANNOT_READ else "$PICTURES_NEED_CLAUDE: this one runs on the PC itself", Modifier.fillMaxWidth().padding(top = 2.dp), caption, c.ink3, maxLines = 2)
        }
    }
}
