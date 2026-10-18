package com.quietsoftware.relay.ui.shell

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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Fill
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.RelayMark
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.lampAlpha
import com.quietsoftware.relay.ui.theme.Relay
import com.quietsoftware.relay.core.wire.BusException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * The top bar (theme.css `.topbar`, 52px): the sidebar key, the ring mark and wordmark, the
 * Dev | Threads switch, then the bell. A page with its own title shows a back key instead.
 */
@Composable
fun TopBar(
    space: String,
    onSpace: (String) -> Unit,
    onSidebar: () -> Unit,
    bell: Int,
    bellRed: Boolean,
    onBell: () -> Unit,
    onSearch: () -> Unit,
) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().background(c.wall).statusBarsPadding().height(56.dp).padding(start = 4.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconKey("sidebar", onSidebar, size = 18.dp)
        RelayMark(22.dp)
        T("relay", Modifier.padding(start = 7.dp, end = 10.dp), Relay.type.brand, c.ink)
        SpaceSwitch(space, onSpace)
        Fill()
        IconKey("search", onSearch, size = 18.dp)
        IconKey("bell", onBell, size = 18.dp, badge = bell, badgeColor = if (bellRed) c.held else c.ink)
    }
}

/** The space switch (money.css): a pill on the screen colour, the chosen space in ink. */
@Composable
fun SpaceSwitch(space: String, onSpace: (String) -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.clip(Radii.pill).background(c.screen).border(1.dp, c.edge, Radii.pill).padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        for ((key, label) in listOf("dev" to "Dev", "threads" to "Threads")) {
            val on = key == space
            Box(
                Modifier.heightIn(min = 26.dp).clip(Radii.pill).background(if (on) c.ink else Color.Transparent)
                    .clickable { onSpace(key) }.padding(horizontal = 11.dp),
                contentAlignment = Alignment.Center,
            ) {
                T(label, style = Relay.type.caption, color = if (on) c.wall else c.ink3, weight = FontWeight.Medium)
            }
        }
    }
}

/** A page's own header (panel.rs `page`): back, title, then its keys. */
@Composable
fun PageBar(title: String, onBack: () -> Unit, subtitle: String? = null, actions: @Composable () -> Unit = {}) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().background(c.wall).statusBarsPadding().heightIn(min = 56.dp).padding(start = 4.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconKey("arrow-left", onBack, size = 18.dp, tint = c.ink2)
        Column(Modifier.weight(1f).padding(start = 2.dp)) {
            T(title, style = Relay.type.title, color = c.ink, maxLines = 1)
            subtitle?.let { T(it, style = Relay.type.caption, color = c.ink3, maxLines = 1) }
        }
        actions()
    }
}

/**
 * The status bar (theme.css, 30px, Geist Mono faint): where the link stands and by which route,
 * what the outbox still holds, and how many agents need the person or are live.
 */
@Composable
fun StatusBar(s: ShellState, onLink: () -> Unit, onOutbox: () -> Unit) {
    val c = Relay.colors
    Row(
        Modifier.fillMaxWidth().background(c.console).navigationBarsPadding(),
    ) {
        Column(Modifier.fillMaxWidth()) {
            Box(Modifier.fillMaxWidth().height(1.dp).background(c.edge))
            Row(
                Modifier.fillMaxWidth().height(32.dp).padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Row(Modifier.clickable(onClick = onLink), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Dot(linkColor(s.link), 6.dp)
                    T(linkLine(s), style = Relay.type.mono, color = c.ink3, maxLines = 1)
                }
                Fill()
                if (s.outbox.isNotEmpty()) {
                    Row(Modifier.clickable(onClick = onOutbox), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                        Glyph("outbox", 12.dp, if (s.parked > 0) c.heldText else c.ink3)
                        T(if (s.parked > 0) "${s.parked} to decide" else "${s.outbox.size} waiting", style = Relay.type.mono, color = if (s.parked > 0) c.heldText else c.ink3)
                    }
                }
                val live = s.sessions.count { it.state in setOf("running", "idle", "spawning") }
                if (s.needsYou > 0) {
                    Dot(c.held.copy(alpha = lampAlpha()), 6.dp)
                    T("${s.needsYou} needs you", style = Relay.type.mono, color = c.ink3)
                }
                Dot(if (live > 0) c.live.copy(alpha = lampAlpha()) else c.ink3, 6.dp)
                T("$live live", style = Relay.type.mono, color = c.ink3)
            }
        }
    }
}

@Composable
fun linkColor(link: LinkState) = when (link) {
    is LinkState.Online -> Relay.colors.live
    is LinkState.Starting, is LinkState.Connecting -> Relay.colors.waiting
    is LinkState.Revoked -> Relay.colors.held
    else -> Relay.colors.ink3
}

fun routeName(r: Route) = when (r.kind) {
    Route.Kind.Lan -> "WiFi"
    Route.Kind.Tailscale -> "Tailscale"
    Route.Kind.Rendezvous -> "your server"
    Route.Kind.Other -> "direct"
}

fun linkLine(s: ShellState): String {
    val pc = s.pc?.name?.ifBlank { null } ?: "PC"
    return when (val l = s.link) {
        is LinkState.Online -> "$pc · ${routeName(l.route)}"
        is LinkState.Starting -> "starting Relay on $pc"
        is LinkState.Connecting -> "reaching $pc"
        is LinkState.Offline -> if (s.lastSeen > 0) "$pc away · seen ${ago(s.lastSeen)}" else "$pc away"
        LinkState.Stopped -> "disconnected"
        LinkState.Unpaired -> "no PC paired"
        is LinkState.Revoked -> "$pc revoked this phone"
    }
}

/** One-line messages at the bottom, with Undo when there is one (money.rs's toast: 7 s). */
@Composable
fun BoxScope.ToastHost(model: ShellModel) {
    var toast by remember { mutableStateOf<ShellModel.Toast?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) {
        model.toasts.collect {
            toast = it
            delay(if (it.undo != null) 7_000 else 4_000)
            if (toast === it) toast = null
        }
    }
    val c = Relay.colors
    AnimatedVisibility(
        visible = toast != null,
        modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 44.dp, start = 16.dp, end = 16.dp),
        enter = fadeIn() + slideInVertically { it / 2 },
        exit = fadeOut() + slideOutVertically { it / 2 },
    ) {
        val t = toast ?: return@AnimatedVisibility
        Row(
            Modifier.widthIn(max = 520.dp).shadow(8.dp, Radii.popover).clip(Radii.popover).background(c.slab).border(1.dp, c.strong, Radii.popover).padding(start = 14.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            T(t.text, Modifier.weight(1f, fill = false).padding(vertical = 6.dp), Relay.type.ui, c.ink)
            t.undo?.let { undo ->
                Box(Modifier.width(8.dp))
                T("Undo", Modifier.clip(Radii.key).clickable {
                    scope.launch {
                        // An Undo that needs the PC can fail; say so rather than crash the app.
                        try {
                            undo()
                        } catch (e: BusException) {
                            model.toast(if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code })
                        }
                    }
                    toast = null
                }.padding(horizontal = 10.dp, vertical = 8.dp), Relay.type.uiMedium, c.ink, weight = FontWeight.SemiBold)
            }
        }
    }
}

@Composable
fun Page(content: @Composable BoxScope.() -> Unit) = Box(Modifier.fillMaxSize().background(Relay.colors.wall), content = content)
