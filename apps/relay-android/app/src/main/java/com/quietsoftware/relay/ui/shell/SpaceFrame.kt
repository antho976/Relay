package com.quietsoftware.relay.ui.shell

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.background
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Dot
import androidx.compose.ui.Alignment
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.clickable
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.currentBackStackEntryAsState
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch

/**
 * The frame of the main pages (design 14): the top bar, the drawer, and the page. With the PC out
 * of reach a quiet line under the bar says so; everything else about the link is in the drawer.
 * [center] replaces the title with a chip of the page's own, as Home's project picker.
 */
@Composable
fun SpaceFrame(
    nav: Nav,
    title: String? = null,
    center: (@Composable () -> Unit)? = null,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable () -> Unit,
) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val entry by nav.controller.currentBackStackEntryAsState()
    val route = entry?.destination?.route?.substringBefore('?')?.let { r ->
        if (r.startsWith("thread/")) "thread/" + entry?.arguments?.getString("id") else r
    }
    // A page of one space opened from the other (search, a notification) brings its space along.
    val space = spaceOf(route) ?: s.prefs.space
    LaunchedEffect(space, s.loaded) { if (s.loaded && space != s.prefs.space) nav.shell.pickSpace(space) }
    val c = Relay.colors
    val close: () -> Unit = { scope.launch { drawer.close() } }
    val sidebarNav = object : SidebarNav by nav {
        override fun home() { close(); nav.home() }
        override fun agents() { close(); nav.agents() }
        override fun chat(name: String) { close(); nav.chat(name) }
        override fun search() { close(); nav.search() }
        override fun board() { close(); nav.board() }
        override fun changes() { close(); nav.changes() }
        override fun notes() { close(); nav.notes() }
        override fun inbox() { close(); nav.inbox() }
        override fun outbox() { close(); nav.outbox() }
        override fun skills() { close(); nav.skills() }
        override fun plugins() { close(); nav.plugins() }
        override fun project(id: Long) { close(); nav.project(id) }
        override fun projectSettings(id: Long) { close(); nav.projectSettings(id) }
        override fun addProject() { close(); nav.addProject() }
        override fun pc() { close(); nav.pc() }
        override fun settings() { close(); nav.settings() }
        override fun newThread() { close(); nav.newThread() }
        override fun thread(id: String) { close(); nav.thread(id) }
        override fun tally() { close(); nav.tally() }
        override fun arbiter() { close(); nav.arbiter() }
        override fun avex() { close(); nav.avex() }
    }
    ModalNavigationDrawer(
        drawerState = drawer,
        scrimColor = Color.Black.copy(alpha = .55f),
        drawerContent = {
            ModalDrawerSheet(
                drawerContainerColor = c.pane,
                drawerShape = RoundedCornerShape(topEnd = 24.dp, bottomEnd = 24.dp),
                windowInsets = androidx.compose.foundation.layout.WindowInsets(0.dp),
            ) {
                Sidebar(s, route, sidebarNav, space)
            }
        },
    ) {
        Column(Modifier.fillMaxSize().background(c.wall)) {
            TopBar(
                title = title ?: titleOf(route),
                onMenu = { scope.launch { drawer.open() } },
                onNew = { nav.home() },
                bell = s.needsYou,
                center = center,
                actions = actions,
            )
            if (s.loaded && !s.online && s.pc != null) {
                Row(
                    Modifier.fillMaxWidth().clickable { nav.pc() }.padding(horizontal = 16.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Dot(linkColor(s.link), 6.dp)
                    T(linkLine(s) + " · showing what this phone saved", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                }
            }
            Box(Modifier.fillMaxWidth().weight(1f).navigationBarsPadding()) { content() }
        }
    }
    // Back closes an open drawer first. Set after the page's own handlers, it is asked before them.
    BackHandler(enabled = drawer.isOpen) { close() }
}

/**
 * The title the top bar shows for a page that does not name its own. The board, the inbox, notes,
 * the outbox, skills, plugins and the three spaces set their title over a line of their own.
 */
internal fun titleOf(route: String?): String? = when (route) {
    "agents" -> "Agents"
    else -> null
}

/** The space a page belongs to; null for a page both share (the inbox, search, settings). */
internal fun spaceOf(route: String?): String? = when {
    route == null -> null
    route == "threads" || route.startsWith("thread/") || route == "tally" || route == "arbiter" || route == "avex" -> "threads"
    route == "home" -> null
    route == "agents" || route == "board" || route == "notes" || route == "skills" || route == "plugins" -> "dev"
    else -> null
}
