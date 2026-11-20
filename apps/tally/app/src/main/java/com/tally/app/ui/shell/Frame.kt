package com.tally.app.ui.shell

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import com.tally.app.ui.kit.RoundKey
import com.tally.app.ui.theme.ground
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
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.tally.app.BuildConfig
import com.tally.app.ui.common.LocalMoney
import com.tally.app.ui.home.statusMonthLine
import com.tally.app.ui.common.Notice
import com.tally.app.ui.common.Notices
import com.tally.app.ui.kit.Dot
import com.tally.app.ui.kit.DrawerRow
import com.tally.app.ui.kit.Fill
import com.tally.app.ui.kit.Glyph
import com.tally.app.ui.kit.Hairline
import com.tally.app.ui.kit.IconKey
import com.tally.app.ui.kit.Key
import com.tally.app.ui.kit.KeyKind
import com.tally.app.ui.kit.ListRow
import com.tally.app.ui.kit.LocalToastClearance
import com.tally.app.ui.kit.NavRow
import com.tally.app.ui.kit.Radii
import com.tally.app.ui.kit.SectionLabel
import com.tally.app.ui.kit.T
import com.tally.app.ui.kit.TallyMark
import com.tally.app.ui.nav.AppNav
import com.tally.app.ui.nav.Routes
import com.tally.app.ui.nav.Space
import com.tally.app.ui.theme.Tally
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
 * The frame of the spaces' pages, direction C: the sky behind everything, a slim row of round keys
 * (the drawer, the section's pages, search, settings), the page, and a frosted bottom bar with the
 * four places and the Add key in its middle. Sync stands in the drawer's foot and on Settings.
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
        scrimColor = Color(0xFF1A1A2E).copy(alpha = .45f),
        drawerContent = {
            ModalDrawerSheet(drawerContainerColor = c.raised, drawerShape = RoundedCornerShape(topEnd = 28.dp, bottomEnd = 28.dp), windowInsets = WindowInsets(0.dp)) {
                Sidebar(route, shell, onGo = { r -> close { nav.page(r) } }, onPage = { go -> close(go) }, nav = nav, onSync = onSync)
            }
        },
    ) {
        Column(Modifier.fillMaxSize().ground(c)) {
            TopRow(
                route = route,
                onGo = { nav.page(it) },
                onDrawer = { scope.launch { drawer.open() } },
                onSearch = { nav.search() },
                onSettings = { nav.settings() },
            )
            Box(Modifier.fillMaxWidth().weight(1f)) { content() }
            BottomBar(route, onGo = { nav.page(it) }, onAdd = onAdd)
        }
    }
}

/** A section's pages, as pills in the top row: Spend's three, Plan's three. */
private val SECTIONS = listOf(
    listOf(Routes.OVERVIEW to "Overview", Routes.ENTRIES to "Entries", Routes.INSIGHTS to "Insights"),
    listOf(Routes.BUDGETS to "Budgets", Routes.BILLS to "Bills", Routes.GOALS to "Goals"),
)

/** The top row: the drawer key, the section's pages, then search and settings. */
@Composable
fun TopRow(route: String, onGo: (String) -> Unit, onDrawer: () -> Unit, onSearch: () -> Unit, onSettings: () -> Unit) {
    val c = Tally.colors
    Row(
        Modifier.fillMaxWidth().statusBarsPadding().height(60.dp).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        RoundKey("sidebar", onDrawer, label = "Menu")
        val section = SECTIONS.firstOrNull { list -> list.any { it.first == route } }
        Row(Modifier.weight(1f), horizontalArrangement = Arrangement.Center) {
            if (section != null) {
                Row(Modifier.clip(Radii.pill).background(c.slab).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    for ((r, label) in section) {
                        val on = r == route
                        Box(
                            Modifier.heightIn(min = 34.dp).clip(Radii.pill).background(if (on) c.ink else Color.Transparent)
                                .clickable(role = Role.Tab) { if (!on) onGo(r) }.padding(horizontal = 11.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            T(label, style = Tally.type.caption, color = if (on) c.onInk else c.ink2, weight = FontWeight.SemiBold, maxLines = 1)
                        }
                    }
                }
            }
        }
        RoundKey("search", onSearch, label = "Search entries")
        RoundKey("gear", onSettings, label = "Settings")
    }
}

private data class Place(val route: String, val glyph: String, val label: String, val owns: Set<String>)

private val PLACES = listOf(
    Place(Routes.OVERVIEW, "home", "Overview", setOf(Routes.OVERVIEW, Routes.ENTRIES, Routes.INSIGHTS)),
    Place(Routes.BUDGETS, "pie", "Plan", setOf(Routes.BUDGETS, Routes.BILLS, Routes.GOALS)),
    Place(Routes.ACCOUNTS, "bank", "Accounts", setOf(Routes.ACCOUNTS)),
    Place(Routes.INVESTMENTS, "trend", "Invest", setOf(Routes.INVESTMENTS)),
)

/** The frosted bottom bar: four places, the Add key in the middle when the page has one. */
@Composable
fun BottomBar(route: String, onGo: (String) -> Unit, onAdd: (() -> Unit)?) {
    val c = Tally.colors
    val clearance = LocalToastClearance.current
    val density = LocalDensity.current
    val key = remember { Any() }
    DisposableEffect(clearance, key) { onDispose { clearance.clear(key) } }
    Column(
        Modifier.fillMaxWidth().clip(Radii.sheet).background(c.console).navigationBarsPadding()
            .onSizeChanged { clearance.set(key, with(density) { it.height.toDp() }) },
    ) {
        Row(
            Modifier.fillMaxWidth().height(72.dp).padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceAround,
        ) {
            for ((i, p) in PLACES.withIndex()) {
                if (i == 2) AddKey(onAdd)
                val on = route in p.owns
                Column(
                    Modifier.size(width = 64.dp, height = 56.dp).clip(Radii.key).clickable(role = Role.Tab) { if (route != p.route) onGo(p.route) }
                        .semantics { contentDescription = p.label },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Glyph(p.glyph, 22.dp, if (on) c.ink else c.ink3)
                    Box(Modifier.padding(top = 5.dp).size(width = 16.dp, height = 3.dp).clip(Radii.pill).background(if (on) c.signal else Color.Transparent))
                }
            }
        }
    }
}

/** The Add key: a dark square in the bar's middle; a quiet placeholder where a page has none. */
@Composable
fun AddKey(onClick: (() -> Unit)?, modifier: Modifier = Modifier) {
    val c = Tally.colors
    if (onClick == null) {
        Box(modifier.size(56.dp))
        return
    }
    Box(
        modifier.size(56.dp).shadow(10.dp, RoundedCornerShape(20.dp)).clip(RoundedCornerShape(20.dp)).background(c.ink)
            .clickable(role = Role.Button, onClick = onClick).semantics { contentDescription = "Add" },
        contentAlignment = Alignment.Center,
    ) {
        Glyph("plus", 24.dp, c.onInk)
    }
}

/** The drawer (Relay's sidebar): the spaces' pages, the ledger's own pages, and sync at the foot. */
@Composable
internal fun Sidebar(route: String, s: ShellState, onGo: (String) -> Unit, onPage: (() -> Unit) -> Unit, nav: AppNav, onSync: () -> Unit) {
    val c = Tally.colors
    Column(Modifier.fillMaxHeight().width(300.dp).background(c.raised).statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(start = 18.dp, end = 16.dp, top = 18.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(38.dp).clip(RoundedCornerShape(12.dp)).background(com.tally.app.ui.theme.Hue.lilacSoft), contentAlignment = Alignment.Center) { TallyMark(26.dp) }
            T("tally", Modifier.padding(start = 10.dp), Tally.type.brand.copy(fontSize = 22.sp), c.ink)
            Fill()
            T("v" + BuildConfig.VERSION_NAME, style = Tally.type.mono, color = c.ink3)
        }
        LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
            item { DrawerLabel("Spend") }
            item { DrawerRow("Overview", "home", route == Routes.OVERVIEW) { onGo(Routes.OVERVIEW) } }
            item { DrawerRow("Entries", "list", route == Routes.ENTRIES) { onGo(Routes.ENTRIES) } }
            item { DrawerRow("Insights", "bars", route == Routes.INSIGHTS) { onGo(Routes.INSIGHTS) } }
            item { DrawerLabel("Plan") }
            item { DrawerRow("Budgets", "pie", route == Routes.BUDGETS) { onGo(Routes.BUDGETS) } }
            item { DrawerRow("Bills", "repeat", route == Routes.BILLS) { onGo(Routes.BILLS) } }
            item { DrawerRow("Goals", "target", route == Routes.GOALS) { onGo(Routes.GOALS) } }
            item { DrawerRow("Accounts", "bank", route == Routes.ACCOUNTS) { onGo(Routes.ACCOUNTS) } }
            item { DrawerRow("Investments", "trend", route == Routes.INVESTMENTS) { onGo(Routes.INVESTMENTS) } }
            item { DrawerLabel("Ledger") }
            item { DrawerRow("Categories", "tag") { onPage { nav.categories() } } }
            item { DrawerRow("Import a statement", "download") { onPage { nav.import() } } }
            item { DrawerRow("Connected banks", "link") { onPage { nav.banks() } } }
            item { DrawerRow("Backup", "save") { onPage { nav.backup() } } }
            item { DrawerRow("Settings", "gear") { onPage { nav.settings() } } }
        }
        Row(
            Modifier.padding(12.dp).fillMaxWidth().clip(Radii.card).background(com.tally.app.ui.theme.Hue.mintSoft)
                .clickable(role = Role.Button) { onPage { nav.sync() } }.padding(start = 14.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Dot(toneColor(s.sync.tone), 9.dp)
            Column(Modifier.weight(1f)) {
                T(if (s.paired) "Your server" else "Sync", style = Tally.type.uiMedium, color = com.tally.app.ui.theme.Hue.mintInk, maxLines = 1, weight = FontWeight.SemiBold)
                T(s.sync.text, style = Tally.type.caption, color = com.tally.app.ui.theme.Hue.mintInk, maxLines = 1)
            }
            IconKey("sync", onSync, size = 16.dp, tint = com.tally.app.ui.theme.Hue.mintInk, label = "Sync now")
        }
    }
}

/** A drawer section's label: small capitals, quiet. */
@Composable
private fun DrawerLabel(text: String) {
    com.tally.app.ui.kit.Eyebrow(text, Modifier.padding(start = 12.dp, top = 16.dp, bottom = 6.dp))
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
        // Above the status bar on a space page; above the keypad and Save on an editor.
        modifier = modifier.align(Alignment.BottomCenter).navigationBarsPadding().imePadding()
            .padding(bottom = maxOf(24.dp, LocalToastClearance.current.bottom + 10.dp), start = 16.dp, end = 16.dp),
        enter = fadeIn() + slideInVertically { it / 2 },
        exit = fadeOut() + slideOutVertically { it / 2 },
    ) {
        val n = current ?: return@AnimatedVisibility
        Row(
            Modifier.widthIn(max = 520.dp).shadow(12.dp, Radii.popover).clip(Radii.popover).background(c.ink)
                .padding(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            T(n.message, Modifier.weight(1f, fill = false).padding(vertical = 8.dp), Tally.type.ui, c.onInk)
            if (n.undo != null) {
                Box(Modifier.width(8.dp))
                T("Undo", Modifier.clip(Radii.key).clickable(role = Role.Button) { notices.settle(n, undone = true) }.padding(horizontal = 12.dp, vertical = 10.dp), Tally.type.uiMedium, Color(0xFFC9BDFF), weight = FontWeight.SemiBold)
            }
        }
    }
}
