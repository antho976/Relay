package com.quietsoftware.relay.ui.tools

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.asStr
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.put
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/**
 * An act from a screen that shows its own progress (a clone, a provider check): [busy] is true
 * while it runs and false again however it ends. A failure is a toast, worded as the shell's acts.
 */
internal fun CoroutineScope.pcCall(
    nav: Nav,
    op: String,
    payload: JsonObject = JsonObject(emptyMap()),
    busy: (Boolean) -> Unit = {},
    done: String? = null,
    after: (JsonElement) -> Unit = {},
) {
    launch {
        busy(true)
        try {
            val answer = nav.relay.call(op, payload)
            done?.let { nav.shell.toast(it) }
            after(answer)
        } catch (e: BusException) {
            nav.shell.toast(if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code })
        } finally {
            busy(false)
        }
    }
}

/** A key at least a touch target tall; the kit's compact key is shorter than that. */
@Composable
internal fun ToolKey(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    kind: KeyKind = KeyKind.Plain,
    glyph: String? = null,
    enabled: Boolean = true,
) = Key(text, onClick, modifier.heightIn(min = TOUCH), kind, glyph, enabled)

/** `{project_id}` for the reads that take one. */
internal fun byProject(projectId: Long): JsonObject = buildJsonObject { put("project_id", projectId) }

/** A project's changed fields as a `project.update` payload: the id first, then only what changed. */
internal fun withProject(projectId: Long, changes: JsonObject): JsonObject = buildJsonObject {
    put("project_id", projectId)
    changes.forEach { (key, value) -> put(key, value) }
}

/** A byte count as the backups list shows it. */
internal fun formatBytes(bytes: Long): String = when {
    bytes < 1024 -> "$bytes B"
    bytes < 1024L * 1024 -> "%.1f KB".format(bytes / 1024.0)
    bytes < 1024L * 1024 * 1024 -> "%.1f MB".format(bytes / 1024.0 / 1024)
    else -> "%.2f GB".format(bytes / 1024.0 / 1024 / 1024)
}

private val STAMP = DateTimeFormatter.ofPattern("d MMM yyyy, HH:mm")

/** A moment as the phone's local date and time; empty when unknown. */
internal fun stamp(epochMs: Long): String =
    if (epochMs <= 0) "" else STAMP.format(Instant.ofEpochMilli(epochMs).atZone(ZoneId.systemDefault()))

internal fun providerTitle(id: String): String = when (id) {
    "claude" -> "Claude Code"
    "codex" -> "Codex"
    else -> id
}

/** A JSON number as a Double, if it is one. */
internal fun JsonElement.number(): Double? = (this as? JsonPrimitive)?.doubleOrNull

/** One entry per line, blanks dropped: how the PC's lists of patterns are edited. */
internal fun splitLines(text: String): List<String> = text.split('\n').map { it.trim() }.filter { it.isNotEmpty() }

internal fun JsonObject.strings(key: String): List<String> = arr(key).mapNotNull { it.asStr() }

/** A line under the page bar while the PC cannot be reached: the copy is shown, the acts are off. */
@Composable
internal fun OfflineNote(online: Boolean) {
    if (online) return
    Row(
        Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Dot(Relay.colors.waiting, 6.dp)
        T(
            "The PC is out of reach. This is the copy on the phone; the keys that need the PC are off until it is back.",
            style = Relay.type.caption,
            color = Relay.colors.ink3,
        )
    }
}

/** A short line of quiet text, for an empty list or a note. */
@Composable
internal fun Hint(text: String, modifier: Modifier = Modifier, color: androidx.compose.ui.graphics.Color = Relay.colors.ink3) =
    T(text, modifier.padding(horizontal = 10.dp, vertical = 10.dp), Relay.type.caption, color)

/** A field's name, its hint and the field itself. */
@Composable
internal fun FieldBlock(label: String, hint: String? = null, field: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        T(label, style = Relay.type.uiMedium, color = Relay.colors.ink2)
        field()
        hint?.let { T(it, style = Relay.type.caption, color = Relay.colors.ink3) }
    }
}

/** A setting on one line: its name and what it does, and a switch at the end; the whole line takes the tap. */
@Composable
internal fun SwitchLine(title: String, hint: String?, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().heightIn(min = TOUCH).switchRow(checked, enabled, onChange).padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(title, style = Relay.type.body, color = c.ink, weight = FontWeight.Medium, maxLines = 2)
            hint?.let { T(it, style = Relay.type.caption, color = c.ink3) }
        }
        Spacer(Modifier.width(12.dp))
        Toggle(checked, null, enabled = enabled)
    }
}
