package com.quietsoftware.relay.ui.pair

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.RefusalException
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.core.wire.Route
import com.quietsoftware.relay.core.wire.RoutePolicy
import com.quietsoftware.relay.service.LinkService
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.RelayMark
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.Toggle
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.shell.linkColor
import com.quietsoftware.relay.ui.shell.linkLine
import com.quietsoftware.relay.ui.shell.routeName
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch

/**
 * Pairing (docs/MOBILE.md §5): scan the QR `relay remote pair` prints, paste its link, or type
 * the address and code. The PC asks its person to approve this phone before it gets a token.
 */
@Composable
fun PairScreen(link: String?, nav: Nav) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current
    val paired by nav.shell.state.collectAsStateWithLifecycle()
    var mode by remember { mutableStateOf(if (link != null) "link" else "scan") }
    var pasted by remember { mutableStateOf(link.orEmpty()) }
    var address by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var waiting by remember { mutableStateOf<PairLink?>(null) }
    var problem by remember { mutableStateOf<String?>(null) }

    fun pair(target: PairLink) {
        if (waiting != null) return
        waiting = target
        problem = null
        scope.launch {
            nav.relay.pair(target)
                .onSuccess {
                    waiting = null
                    LinkService.start(context)
                    nav.start()
                }
                .onFailure { e ->
                    waiting = null
                    problem = (e as? RefusalException)?.refusal?.text ?: e.message ?: "Pairing failed"
                }
        }
    }
    // A `relay://pair` link from another app only fills the field: pairing replaces the PC this
    // phone talks to (and drops its copy and unsent edits), so it waits for the person's Pair.

    Page {
        Column(Modifier.fillMaxSize()) {
            if (paired.pc != null) PageBar("Pair a PC", nav::back) else Gap(0.dp)
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(Modifier.column(560.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (paired.pc == null) {
                        Row(Modifier.padding(top = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                            RelayMark(34.dp)
                            T("relay", Modifier.padding(start = 9.dp), Relay.type.brand.copy(fontSize = Relay.type.heading.fontSize), c.ink)
                        }
                    }
                    T("Pair this phone with your PC", Modifier.padding(top = 12.dp), Relay.type.heading, c.ink)
                    T(
                        "On the PC, run relay remote pair. It prints a code to scan and asks you to approve this phone. Pairing works on your WiFi, over Tailscale, or through your own server.",
                        style = Relay.type.body,
                        color = c.ink2,
                    )
                    paired.pc?.let { pc ->
                        val unsent = paired.outbox.size
                        T(
                            "Pairing a different PC replaces ${pc.name.ifBlank { "this one" }}: this phone's copy of its data" +
                                (if (unsent > 0) " and $unsent changes not yet sent" else "") + " will be dropped.",
                            style = Relay.type.caption,
                            color = if (unsent > 0) c.heldText else c.ink3,
                        )
                    }
                    Segmented(listOf("scan" to "Scan", "link" to "Paste link", "type" to "Type code"), mode, { mode = it }, Modifier.fillMaxWidth(), fill = true)
                    when (mode) {
                        "scan" -> QrScanner(
                            onCode = { text -> PairLink.parse(text)?.let(::pair) ?: run { problem = "That code is not a Relay pairing code" } },
                            modifier = Modifier.fillMaxWidth().aspectRatio(1f).clip(Radii.card),
                        )
                        "link" -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Field(pasted, { pasted = it }, Modifier.fillMaxWidth(), placeholder = "relay://pair?v=1&code=…", singleLine = false, maxLines = 4, leading = "link")
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Key("Paste", { clipboard.getText()?.text?.let { pasted = it.trim() } }, glyph = "copy")
                                Key("Pair", {
                                    PairLink.parse(pasted)?.let(::pair) ?: run { problem = "That is not a relay://pair link" }
                                }, kind = KeyKind.Primary, enabled = pasted.isNotBlank())
                            }
                        }
                        else -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Field(address, { address = it }, Modifier.fillMaxWidth(), placeholder = "PC address, e.g. 192.168.1.20 or my-pc.tail1234.ts.net", leading = "wifi",
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri))
                            Field(code, { code = it.uppercase() }, Modifier.fillMaxWidth(), placeholder = "Code, e.g. ABCD-EFGH", leading = "shield",
                                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters))
                            Key("Pair", {
                                PairLink.manual(address, code)?.let(::pair) ?: run { problem = "Enter the PC's address and the code it printed" }
                            }, kind = KeyKind.Primary, enabled = address.isNotBlank() && code.isNotBlank())
                        }
                    }
                    waiting?.let { w ->
                        Slab(Modifier.fillMaxWidth()) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Dot(c.waiting, 7.dp)
                                T("  Waiting for ${w.host.ifBlank { "the PC" }} to approve this phone", style = Relay.type.uiMedium, color = c.ink)
                            }
                            T("Answer y in the terminal that ran relay remote pair. The code works once, for about a minute and a half.", Modifier.padding(top = 6.dp), Relay.type.caption, c.ink3)
                        }
                    }
                    problem?.let { T(it, style = Relay.type.ui, color = c.heldText) }
                    Gap(24.dp)
                }
            }
        }
    }
}

/**
 * The paired PC (docs/MOBILE.md §5, "Paired PCs"): how it is reached, which routes the phone may
 * use, waking it, keeping the link open, and forgetting it.
 */
@Composable
fun PcScreen(nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var extra by remember { mutableStateOf("") }
    var confirmForget by remember { mutableStateOf(false) }
    val pc = s.pc
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(pc?.name?.ifBlank { null } ?: "Your PC", nav::back, subtitle = pc?.let { "Relay ${it.version.ifBlank { "" }} · ${it.instance}" })
            if (pc == null) {
                Column(Modifier.padding(20.dp)) {
                    T("No PC is paired yet.", style = Relay.type.body, color = c.ink2)
                    Gap(12.dp)
                    Key("Pair a PC", { nav.pair() }, kind = KeyKind.Primary, glyph = "qr")
                }
                return@Column
            }
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Column(Modifier.column().fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    Slab(Modifier.fillMaxWidth()) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Dot(linkColor(s.link), 8.dp)
                            T("  " + linkLine(s), Modifier.weight(1f), Relay.type.uiMedium, c.ink)
                            when (s.link) {
                                is LinkState.Online, is LinkState.Connecting, is LinkState.Starting -> Key("Disconnect", nav.shell::disconnect, compact = true)
                                else -> Key("Connect", { nav.shell.connect(); nav.relay.kick() }, kind = KeyKind.Primary, compact = true)
                            }
                        }
                        (s.link as? LinkState.Offline)?.let { T(it.reason, Modifier.padding(top = 6.dp), Relay.type.caption, c.ink3) }
                        (s.link as? LinkState.Revoked)?.let {
                            T(it.reason, Modifier.padding(top = 6.dp), Relay.type.caption, c.heldText)
                            Key("Pair again", { nav.pair() }, Modifier.padding(top = 8.dp), KeyKind.Primary)
                        }
                        if (s.lastSeen > 0 && s.link !is LinkState.Online) T("Last reached ${ago(s.lastSeen)}", Modifier.padding(top = 4.dp), Relay.type.caption, c.ink3)
                    }

                    if (s.link !is LinkState.Online) {
                        Slab(Modifier.fillMaxWidth()) {
                            Eyebrow("When the PC does not answer")
                            Gap(6.dp)
                            T("Asleep: wake it from here when this phone is on the same network. Its network card must allow Wake-on-LAN.", style = Relay.type.caption, color = c.ink2)
                            Key(if (pc.wake.isEmpty()) "Wake the PC (not known yet)" else "Wake the PC", nav.shell::wake, Modifier.padding(top = 8.dp), glyph = "power", enabled = pc.wake.isNotEmpty())
                            Gap(10.dp)
                            T("On, with Relay closed: if the PC runs Relay's door at login (deploy/relay-door.service), the door starts Relay when this phone connects. Otherwise open Relay on the PC.", style = Relay.type.caption, color = c.ink2)
                            Gap(6.dp)
                            T("Off or out of reach: everything here is this phone's copy. Edits wait in the outbox and go, once, when the PC is back.", style = Relay.type.caption, color = c.ink2)
                        }
                    }

                    Slab(Modifier.fillMaxWidth(), padding = androidx.compose.foundation.layout.PaddingValues(vertical = 6.dp)) {
                        Eyebrow("Routes", Modifier.padding(horizontal = 14.dp, vertical = 8.dp))
                        val inUse = (s.link as? LinkState.Online)?.route?.url
                        for (r in pc.routes) {
                            ListRow {
                                Glyph(if (r.kind == Route.Kind.Rendezvous) "cloud" else "wifi", 16.dp)
                                Column(Modifier.weight(1f)) {
                                    T(routeName(r) + if (r.url == inUse) " · in use" else "", style = Relay.type.uiMedium, color = c.ink)
                                    T(r.url, style = Relay.type.mono, color = c.ink3, maxLines = 1)
                                }
                                if (pc.routes.size > 1) IconKey("close", {
                                    scope.launch { nav.relay.pc.update { p -> p.copy(routes = p.routes - r) }; nav.relay.hub.link.restart() }
                                }, size = 14.dp)
                            }
                        }
                        Row(Modifier.padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Field(extra, { extra = it }, Modifier.weight(1f), placeholder = "Add Tailscale or other address", keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri))
                            Key("Add", {
                                PairLink.address(extra)?.let { url ->
                                    scope.launch {
                                        nav.relay.pc.update { p -> p.copy(routes = p.routes + Route(url, Route.Kind.of(url))) }
                                        nav.relay.hub.link.restart()
                                    }
                                    extra = ""
                                } ?: nav.shell.toast("That is not an address")
                            }, enabled = extra.isNotBlank(), compact = true)
                        }
                        if (pc.routes.any { it.kind == Route.Kind.Rendezvous } && pc.routes.any { it.kind != Route.Kind.Rendezvous }) {
                            Segmented(
                                listOf(RoutePolicy.Auto to "Auto", RoutePolicy.DirectOnly to "Direct only", RoutePolicy.ServerOnly to "Server only"),
                                pc.policy,
                                { p -> scope.launch { nav.relay.pc.update { it.copy(policy = p) }; nav.relay.hub.link.restart() } },
                                Modifier.fillMaxWidth().padding(12.dp),
                                fill = true,
                            )
                        }
                    }

                    Slab(Modifier.fillMaxWidth()) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                T("Stay connected", style = Relay.type.uiMedium, color = c.ink)
                                T("Keeps the link open in the background so agents that need you can say so. Shows an ongoing notification.", style = Relay.type.caption, color = c.ink3)
                            }
                            Toggle(s.prefs.stayConnected, { on ->
                                nav.shell.update { it.copy(stayConnected = on) }
                                if (on) LinkService.start(context) else LinkService.stop(context)
                            })
                        }
                    }

                    Slab(Modifier.fillMaxWidth(), onClick = { nav.pcTools() }) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Glyph("cpu", 16.dp)
                            Column(Modifier.weight(1f).padding(start = 10.dp)) {
                                T("Agents and backups on the PC", style = Relay.type.uiMedium, color = c.ink)
                                T("Claude Code and Codex versions and updates, the store's backups", style = Relay.type.caption, color = c.ink3)
                            }
                            Glyph("chevron-right", 14.dp)
                        }
                    }

                    Slab(Modifier.fillMaxWidth()) {
                        T("This phone is device ${pc.device} on ${pc.name.ifBlank { "the PC" }}. To cut it off from the PC: relay remote revoke ${pc.device}.", style = Relay.type.caption, color = c.ink3)
                        Hairline(Modifier.padding(vertical = 10.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Key("Pair another PC", { nav.pair() }, glyph = "qr")
                            if (!confirmForget) Key("Forget this PC", { confirmForget = true }, kind = KeyKind.Danger)
                        }
                        if (confirmForget) {
                            Gap(10.dp)
                            T(
                                if (s.outbox.isEmpty()) "Forget the PC and this phone's copy of its data?" else "Forget the PC? ${s.outbox.size} changes not yet sent will be lost.",
                                style = Relay.type.ui, color = c.heldText,
                            )
                            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Key("Forget", {
                                    scope.launch {
                                        LinkService.stop(context)
                                        nav.relay.forget()
                                        nav.pair()
                                    }
                                }, kind = KeyKind.Danger)
                                Key("Keep", { confirmForget = false }, kind = KeyKind.Quiet)
                            }
                        }
                    }
                    Box(Modifier.fillMaxWidth().background(c.wall).padding(bottom = 24.dp))
                }
            }
        }
    }
}
