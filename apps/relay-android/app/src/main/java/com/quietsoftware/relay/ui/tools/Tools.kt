package com.quietsoftware.relay.ui.tools

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateMap
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Workspace
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.asStr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Meter
import com.quietsoftware.relay.ui.kit.NavRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.pages.Ask
import com.quietsoftware.relay.ui.pages.ScrollBody
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.tasks.BoardSheet
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.flowOf
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private val numberKeys = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Next)

/** A share like 33.5. */
private val decimalKeys = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Next)

/** Commands, paths, branches and globs: typed as they are, with no capital or correction. */
private val codeKeys = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false)

@Serializable
private data class SkillRow(
    val id: Long,
    val name: String = "",
    val description: String = "",
    @SerialName("enabled_in") val enabledIn: List<Long> = emptyList(),
)

@Serializable
private data class PluginRow(
    val id: String,
    val name: String = "",
    val version: String = "",
    val summary: String = "",
    @SerialName("enabled_in") val enabledIn: List<Long> = emptyList(),
)

@Serializable
private data class DeviceRow(val serial: String, val model: String = "", val state: String = "", val kind: String = "usb")

@Serializable
private data class AvdRow(val name: String, val device: String? = null, @SerialName("running_serial") val runningSerial: String? = null)

@Serializable
private data class WorktreeRow(val path: String, val branch: String = "", val session: String? = null)

@Serializable
private data class RunRow(
    val id: Long,
    val kind: String = "",
    val device: String = "",
    val worktree: String = "",
    val state: String = "",
    val artifact: String? = null,
    val variant: String? = null,
    val format: String? = null,
    @SerialName("started_at") val startedAt: String = "",
)

@Serializable
private data class IntegrationRow(
    val id: Long,
    val branches: List<String> = emptyList(),
    val worktree: String? = null,
    val state: String = "",
    val conflict: List<String>? = null,
    @SerialName("log_tail") val logTail: String = "",
    @SerialName("started_at") val startedAt: String? = null,
    @SerialName("finished_at") val finishedAt: String? = null,
)

@Serializable
private data class LocalRepo(val path: String, val name: String = "")

@Serializable
private data class GitHubRepo(
    @SerialName("full_name") val fullName: String,
    val description: String? = null,
    @SerialName("clone_url") val cloneUrl: String,
    @SerialName("private") val isPrivate: Boolean = false,
    val archived: Boolean = false,
)

@Serializable
private data class ProviderRow(
    val provider: String,
    val installed: Boolean = false,
    val path: String? = null,
    val version: String? = null,
    @SerialName("signed_in_as") val signedInAs: String? = null,
    val guarded: Boolean = false,
)

@Serializable
private data class BackupRow(
    val path: String,
    val bytes: Long = 0,
    @SerialName("created_at") val createdAt: String = "",
    val reason: String = "",
)

private class UsageWindow(val name: String, val pct: Double, val resets: String?)

private class UsageGroup(val provider: String, val windows: List<UsageWindow>)

/** `usage.get`'s windows are per provider (`{name: {used_pct, resets_in}}`); this reads the ones with a figure. */
private fun parseUsage(result: JsonElement?): List<UsageGroup> =
    (result as? JsonObject)?.arr("usage")?.mapNotNull { item ->
        val group = item as? JsonObject ?: return@mapNotNull null
        val windows = group.o("windows")?.entries?.mapNotNull { (name, value) ->
            val window = value as? JsonObject ?: return@mapNotNull null
            val pct = (window["used_pct"] ?: window["pct"])?.number() ?: return@mapNotNull null
            UsageWindow(name.replace('_', ' '), pct.coerceIn(0.0, 100.0), window.s("resets_in"))
        }.orEmpty()
        UsageGroup(group.s("provider").orEmpty(), windows)
    }.orEmpty()

private const val SHOWN_REPOS = 60

/** Integration states that are still moving: queued, merging, building, deploying. */
private val INTEGRATION_RUNNING = setOf("queued", "merging", "building", "deploying")

private fun integrationColor(state: String, c: Palette): Color = when (state) {
    "passed" -> c.live
    "failed", "conflict" -> c.held
    "discarded" -> c.ink3
    else -> c.waiting
}

private fun runColor(state: String, c: Palette): Color = when (state) {
    "building", "running" -> c.live
    "failed" -> c.held
    else -> c.ink3
}

/**
 * One project's settings: its name, base branch, build and run commands and pinning (edits, so
 * they queue offline), the skills and plugins its agents get, its guardrail values, and removal.
 */
@Composable
fun ProjectSettingsScreen(projectId: Long, nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val project by nav.relay.project(projectId).collectAsStateWithLifecycle(initialValue = null)
    val p = project
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(p?.name ?: "Project settings", nav::back, subtitle = if (p != null) "Project settings" else null)
            ScrollBody {
                OfflineNote(s.online)
                if (p == null) {
                    Empty("gear", "No project here", "It may have been removed on the PC.")
                } else {
                    ProjectFields(projectId, p, nav)
                    SectionLabel("Tools")
                    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                        NavRow("Devices", "device") { nav.devices(projectId) }
                        Hairline()
                        NavRow("Merge tests", "merge") { nav.integration(projectId) }
                        Hairline()
                        NavRow("Guardrails and holds", "shield") { nav.guardrails(projectId) }
                    }
                    SkillsCard(projectId, nav, s.online)
                    PluginsCard(projectId, nav, s.online)
                    GuardrailValues(projectId, nav, s.online)
                    DangerZone(projectId, p.name, nav, s.online)
                }
            }
        }
    }
}

@Composable
private fun ProjectFields(projectId: Long, p: Project, nav: Nav) {
    val c = Relay.colors
    // Saveable, so an edit is still here after Devices, Merge tests or Guardrails and back.
    var name by rememberSaveable(p.name) { mutableStateOf(p.name) }
    var branch by rememberSaveable(p.baseBranch) { mutableStateOf(p.baseBranch) }
    var buildCmd by rememberSaveable(p.buildCmd) { mutableStateOf(p.buildCmd.orEmpty()) }
    var runCmd by rememberSaveable(p.runCmd) { mutableStateOf(p.runCmd.orEmpty()) }
    val nameText = name.trim()
    val branchText = branch.trim()
    // An empty command goes back to Relay's default, which the PC stores as null.
    val buildText = buildCmd.trim().ifEmpty { null }
    val runText = runCmd.trim().ifEmpty { null }
    val changes = buildJsonObject {
        if (nameText.isNotEmpty() && nameText != p.name) put("name", nameText)
        if (branchText.isNotEmpty() && branchText != p.baseBranch) put("base_branch", branchText)
        if (buildText != p.buildCmd) put("build_cmd", buildText)
        if (runText != p.runCmd) put("run_cmd", runText)
    }
    val canSave = changes.isNotEmpty() && nameText.isNotEmpty() && branchText.isNotEmpty()

    SectionLabel("Project")
    Slab(Modifier.fillMaxWidth()) {
        FieldBlock("Name") {
            Field(name, { name = it }, Modifier.fillMaxWidth(), placeholder = "Project name")
        }
        Gap(14.dp)
        FieldBlock("Folder", "Where it lives on the PC; it moves only from there.") {
            SelectionContainer { T(p.path, style = Relay.type.mono, color = c.ink2, maxLines = 2) }
        }
        Gap(14.dp)
        FieldBlock("Base branch", "The branch new agent worktrees start from and integration merges into.") {
            Field(branch, { branch = it }, Modifier.fillMaxWidth(), placeholder = "main", style = Relay.type.mono, keyboardOptions = codeKeys)
        }
        Gap(14.dp)
        FieldBlock("Build command", "Run by an integration build. Empty uses Relay's default.") {
            Field(buildCmd, { buildCmd = it }, Modifier.fillMaxWidth(), placeholder = "Automatic", style = Relay.type.mono, keyboardOptions = codeKeys)
        }
        Gap(14.dp)
        FieldBlock("Run command", "Run for Run on device. Empty uses Gradle's install and launch.") {
            Field(runCmd, { runCmd = it }, Modifier.fillMaxWidth(), placeholder = "Automatic", style = Relay.type.mono, keyboardOptions = codeKeys)
        }
        Gap(12.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
            if (changes.isNotEmpty()) {
                ToolKey(
                    "Revert",
                    {
                        name = p.name
                        branch = p.baseBranch
                        buildCmd = p.buildCmd.orEmpty()
                        runCmd = p.runCmd.orEmpty()
                    },
                    kind = KeyKind.Quiet,
                )
            }
            ToolKey(
                "Save",
                { nav.shell.change("project.update", withProject(projectId, changes), "Project settings") },
                kind = KeyKind.Primary,
                enabled = canSave,
            )
        }
    }
    Gap(8.dp)
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
        SwitchLine(
            "Pinned",
            "Pinned projects come first in the sidebar, on the phone and on the PC.",
            p.pinned,
        ) { on ->
            nav.shell.change("project.update", buildJsonObject { put("project_id", projectId); put("pinned", on) }, "Pinned")
        }
    }
}

@Composable
private fun SkillsCard(projectId: Long, nav: Nav, online: Boolean) {
    val flow = remember(projectId) { nav.relay.live("skill.list", byProject(projectId)) }
    val live by flow.collectAsStateWithLifecycle(initialValue = Live())
    val skills = (live.result as? JsonObject)?.arr("skills")?.mapNotNull { it.decodeAs<SkillRow>() }.orEmpty()
    // A switch shows what was just set until the PC's list says so; a new list drops it.
    val overrides = remember(live.result) { mutableStateMapOf<Long, Boolean>() }
    // While its call is out, a switch shows where it is going and takes no second tap.
    val sending = remember { mutableStateMapOf<Long, Boolean>() }
    SectionLabel("Skills")
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
        when {
            live.result == null -> Hint(live.error?.message ?: "Loading…")
            skills.isEmpty() -> Hint("No skills on this PC. A skill is a Markdown file of instructions agents load when a task calls for it.")
            else -> skills.forEachIndexed { index, skill ->
                if (index > 0) Hairline()
                SwitchLine(
                    title = skill.name,
                    hint = skill.description.ifBlank { null },
                    checked = sending[skill.id] ?: overrides[skill.id] ?: skill.enabledIn.contains(projectId),
                    enabled = online && skill.id !in sending,
                ) { on ->
                    sending[skill.id] = on
                    nav.shell.act(
                        "skill.enable",
                        buildJsonObject { put("skill_id", skill.id); put("project_id", projectId); put("enabled", on) },
                    ) { overrides[skill.id] = on }.invokeOnCompletion { sending.remove(skill.id) }
                }
            }
        }
    }
    if (!live.fresh && live.result != null) StaleNote(live.at)
}

@Composable
private fun PluginsCard(projectId: Long, nav: Nav, online: Boolean) {
    val flow = remember(projectId) { nav.relay.live("plugin.list", byProject(projectId)) }
    val live by flow.collectAsStateWithLifecycle(initialValue = Live())
    val plugins = (live.result as? JsonObject)?.arr("plugins")?.mapNotNull { it.decodeAs<PluginRow>() }.orEmpty()
    val overrides = remember(live.result) { mutableStateMapOf<String, Boolean>() }
    val sending = remember { mutableStateMapOf<String, Boolean>() }
    SectionLabel("Plugins")
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
        when {
            live.result == null -> Hint(live.error?.message ?: "Loading…")
            plugins.isEmpty() -> Hint("This PC bundles no plugins.")
            else -> plugins.forEachIndexed { index, plugin ->
                if (index > 0) Hairline()
                SwitchLine(
                    title = listOf(plugin.name, plugin.version).filter { it.isNotBlank() }.joinToString(" "),
                    hint = plugin.summary.ifBlank { null },
                    checked = sending[plugin.id] ?: overrides[plugin.id] ?: plugin.enabledIn.contains(projectId),
                    enabled = online && plugin.id !in sending,
                ) { on ->
                    sending[plugin.id] = on
                    nav.shell.act(
                        "plugin.enable",
                        buildJsonObject { put("plugin_id", plugin.id); put("project_id", projectId); put("enabled", on) },
                    ) { overrides[plugin.id] = on }.invokeOnCompletion { sending.remove(plugin.id) }
                }
            }
        }
    }
    if (!live.fresh && live.result != null) StaleNote(live.at)
}

@Composable
private fun GuardrailValues(projectId: Long, nav: Nav, online: Boolean) {
    // Read again after a whole reset, whose event is a settings one, not a guardrail one.
    var reread by remember { mutableIntStateOf(0) }
    // The layers say, field by field, whether a value is set here or follows the layer above.
    val layersFlow = remember(projectId, reread) { nav.relay.live("guardrail.config.layers", byProject(projectId)) }
    val layers by layersFlow.collectAsStateWithLifecycle(initialValue = Live())
    // An engine older than the layers still answers with the values in force.
    val older = layers.result == null && layers.error != null && layers.error?.code != "link.down"
    val plainFlow = remember(projectId, older, reread) { if (older) nav.relay.live("guardrail.config.get", byProject(projectId)) else flowOf(Live()) }
    val plain by plainFlow.collectAsStateWithLifecycle(initialValue = Live())
    val layered = layers.result as? JsonObject
    val config = layered?.o("effective") ?: plain.result as? JsonObject
    SectionLabel("Guardrail values")
    Slab(Modifier.fillMaxWidth()) {
        if (config == null) {
            Hint((layers.error ?: plain.error)?.message ?: "Loading…")
        } else {
            GuardrailForm(projectId, config, layered, nav, online) { reread++ }
        }
    }
    val shown = if (layered != null) layers else plain
    if (!shown.fresh && config != null) StaleNote(shown.at)
}

private enum class GuardKind { Count, Percent, Toggle, List }

/** One field of the form: where it is in the config, the desktop's words for it, and its least value. */
private class GuardField(val path: String, val title: String, val hint: String, val kind: GuardKind, val min: Long = 0)

/** The desktop's form (guardrail_settings.rs `GROUPS`), group by group. */
private val GUARD_GROUPS = listOf(
    "Commit size" to listOf(
        GuardField("caps.files", "Files per commit", "A commit that changes more files is refused.", GuardKind.Count, min = 1),
        GuardField("caps.lines", "Lines per commit", "Added plus removed lines, across the whole commit.", GuardKind.Count, min = 1),
    ),
    "Large rewrites" to listOf(
        GuardField("destructive_write.min_removed_lines", "Removed lines", "A write that removes more lines than this is held for you.", GuardKind.Count),
        GuardField("destructive_write.min_removed_pct", "Removed share of a file (%)", "Or removes more than this share of an existing file.", GuardKind.Percent),
        GuardField("destructive_write.min_file_lines", "Ignore the share rule below (lines)", "One line of a two-line file is 50%; short files are judged by lines only.", GuardKind.Count),
        GuardField("destructive_write.allow_if_recoverable", "Allow rewrites git can restore", "Committed, unmodified or ignored files can always come back.", GuardKind.Toggle),
    ),
    "Paths and commands" to listOf(
        GuardField("protected_paths", "Protected paths", "Worktree-relative paths or globs agents may not write. One per line.", GuardKind.List),
        GuardField("denied_commands", "Denied commands", "Commands agents may not run, matched word by word. One per line.", GuardKind.List),
        GuardField("allowed_write_roots", "Extra write roots", "Absolute directories outside the worktree agents may write to. One per line.", GuardKind.List),
    ),
    "Pushes" to listOf(
        GuardField("check_on_push", "Check an agent's push", "Runs the project's build command, or its detected tests, first. A failing check refuses the push.", GuardKind.Toggle),
    ),
)

private val GUARD_FIELDS = GUARD_GROUPS.flatMap { it.second }

/** The form's texts by field path, kept across a trip to another page and back. */
private val GUARD_SAVER = listSaver<SnapshotStateMap<String, String>, String>(
    save = { m -> m.flatMap { listOf(it.key, it.value) } },
    restore = { l -> mutableStateMapOf<String, String>().apply { l.chunked(2).forEach { (k, v) -> put(k, v) } } },
)

/** The value at a dotted [path] (`caps.files`) of a config. */
private fun JsonObject.at(path: String): JsonElement? = path.split('.').fold<String, JsonElement?>(this) { v, part -> (v as? JsonObject)?.get(part) }

/** A config value as its field edits it: a number, `true`/`false`, or one entry per line. */
private fun fieldText(f: GuardField, v: JsonElement?): String = when (f.kind) {
    GuardKind.List -> (v as? JsonArray)?.mapNotNull { it.asStr() }.orEmpty().joinToString("\n")
    GuardKind.Toggle -> ((v as? JsonPrimitive)?.booleanOrNull == true).toString()
    else -> v?.number()?.let { if (it % 1.0 == 0.0) it.toLong().toString() else it.toString() }.orEmpty()
}

/** What a field's text means as a config value; null while it is not one. */
private fun fieldValue(f: GuardField, text: String): JsonElement? = when (f.kind) {
    GuardKind.Count -> text.trim().toLongOrNull()?.takeIf { it >= f.min }?.let { JsonPrimitive(it) }
    GuardKind.Percent -> text.trim().toDoubleOrNull()?.takeIf { it in 0.0..100.0 }?.let { JsonPrimitive(it) }
    GuardKind.Toggle -> JsonPrimitive(text == "true")
    GuardKind.List -> JsonArray(splitLines(text).map { JsonPrimitive(it) })
}

/** The same value: 50 and 50.0 are one share. */
private fun sameValue(a: JsonElement?, b: JsonElement?): Boolean {
    val x = a?.number()
    val y = b?.number()
    return if (x != null && y != null) kotlin.math.abs(x - y) < 1e-9 else a == b
}

/** A value as a caption says it (guardrail_settings.rs `shown`): "none", "1 entry (…)", "on", "50". */
private fun shownValue(v: JsonElement?): String = when {
    v is JsonArray && v.isEmpty() -> "none"
    v is JsonArray && v.size == 1 -> "1 entry (${v[0].asStr().orEmpty()})"
    v is JsonArray -> "${v.size} entries"
    v is JsonPrimitive && v.booleanOrNull != null -> if (v.booleanOrNull == true) "on" else "off"
    else -> v?.number()?.let { if (it % 1.0 == 0.0) "%.0f".format(it) else "%.1f".format(it) } ?: v?.toString().orEmpty()
}

/** Where an inherited value was set, in the caption's words. */
private fun layerName(layer: String) = when (layer) {
    "global" -> "Global"
    "workspace" -> "the workspace"
    "project" -> "this project"
    else -> "Relay's default"
}

/**
 * The project's guardrail values (guardrail_settings.rs, at the project's level): every field
 * says whether it is set here or inherited, and from where. Only the fields changed are saved,
 * so an untouched one keeps following its parent; Reset on one set here makes it follow again.
 */
@Composable
private fun GuardrailForm(projectId: Long, config: JsonObject, layers: JsonObject?, nav: Nav, online: Boolean, onReset: () -> Unit) {
    val c = Relay.colors
    val inherited = layers?.o("inherited")
    val sources = layers?.o("sources")
    val basis: JsonObject = layers ?: config
    // Saveable, so an edit is still here after another page and back; new values from the PC reset them.
    val edits = rememberSaveable(basis, saver = GUARD_SAVER) {
        mutableStateMapOf<String, String>().apply { GUARD_FIELDS.forEach { put(it.path, fieldText(it, config.at(it.path))) } }
    }
    // Fields whose Reset was pressed: saving sends null for them, and they follow the layer above again.
    val cleared = remember(basis) { mutableStateListOf<String>() }
    var resetAsk by remember { mutableStateOf(false) }

    fun valueOf(f: GuardField) = fieldValue(f, edits[f.path] ?: fieldText(f, config.at(f.path)))
    fun changed(f: GuardField) = valueOf(f)?.let { !sameValue(it, config.at(f.path)) } ?: true
    fun here(f: GuardField) = sources?.s(f.path) == "project"
    fun edit(f: GuardField, text: String) {
        edits[f.path] = text
        // Typed over a reset: it is this project's own value again.
        if (f.path in cleared && !sameValue(fieldValue(f, text), inherited?.at(f.path))) cleared.remove(f.path)
    }
    fun reset(f: GuardField) {
        // Set here: back to the inherited value, saved as following it. Merely edited: back to what is saved.
        edits[f.path] = fieldText(f, if (here(f)) inherited?.at(f.path) else config.at(f.path))
        if (here(f) && f.path !in cleared) cleared.add(f.path)
    }
    fun caption(f: GuardField): String? {
        if (layers == null) return null
        val source = sources?.s(f.path) ?: "default"
        val before = shownValue(inherited?.at(f.path))
        return when {
            f.path in cleared -> "Will follow the inherited value ($before) once saved"
            changed(f) -> "Changed · not saved"
            source == "project" -> "Set here · would otherwise be $before"
            else -> "Inherited from ${layerName(source)}"
        }
    }

    val bad = GUARD_FIELDS.firstOrNull { valueOf(it) == null }
    val problem = bad?.let { f ->
        if (f.kind == GuardKind.Percent) "${f.title} must be a number from 0 to 100." else "${f.title} must be a whole number, ${f.min} or more."
    }
    // Only what the person changed goes in, so a project's override stays as small as they meant it.
    val patch = buildJsonObject {
        val groups = LinkedHashMap<String, MutableMap<String, JsonElement>>()
        for (f in GUARD_FIELDS) {
            val value = if (f.path in cleared) JsonNull else valueOf(f)?.takeIf { changed(f) } ?: continue
            val dot = f.path.indexOf('.')
            if (dot < 0) put(f.path, value) else groups.getOrPut(f.path.substring(0, dot)) { LinkedHashMap() }[f.path.substring(dot + 1)] = value
        }
        groups.forEach { (group, values) -> put(group, JsonObject(values)) }
    }
    val dirty = patch.isNotEmpty() && problem == null

    T("Applies to this project only. Unset fields follow its workspace, then Global.", style = Relay.type.caption, color = c.ink3)
    for ((group, fields) in GUARD_GROUPS) {
        Gap(16.dp)
        Eyebrow(group)
        for (f in fields) {
            Gap(10.dp)
            val text = edits[f.path] ?: fieldText(f, config.at(f.path))
            when (f.kind) {
                GuardKind.Toggle -> SwitchLine(f.title, f.hint, text == "true") { on -> edit(f, on.toString()) }
                GuardKind.List -> FieldBlock(f.title, f.hint) {
                    Field(
                        text,
                        { edit(f, it) },
                        Modifier.fillMaxWidth(),
                        singleLine = false,
                        minLines = 3,
                        maxLines = 8,
                        style = Relay.type.mono,
                        keyboardOptions = codeKeys,
                    )
                }
                else -> FieldBlock(f.title, f.hint) {
                    Field(text, { edit(f, it) }, Modifier.fillMaxWidth(), keyboardOptions = if (f.kind == GuardKind.Percent) decimalKeys else numberKeys)
                }
            }
            val said = caption(f)
            val canReset = layers != null && (here(f) || changed(f)) && f.path !in cleared
            if (said != null || canReset) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    T(
                        said.orEmpty(),
                        Modifier.weight(1f),
                        Relay.type.caption,
                        when {
                            f.path in cleared || changed(f) -> c.waiting
                            here(f) -> c.ink2
                            else -> c.ink3
                        },
                    )
                    if (canReset) ToolKey("Reset", { reset(f) }, kind = KeyKind.Quiet)
                }
            }
        }
    }
    problem?.let {
        Gap(8.dp)
        T(it, style = Relay.type.caption, color = c.heldText)
    }
    Gap(12.dp)
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        ToolKey("Reset to the workspace's guardrails", { resetAsk = true }, kind = KeyKind.Quiet, enabled = online)
    }
    Gap(6.dp)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
        ToolKey(
            "Save",
            {
                nav.shell.act(
                    "guardrail.config.set",
                    buildJsonObject { put("project_id", projectId); put("patch", patch) },
                    done = "Saved for this project.",
                )
            },
            kind = KeyKind.Primary,
            enabled = dirty && online,
        )
    }
    if (resetAsk) {
        Ask(
            title = "Use the workspace's guardrails?",
            body = "This project forgets its own guardrail values and follows the PC-wide settings.",
            onDismiss = { resetAsk = false },
        ) {
            ToolKey("Cancel", { resetAsk = false }, kind = KeyKind.Quiet)
            ToolKey(
                "Reset",
                {
                    resetAsk = false
                    nav.shell.act(
                        "settings.reset",
                        buildJsonObject { put("path", "guardrails.projects.$projectId") },
                        done = "Guardrails reset",
                    ) { onReset() }
                },
                kind = KeyKind.Danger,
            )
        }
    }
}

@Composable
private fun DangerZone(projectId: Long, name: String, nav: Nav, online: Boolean) {
    val c = Relay.colors
    var asking by remember { mutableStateOf(false) }
    SectionLabel("Danger zone")
    Slab(Modifier.fillMaxWidth()) {
        T("Remove project", style = Relay.type.body, color = c.ink, weight = FontWeight.Medium)
        T("Forget it in Relay. The folder on disk is not touched.", style = Relay.type.caption, color = c.ink3)
        Gap(12.dp)
        ToolKey("Remove project", { asking = true }, kind = KeyKind.Danger, glyph = "trash", enabled = online)
    }
    if (asking) {
        Ask(
            title = "Remove $name?",
            body = "Relay forgets this project: its board, notes and history on the PC go with it. The folder on disk is not touched. A project with live sessions cannot be removed.",
            onDismiss = { asking = false },
        ) {
            ToolKey("Cancel", { asking = false }, kind = KeyKind.Quiet)
            ToolKey(
                "Remove",
                {
                    asking = false
                    nav.shell.act(
                        "project.remove",
                        buildJsonObject { put("project_id", projectId) },
                        done = "$name removed",
                    ) { nav.agents() }
                },
                kind = KeyKind.Danger,
            )
        }
    }
}

/**
 * The PC's devices and emulators for one project: run it on a device, build an APK or bundle,
 * boot an emulator, and see the runs and stop one. Logcat streaming is not here; the log is on the PC.
 */
@Composable
fun DevicesScreen(projectId: Long, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val project by nav.relay.project(projectId).collectAsStateWithLifecycle(initialValue = null)
    val devicesFlow = remember { nav.relay.live("device.list") }
    val devicesLive by devicesFlow.collectAsStateWithLifecycle(initialValue = Live())
    val avdFlow = remember { nav.relay.live("avd.list") }
    val avdLive by avdFlow.collectAsStateWithLifecycle(initialValue = Live())
    val treeFlow = remember(projectId) { nav.relay.live("worktree.list", byProject(projectId)) }
    val treeLive by treeFlow.collectAsStateWithLifecycle(initialValue = Live())
    val runFlow = remember(projectId) { nav.relay.live("device.run.list", byProject(projectId)) }
    val runLive by runFlow.collectAsStateWithLifecycle(initialValue = Live())
    val scope = rememberCoroutineScope()

    val devices = (devicesLive.result as? JsonObject)?.arr("devices")?.mapNotNull { it.decodeAs<DeviceRow>() }.orEmpty()
    val avds = (avdLive.result as? JsonObject)?.arr("avds")?.mapNotNull { it.decodeAs<AvdRow>() }.orEmpty()
    // The primary checkout is the first chip already, so the worktrees list leaves it out.
    val trees = (treeLive.result as? JsonObject)?.arr("worktrees")?.mapNotNull { it.decodeAs<WorktreeRow>() }.orEmpty()
        .filter { it.path != project?.path }
    val runs = (runLive.result as? JsonObject)?.arr("runs")?.mapNotNull { it.decodeAs<RunRow>() }.orEmpty()
        .sortedByDescending { it.id }
    val ready = devices.filter { it.state == "device" }
    var picked by remember { mutableStateOf<String?>(null) }
    // The first ready device is the usual target; a pick holds while it stays connected.
    val device = ready.firstOrNull { it.serial == picked } ?: ready.firstOrNull()
    var worktree by remember { mutableStateOf<String?>(null) }
    var variant by remember { mutableStateOf("debug") }
    var format by remember { mutableStateOf("apk") }
    var starting by remember { mutableStateOf(false) }
    val variantText = variant.trim().ifEmpty { null }
    val deviceName = device?.let { it.model.ifBlank { it.serial } }

    fun runOn(serial: String) {
        scope.pcCall(
            nav,
            "device.run",
            buildJsonObject {
                put("project_id", projectId)
                put("device", serial)
                variantText?.let { put("variant", it) }
                worktree?.let { put("worktree", it) }
            },
            busy = { starting = it },
            done = "Run started on the PC",
        )
    }

    fun buildArtifact() {
        scope.pcCall(
            nav,
            "device.build",
            buildJsonObject {
                put("project_id", projectId)
                variantText?.let { put("variant", it) }
                worktree?.let { put("worktree", it) }
                put("format", format)
                put("publish", false)
            },
            busy = { starting = it },
            done = "Build started on the PC",
        )
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Android devices", nav::back, subtitle = project?.name)
            ScrollBody {
                OfflineNote(s.online)

                SectionLabel("Devices")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                    when {
                        devicesLive.result == null -> Hint(devicesLive.error?.message ?: "Loading…")
                        devices.isEmpty() -> Hint("No device is connected to the PC. Plug one in with USB debugging on, or boot an emulator below.")
                        else -> devices.forEachIndexed { index, d ->
                            if (index > 0) Hairline()
                            val isReady = d.state == "device"
                            val selected = d.serial == device?.serial
                            ListRow(
                                selected = selected,
                                onClick = if (isReady) ({ picked = d.serial }) else null,
                                padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                            ) {
                                Dot(if (isReady) c.live else c.waiting, 7.dp)
                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    T(d.model.ifBlank { d.serial }, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                                    T(
                                        "${d.serial} · ${if (d.kind == "avd") "emulator" else "USB"}",
                                        style = Relay.type.caption,
                                        color = c.ink3,
                                        maxLines = 1,
                                    )
                                }
                                if (!isReady) {
                                    Pill(d.state)
                                } else if (selected) {
                                    Glyph("check", 16.dp, c.ink)
                                }
                            }
                        }
                    }
                }
                if (!devicesLive.fresh && devicesLive.result != null) StaleNote(devicesLive.at)

                SectionLabel("Emulators")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                    when {
                        avdLive.result == null -> Hint(avdLive.error?.message ?: "Loading…")
                        avds.isEmpty() -> Hint("No Android Virtual Device on the PC. Create one on the desktop.")
                        else -> avds.forEachIndexed { index, avd ->
                            if (index > 0) Hairline()
                            val running = avd.runningSerial != null
                            ListRow(
                                onClick = if (running || !s.online) null else ({
                                    scope.pcCall(nav, "avd.boot", buildJsonObject { put("name", avd.name) }, done = "Booting ${avd.name} on the PC")
                                }),
                                padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                            ) {
                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    T(avd.name, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                                    T(
                                        listOfNotNull(
                                            avd.device,
                                            if (running) "running as ${avd.runningSerial}" else "Tap to boot on the PC",
                                        ).joinToString(" · "),
                                        style = Relay.type.caption,
                                        color = c.ink3,
                                        maxLines = 1,
                                    )
                                }
                                if (running) Pill("Running", dot = c.live)
                            }
                        }
                    }
                }

                SectionLabel("Run or build")
                Slab(Modifier.fillMaxWidth()) {
                    FieldBlock("Build from") {
                        Row(
                            Modifier.horizontalScroll(rememberScrollState()),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Pill("Primary checkout", Modifier.heightIn(min = TOUCH), selected = worktree == null, onClick = { worktree = null })
                            trees.forEach { tree ->
                                Pill(
                                    tree.session?.let { "${tree.branch} ($it)" } ?: tree.branch,
                                    Modifier.heightIn(min = TOUCH),
                                    selected = worktree == tree.path,
                                    onClick = { worktree = tree.path },
                                )
                            }
                        }
                    }
                    Gap(14.dp)
                    FieldBlock("Gradle variant", "Empty uses the project's default.") {
                        Field(variant, { variant = it }, Modifier.fillMaxWidth(), placeholder = "debug", style = Relay.type.mono, keyboardOptions = codeKeys)
                    }
                    Gap(14.dp)
                    ToolKey(
                        text = if (starting) "Starting…" else deviceName?.let { "Run on $it" } ?: "Run on device",
                        onClick = { device?.let { runOn(it.serial) } },
                        modifier = Modifier.fillMaxWidth(),
                        kind = KeyKind.Primary,
                        glyph = "play",
                        enabled = s.online && device != null && !starting,
                    )
                    Gap(16.dp)
                    T("Build an artifact", style = Relay.type.uiMedium, color = c.ink2)
                    Gap(6.dp)
                    Segmented(
                        listOf("apk" to "APK", "aab" to "App Bundle"),
                        selected = format,
                        onSelect = { format = it },
                    )
                    Gap(8.dp)
                    ToolKey(
                        text = if (format == "apk") "Build APK" else "Build App Bundle",
                        onClick = { buildArtifact() },
                        modifier = Modifier.fillMaxWidth(),
                        glyph = "phone-install",
                        enabled = s.online && !starting,
                    )
                }

                SectionLabel("Runs")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                    when {
                        runLive.result == null -> Hint(runLive.error?.message ?: "Loading…")
                        runs.isEmpty() -> Hint("No runs or builds yet.")
                        else -> runs.forEachIndexed { index, run ->
                            if (index > 0) Hairline()
                            RunLine(run, online = s.online, onStop = {
                                scope.pcCall(nav, "device.run.stop", buildJsonObject { put("run_id", run.id) }, done = "Stopped")
                            })
                        }
                    }
                }
                if (!runLive.fresh && runLive.result != null) StaleNote(runLive.at)
                Gap(8.dp)
                T(
                    "The run's log is kept on the PC. Screen mirroring and release signing stay on the desktop.",
                    style = Relay.type.caption,
                    color = c.ink3,
                )
            }
        }
    }
}

@Composable
private fun RunLine(run: RunRow, online: Boolean, onStop: () -> Unit) {
    val c = Relay.colors
    val label = if (run.kind == "build") "Build ${run.format.orEmpty()}".trim() else "Run on ${run.device}"
    val active = run.state == "building" || run.state == "running"
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T("#${run.id} · $label", Modifier.weight(1f), Relay.type.uiMedium, c.ink, maxLines = 1)
            Pill(run.state, dot = runColor(run.state, c))
        }
        val details = listOfNotNull(
            run.variant,
            run.worktree.substringAfterLast('/').ifBlank { null },
            stamp(epoch(run.startedAt)).ifBlank { null },
        )
        if (details.isNotEmpty()) T(details.joinToString(" · "), style = Relay.type.caption, color = c.ink3, maxLines = 2)
        run.artifact?.let { T(it, style = Relay.type.mono, color = c.ink2, maxLines = 2) }
        if (active) ToolKey("Stop", onStop, glyph = "stop", enabled = online)
    }
}

/**
 * Test agents' branches together: merge two or more sessions' branches into a disposable worktree
 * and build it, leaving their own branches alone. Runs update live; a finished one can be read and
 * discarded.
 */
@Composable
fun IntegrationScreen(projectId: Long, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val project by nav.relay.project(projectId).collectAsStateWithLifecycle(initialValue = null)
    val sessions by nav.relay.sessions().collectAsStateWithLifecycle(initialValue = emptyList())
    val listFlow = remember(projectId) { nav.relay.live("integration.list", byProject(projectId)) }
    val listLive by listFlow.collectAsStateWithLifecycle(initialValue = Live())
    val scope = rememberCoroutineScope()

    val runs = (listLive.result as? JsonObject)?.arr("integrations")?.mapNotNull { it.decodeAs<IntegrationRow>() }.orEmpty()
        .sortedByDescending { it.id }
    val open = sessions.filter { it.projectId == projectId && it.state != "closed" }
    var picked by remember { mutableStateOf(setOf<String>()) }
    val chosen = open.filter { it.name in picked }
    var requesting by remember { mutableStateOf(false) }
    var detail by remember { mutableStateOf<Long?>(null) }
    var toDiscard by remember { mutableStateOf<IntegrationRow?>(null) }

    fun request() {
        scope.pcCall(
            nav,
            "integration.request",
            buildJsonObject {
                put("project_id", projectId)
                put("sessions", JsonArray(chosen.map { JsonPrimitive(it.name) }))
                put("build", true)
            },
            busy = { requesting = it },
            done = "Merge test started",
        ) { answer ->
            picked = emptySet()
            (answer as? JsonObject)?.l("id")?.let { detail = it }
        }
    }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Merge tests", nav::back, subtitle = project?.name)
            ScrollBody {
                OfflineNote(s.online)

                SectionLabel("New merge test")
                T(
                    "Pick at least two sessions. Their branches are merged in a disposable worktree and built there; your own branches are left alone.",
                    style = Relay.type.caption,
                    color = c.ink3,
                )
                Gap(8.dp)
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                    if (open.isEmpty()) Hint("No open sessions in this project.")
                    open.forEachIndexed { index, session ->
                        if (index > 0) Hairline()
                        val on = session.name in picked
                        ListRow(
                            selected = on,
                            onClick = { picked = if (on) picked - session.name else picked + session.name },
                            padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                        ) {
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                T(session.name, style = Relay.type.mono, color = c.ink, maxLines = 1)
                                T(session.branch.ifBlank { session.state }, style = Relay.type.caption, color = c.ink3, maxLines = 1)
                            }
                            Box(
                                Modifier
                                    .size(18.dp)
                                    .clip(Radii.keycap)
                                    .background(if (on) c.ink else Color.Transparent)
                                    .border(1.dp, if (on) c.ink else c.strong, Radii.keycap),
                                contentAlignment = Alignment.Center,
                            ) {
                                if (on) Glyph("check", 12.dp, c.wall)
                            }
                        }
                    }
                }
                Gap(10.dp)
                ToolKey(
                    text = when {
                        requesting -> "Starting…"
                        chosen.size >= 2 -> "Test merge and build · ${chosen.size}"
                        else -> "Select at least 2 sessions"
                    },
                    onClick = { request() },
                    modifier = Modifier.fillMaxWidth(),
                    kind = KeyKind.Primary,
                    glyph = "merge",
                    enabled = s.online && chosen.size >= 2 && !requesting,
                )

                SectionLabel("Runs")
                when {
                    listLive.result == null -> Hint(listLive.error?.message ?: "Loading…")
                    runs.isEmpty() -> Empty("merge", "No merge tests yet", "Pick two sessions above to test their branches together.")
                    else -> Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                        runs.forEachIndexed { index, run ->
                            if (index > 0) Hairline()
                            IntegrationLine(run) { detail = run.id }
                        }
                    }
                }
                if (!listLive.fresh && listLive.result != null) StaleNote(listLive.at)
            }
        }
    }

    detail?.let { id ->
        IntegrationSheet(
            id = id,
            nav = nav,
            online = s.online,
            onDismiss = { detail = null },
            onDiscard = { row ->
                detail = null
                toDiscard = row
            },
        )
    }
    toDiscard?.let { row ->
        Ask(
            title = "Discard integration #${row.id}?",
            body = if (row.state in INTEGRATION_RUNNING) {
                "It is still running. Discarding removes its throwaway worktree."
            } else {
                "Removes its throwaway worktree."
            },
            onDismiss = { toDiscard = null },
        ) {
            ToolKey("Cancel", { toDiscard = null }, kind = KeyKind.Quiet)
            ToolKey(
                "Discard",
                {
                    toDiscard = null
                    nav.shell.act(
                        "integration.discard",
                        buildJsonObject { put("integration_id", row.id) },
                        done = "Discarded #${row.id}",
                    )
                },
                kind = KeyKind.Danger,
            )
        }
    }
}

@Composable
private fun IntegrationLine(run: IntegrationRow, onOpen: () -> Unit) {
    val c = Relay.colors
    val conflict = run.conflict?.takeIf { it.size >= 2 }
    val detail = when {
        conflict != null -> "Conflict between ${conflict[0]} and ${conflict[1]}"
        run.finishedAt != null -> "Finished ${ago(epoch(run.finishedAt))}"
        run.startedAt != null -> "Started ${ago(epoch(run.startedAt))}"
        else -> "Waiting"
    }
    ListRow(onClick = onOpen, padding = PaddingValues(horizontal = 10.dp, vertical = 10.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            T("#${run.id} · ${run.branches.joinToString(" + ")}", style = Relay.type.mono, color = c.ink, maxLines = 1)
            T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 2)
        }
        Pill(run.state, dot = integrationColor(run.state, c))
    }
}

/** One run in full, kept live: its state, branches, the conflicting pair, and the build log's tail. */
@Composable
private fun IntegrationSheet(id: Long, nav: Nav, online: Boolean, onDismiss: () -> Unit, onDiscard: (IntegrationRow) -> Unit) {
    val flow = remember(id) { nav.relay.live("integration.get", buildJsonObject { put("integration_id", id) }) }
    val live by flow.collectAsStateWithLifecycle(initialValue = Live())
    val item = live.result?.decodeAs<IntegrationRow>()
    BoardSheet(onDismiss = onDismiss, title = "Integration #$id", subtitle = item?.branches?.joinToString(" + ")) { _ ->
        if (item == null) {
            Hint(live.error?.message ?: "Loading…")
        } else {
            IntegrationDetail(item, online, onDismiss, onDiscard)
        }
    }
}

@Composable
private fun IntegrationDetail(item: IntegrationRow, online: Boolean, onDismiss: () -> Unit, onDiscard: (IntegrationRow) -> Unit) {
    val c = Relay.colors
    val conflict = item.conflict?.takeIf { it.size >= 2 }
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Pill(item.state, dot = integrationColor(item.state, c))
        item.branches.forEach { Pill(it, glyph = "branch") }
    }
    if (conflict != null) {
        Gap(10.dp)
        T(
            "${conflict[0]} conflicts with ${conflict[1]}. Resolve it in one of those sessions, then test again.",
            style = Relay.type.caption,
            color = c.heldText,
        )
    }
    item.worktree?.takeIf { it.isNotBlank() }?.let {
        Gap(8.dp)
        T(it, style = Relay.type.mono, color = c.ink3, maxLines = 2)
    }
    Gap(6.dp)
    val times = listOfNotNull(
        item.startedAt?.let { "Started ${ago(epoch(it))}" },
        item.finishedAt?.let { "finished ${ago(epoch(it))}" },
        if (item.state in INTEGRATION_RUNNING) "running" else null,
    )
    T(times.ifEmpty { listOf("Queued") }.joinToString(" · "), style = Relay.type.caption, color = c.ink3)
    if (item.logTail.isNotBlank()) {
        Gap(10.dp)
        Column(
            Modifier
                .fillMaxWidth()
                .heightIn(max = 280.dp)
                .clip(Radii.key)
                .background(c.console)
                .border(1.dp, c.edge, Radii.key)
                .verticalScroll(rememberScrollState())
                .padding(12.dp),
        ) {
            Box(Modifier.horizontalScroll(rememberScrollState())) {
                T(item.logTail, style = Relay.type.mono, color = c.ink2)
            }
        }
    }
    Gap(14.dp)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
        ToolKey("Close", onDismiss, kind = KeyKind.Quiet)
        ToolKey(
            "Discard",
            { onDiscard(item) },
            kind = KeyKind.Danger,
            enabled = online && item.state != "discarded",
        )
    }
}

/**
 * Add a project to the PC: pick or create the workspace it goes in, then register a repository
 * already in that folder, or clone one from GitHub (or any URL).
 */
@Composable
fun AddProjectScreen(nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val workspaces by nav.relay.workspaces().collectAsStateWithLifecycle(initialValue = emptyList())
    var pickedId by remember { mutableStateOf<String?>(null) }
    var creating by remember { mutableStateOf(false) }
    var source by remember { mutableStateOf("local") }
    // The first workspace is the usual answer; a new one becomes the pick once the PC has it.
    val workspace = workspaces.firstOrNull { it.id == pickedId } ?: workspaces.firstOrNull()
    val knownPaths = s.projects.map { it.path }.toSet()

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Add a project", nav::back)
            ScrollBody {
                OfflineNote(s.online)

                SectionLabel("Workspace")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                    workspaces.forEachIndexed { index, ws ->
                        if (index > 0) Hairline()
                        val chosen = ws.id == workspace?.id
                        ListRow(
                            selected = chosen,
                            onClick = {
                                pickedId = ws.id
                                creating = false
                            },
                            padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                        ) {
                            Glyph("folder", 18.dp, if (chosen) c.ink else c.ink3)
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                T(ws.name.ifBlank { ws.path }, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                                T(ws.path, style = Relay.type.mono, color = c.ink3, maxLines = 1)
                            }
                            if (chosen) Glyph("check", 16.dp, c.ink)
                        }
                    }
                    if (workspaces.isNotEmpty()) Hairline()
                    ListRow(
                        onClick = { creating = !creating },
                        padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                    ) {
                        Glyph("folder-plus", 18.dp, c.ink3)
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            T("New workspace", style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                            T("A folder on the PC that holds projects", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                        }
                    }
                }
                if (creating || workspaces.isEmpty()) {
                    Gap(8.dp)
                    NewWorkspace(nav, s.online) { created ->
                        pickedId = created
                        creating = false
                    }
                }

                if (workspace != null) {
                    Gap(14.dp)
                    Segmented(
                        listOf("local" to "On this PC", "github" to "From GitHub"),
                        selected = source,
                        onSelect = { source = it },
                        modifier = Modifier.fillMaxWidth(),
                        fill = true,
                    )
                    Gap(10.dp)
                    if (source == "local") {
                        OnThisPc(nav, workspace, knownPaths, s.online)
                    } else {
                        FromGitHub(nav, workspace, s.online)
                    }
                } else if (workspaces.isNotEmpty()) {
                    Hint("Pick the workspace the project goes in.")
                }
            }
        }
    }
}

/** A workspace is a folder on the PC: its path is typed (the phone cannot browse the PC), prefilled with the PC's suggestion. */
@Composable
private fun NewWorkspace(nav: Nav, online: Boolean, onCreated: (String) -> Unit) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val suggestedFlow = remember { nav.relay.live("workspace.discover") }
    val suggested by suggestedFlow.collectAsStateWithLifecycle(initialValue = Live())
    val suggestedPath = (suggested.result as? JsonObject)?.s("path").orEmpty()
    var typed by remember { mutableStateOf<String?>(null) }
    var name by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val path = typed ?: suggestedPath
    val ready = path.trim().startsWith("/") && !busy && online
    Slab(Modifier.fillMaxWidth()) {
        T(
            "The workspace's folder is on the PC, not on this phone. Type its full path there; the PC creates the folder if it does not exist yet.",
            style = Relay.type.caption,
            color = c.ink3,
        )
        Gap(12.dp)
        FieldBlock("Folder on the PC") {
            Field(path, { typed = it }, Modifier.fillMaxWidth(), placeholder = "/home/you/code", style = Relay.type.mono, keyboardOptions = codeKeys)
        }
        Gap(12.dp)
        FieldBlock("Name (optional)") {
            Field(name, { name = it }, Modifier.fillMaxWidth(), placeholder = "The folder's name")
        }
        Gap(12.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            ToolKey(
                if (busy) "Adding…" else "Add workspace",
                {
                    scope.pcCall(
                        nav,
                        "workspace.create",
                        buildJsonObject {
                            put("path", path.trim())
                            name.trim().ifEmpty { null }?.let { put("name", it) }
                        },
                        busy = { busy = it },
                        done = "Workspace added",
                    ) { answer ->
                        (answer as? JsonObject)?.l("id")?.let { onCreated(it.toString()) }
                    }
                },
                kind = KeyKind.Primary,
                enabled = ready,
            )
        }
    }
}

@Composable
private fun OnThisPc(nav: Nav, workspace: Workspace, knownPaths: Set<String>, online: Boolean) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    var scan by remember(workspace.path) { mutableIntStateOf(0) }
    val flow = remember(workspace.path, scan) {
        nav.relay.live("workspace.discover", buildJsonObject { put("path", workspace.path) })
    }
    val found by flow.collectAsStateWithLifecycle(initialValue = Live())
    val repos = (found.result as? JsonObject)?.arr("repositories")?.mapNotNull { it.decodeAs<LocalRepo>() }.orEmpty()
    var adding by remember(workspace.path) { mutableStateOf<String?>(null) }
    val workspaceId = workspace.id.toLongOrNull()
    val canAdd = online && workspaceId != null && adding == null

    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        T(
            "Repositories in ${workspace.name.ifBlank { workspace.path }}",
            Modifier.weight(1f),
            Relay.type.section,
            c.ink3,
            maxLines = 1,
        )
        ToolKey("Scan again", { scan += 1 }, kind = KeyKind.Quiet, glyph = "refresh")
    }
    Gap(4.dp)
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
        when {
            found.result == null -> Hint(found.error?.message ?: "Looking for Git repositories…")
            repos.isEmpty() -> Hint("No Git repositories in ${workspace.path}. Clone one from GitHub instead.")
            else -> repos.forEachIndexed { index, repo ->
                if (index > 0) Hairline()
                val added = repo.path in knownPaths
                val busy = adding == repo.path
                ListRow(
                    onClick = if (added || !canAdd) null else ({
                        scope.pcCall(
                            nav,
                            "project.add",
                            buildJsonObject { put("workspace_id", workspaceId); put("path", repo.path) },
                            busy = { adding = if (it) repo.path else null },
                            done = "${repo.name} added",
                        ) { answer ->
                            (answer as? JsonObject)?.l("id")?.let { nav.project(it) }
                        }
                    }),
                    padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                ) {
                    Glyph("folder", 18.dp, c.ink3)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        T(repo.name, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                        T(repo.path, style = Relay.type.mono, color = c.ink3, maxLines = 1)
                    }
                    Pill(
                        if (added) "Added" else if (busy) "Adding…" else "Add",
                        dot = if (busy) c.live else null,
                    )
                }
            }
        }
    }
    if (!found.fresh && found.result != null) StaleNote(found.at)
}

@Composable
private fun FromGitHub(nav: Nav, workspace: Workspace, online: Boolean) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val workspaceId = workspace.id.toLongOrNull()
    val statusFlow = remember { nav.relay.live("github.status") }
    val statusLive by statusFlow.collectAsStateWithLifecycle(initialValue = Live())
    val status = statusLive.result as? JsonObject
    val installed = status?.b("installed") == true
    val connected = status?.b("connected") == true
    val login = status?.s("login")
    val reposFlow = remember(connected) { if (connected) nav.relay.live("github.repo.list") else flowOf(Live()) }
    val reposLive by reposFlow.collectAsStateWithLifecycle(initialValue = Live())
    val repos = (reposLive.result as? JsonObject)?.arr("repositories")?.mapNotNull { it.decodeAs<GitHubRepo>() }.orEmpty()
    var search by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    var dest by remember { mutableStateOf("") }
    var cloning by remember { mutableStateOf<String?>(null) }
    var signingIn by remember { mutableStateOf(false) }
    var toClone by remember { mutableStateOf<Pair<String, String>?>(null) }
    val canClone = online && workspaceId != null && cloning == null

    fun startClone(cloneUrl: String, label: String) {
        val id = workspaceId ?: return
        scope.pcCall(
            nav,
            "project.clone",
            buildJsonObject {
                put("workspace_id", id)
                put("url", cloneUrl)
                dest.trim().ifEmpty { null }?.let { put("dest", it) }
            },
            busy = { cloning = if (it) cloneUrl else null },
            done = "Cloned $label",
        ) { answer ->
            (answer as? JsonObject)?.o("project")?.l("id")?.let { nav.project(it) }
        }
    }

    when {
        status == null -> Hint(statusLive.error?.message ?: "Loading…")
        !installed -> Empty(
            "cloud",
            "GitHub CLI not installed",
            "Install gh on the PC and sign in there to list your repositories. You can still clone any URL below.",
        )
        !connected -> Empty(
            "cloud",
            "Sign in on the PC",
            if (signingIn) {
                "A browser opened on the PC. Finish signing in there; this updates by itself."
            } else {
                "The PC is not signed in to GitHub. Signing in opens a browser on the PC, so do it at the PC. You can still clone any URL below."
            },
        ) {
            if (!signingIn) {
                ToolKey(
                    "Sign in on the PC",
                    { scope.pcCall(nav, "github.connect") { signingIn = true } },
                    kind = KeyKind.Primary,
                    glyph = "user",
                    enabled = online,
                )
            }
        }
        else -> {
            SectionLabel("GitHub · ${login ?: "signed in"}")
            Field(search, { search = it }, Modifier.fillMaxWidth(), placeholder = "Search repositories")
            Gap(8.dp)
            val needle = search.trim().lowercase()
            val matches = repos.filter {
                needle.isEmpty() || it.fullName.lowercase().contains(needle) || (it.description ?: "").lowercase().contains(needle)
            }
            Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                when {
                    reposLive.result == null -> Hint(reposLive.error?.message ?: "Asking GitHub…")
                    matches.isEmpty() -> Hint("No repository matches.")
                    else -> matches.take(SHOWN_REPOS).forEachIndexed { index, repo ->
                        if (index > 0) Hairline()
                        ListRow(
                            onClick = if (canClone) ({ toClone = repo.cloneUrl to repo.fullName }) else null,
                            padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                        ) {
                            Glyph("cloud", 18.dp, c.ink3)
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                T(repo.fullName, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                                repo.description?.takeIf { it.isNotBlank() }?.let {
                                    T(it, style = Relay.type.caption, color = c.ink3, maxLines = 2)
                                }
                            }
                            when {
                                cloning == repo.cloneUrl -> Pill("Cloning…", dot = c.live)
                                repo.archived -> Pill("Archived")
                                repo.isPrivate -> Pill("Private")
                            }
                        }
                    }
                }
            }
            if (matches.size > SHOWN_REPOS) {
                Hint("${matches.size - SHOWN_REPOS} more; search to narrow the list.")
            }
        }
    }
    // Any URL clones without GitHub's CLI, so this stays when gh is missing or signed out.
    Gap(12.dp)
    Slab(Modifier.fillMaxWidth()) {
        FieldBlock("Folder name (optional)", "Created inside ${workspace.path}.") {
            Field(dest, { dest = it }, Modifier.fillMaxWidth(), placeholder = "The repository's name", style = Relay.type.mono, keyboardOptions = codeKeys)
        }
        Gap(12.dp)
        FieldBlock("Or clone a URL") {
            Field(
                url,
                { url = it },
                Modifier.fillMaxWidth(),
                placeholder = "https://github.com/owner/repo.git",
                style = Relay.type.mono,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
            )
        }
        if (cloning != null) {
            Gap(12.dp)
            T(
                "Cloning on the PC. This can take several minutes; keep the app open until it finishes.",
                style = Relay.type.caption,
                color = c.ink2,
            )
            Gap(8.dp)
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth(), color = c.ink, trackColor = c.track)
        }
        Gap(12.dp)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            ToolKey(
                "Clone URL",
                { toClone = url.trim() to url.trim() },
                kind = KeyKind.Primary,
                glyph = "download",
                enabled = canClone && url.trim().isNotEmpty(),
            )
        }
    }

    toClone?.let { (cloneUrl, label) ->
        Ask(
            title = "Clone $label?",
            body = "Into ${workspace.path}${dest.trim().let { if (it.isEmpty()) "" else "/$it" }}. A large repository can take several minutes; keep the app open until it finishes.",
            onDismiss = { toClone = null },
        ) {
            ToolKey("Cancel", { toClone = null }, kind = KeyKind.Quiet)
            ToolKey(
                "Clone",
                {
                    toClone = null
                    startClone(cloneUrl, label)
                },
                kind = KeyKind.Primary,
            )
        }
    }
}

/** The agent CLIs on the PC, its usage, and its database backups. */
@Composable
fun PcToolsScreen(nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val providersFlow = remember { nav.relay.live("provider.list") }
    val providersLive by providersFlow.collectAsStateWithLifecycle(initialValue = Live())
    val usageFlow = remember { nav.relay.live("usage.get") }
    val usageLive by usageFlow.collectAsStateWithLifecycle(initialValue = Live())
    var backupsVersion by remember { mutableIntStateOf(0) }
    val backupsFlow = remember(backupsVersion) { nav.relay.live("app.backup.list") }
    val backupsLive by backupsFlow.collectAsStateWithLifecycle(initialValue = Live())
    val scope = rememberCoroutineScope()

    val providers = (providersLive.result as? JsonObject)?.arr("providers")?.mapNotNull { it.decodeAs<ProviderRow>() }.orEmpty()
    val usage = parseUsage(usageLive.result)
    val backups = (backupsLive.result as? JsonObject)?.arr("backups")?.mapNotNull { it.decodeAs<BackupRow>() }.orEmpty()
        .sortedByDescending { epoch(it.createdAt) }
    var checking by remember { mutableStateOf(false) }
    var updating by remember { mutableStateOf<String?>(null) }
    var toUpdate by remember { mutableStateOf<ProviderRow?>(null) }
    val updateNotes = remember { mutableStateMapOf<String, String>() }
    var backingUp by remember { mutableStateOf(false) }

    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Agents and backups", nav::back)
            ScrollBody {
                OfflineNote(s.online)

                SectionLabel("Agent providers") {
                    ToolKey(
                        if (checking) "Checking…" else "Check again",
                        { scope.pcCall(nav, "provider.refresh", busy = { checking = it }) },
                        kind = KeyKind.Quiet,
                        glyph = "refresh",
                        enabled = s.online && !checking,
                    )
                }
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    when {
                        providersLive.result == null -> Hint(providersLive.error?.message ?: "Loading…")
                        providers.isEmpty() -> Hint("This PC reports no agent provider.")
                        else -> providers.forEachIndexed { index, provider ->
                            if (index > 0) Hairline()
                            ProviderLine(
                                provider = provider,
                                note = updateNotes[provider.provider],
                                updating = updating == provider.provider,
                                online = s.online,
                                onUpdate = { toUpdate = provider },
                            )
                        }
                    }
                }
                if (!providersLive.fresh && providersLive.result != null) StaleNote(providersLive.at)

                SectionLabel("Usage")
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    val reported = usage.filter { it.windows.isNotEmpty() }
                    when {
                        usageLive.result == null -> Hint(usageLive.error?.message ?: "Loading…")
                        reported.isEmpty() -> Hint("Nothing reported yet. A provider reports its usage while an agent runs.")
                        else -> reported.forEachIndexed { groupIndex, group ->
                            if (groupIndex > 0) Hairline()
                            Gap(10.dp)
                            T(providerTitle(group.provider), style = Relay.type.uiMedium, color = c.ink2)
                            group.windows.forEach { window ->
                                Gap(10.dp)
                                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                                    T(window.name, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                                    T("${window.pct.toInt()}%", style = Relay.type.mono, color = c.ink2)
                                }
                                Gap(6.dp)
                                Meter(fraction = (window.pct / 100.0).toFloat())
                                window.resets?.let {
                                    Gap(4.dp)
                                    T("Resets in $it", style = Relay.type.caption, color = c.ink3)
                                }
                            }
                            Gap(6.dp)
                        }
                    }
                }
                if (!usageLive.fresh && usageLive.result != null) StaleNote(usageLive.at)

                SectionLabel("Backups") {
                    ToolKey(
                        if (backingUp) "Backing up…" else "Back up now",
                        {
                            scope.pcCall(
                                nav,
                                "app.backup.now",
                                busy = { backingUp = it },
                            ) { answer ->
                                val bytes = (answer as? JsonObject)?.l("bytes") ?: 0L
                                nav.shell.toast("Backed up · ${formatBytes(bytes)}")
                                backupsVersion += 1
                            }
                        },
                        kind = KeyKind.Primary,
                        glyph = "save",
                        enabled = s.online && !backingUp,
                    )
                }
                T(
                    "A copy of the PC's Relay database: boards, notes and history. The PC keeps the last five.",
                    style = Relay.type.caption,
                    color = c.ink3,
                )
                Gap(6.dp)
                Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                    when {
                        backupsLive.result == null -> Hint(backupsLive.error?.message ?: "Loading…")
                        backups.isEmpty() -> Hint("No backup on the PC yet.")
                        else -> backups.forEachIndexed { index, backup ->
                            if (index > 0) Hairline()
                            Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                                T(
                                    stamp(epoch(backup.createdAt)).ifBlank { backup.createdAt },
                                    style = Relay.type.uiMedium,
                                    color = c.ink,
                                    maxLines = 1,
                                )
                                T("${formatBytes(backup.bytes)} · ${backup.reason}", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                                T(backup.path, style = Relay.type.mono, color = c.ink3, maxLines = 1)
                            }
                        }
                    }
                }
                if (!backupsLive.fresh && backupsLive.result != null) StaleNote(backupsLive.at)
            }
        }
    }

    toUpdate?.let { provider ->
        Ask(
            title = "Update ${providerTitle(provider.provider)}?",
            body = "The PC updates it in the background. Running sessions keep their version; new ones use the update. Relay skips it while a session of this provider is live.",
            onDismiss = { toUpdate = null },
        ) {
            ToolKey("Cancel", { toUpdate = null }, kind = KeyKind.Quiet)
            ToolKey(
                "Update",
                {
                    toUpdate = null
                    scope.pcCall(
                        nav,
                        "provider.update",
                        buildJsonObject { put("provider", provider.provider) },
                        busy = { updating = if (it) provider.provider else null },
                    ) { answer ->
                        (answer as? JsonObject)?.s("message")?.let { updateNotes[provider.provider] = it }
                    }
                },
                kind = KeyKind.Primary,
            )
        }
    }
}

@Composable
private fun ProviderLine(provider: ProviderRow, note: String?, updating: Boolean, online: Boolean, onUpdate: () -> Unit) {
    val c = Relay.colors
    val detail = if (provider.installed) {
        listOf(
            provider.version?.let { "Version $it" } ?: "Version unknown",
            provider.signedInAs?.let { "signed in as $it" } ?: "not signed in",
        ).joinToString(" · ")
    } else {
        "Not installed on the PC"
    }
    Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                T(providerTitle(provider.provider), style = Relay.type.body, color = c.ink, weight = FontWeight.Medium, maxLines = 1)
                T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 2)
            }
            if (provider.installed && !provider.guarded) Pill("No guardrail hooks", dot = c.waiting)
        }
        provider.path?.let { T(it, style = Relay.type.mono, color = c.ink3, maxLines = 1) }
        note?.let { T(it, style = Relay.type.caption, color = c.ink2, maxLines = 3) }
        if (provider.installed) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                ToolKey(
                    if (updating) "Updating…" else "Update",
                    onUpdate,
                    glyph = "download",
                    enabled = online && !updating,
                )
            }
        }
    }
}

