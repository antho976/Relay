package com.tally.app.ui.shell

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.BuildConfig
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.common.Notice
import com.tally.app.ui.common.Notices
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.Fill
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.Hairline
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.NavRow
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.SectionLabel
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TallyMark
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.Routes
import com.tally.app.ui.nav.Space
import com.tally.app.ui.theme.Tally
import com.tally.core.Copy
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@Composable
fun toneColor(tone: Tone): Color = when (tone) {
    Tone.LIVE -> Tally.colors.live
    Tone.WAITING -> Tally.colors.waiting
    Tone.HELD -> Tally.colors.held
    Tone.OFF -> Tally.colors.ink3
}

/**
 * The frame of the spaces' pages, as Relay's: the top bar with the Spend | Plan switch, the drawer,
 * the page, the floating Add key, and the status bar where sync and the month say where they stand.
 */
@Composable
fun SpaceFrame(
    nav: AppNav,
    route: String,
    shell: ShellState,
    onSync: () -> Unit,
    onAdd: (() -> Unit)?,
    content: @Composable BoxScope.() -> Unit,
) {
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val c = Tally.colors
    fun close(then: () -> Unit) {
        scope.launch { drawer.close() }
        then()
    }
    ModalNavigationDrawer(
        drawerState = drawer,
        scrimColor = Color.Black.copy(alpha = .45f),
        drawerContent = {
            ModalDrawerSheet(drawerContainerColor = c.wall, drawerShape = Radii.icon, windowInsets = WindowInsets(0.dp)) {
                Sidebar(route, shell, onGo = { r -> close { nav.page(r) } }, onPage = { go -> close(go) }, nav = nav, onSync = onSync)
            }
        },
    ) {
        Column(Modifier.fillMaxSize().background(c.wall)) {
            TopBar(
                space = Space.of(route),
                onSpace = { nav.space(it) },
                onDrawer = { scope.launch { drawer.open() } },
                onSearch = { nav.search() },
                onSettings = { nav.settings() },
            )
            Box(Modifier.fillMaxWidth().weight(1f)) {
                content()
                if (onAdd != null) AddKey(onAdd, Modifier.align(Alignment.BottomEnd).padding(end = 16.dp, bottom = 14.dp))
            }
            StatusBar(shell, onSync = onSync, onMonth = { nav.overview() })
        }
    }
}

/** Relay's top bar (theme.css `.topbar`): the drawer key, the mark and wordmark, the space switch, then search and settings. */
@Composable
fun TopBar(space: Space, onSpace: (Space) -> Unit, onDrawer: () -> Unit, onSearch: () -> Unit, onSettings: () -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.fillMaxWidth().background(c.wall).statusBarsPadding().height(56.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconKey("sidebar", onDrawer, size = 18.dp, label = "Menu")
        TallyMark(22.dp)
        T("tally", Modifier.padding(start = 6.dp, end = 10.dp), Tally.type.brand, c.ink)
        SpaceSwitch(space, onSpace)
        Fill()
        IconKey("search", onSearch, size = 18.dp, label = "Search entries")
        IconKey("gear", onSettings, size = 18.dp, label = "Settings")
    }
}

/** Relay's space switch (money.css): a pill on the screen colour, the chosen space in ink. */
@Composable
fun SpaceSwitch(space: Space, onSpace: (Space) -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.clip(Radii.pill).background(c.screen).border(1.dp, c.edge, Radii.pill).padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        for (s in Space.entries) {
            val on = s == space
            Box(
                Modifier.heightIn(min = 28.dp).clip(Radii.pill).background(if (on) c.ink else Color.Transparent)
                    .clickable(role = Role.Tab) { onSpace(s) }.padding(horizontal = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                T(s.label, style = Tally.type.caption, color = if (on) c.wall else c.ink3, weight = FontWeight.Medium)
            }
        }
    }
}

/** The floating Add key: Relay's primary key, as a pill, where a thumb rests. */
@Composable
fun AddKey(onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = Tally.colors
    Row(
        modifier.shadow(10.dp, Radii.pill).clip(Radii.pill).background(c.ink)
            .clickable(role = Role.Button, onClick = onClick).heightIn(min = 52.dp).padding(start = 16.dp, end = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Glyph("plus", 18.dp, c.wall)
        T("Add", style = Tally.type.uiMedium, color = c.wall, weight = FontWeight.SemiBold)
    }
}

/**
 * Relay's status bar (30px, Geist Mono, quiet): where sync stands on the left, the month on the
 * right. Tapping the sync side syncs now, or opens what needs the person.
 */
@Composable
fun StatusBar(s: ShellState, onSync: () -> Unit, onMonth: () -> Unit) {
    val c = Tally.colors
    val fmt = LocalMoney.current
    Column(Modifier.fillMaxWidth().background(c.console).navigationBarsPadding()) {
        Hairline(color = c.edge)
        Row(
            Modifier.fillMaxWidth().height(34.dp).padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(Modifier.weight(1f).clickable(role = Role.Button, onClick = onSync), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Dot(toneColor(s.sync.tone), 6.dp)
                T(s.sync.text, style = Tally.type.mono, color = c.ink3, maxLines = 1)
            }
            if (s.period != null) {
                val text = when {
                    s.left != null && s.left >= 0 -> fmt.formatWhole(s.left) + " left · " + Copy.plural(s.daysLeft, "day")
                    s.left != null -> fmt.formatWhole(-s.left) + " over · " + Copy.plural(s.daysLeft, "day")
                    else -> fmt.formatWhole(s.spent) + " spent · " + Copy.plural(s.daysLeft, "day")
                }
                T(text, Modifier.clickable(role = Role.Button, onClick = onMonth), Tally.type.mono, if (s.left != null && s.left < 0) c.heldText else c.ink3, maxLines = 1)
            }
        }
    }
}

/** The drawer (Relay's sidebar): the spaces' pages, the ledger's own pages, and sync at the foot. */
@Composable
private fun Sidebar(route: String, s: ShellState, onGo: (String) -> Unit, onPage: (() -> Unit) -> Unit, nav: AppNav, onSync: () -> Unit) {
    val c = Tally.colors
    Column(Modifier.fillMaxHeight().width(300.dp).background(c.wall).statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, top = 14.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            TallyMark(24.dp)
            T("tally", Modifier.padding(start = 8.dp), Tally.type.brand, c.ink)
            Fill()
            T("v" + BuildConfig.VERSION_NAME, style = Tally.type.mono, color = c.ink3)
        }
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)) {
            item { SectionLabel("Spend") }
            item { NavRow("Overview", "home", route == Routes.OVERVIEW) { onGo(Routes.OVERVIEW) } }
            item { NavRow("Entries", "list", route == Routes.ENTRIES) { onGo(Routes.ENTRIES) } }
            item { NavRow("Insights", "bars", route == Routes.INSIGHTS) { onGo(Routes.INSIGHTS) } }
            item { SectionLabel("Plan") }
            item { NavRow("Budgets", "pie", route == Routes.BUDGETS) { onGo(Routes.BUDGETS) } }
            item { NavRow("Bills", "repeat", route == Routes.BILLS) { onGo(Routes.BILLS) } }
            item { NavRow("Goals", "target", route == Routes.GOALS) { onGo(Routes.GOALS) } }
            item { NavRow("Accounts", "bank", route == Routes.ACCOUNTS) { onGo(Routes.ACCOUNTS) } }
            item { NavRow("Investments", "trend", route == Routes.INVESTMENTS) { onGo(Routes.INVESTMENTS) } }
            item { SectionLabel("Ledger") }
            item { NavRow("Categories", "tag") { onPage { nav.categories() } } }
            item { NavRow("Import a statement", "download") { onPage { nav.import() } } }
            item { NavRow("Backup", "save") { onPage { nav.backup() } } }
            item { NavRow("Settings", "gear") { onPage { nav.settings() } } }
        }
        Hairline(Modifier.padding(horizontal = 10.dp), c.edge)
        Column(Modifier.padding(10.dp)) {
            ListRow(onClick = { onPage { nav.sync() } }) {
                Dot(toneColor(s.sync.tone), 7.dp)
                Column(Modifier.weight(1f)) {
                    T(if (s.paired) "Your server" else "Sync", style = Tally.type.uiMedium, color = c.ink, maxLines = 1)
                    T(s.sync.text, style = Tally.type.caption, color = c.ink3, maxLines = 1)
                }
                IconKey("sync", onSync, size = 16.dp, label = "Sync now")
            }
        }
    }
}

/**
 * The one toast (Relay's money toast): a line at the bottom, with Undo when the act has one, for
 * seven seconds; four without. The newest replaces the one showing, which lets that act stand.
 */
@Composable
fun BoxScope.ToastHost(notices: Notices, modifier: Modifier = Modifier) {
    val current by notices.current.collectAsStateWithLifecycle()
    val c = Tally.colors
    LaunchedEffect(current) {
        val n: Notice = current ?: return@LaunchedEffect
        delay(if (n.undo != null) 7_000 else 4_000)
        notices.settle(n, undone = false)
    }
    AnimatedVisibility(
        visible = current != null,
        modifier = modifier.align(Alignment.BottomCenter).navigationBarsPadding().imePadding().padding(bottom = 44.dp, start = 16.dp, end = 16.dp),
        enter = fadeIn() + slideInVertically { it / 2 },
        exit = fadeOut() + slideOutVertically { it / 2 },
    ) {
        val n = current ?: return@AnimatedVisibility
        Row(
            Modifier.widthIn(max = 520.dp).shadow(8.dp, Radii.popover).clip(Radii.popover).background(c.slab).border(1.dp, c.strong, Radii.popover)
                .padding(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            T(n.message, Modifier.weight(1f, fill = false).padding(vertical = 8.dp), Tally.type.ui, c.ink)
            if (n.undo != null) {
                Box(Modifier.width(8.dp))
                Key("Undo", { notices.settle(n, undone = true) }, kind = KeyKind.Quiet, compact = true)
            }
        }
    }
}
