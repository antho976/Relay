package com.quietsoftware.relay.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.model.Lamp
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Thread
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.NavRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.theme.Relay
import java.time.LocalDate
import java.time.ZoneId

/** Where the sidebar sends the person. */
interface SidebarNav {
    fun board()
    fun notes()
    fun inbox()
    fun outbox()
    fun skills()
    fun plugins()
    fun project(id: Long)
    fun projectSettings(id: Long)
    fun addProject()
    fun pc()
    fun settings()
    fun newThread()
    fun thread(id: String)
    fun tally()
    fun arbiter()
    fun avex()
}

/**
 * The sidebar (theme.css, 248px on the PC), as a drawer: Dev's navigation and the workspace
 * tree, or Threads' keys and thread list; the PC and the person at the foot.
 */
@Composable
fun Sidebar(s: ShellState, route: String?, nav: SidebarNav, space: String = s.prefs.space) {
    val c = Relay.colors
    Column(Modifier.fillMaxHeight().width(300.dp).background(c.wall).statusBarsPadding().navigationBarsPadding()) {
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(horizontal = 10.dp, vertical = 10.dp)) {
            if (space == "threads") threads(s, route, nav) else dev(s, route, nav)
        }
        Hairline(Modifier.padding(horizontal = 10.dp), c.edge)
        Column(Modifier.padding(10.dp)) {
            ListRow(onClick = nav::pc) {
                Dot(linkColor(s.link), 7.dp)
                Column(Modifier.weight(1f)) {
                    T(s.pc?.name?.ifBlank { null } ?: "Pair a PC", style = Relay.type.uiMedium, color = c.ink, maxLines = 1)
                    T(linkLine(s), style = Relay.type.mono, color = c.ink3, maxLines = 1)
                }
                Glyph("chevron-right", 14.dp)
            }
            ListRow(onClick = nav::settings) {
                Box(Modifier.size(30.dp).clip(CircleShape).background(c.track), contentAlignment = Alignment.Center) {
                    T(s.prefs.name.take(1).uppercase().ifEmpty { "·" }, style = Relay.type.caption, color = c.ink, weight = FontWeight.SemiBold)
                }
                T(s.prefs.name.ifBlank { "You" }, Modifier.weight(1f), Relay.type.uiMedium, c.ink)
                Glyph("gear", 17.dp)
            }
        }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.dev(s: ShellState, route: String?, nav: SidebarNav) {
    item { NavRow("Board", "board", route?.startsWith("board") == true, onClick = nav::board) }
    item { NavRow("Notes", "brief", route?.startsWith("notes") == true, onClick = nav::notes) }
    item {
        NavRow("Inbox", "inbox", route == "inbox", count = (s.holds.size + s.unread).takeIf { it > 0 }?.toString(), onClick = nav::inbox)
    }
    if (s.outbox.isNotEmpty()) item { NavRow("Outbox", "outbox", route == "outbox", count = s.outbox.size.toString(), onClick = nav::outbox) }
    item { NavRow("Skills", "skills", route == "skills", onClick = nav::skills) }
    item { NavRow("Plugins", "dashboard", route == "plugins", onClick = nav::plugins) }
    item { SectionLabel("Workspaces") }
    val byWorkspace = s.projects.groupBy { it.workspaceId }
    val named = s.workspaces.map { it to byWorkspace[it.id].orEmpty() }
    val orphans = s.projects.filter { p -> s.workspaces.none { it.id == p.workspaceId } }
    for ((w, projects) in named) {
        item(key = "w${w.id}") {
            T(w.name.ifBlank { w.path.substringAfterLast('/') }, Modifier.padding(start = 12.dp, top = 8.dp, bottom = 2.dp), Relay.type.ui, Relay.colors.ink, weight = FontWeight.SemiBold)
        }
        items(projects, key = { "p${it.id}" }) { p -> ProjectRow(p, s, nav) }
    }
    if (orphans.isNotEmpty()) {
        item { T("Other projects", Modifier.padding(start = 12.dp, top = 8.dp, bottom = 2.dp), Relay.type.ui, Relay.colors.ink, weight = FontWeight.SemiBold) }
        items(orphans, key = { "o${it.id}" }) { p -> ProjectRow(p, s, nav) }
    }
    item { NavRow("Add a project", "folder-plus", onClick = nav::addProject) }
    if (s.projects.isEmpty()) item {
        T(if (s.online) "No projects on this PC yet." else "Projects show here once the phone has reached the PC.", Modifier.padding(12.dp), Relay.type.caption, Relay.colors.ink3)
    }
}

@Composable
private fun ProjectRow(p: Project, s: ShellState, nav: SidebarNav) {
    val c = Relay.colors
    val mine = s.sessions.filter { it.projectId == p.num }
    val lamp = when {
        mine.any { it.state == "blocked" } -> Lamp.Held
        mine.any { it.state == "running" } -> Lamp.Live
        else -> Lamp.Off
    }
    val live = s.liveIn(p.num)
    ListRow(selected = s.project?.id == p.id, onClick = { p.num?.let(nav::project) }, onLongClick = { p.num?.let(nav::projectSettings) }, padding = PaddingValues(start = 18.dp, end = 12.dp, top = 6.dp, bottom = 6.dp)) {
        LampDot(lamp, 5.dp)
        T(p.name, Modifier.weight(1f), Relay.type.ui, if (s.project?.id == p.id) c.ink else c.ink2, maxLines = 1)
        if (p.pinned) Glyph("pin", 12.dp)
        if (live > 0) T(live.toString(), style = Relay.type.mono, color = c.ink3)
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.threads(s: ShellState, route: String?, nav: SidebarNav) {
    item {
        Key("New thread", nav::newThread, Modifier.fillMaxWidth().padding(bottom = 6.dp), KeyKind.Plain, glyph = "plus")
    }
    item { NavRow("Tally", "home", route == "tally", onClick = nav::tally) }
    item { NavRow("Arbiter", "arbiter", route == "arbiter", onClick = nav::arbiter) }
    item { NavRow("Avex", "avex", route == "avex", onClick = nav::avex) }
    val zone = ZoneId.systemDefault()
    val today = LocalDate.now(zone)
    fun group(t: Thread): String {
        val d = java.time.Instant.ofEpochMilli(epoch(t.updatedAt)).atZone(zone).toLocalDate()
        return when {
            !d.isBefore(today) -> "Today"
            d == today.minusDays(1) -> "Yesterday"
            d.isAfter(today.minusDays(7)) -> "This week"
            else -> "Earlier"
        }
    }
    val groups = s.threads.groupBy(::group)
    for (name in listOf("Today", "Yesterday", "This week", "Earlier")) {
        val list = groups[name] ?: continue
        item { SectionLabel(name) }
        items(list, key = { "t${it.id}" }) { t ->
            ListRow(selected = route == "thread/${t.id}", onClick = { nav.thread(t.id) }) {
                T(t.title.ifBlank { "New thread" }, Modifier.weight(1f), Relay.type.ui, Relay.colors.ink2, maxLines = 1)
                if (t.working) Dot(Relay.colors.live, 6.dp)
            }
        }
    }
    if (s.threads.isEmpty()) item { T("No threads yet.", Modifier.padding(12.dp), Relay.type.caption, Relay.colors.ink3) }
}
