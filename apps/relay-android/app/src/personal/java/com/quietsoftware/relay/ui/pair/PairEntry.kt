package com.quietsoftware.relay.ui.pair

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.link.RefusalException
import com.quietsoftware.relay.core.wire.PairLink
import com.quietsoftware.relay.personal.Asked
import com.quietsoftware.relay.personal.ConnectFailure
import com.quietsoftware.relay.personal.Reply
import com.quietsoftware.relay.personal.SERVER
import com.quietsoftware.relay.personal.ServerConnect
import com.quietsoftware.relay.service.LinkService
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Eyebrow
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.RingMark
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.column
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * PERSONAL BUILD ONLY. Pairing goes through Antho's server instead of a QR code (docs/CLOUD.md,
 * "Connecting through your server"): one tap on Discord, then Accept on the PC's screen. The
 * builds anyone can install keep `PairScreen` (src/standard).
 */
@Composable
fun PairEntry(@Suppress("UNUSED_PARAMETER") link: String?, nav: Nav) = ConnectScreen(nav)

/** Where asking stands, as the screen shows it. */
private sealed interface Step {
    data object Idle : Step
    data object Reaching : Step
    data class Waiting(val asked: Asked) : Step
    data class WaitingPc(val asked: Asked) : Step
    /** The PC's window is open: the phone presents its code, and the PC asks on its screen. */
    data class Pairing(val host: String) : Step
    data class Problem(val text: String) : Step
}

@Composable
private fun ConnectScreen(nav: Nav) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val uri = LocalUriHandler.current
    val paired by nav.shell.state.collectAsStateWithLifecycle()
    val api = remember { ServerConnect() }
    var step by remember { mutableStateOf<Step>(Step.Idle) }
    var job by remember { mutableStateOf<Job?>(null) }
    DisposableEffect(Unit) { onDispose { job?.cancel() } }

    fun connect() {
        if (job?.isActive == true) return
        job = scope.launch {
            step = Step.Reaching
            val asked = try {
                if (!api.pcConnected()) {
                    step = Step.Problem("No PC is connected to your server yet. On the PC, run: relay cloud connect ${SERVER.removePrefix("https://")}")
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
                                LinkService.start(context)
                                nav.start()
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
                            RingMark(34.dp)
                            T("relay", Modifier.padding(start = 9.dp), Relay.type.brand.copy(fontSize = Relay.type.heading.fontSize), c.ink)
                        }
                    }
                    T("Pair this phone with your PC", Modifier.padding(top = 12.dp), Relay.type.heading, c.ink)
                    T(
                        "Your server asks you on Discord, then your PC asks you on its screen. Nothing to scan or type, on your WiFi or away from it.",
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
                    when (val s = step) {
                        Step.Idle -> Key("Connect through my server", ::connect, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "link")
                        Step.Reaching -> Waiting("Asking your server…", null)
                        is Step.Waiting -> {
                            Code(s.asked.code)
                            Waiting(
                                if (s.asked.notified) "Your bot sent you a message on Discord. Tap Allow if it shows this code."
                                else "Your bot could not message you. Allow the request on its dashboard, Devices, where it shows this code.",
                                null,
                            )
                            if (!s.asked.notified) {
                                Key("Open the dashboard", { runCatching { uri.openUri("$SERVER/dashboard/devices") } }, glyph = "external")
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
                        is Step.Problem -> {
                            T(s.text, style = Relay.type.ui, color = c.heldText)
                            Key("Ask again", { step = Step.Idle; connect() }, Modifier.fillMaxWidth(), kind = KeyKind.Primary, glyph = "refresh")
                        }
                    }
                    T("Through ${SERVER.removePrefix("https://")}", Modifier.padding(top = 8.dp), Relay.type.caption, c.ink3)
                    Gap(24.dp)
                }
            }
        }
    }
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
