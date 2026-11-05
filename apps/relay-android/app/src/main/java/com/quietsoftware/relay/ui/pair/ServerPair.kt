package com.quietsoftware.relay.ui.pair

import android.content.Context
import android.os.Build
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.core.link.RefusalException
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.data.link.Asked
import com.quietsoftware.relay.data.link.BUILT_IN_SERVER
import com.quietsoftware.relay.data.link.ConnectFailure
import com.quietsoftware.relay.data.link.Reply
import com.quietsoftware.relay.data.link.ServerConnect
import com.quietsoftware.relay.service.LinkService
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Field
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/*
 * Pairing through the owner's server, the way Tally 2 connects (docs/CLOUD.md, "Connecting through
 * your server"): the person types their server's address (the personal build fills in its owner's),
 * the server asks them on Discord, then the PC asks on its own screen. Nothing to scan, on the WiFi
 * or away from it. Then the server's copy, the same way, as a second host beside the PC.
 */

/** Where asking stands, as the screen shows it. */
internal sealed interface Step {
    data object Idle : Step
    data object Reaching : Step
    data class Waiting(val asked: Asked) : Step
    data class WaitingPc(val asked: Asked) : Step
    /** The PC's window is open: the phone presents its code, and the PC asks on its screen. */
    data class Pairing(val host: String) : Step
    data class Problem(val text: String) : Step
}

/** Once the PC is paired: its server's copy too (docs/CLOUD.md), asked of the bot as `relay-cloud`. */
internal sealed interface CopyStep : Step {
    /** Paired with the PC; the server's copy is offered. */
    data class Offered(val host: String) : CopyStep

    /** Asking the bot; [asked] is null until it took the request. */
    data class Waiting(val asked: Asked?) : CopyStep

    /** The bot does not know `relay-cloud` yet: a link printed on the server, pasted. */
    data object Paste : CopyStep
    data class Problem(val text: String) : CopyStep
}

/** The address the pairing screen starts with: the one used last, else this build's own. */
internal fun startingServer(saved: String): String = shown(saved.ifBlank { BUILT_IN_SERVER })

/** An address as the field shows it: `https://` is understood. */
private fun shown(server: String): String = ServerConnect.normalize(server).removePrefix("https://")

/** Asking the server, and pairing with what it hands over; lives as long as the pairing screen. */
@Stable
internal class ServerPairing(private val nav: Nav, private val scope: CoroutineScope, private val context: Context) {
    var step by mutableStateOf<Step>(Step.Idle)
        private set

    /** The server asked last: the server's copy is asked of the same one. */
    var api: ServerConnect? = null
        private set

    private var job: Job? = null

    /** Past the address: the server, Discord or the PC has the next move, or the PC is paired. */
    val busy: Boolean get() = step !is Step.Idle && step !is Step.Problem

    val copying: Boolean get() = step is CopyStep

    fun connect(typed: String) {
        if (job?.isActive == true) return
        val server = ServerConnect.normalize(typed)
        if (server.isEmpty()) return
        val api = ServerConnect(server).also { this.api = it }
        // Remembered for next time; the build's own address is kept as blank, so it follows the build.
        nav.shell.update { it.copy(server = if (server == ServerConnect.normalize(BUILT_IN_SERVER)) "" else server) }
        job = scope.launch {
            step = Step.Reaching
            val asked = try {
                if (!api.pcConnected()) {
                    step = Step.Problem(ServerConnect.noPc(api.host))
                    return@launch
                }
                api.ask(deviceName())
            } catch (e: ConnectFailure) {
                step = Step.Problem(e.message.orEmpty())
                return@launch
            }
            step = Step.Waiting(asked)
            var troubles = 0
            while (true) {
                delay(2_000)
                when (val reply = api.claim(asked)) {
                    Reply.Waiting -> step = Step.Waiting(asked)
                    Reply.WaitingPc -> step = Step.WaitingPc(asked)
                    is Reply.Ready -> {
                        val target = PairLink.parse(reply.link)
                        if (target == null) {
                            step = Step.Problem("Your PC's pairing link did not read. Ask again.")
                            return@launch
                        }
                        step = Step.Pairing(target.host)
                        nav.relay.pair(target)
                            .onSuccess {
                                // The background link only when the person keeps it (Stay connected).
                                if (nav.shell.state.value.prefs.stayConnected) LinkService.start(context)
                                // Its server's copy is offered next, unless this phone holds one already.
                                if (nav.relay.cloud.current() != null) nav.start() else step = CopyStep.Offered(target.host)
                            }
                            .onFailure { e ->
                                step = Step.Problem((e as? RefusalException)?.refusal?.text ?: e.message ?: "Pairing failed")
                            }
                        return@launch
                    }
                    Reply.Denied -> {
                        step = Step.Problem("The request was denied on Discord.")
                        return@launch
                    }
                    Reply.Expired -> {
                        step = Step.Problem("The request expired before your PC answered. Ask again.")
                        return@launch
                    }
                    Reply.Spent -> {
                        step = Step.Problem("The request was used up. Ask again.")
                        return@launch
                    }
                    is Reply.Trouble -> if (++troubles >= 10) {
                        step = Step.Problem(reply.reason)
                        return@launch
                    }
                }
            }
        }
    }

    /** Stops asking and comes back to the address; the request left on the server runs out on its own. */
    fun cancel() {
        stop()
        step = Step.Idle
    }

    fun stop() {
        job?.cancel()
        job = null
    }

    /** The server's copy, asked of the bot the same way; a bot that does not offer it yet gets a pasted link instead. */
    fun connectCopy() {
        val api = api ?: return
        if (job?.isActive == true) return
        job = scope.launch {
            step = CopyStep.Waiting(null)
            val asked = try {
                api.ask(deviceName(), ServerConnect.KIND_CLOUD)
            } catch (e: ConnectFailure) {
                step = if (e.unsupported) CopyStep.Paste else CopyStep.Problem(e.message.orEmpty())
                return@launch
            }
            step = CopyStep.Waiting(asked)
            var troubles = 0
            while (true) {
                delay(2_000)
                when (val reply = api.claim(asked)) {
                    Reply.Waiting, Reply.WaitingPc -> Unit
                    is Reply.Ready -> {
                        val target = PairLink.parse(reply.link)
                        if (target == null) {
                            step = CopyStep.Problem("Your server's link did not read. Ask again.")
                            return@launch
                        }
                        nav.relay.pairCloud(target)
                            .onSuccess { nav.start() }
                            .onFailure { e -> step = CopyStep.Problem((e as? RefusalException)?.refusal?.text ?: e.message ?: "Connecting failed") }
                        return@launch
                    }
                    Reply.Denied -> {
                        step = CopyStep.Problem("The request was denied on Discord.")
                        return@launch
                    }
                    Reply.Expired -> {
                        step = CopyStep.Problem("The request expired. Ask again.")
                        return@launch
                    }
                    Reply.Spent -> {
                        step = CopyStep.Problem("The request was used up. Ask again.")
                        return@launch
                    }
                    is Reply.Trouble -> if (++troubles >= 10) {
                        step = CopyStep.Problem(reply.reason)
                        return@launch
                    }
                }
            }
        }
    }

    /** Done with pairing, with or without the server's copy. */
    fun finish() {
        stop()
        nav.start()
    }
}

/** The pairing screen's server way in: the address, then where asking stands. */
@Composable
internal fun ServerPair(p: ServerPairing, server: String, onServer: (String) -> Unit, nav: Nav) {
    val c = Relay.colors
    val uri = LocalUriHandler.current
    val connect = { p.connect(server) }
    when (val s = p.step) {
        Step.Idle, is Step.Problem -> {
            Field(
                server, onServer, Modifier.fillMaxWidth(), placeholder = shown(BUILT_IN_SERVER).ifBlank { "Your server, e.g. my-bot.onrender.com" }, leading = "cloud",
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { if (server.isNotBlank()) connect() }),
            )
            if (s is Step.Problem) T(s.text, style = Relay.type.ui, color = c.heldText)
            Key(
                if (s is Step.Problem) "Ask again" else "Connect through my server", connect, Modifier.fillMaxWidth(),
                kind = KeyKind.Primary, glyph = if (s is Step.Problem) "refresh" else "link", enabled = server.isNotBlank(),
            )
            T("The address of your bot's server: the one Tally uses. Your PC connects to it once with relay cloud connect.", style = Relay.type.caption, color = c.ink3)
        }
        Step.Reaching -> Waiting("Asking your server…", null)
        is Step.Waiting -> {
            Code(s.asked.code)
            Waiting(
                if (s.asked.notified) "Your bot sent you a message on Discord. Tap Allow if it shows this code."
                else "Your bot could not message you. Allow the request on its dashboard, Devices, where it shows this code.",
                null,
            )
            val dashboard = p.api?.let { "${it.server}/dashboard/devices" }
            if (!s.asked.notified && dashboard != null) {
                Key("Open the dashboard", { runCatching { uri.openUri(dashboard) } }, glyph = "external")
            }
        }
        is Step.WaitingPc -> {
            Code(s.asked.code)
            Waiting("Allowed. Waiting for your PC to open a window: it has to be on, with Relay running.", null)
        }
        is Step.Pairing -> Waiting(
            "Accept on ${s.host.ifBlank { "your PC" }}",
            "A notification there asks to pair this phone. It waits about a minute and a half.",
        )
        is CopyStep.Offered -> {
            Slab(Modifier.fillMaxWidth()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Dot(c.live, 7.dp)
                    T("  Paired with ${s.host.ifBlank { "your PC" }}", style = Relay.type.uiMedium, color = c.ink)
                }
                T("Connect your server's copy as well, and Tally's and Avex's readings stay current here while the PC is off.", Modifier.padding(top = 6.dp), Relay.type.caption, c.ink3)
            }
            Key("Also connect your server's copy", p::connectCopy, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "cloud")
            Key("Not now", p::finish, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
        }
        is CopyStep.Waiting -> {
            val asked = s.asked
            if (asked == null) {
                Waiting("Asking your server…", null)
            } else {
                Code(asked.code)
                Waiting(
                    if (asked.notified) "Your bot sent you a message on Discord. Tap Allow if it shows this code."
                    else "Your bot could not message you. Allow the request on its dashboard, Devices, where it shows this code.",
                    null,
                )
            }
            Key("Not now", p::finish, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
        }
        CopyStep.Paste -> {
            T("Your server does not hand out its copy's link yet. Paste one printed there instead.", style = Relay.type.ui, color = c.ink2)
            Slab(Modifier.fillMaxWidth()) { ServerCopyPaste(nav, onConnected = { nav.start() }) }
            Key("Not now", p::finish, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
        }
        is CopyStep.Problem -> {
            T(s.text, style = Relay.type.ui, color = c.heldText)
            Key("Ask again", p::connectCopy, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "refresh")
            Key("Not now", p::finish, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
        }
    }
    // While it waits on the server, Discord or the PC; pairing itself ends on its own.
    val step = p.step
    if (step is Step.Reaching || step is Step.Waiting || step is Step.WaitingPc) {
        Key("Cancel", p::cancel, Modifier.fillMaxWidth(), kind = KeyKind.Quiet)
    }
    if (p.busy) p.api?.let { T("Through ${it.host}", Modifier.padding(top = 8.dp), Relay.type.caption, c.ink3) }
}

/** The four characters the Discord message shows too. */
@Composable
private fun Code(code: String) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth()) {
        Eyebrow("This phone's code")
        T(code, Modifier.padding(top = 6.dp), Relay.type.heading.copy(fontFamily = Relay.type.mono.fontFamily, fontSize = 30.sp, letterSpacing = 4.sp), c.ink)
    }
}

@Composable
private fun Waiting(text: String, detail: String?) {
    val c = Relay.colors
    Slab(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Dot(c.waiting, 7.dp)
            T("  $text", style = Relay.type.uiMedium, color = c.ink)
        }
        detail?.let { T(it, Modifier.padding(top = 6.dp), Relay.type.caption, c.ink3) }
    }
}

/** How the DM names this phone: "the Relay app on Pixel 8". */
private fun deviceName(): String = Build.MODEL.take(48)
