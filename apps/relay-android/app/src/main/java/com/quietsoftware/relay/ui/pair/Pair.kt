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
import androidx.compose.foundation.text.KeyboardActions
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.link.LinkState
import com.quietsoftware.relay.core.link.PcProfile
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
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.kit.switchRow
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
                    // The background link only when the person keeps it (Stay connected).
                    if (nav.shell.state.value.prefs.stayConnected) LinkService.start(context)
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
                                (if (unsent > 0) " and ${plural(unsent, "change")} not yet sent" else "") + " will be dropped.",
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
                            val typed: () -> Unit = {
                                PairLink.manual(address, code)?.let(::pair) ?: run { problem = "Enter the PC's address and the code it printed" }
                            }
                            Field(address, { address = it }, Modifier.fillMaxWidth(), placeholder = "PC address, e.g. 192.168.1.20 or my-pc.tail1234.ts.net", leading = "wifi",
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next))
                            Field(code, { code = it.uppercase() }, Modifier.fillMaxWidth(), placeholder = "Code, e.g. ABCD-EFGH", leading = "shield",
                                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false, imeAction = ImeAction.Go),
                                keyboardActions = KeyboardActions(onGo = { if (address.isNotBlank() && code.isNotBlank()) typed() }))
                            Key("Pair", typed, kind = KeyKind.Primary, enabled = address.isNotBlank() && code.isNotBlank())
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
                                    val at = pc.routes.indexOf(r)
                                    scope.launch { nav.relay.pc.update { p -> p.copy(routes = p.routes - r) }; nav.relay.hub.link.restart() }
                                    nav.shell.toast("Removed ${r.url}", undo = {
                                        nav.relay.pc.update { p -> if (r in p.routes) p else p.copy(routes = p.routes.toMutableList().apply { add(at.coerceIn(0, size), r) }) }
                                        nav.relay.hub.link.restart()
                                    })
                                }, size = 14.dp, label = "Remove this route")
                            }
                        }
                        Row(Modifier.padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            val add: () -> Unit = {
                                PairLink.address(extra)?.let { url ->
                                    scope.launch {
                                        nav.relay.pc.update { p -> p.copy(routes = p.routes + Route(url, Route.Kind.of(url))) }
                                        nav.relay.hub.link.restart()
                                    }
                                    extra = ""
                                } ?: nav.shell.toast("That is not an address")
                            }
                            Field(extra, { extra = it }, Modifier.weight(1f), placeholder = "Add Tailscale or other address",
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
                                keyboardActions = KeyboardActions(onDone = { if (extra.isNotBlank()) add() }))
                            Key("Add", add, enabled = extra.isNotBlank(), compact = true)
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

                    ServerCopy(nav, pc = s.link)

                    Slab(Modifier.fillMaxWidth()) {
                        Row(
                            Modifier.switchRow(s.prefs.stayConnected) { on ->
                                nav.shell.update { it.copy(stayConnected = on) }
                                if (on) LinkService.start(context) else LinkService.stop(context)
                            },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                T("Stay connected", style = Relay.type.uiMedium, color = c.ink)
                                T("Keeps the link open in the background so agents that need you can say so. Shows an ongoing notification.", style = Relay.type.caption, color = c.ink3)
                            }
                            Toggle(s.prefs.stayConnected, null)
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
                                if (s.outbox.isEmpty()) "Forget the PC and this phone's copy of its data?" else "Forget the PC? ${plural(s.outbox.size, "change")} not yet sent will be lost.",
                                style = Relay.type.ui, color = c.heldText,
                            )
                            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Key("Forget", {
                                    scope.launch {
                                        LinkService.stop(context)
                                        nav.relay.forget()
                                        nav.pair(top = true)
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

/**
 * The server's copy (docs/CLOUD.md): a second, data-only host that answers Tally's and Avex's
 * reads while the PC is away. Paired here with a link printed on the server, beside the PC and
 * never in its place: nothing here pairs, forgets or wipes the PC or this phone's copy of its data.
 */
@Composable
private fun ServerCopy(nav: Nav, pc: LinkState) {
    val copy by nav.relay.cloudProfile.collectAsStateWithLifecycle(null)
    val state by nav.relay.cloudState.collectAsStateWithLifecycle()
    val seen by nav.relay.cloudLastSeen.collectAsStateWithLifecycle(0L)
    Slab(Modifier.fillMaxWidth()) {
        Eyebrow("Your server's copy")
        Gap(6.dp)
        val p = copy
        if (p == null) ServerCopyPaste(nav) else ServerCopyState(nav, p, state, seen, pc)
    }
}

/** A paired server's copy: how its link stands, and forgetting it. */
@Composable
private fun ServerCopyState(nav: Nav, p: PcProfile, state: LinkState, seen: Long, pc: LinkState) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    var confirmForget by remember { mutableStateOf(false) }
    Column {
        val refused = state is LinkState.Revoked
        Row(verticalAlignment = Alignment.CenterVertically) {
            Dot(linkColor(state), 8.dp)
            T("  " + serverLine(state, pc), Modifier.weight(1f), Relay.type.uiMedium, c.ink)
        }
        p.routes.firstOrNull()?.let { T(it.url, Modifier.padding(top = 4.dp), Relay.type.mono, c.ink3, maxLines = 1) }
        T(
            if (refused) "It did not let this phone in. Forget it here, then connect again with a new link."
            else "While the PC is off, Tally's and Avex's readings come from it, marked \"From your server's copy\". Nothing else is sent there.",
            Modifier.padding(top = 6.dp), Relay.type.caption, if (refused) c.heldText else c.ink3,
        )
        if (seen > 0 && state !is LinkState.Online) T("Last reached ${ago(seen)}", Modifier.padding(top = 4.dp), Relay.type.caption, c.ink3)
        Hairline(Modifier.padding(vertical = 10.dp))
        T("This phone is device ${p.device} there. To cut it off from the server: relay remote revoke ${p.device}.", style = Relay.type.caption, color = c.ink3)
        if (!confirmForget) {
            Key("Forget the server's copy", { confirmForget = true }, Modifier.padding(top = 8.dp), KeyKind.Danger)
        } else {
            T("Forget the server's copy? The PC and this phone's copy of its data stay.", Modifier.padding(top = 10.dp), Relay.type.ui, c.heldText)
            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Key("Forget", {
                    scope.launch {
                        nav.relay.forgetCloud()
                        confirmForget = false
                    }
                }, kind = KeyKind.Danger)
                Key("Keep", { confirmForget = false }, kind = KeyKind.Quiet)
            }
        }
    }
}

/** Where the link to the server's copy stands, in the words of the PC's line above it. */
private fun serverLine(state: LinkState, pc: LinkState): String = when (state) {
    is LinkState.Online -> "Connected · reading Tally and Avex"
    is LinkState.Connecting, is LinkState.Starting, LinkState.Unpaired -> "Reaching your server"
    is LinkState.Offline -> "Your server is out of reach"
    LinkState.Stopped -> when (pc) {
        is LinkState.Online -> "Not needed while the PC is connected"
        // It stands in for a PC that is away, not for one disconnected here.
        LinkState.Stopped, LinkState.Unpaired -> "Off while the PC is disconnected"
        else -> "Waiting on the PC"
    }
    is LinkState.Revoked -> "Your server refused this phone"
}

/**
 * Connecting the server's copy with the link `relay --instance stable remote pair --via-only`
 * printed on the server (docs/CLOUD.md, "Setting it up"): on the PC's page, and in the personal
 * build when its bot cannot hand the link over itself. [onConnected] runs once it is paired.
 */
@Composable
internal fun ServerCopyPaste(nav: Nav, onConnected: () -> Unit = {}) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboardManager.current
    var pasted by remember { mutableStateOf("") }
    var waiting by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<String?>(null) }

    fun connect(target: PairLink) {
        if (waiting) return
        waiting = true
        problem = null
        scope.launch {
            nav.relay.pairCloud(target)
                .onSuccess {
                    pasted = ""
                    onConnected()
                }
                .onFailure { e -> problem = (e as? RefusalException)?.refusal?.text ?: e.message ?: "Connecting failed" }
            waiting = false
        }
    }

    T("Your server keeps Tally's ledger and Avex's history too. Connect it, and this phone reads them from there while the PC is off.", style = Relay.type.caption, color = c.ink2)
    T("On the server, run relay --instance stable remote pair --via-only and paste the link it prints.", Modifier.padding(top = 6.dp), Relay.type.caption, c.ink3)
    Field(pasted, { pasted = it }, Modifier.fillMaxWidth().padding(top = 10.dp), placeholder = "relay://pair?v=1&code=…", singleLine = false, maxLines = 4, leading = "link")
    Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Key("Paste", { clipboard.getText()?.text?.let { pasted = it.trim() } }, glyph = "copy")
        Key(if (waiting) "Connecting…" else "Connect", {
            PairLink.parse(pasted)?.let(::connect) ?: run { problem = "That is not a relay://pair link" }
        }, kind = KeyKind.Primary, enabled = pasted.isNotBlank() && !waiting)
    }
    if (waiting) T("Answer y on the server when it asks. The code works once.", Modifier.padding(top = 6.dp), Relay.type.caption, c.ink3)
    problem?.let { T(it, Modifier.padding(top = 6.dp), Relay.type.ui, c.heldText) }
}
