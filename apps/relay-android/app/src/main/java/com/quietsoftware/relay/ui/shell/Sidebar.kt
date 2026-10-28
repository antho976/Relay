package com.quietsoftware.relay.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.kit.ago
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
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.ui.dev.AgentBadge
import com.quietsoftware.relay.ui.dev.agentTitle
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.NavRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.theme.Relay

/** Where the sidebar sends the person. */
interface SidebarNav {
    fun home()
    fun agents()
    fun chat(name: String)
    fun search()
    fun board()
    fun changes()
    fun files()
    fun designs()
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
    fun aide()
}

/**
 * The drawer, as Claude's app has one (design 14, "Drawer"): New chat, the places, the three
 * money and training spaces as tiles, the agents at work and the recent threads, then the person
 * and their PC at the foot. One list for both spaces: nothing here switches between them.
 */
@Composable
fun Sidebar(s: ShellState, route: String?, nav: SidebarNav, @Suppress("UNUSED_PARAMETER") space: String = s.prefs.space) {
    val c = Relay.colors
    Column(Modifier.fillMaxHeight().width(320.dp).background(c.pane).statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(start = 20.dp, end = 8.dp, top = 8.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            T("Relay", Modifier.weight(1f), Relay.type.brand.copy(fontSize = 22.sp), c.ink)
            IconKey("search", nav::search, size = 19.dp, tint = c.ink, label = "Search")
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).heightIn(min = TOUCH).clip(Radii.key).background(c.accentSoft)
                .clickable(onClick = nav::home).padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Glyph("compose", 18.dp, c.accentText)
            T("New chat", style = Relay.type.nav, color = c.accentText, weight = FontWeight.SemiBold)
        }
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
            val live = s.sessions.count { it.attachable }
            item { NavRow("Agents", "agents", route == "agents", count = live.takeIf { it > 0 }?.toString(), onClick = nav::agents) }
            item { NavRow("Board", "board", route?.startsWith("board") == true, onClick = nav::board) }
            item {
                NavRow("Inbox", "inbox", route == "inbox", count = (s.holds.size + s.unread).takeIf { it > 0 }?.toString(), onClick = nav::inbox)
            }
            item { NavRow("Changes", "branch", route?.startsWith("git/") == true, onClick = nav::changes) }
            item { NavRow("Files", "folder", route?.startsWith("files/") == true, onClick = nav::files) }
            item { NavRow("Designs", "pen-tool", route?.startsWith("design") == true, onClick = nav::designs) }
            item { NavRow("Notes", "notes", route?.startsWith("notes") == true, onClick = nav::notes) }
            item { NavRow("Aide", "aide", route == "aide", onClick = nav::aide) }
            if (s.outbox.isNotEmpty()) item { NavRow("Waiting to send", "outbox", route == "outbox", count = s.outbox.size.toString(), onClick = nav::outbox) }
            item {
                Row(Modifier.fillMaxWidth().padding(top = 12.dp, bottom = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SpaceTile("Tally", "wallet", Color(0xFF8CC08F), route == "tally", nav::tally, Modifier.weight(1f))
                    SpaceTile("Arbiter", "arbiter", Color(0xFF8FB3E8), route == "arbiter", nav::arbiter, Modifier.weight(1f))
                    SpaceTile("Avex", "avex", Color(0xFFE5B567), route == "avex", nav::avex, Modifier.weight(1f))
                }
            }
            val open = s.sessions.filter { it.attachable }.sortedByDescending { it.blocked }
            if (open.isNotEmpty()) {
                // Every agent with a live terminal, each line saying whether it works, waits or needs you.
                item { Eyebrow("Open agents", Modifier.padding(start = 4.dp, top = 16.dp, bottom = 4.dp)) }
                val mixed = open.map { it.projectId }.distinct().size > 1
                items(open, key = { "s${it.name}" }) { a ->
                    val blocked = a.blocked
                    val title = agentTitle(a)
                    AgentRow(
                        a,
                        title = title,
                        line = listOfNotNull(
                            a.name.takeIf { it != title },
                            s.projects.firstOrNull { it.num == a.projectId }?.name?.takeIf { mixed },
                            if (blocked) "needs you" else if (a.state == "idle") "ready" else "working",
                        ).joinToString(" · "),
                        lineColor = if (blocked) c.waiting else c.ink3,
                    ) { nav.chat(a.name) }
                }
            }
            val recent = s.threads.take(8)
            if (recent.isNotEmpty()) {
                item { Eyebrow("Recent", Modifier.padding(start = 4.dp, top = 16.dp, bottom = 4.dp)) }
                items(recent, key = { "t${it.id}" }) { t ->
                    ChatRow(
                        title = t.title.ifBlank { "New thread" },
                        line = if (t.working) "answering" else ago(epoch(t.updatedAt)),
                        dot = if (t.working) c.live else null,
                        selected = route == "thread/${t.id}",
                    ) { nav.thread(t.id) }
                }
            }
            item { Eyebrow("Projects", Modifier.padding(start = 4.dp, top = 16.dp, bottom = 4.dp)) }
            items(s.projects, key = { "p${it.id}" }) { p -> ProjectRow(p, s, nav) }
            item { NavRow("Add a project", "folder-plus", onClick = nav::addProject) }
        }
        Hairline(color = c.edge)
        Row(
            Modifier.fillMaxWidth().clickable(onClick = nav::pc).padding(start = 20.dp, end = 8.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // The person's initial on the accent, or the person glyph until they give a name (Settings).
            val initial = s.prefs.name.trim().take(1).uppercase()
            Box(Modifier.size(36.dp).clip(CircleShape).background(if (initial.isEmpty()) c.wash else c.accentSoft), contentAlignment = Alignment.Center) {
                if (initial.isEmpty()) Glyph("user", 18.dp, c.ink2)
                else T(initial, style = Relay.type.uiMedium, color = c.accentText, weight = FontWeight.SemiBold)
            }
            Column(Modifier.weight(1f)) {
                T(s.prefs.name.ifBlank { "You" }, style = Relay.type.uiMedium, color = c.ink, weight = FontWeight.SemiBold, maxLines = 1)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Dot(linkColor(s.link), 7.dp)
                    T(linkLine(s), style = Relay.type.caption, color = c.ink3, maxLines = 1)
                }
            }
            IconKey("gear", nav::settings, size = 19.dp, label = "Settings")
        }
    }
}

/** One of the spaces as a tile: its mark in its own colour over its name. */
@Composable
private fun SpaceTile(label: String, glyph: String, tint: Color, selected: Boolean, onClick: () -> Unit, modifier: Modifier) {
    val c = Relay.colors
    Column(
        modifier.clip(Radii.key).background(if (selected) c.wash else c.slab).clickable(onClick = onClick).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Glyph(glyph, 18.dp, tint)
        T(label, style = Relay.type.uiMedium, color = c.ink, weight = FontWeight.SemiBold, maxLines = 1)
    }
}

/** An agent at work in the drawer: its badge, what it is doing over who it is and its state. */
@Composable
private fun AgentRow(a: Session, title: String, line: String, lineColor: Color, onClick: () -> Unit) {
    val c = Relay.colors
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 6.dp, vertical = 6.dp)) {
        AgentBadge(a, 30.dp, ring = c.pane)
        Column(Modifier.weight(1f)) {
            T(title, style = Relay.type.ui, color = c.ink, maxLines = 1, weight = FontWeight.Medium)
            T(line, style = Relay.type.caption, color = lineColor, maxLines = 1)
        }
    }
}

/** A conversation in the drawer: its title over a quiet line, with a lamp when it is at work. */
@Composable
private fun ChatRow(title: String, line: String, dot: Color?, selected: Boolean, lineColor: Color = Relay.colors.ink3, onClick: () -> Unit) {
    val c = Relay.colors
    ListRow(selected = selected, onClick = onClick, padding = PaddingValues(horizontal = 10.dp, vertical = 7.dp)) {
        Box(Modifier.width(8.dp), contentAlignment = Alignment.Center) { dot?.let { Dot(it, 7.dp) } }
        Column(Modifier.weight(1f)) {
            T(title, style = Relay.type.ui, color = c.ink, maxLines = 1, weight = FontWeight.Medium)
            T(line, style = Relay.type.caption, color = lineColor, maxLines = 1)
        }
    }
}

@Composable
private fun ProjectRow(p: Project, s: ShellState, nav: SidebarNav) {
    val c = Relay.colors
    val mine = s.sessions.filter { it.projectId == p.num }
    val lamp = when {
        mine.any { it.blocked } -> Lamp.Held
        mine.any { it.working } -> Lamp.Live
        else -> Lamp.Off
    }
    val live = s.liveIn(p.num)
    ListRow(selected = s.project?.id == p.id, onClick = { p.num?.let(nav::project) }, onLongClick = { p.num?.let(nav::projectSettings) }, padding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)) {
        LampDot(lamp, 6.dp)
        T(p.name, Modifier.weight(1f), Relay.type.ui, if (s.project?.id == p.id) c.ink else c.ink2, maxLines = 1)
        if (p.pinned) Glyph("pin", 12.dp)
        if (live > 0) T(live.toString(), style = Relay.type.mono, color = c.ink3)
    }
}
