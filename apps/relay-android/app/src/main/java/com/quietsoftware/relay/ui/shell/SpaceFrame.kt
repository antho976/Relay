package com.quietsoftware.relay.ui.shell

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.background
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
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch

/**
 * The frame of a space's pages, as on the PC: the top bar, the sidebar (a drawer here), the page,
 * and the status bar. Detail pages (a terminal, a task) use [PageBar] instead and fill the screen.
 */
@Composable
fun SpaceFrame(nav: Nav, content: @Composable () -> Unit) {
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
        override fun board() { close(); nav.board() }
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
        scrimColor = Color.Black.copy(alpha = .45f),
        drawerContent = {
            ModalDrawerSheet(drawerContainerColor = c.wall, drawerShape = Radii.board, windowInsets = androidx.compose.foundation.layout.WindowInsets(0.dp)) {
                Sidebar(s, route, sidebarNav, space)
            }
        },
    ) {
        Column(Modifier.fillMaxSize().background(c.wall)) {
            TopBar(
                space = space,
                onSpace = { nav.space(it) },
                onSidebar = { scope.launch { drawer.open() } },
                bell = s.holds.size + s.unread,
                bellRed = s.holds.isNotEmpty(),
                onBell = { nav.inbox() },
                onSearch = { nav.search() },
            )
            Box(Modifier.fillMaxWidth().weight(1f)) { content() }
            StatusBar(s, onLink = { nav.pc() }, onOutbox = { nav.outbox() })
        }
    }
}

/** The space a page belongs to; null for a page both share (the inbox, search, settings). */
internal fun spaceOf(route: String?): String? = when {
    route == null -> null
    route == "threads" || route.startsWith("thread/") || route == "tally" || route == "arbiter" || route == "avex" -> "threads"
    route == "agents" || route == "board" || route == "notes" -> "dev"
    else -> null
}
