package com.quietsoftware.relay.ui.pages

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.code.LiveWait
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** A skill with its whole body (`skill.get`). */
@Serializable
private data class SkillFull(
    val id: Long,
    val name: String = "",
    val description: String = "",
    val body: String = "",
    @SerialName("source_url") val sourceUrl: String? = null,
    @SerialName("source_path") val sourcePath: String? = null,
    val revision: String? = null,
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
)

/** A bundled plugin (types.rs `Plugin`), as `plugin.get` returns it with its rules and documents. */
@Serializable
private data class PluginFull(
    val id: String,
    val name: String = "",
    val version: String = "",
    val category: String = "",
    val summary: String = "",
    val description: String = "",
    val skills: List<PluginSkill> = emptyList(),
    @SerialName("mcp_servers") val mcpServers: List<McpServer> = emptyList(),
    val docs: List<String> = emptyList(),
)

@Serializable
private data class PluginSkill(val name: String, val description: String = "", val files: Int = 0)

@Serializable
private data class McpServer(val name: String, val description: String = "", val tools: List<String> = emptyList())

@Serializable
private data class PluginDoc(val path: String = "", val body: String = "")

@Serializable
private data class PluginGot(
    val plugin: PluginFull? = null,
    val instructions: String = "",
    val docs: List<PluginDoc> = emptyList(),
    val skill: PluginDoc? = null,
)

/**
 * One skill, read only (tools_skills.rs `detail`): where it came from, what it is for, its facts,
 * and its whole SKILL.md as the PC shows it, source and all. Editing and removing stay on the PC.
 */
@Composable
fun SkillScreen(id: Long, nav: Nav) {
    val c = Relay.colors
    val live by remember(id) { nav.relay.live("skill.get", buildJsonObject { put("skill_id", id) }) }.collectAsStateWithLifecycle(Live())
    val skill = live.result?.decodeAs<SkillFull>()
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(skill?.name ?: "Skill", nav::back, subtitle = "Skill")
            ScrollBody {
                if (skill == null) {
                    LiveWait(live, page = true)
                    return@ScrollBody
                }
                T(skill.name, style = Relay.type.heading, color = c.ink)
                T(skill.sourceUrl?.let { "From ${ownerRepo(it)} on GitHub" } ?: "Written in Relay", style = Relay.type.caption, color = c.ink3)
                if (skill.description.isNotBlank()) {
                    Gap(10.dp)
                    T(skill.description, style = Relay.type.body, color = c.ink2)
                }
                if (!live.fresh) StaleNote(live.at)
                Gap(6.dp)
                Facts(
                    listOfNotNull(
                        skill.sourceUrl?.let { "Source" to webUrl(it) },
                        skill.sourcePath?.let { "Path" to it },
                        skill.revision?.let { "Revision" to it.take(8) },
                        "Installed" to (epoch(skill.createdAt).takeIf { it > 0 }?.let { ago(it) } ?: skill.createdAt.take(10)),
                        "Updated" to (epoch(skill.updatedAt).takeIf { it > 0 }?.let { ago(it) } ?: skill.updatedAt.take(10)),
                    ),
                )
                SectionLabel("Instructions")
                Source(skill.body)
            }
        }
    }
}

/**
 * One bundled plugin, read only (tools_plugins.rs `detail`): what it is, its skills (each
 * SKILL.md on a tap), its MCP tools, the rules every agent receives and its documents. It is
 * switched on per project from the list.
 */
@Composable
fun PluginScreen(id: String, nav: Nav) {
    val c = Relay.colors
    val live by remember(id) { nav.relay.live("plugin.get", buildJsonObject { put("plugin_id", id) }) }.collectAsStateWithLifecycle(Live())
    val got = live.result?.decodeAs<PluginGot>()
    val plugin = got?.plugin
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(plugin?.name ?: "Plugin", nav::back, subtitle = "Plugin")
            ScrollBody {
                if (got == null || plugin == null) {
                    LiveWait(live, page = true)
                    return@ScrollBody
                }
                T(plugin.name, style = Relay.type.heading, color = c.ink)
                T(listOf(plugin.category, "v${plugin.version}", "bundled with Relay").filter { it.isNotBlank() }.joinToString(" · "), style = Relay.type.caption, color = c.ink3)
                if (plugin.summary.isNotBlank()) {
                    Gap(10.dp)
                    T(plugin.summary, style = Relay.type.body, color = c.ink)
                }
                if (plugin.description.isNotBlank()) {
                    Gap(8.dp)
                    T(plugin.description, style = Relay.type.ui, color = c.ink2)
                }
                if (!live.fresh) StaleNote(live.at)

                SectionLabel("Skills · ${plugin.skills.size}")
                if (plugin.skills.isEmpty()) T("This plugin ships no skills.", style = Relay.type.caption, color = c.ink3)
                else Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                    plugin.skills.forEachIndexed { index, skill ->
                        if (index > 0) Hairline()
                        PluginSkillRow(plugin.id, skill, nav)
                    }
                }

                SectionLabel("MCP tools · ${plugin.mcpServers.sumOf { it.tools.size }}")
                if (plugin.mcpServers.isEmpty()) T("This plugin adds no MCP servers.", style = Relay.type.caption, color = c.ink3)
                for (server in plugin.mcpServers) {
                    McpBlock(server)
                    Gap(8.dp)
                }
                T("Agents get these tools when they start or resume in a project with the plugin on.", style = Relay.type.caption, color = c.ink3)

                SectionLabel("Rules every agent receives")
                if (got.instructions.isBlank()) T("None.", style = Relay.type.caption, color = c.ink3) else Source(got.instructions)
                for (doc in got.docs) {
                    SectionLabel(doc.path)
                    Source(doc.body)
                }

                SectionLabel("Package")
                Facts(
                    listOf(
                        "Identifier" to plugin.id,
                        "Version" to plugin.version,
                        "MCP servers" to plugin.mcpServers.joinToString(", ") { it.name }.ifEmpty { "None" },
                        "Documents" to plugin.docs.joinToString("\n").ifEmpty { "None" },
                    ),
                )
            }
        }
    }
}

/** A skill of a plugin: its name and what it is for; a tap shows its SKILL.md (`plugin.get` with `skill`). */
@Composable
private fun PluginSkillRow(plugin: String, skill: PluginSkill, nav: Nav) {
    val c = Relay.colors
    var open by rememberSaveable(plugin, skill.name) { mutableStateOf(false) }
    ListRow(onClick = { open = !open }, padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)) {
        Glyph(if (open) "chevron-down" else "chevron-right", 12.dp, c.ink3)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            T(skill.name, style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
            if (skill.description.isNotBlank()) T(skill.description, style = Relay.type.caption, color = c.ink3, maxLines = if (open) Int.MAX_VALUE else 3)
        }
        if (skill.files > 0) T(plural(skill.files, "file"), style = Relay.type.mono, color = c.ink3)
    }
    if (open) {
        val flow = remember(plugin, skill.name) { nav.relay.live("plugin.get", buildJsonObject { put("plugin_id", plugin); put("skill", skill.name) }) }
        val live by flow.collectAsStateWithLifecycle(Live())
        val body = (live.result as? JsonObject)?.o("skill")?.decodeAs<PluginDoc>()?.body
        Box(Modifier.padding(start = 10.dp, end = 10.dp, bottom = 10.dp)) {
            if (body != null) Source(body) else LiveWait(live, reading = "Loading SKILL.md…")
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun McpBlock(server: McpServer) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            T(server.name, Modifier.weight(1f), Relay.type.uiMedium, c.ink, maxLines = 1)
            T(plural(server.tools.size, "tool"), style = Relay.type.mono, color = c.ink3)
        }
        if (server.description.isNotBlank()) {
            Gap(4.dp)
            T(server.description, style = Relay.type.caption, color = c.ink2)
        }
        if (server.tools.isNotEmpty()) {
            Gap(8.dp)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for (tool in server.tools) Pill(tool, color = c.ink2)
            }
        }
    }
}

/** Markdown as the PC shows it here: its source, read only, in a selectable block. */
@Composable
private fun Source(text: String) {
    val c = Relay.colors
    SelectionContainer {
        Box(Modifier.fillMaxWidth().clip(Radii.key).background(c.raised).border(1.dp, c.edge, Radii.key).padding(horizontal = 12.dp, vertical = 10.dp)) {
            T(text.trimEnd().ifEmpty { " " }, style = Relay.type.code, color = c.ink2)
        }
    }
}

/** Name and value, one per line (tools_market.rs `facts`). */
@Composable
private fun Facts(rows: List<Pair<String, String>>) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
        rows.forEachIndexed { index, (name, value) ->
            if (index > 0) Hairline()
            Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                T(name, Modifier.width(92.dp), Relay.type.caption, c.ink3)
                SelectionContainer(Modifier.weight(1f)) { T(value, style = Relay.type.mono, color = c.ink2) }
            }
        }
    }
}

/** A repository's web address: its clone URL without `.git`. */
private fun webUrl(url: String): String = url.trimEnd('/').removeSuffix(".git")

/** `owner/repository` from a GitHub URL (tools_skills.rs `source`). */
private fun ownerRepo(url: String): String {
    val parts = webUrl(url).split('/').filter { it.isNotEmpty() }
    return if (parts.size >= 2) "${parts[parts.size - 2]}/${parts.last()}" else url
}
