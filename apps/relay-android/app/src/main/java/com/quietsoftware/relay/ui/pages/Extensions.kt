package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.code.NotReadYet
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.kit.switchRow
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** A skill as `skill.list` returns it: enabled per project through `enabled_in`. */
@Serializable
private data class SkillRow(
    val id: Long,
    val name: String = "",
    val description: String = "",
    @SerialName("source_url") val sourceUrl: String? = null,
    @SerialName("enabled_in") val enabledIn: List<Long> = emptyList(),
)

/** A plugin as `plugin.list` returns it (types.rs `Plugin`). */
@Serializable
private data class PluginRow(
    val id: String,
    val name: String = "",
    val version: String = "",
    val summary: String = "",
    val skills: List<JsonElement> = emptyList(),
    @SerialName("mcp_servers") val mcpServers: List<JsonElement> = emptyList(),
    @SerialName("enabled_in") val enabledIn: List<Long> = emptyList(),
)

@Composable
fun SkillsScreen(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val pid = s.project?.num
    val flow = remember(pid) { nav.relay.live("skill.list", projectPayload(pid)) }
    val live by flow.collectAsStateWithLifecycle(initialValue = Live())
    val skills = (live.result as? JsonObject)?.arr("skills")?.mapNotNull { it.decodeAs<SkillRow>() }.orEmpty()
    // The switch shows what was just set until the PC's list says so; a new list drops it.
    val overrides = remember(live.result) { mutableStateMapOf<String, Boolean>() }
    val hint = if (pid == null) "Open a project to switch them for it." else "The switches here are for ${s.project?.name ?: "this project"}."
    SpaceFrame(nav) {
        ExtensionBody(
            title = "Skills",
            // The PC's own intro (tools_skills.rs), then which project the switches are for.
            intro = "Reusable agent instructions, written into every checkout and provider home as real skill folders. A new skill is on everywhere; switch it off where it does not belong. $hint",
            live = live,
            count = skills.size,
            emptyGlyph = "skills",
            emptyTitle = "No skills installed",
            emptyBody = "A skill is a Markdown file of instructions agents load when a task calls for it.",
        ) {
            skills.forEachIndexed { index, skill ->
                if (index > 0) Hairline()
                val key = skill.id.toString()
                ExtensionSwitch(
                    title = skill.name,
                    detail = skill.description.ifBlank { skill.sourceUrl.orEmpty() },
                    checked = overrides[key] ?: (pid != null && skill.enabledIn.contains(pid)),
                    enabled = pid != null && s.online,
                ) { on ->
                    if (pid != null) {
                        nav.shell.act("skill.enable", buildJsonObject { put("skill_id", skill.id); put("project_id", pid); put("enabled", on) }) {
                            overrides[key] = on
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun PluginsScreen(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val pid = s.project?.num
    val flow = remember(pid) { nav.relay.live("plugin.list", projectPayload(pid)) }
    val live by flow.collectAsStateWithLifecycle(initialValue = Live())
    val plugins = (live.result as? JsonObject)?.arr("plugins")?.mapNotNull { it.decodeAs<PluginRow>() }.orEmpty()
    val overrides = remember(live.result) { mutableStateMapOf<String, Boolean>() }
    val hint = if (pid == null) "Open a project to switch them for it." else "The switches here are for ${s.project?.name ?: "this project"}."
    SpaceFrame(nav) {
        ExtensionBody(
            title = "Plugins",
            // The PC's own intro (tools_plugins.rs), then which project the switches are for.
            intro = "Whole toolkits for a project's agents: skill folders, standing rules in every brief, and MCP tools. Switch one on per project. $hint",
            live = live,
            count = plugins.size,
            emptyGlyph = "plugins",
            emptyTitle = "No plugins found",
            emptyBody = "Relay has no bundled plugins on this PC.",
        ) {
            plugins.forEachIndexed { index, plugin ->
                if (index > 0) Hairline()
                ExtensionSwitch(
                    title = listOf(plugin.name, plugin.version).filter { it.isNotBlank() }.joinToString(" "),
                    detail = listOf(plugin.summary, "${plural(plugin.skills.size, "skill")} · ${plural(plugin.mcpServers.size, "MCP server")}").filter { it.isNotBlank() }.joinToString("\n"),
                    checked = overrides[plugin.id] ?: (pid != null && plugin.enabledIn.contains(pid)),
                    enabled = pid != null && s.online,
                ) { on ->
                    if (pid != null) {
                        nav.shell.act("plugin.enable", buildJsonObject { put("plugin_id", plugin.id); put("project_id", pid); put("enabled", on) }) {
                            overrides[plugin.id] = on
                        }
                    }
                }
            }
        }
    }
}

/** The list's payload: the current project's, or every project's when there is none. */
private fun projectPayload(pid: Long?): JsonObject = buildJsonObject { if (pid != null) put("project_id", pid) }

/** A page of rows from a PC query: its intro, the phone's copy's age, and the list or what stands in for it. */
@Composable
private fun ExtensionBody(
    title: String,
    intro: String,
    live: Live,
    count: Int,
    emptyGlyph: String,
    emptyTitle: String,
    emptyBody: String,
    rows: @Composable () -> Unit,
) {
    val c = Relay.colors
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        LazyColumn(
            Modifier.column().fillMaxSize(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp),
        ) {
            item { T(title, style = Relay.type.heading, color = c.ink) }
            item { T(intro, style = Relay.type.caption, color = c.ink3) }
            if (!live.fresh && live.result != null) item { StaleNote(live.at) }
            when {
                live.result == null && live.error?.code == "link.down" -> item { NotReadYet() }
                live.result == null && live.error != null -> item { Empty(emptyGlyph, "The PC has not answered", live.error?.message) }
                live.result == null -> item { T("Loading…", style = Relay.type.caption, color = c.ink3) }
                count == 0 -> item { Empty(emptyGlyph, emptyTitle, emptyBody) }
                else -> item {
                    Gap(6.dp)
                    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) { rows() }
                }
            }
        }
    }
}

@Composable
private fun ExtensionSwitch(title: String, detail: String, checked: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    val c = Relay.colors
    // The whole row is the switch: its words take the tap too.
    Row(Modifier.fillMaxWidth().switchRow(checked, enabled, onChange).padding(vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(title, style = Relay.type.body, color = c.ink, weight = FontWeight.Medium, maxLines = 2)
            if (detail.isNotBlank()) T(detail, style = Relay.type.caption, color = c.ink3, maxLines = 3)
        }
        Spacer(Modifier.width(12.dp))
        Toggle(checked, null, enabled = enabled)
    }
}
