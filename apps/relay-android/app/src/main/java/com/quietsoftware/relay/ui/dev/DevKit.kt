package com.quietsoftware.relay.ui.dev

import android.content.ClipData
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.Clipboard
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Workspace
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** What `nav.shell.act` says with the PC away, for keys that check before they ask. */
internal const val NEEDS_PC = "Needs the PC, which is out of reach"

/** What a refused act says to the person. */
internal fun refusal(e: BusError): String = when {
    e.code == "link.down" -> NEEDS_PC
    e.kind == "held" -> "Held for approval on the PC. Decide it in the inbox."
    e.message.isNotBlank() -> e.message
    else -> e.code
}

/** `{"session": name}`, the payload most session ops take. */
internal fun named(name: String): JsonObject = buildJsonObject { put("session", name) }

internal suspend fun Clipboard.copy(text: String) = setClipEntry(ClipEntry(ClipData.newPlainText("relay", text)))

/**
 * A sheet from the bottom, the phone's form of the PC's popovers. [content] gets a `close` that
 * slides the sheet away and then runs what was chosen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun DevSheet(
    onDismiss: () -> Unit,
    title: String? = null,
    subtitle: String? = null,
    content: @Composable ColumnScope.(close: (() -> Unit) -> Unit) -> Unit,
) {
    val c = Relay.colors
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    val close: (() -> Unit) -> Unit = { then ->
        scope.launch { state.hide() }.invokeOnCompletion {
            onDismiss()
            then()
        }
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        shape = Radii.sheet,
        containerColor = c.slab,
        contentColor = c.ink,
        scrimColor = Color.Black.copy(alpha = .45f),
        dragHandle = { Box(Modifier.padding(top = 8.dp, bottom = 6.dp).size(width = 36.dp, height = 4.dp).clip(Radii.pill).background(c.track)) },
    ) {
        Column(Modifier.fillMaxWidth().imePadding().verticalScroll(rememberScrollState()).padding(start = 12.dp, end = 12.dp, bottom = 16.dp)) {
            if (title != null) {
                Column(Modifier.padding(start = 6.dp, end = 6.dp, bottom = 8.dp)) {
                    T(title, style = Relay.type.title, color = c.ink, maxLines = 2)
                    subtitle?.let { T(it, style = Relay.type.caption, color = c.ink3, maxLines = 3) }
                }
            }
            content(close)
        }
    }
}

/** One choice in a sheet's menu; a [dim] one still answers, to say why it cannot run. */
@Composable
internal fun SheetRow(
    label: String,
    glyph: String,
    onClick: () -> Unit,
    detail: String? = null,
    danger: Boolean = false,
    enabled: Boolean = true,
    selected: Boolean = false,
    dim: Boolean = false,
) {
    val c = Relay.colors
    ListRow(Modifier.alpha(if (enabled && !dim) 1f else .4f), selected = selected, onClick = if (enabled) onClick else null, padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)) {
        Glyph(glyph, 17.dp, if (danger) c.heldText else c.ink2)
        T(label, Modifier.weight(1f), Relay.type.ui, if (danger) c.heldText else c.ink, maxLines = 1)
        detail?.let { T(it, style = Relay.type.caption, color = c.ink3, maxLines = 1) }
        if (selected) Glyph("check", 15.dp, c.ink)
    }
}

/** The keys at the foot of a sheet: Cancel, then the one that does it. */
@Composable
internal fun SheetKeys(confirm: String, onConfirm: () -> Unit, onCancel: () -> Unit, kind: KeyKind = KeyKind.Primary, enabled: Boolean = true) {
    Row(Modifier.fillMaxWidth().padding(top = 14.dp, start = 6.dp, end = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
        Key("Cancel", onCancel, kind = KeyKind.Quiet)
        Key(confirm, onConfirm, kind = kind, enabled = enabled)
    }
}

/** A short question before something that cannot be taken back. */
@Composable
internal fun AskSheet(title: String, body: String, confirm: String, danger: Boolean, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    DevSheet(onDismiss) { close ->
        Column(Modifier.padding(horizontal = 6.dp)) {
            T(title, style = Relay.type.title, color = Relay.colors.ink)
            T(body, Modifier.padding(top = 6.dp), Relay.type.ui, Relay.colors.ink2)
        }
        SheetKeys(confirm, { close(onConfirm) }, { close {} }, if (danger) KeyKind.Danger else KeyKind.Primary)
    }
}

/** A labelled switch with a line of what it means, as the PC's settings rows: the whole row is the switch. */
@Composable
internal fun SwitchRow(label: String, detail: String?, checked: Boolean, onChange: (Boolean) -> Unit, enabled: Boolean = true) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().heightIn(min = 52.dp).switchRow(checked, enabled) { onChange(it) }.padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            T(label, style = Relay.type.ui, color = c.ink)
            detail?.let { T(it, style = Relay.type.caption, color = c.ink3) }
        }
        Toggle(checked, null, enabled = enabled)
    }
}

/** "Workspace / Project", or the project alone when its workspace is unknown. */
internal fun crumb(project: Project, workspaces: List<Workspace>): String =
    workspaces.firstOrNull { it.id == project.workspaceId }?.name?.takeIf { it.isNotBlank() }?.let { "$it / ${project.name}" } ?: project.name

/** Every project, under its workspace, the current one checked. */
@Composable
internal fun ProjectSheet(projects: List<Project>, workspaces: List<Workspace>, current: Long?, onPick: (Project) -> Unit, onDismiss: () -> Unit) {
    val c = Relay.colors
    DevSheet(onDismiss, "Project") { close ->
        if (projects.isEmpty()) {
            T("No projects yet. Add one on the PC.", Modifier.padding(horizontal = 6.dp, vertical = 8.dp), Relay.type.caption, c.ink3)
        }
        val groups = projects.groupBy { it.workspaceId }
        val order = workspaces.map { it.id } + groups.keys.filter { k -> workspaces.none { it.id == k } }
        for (wid in order) {
            val list = groups[wid] ?: continue
            val heading = workspaces.firstOrNull { it.id == wid }?.name
            if (!heading.isNullOrBlank() && groups.size > 1) {
                T(heading, Modifier.padding(start = 10.dp, top = 10.dp, bottom = 2.dp), Relay.type.section, c.ink3)
            }
            for (p in list) {
                SheetRow(p.name, "folder", { close { onPick(p) } }, detail = p.baseBranch, selected = p.num == current, enabled = p.num != null)
            }
        }
    }
}
