package com.quietsoftware.relay.ui.shell

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.kit.Fill
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.IconKey
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * The guardrail prompt (guardrails.css), the one floating surface: an agent held by a guardrail
 * waits on the person, wherever they are in the app. The exact held request is read first
 * (`guardrail.hold.get`); Allow once wakes only after it is shown, and half a second later, so a
 * tap meant for something else cannot approve it. Over a page that lists the holds itself, [show]
 * is false and the tray stays out of the way.
 */
@Composable
fun BoxScope.HoldTray(model: ShellModel, show: Boolean = true, onOpen: () -> Unit) {
    val s by model.state.collectAsStateWithLifecycle()
    var dismissed by remember { mutableStateOf(setOf<Long>()) }
    val shown = s.holds.filter { it.id !in dismissed }
    AnimatedVisibility(
        visible = show && shown.isNotEmpty() && s.online,
        modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(start = 12.dp, end = 12.dp, bottom = 44.dp),
        enter = fadeIn(tween(260)) + slideInVertically(tween(260)) { it / 3 },
        exit = fadeOut(tween(180)),
    ) {
        val hold = shown.firstOrNull() ?: return@AnimatedVisibility
        HoldCard(hold, more = shown.size - 1, model = model, onLater = { dismissed = dismissed + hold.id }, onOpen = onOpen)
    }
}

@Composable
fun HoldCard(hold: Hold, more: Int, model: ShellModel, onLater: (() -> Unit)?, onOpen: (() -> Unit)?, modifier: Modifier = Modifier) {
    val c = Relay.colors
    val scope = rememberCoroutineScope()
    var detail by remember(hold.id) { mutableStateOf<JsonObject?>(null) }
    var armed by remember(hold.id) { mutableStateOf(false) }
    var busy by remember(hold.id) { mutableStateOf(false) }
    LaunchedEffect(hold.id) {
        // Read again until it answers: a lost read would leave Allow once off for good.
        while (detail == null) {
            detail = runCatching { model.relay.call("guardrail.hold.get", buildJsonObject { put("hold_id", hold.id) }) as? JsonObject }.getOrNull()
            if (detail == null) delay(3_000)
        }
        delay(500)
        armed = true
    }
    val payload = detail?.o("request")?.o("payload")
    val details = detail?.o("hold")?.o("details") ?: hold.details as? JsonObject
    // A gate names its command or path; an exception request names its value (guardrail_pages.rs `subject`).
    val raw = payload?.s("command") ?: payload?.s("path") ?: payload?.s("value")
        ?: details?.s("command") ?: details?.s("path") ?: details?.s("value") ?: payload?.s("message") ?: hold.op
    val (subject, hidden) = remember(raw) { reveal(raw) }
    val why = payload?.s("reason")?.let { reveal(it).first }
    val open = detail?.o("hold")?.s("state") == "open"
    Column(
        modifier.widthIn(max = 520.dp).fillMaxWidth()
            .shadow(14.dp, com.quietsoftware.relay.ui.kit.Radii.key)
            .clip(com.quietsoftware.relay.ui.kit.Radii.key)
            .background(c.console)
            .border(1.dp, c.strong, com.quietsoftware.relay.ui.kit.Radii.key),
    ) {
        Box(Modifier.fillMaxWidth().height(2.dp).background(c.waiting))
        Column(Modifier.padding(start = 14.dp, end = 6.dp, top = 8.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Glyph("shield", 14.dp, c.waiting)
                T("  Held · ${hold.policy.ifBlank { "guardrail" }} · ${ago(epoch(hold.createdAt))}", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                Fill()
                onLater?.let { IconKey("close", it, size = 14.dp) }
            }
            T("${hold.session ?: "An agent"} wants to ${verb(hold.op, payload?.s("kind"))}", Modifier.padding(end = 8.dp), Relay.type.uiMedium, c.ink)
            if (hidden > 0) {
                T("Contains hidden characters, shown as <U+…>; what was sent may read differently from what it does.", Modifier.padding(end = 8.dp), Relay.type.caption, c.heldText)
            }
            // The whole request, scrolled rather than cut, so nothing approved is out of sight.
            Box(Modifier.padding(end = 8.dp).clip(com.quietsoftware.relay.ui.kit.Radii.keycap).background(c.screen).heightIn(max = 160.dp).verticalScroll(rememberScrollState()).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 8.dp)) {
                T(subject, style = Relay.type.code, color = c.ink2)
            }
            why?.let { T("Why: $it", Modifier.padding(end = 8.dp), Relay.type.caption, c.ink3, maxLines = 3) }
            Row(Modifier.padding(end = 8.dp, top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Key("Allow once", {
                    busy = true
                    scope.launch {
                        try {
                            model.relay.call("guardrail.confirm", buildJsonObject { put("hold_id", hold.id) })
                            model.toast("Allowed once")
                        } catch (e: BusException) {
                            model.toast(if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code })
                        }
                        busy = false
                    }
                }, kind = KeyKind.Primary, enabled = armed && !busy && open, compact = true)
                Key("Deny", {
                    busy = true
                    scope.launch {
                        try {
                            model.relay.call("guardrail.reject", buildJsonObject { put("hold_id", hold.id); put("reason", "Denied from the phone") })
                            model.toast("Denied")
                        } catch (e: BusException) {
                            model.toast(if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code })
                        }
                        busy = false
                    }
                }, kind = KeyKind.Plain, enabled = !busy, compact = true)
                Fill()
                if (more > 0) T("$more more", style = Relay.type.caption, color = c.ink3)
                onOpen?.let { Key("Details", it, kind = KeyKind.Quiet, compact = true) }
            }
        }
    }
}

/** What the held request does; a hook's gate and an exception request say it in their `kind`. */
private fun verb(op: String, kind: String?) = when {
    op == "guardrail.gate" -> when (kind) {
        "exec" -> "run a command"
        "write" -> "write a file"
        "commit" -> "commit"
        else -> "act past a guardrail"
    }
    op == "guardrail.request" -> when (kind) {
        "command" -> "be allowed to run a command"
        "path" -> "be allowed to write to a path"
        else -> "commit past the change caps"
    }
    op.startsWith("git.push") -> "push"
    op.startsWith("git.") -> "run git (${op.removePrefix("git.")})"
    op.startsWith("file.write") -> "write a file"
    op.startsWith("file.delete") -> "delete a file"
    op.contains("command") || op.startsWith("bash") -> "run a command"
    else -> op
}

/** Bidi controls, zero-width and other invisible characters (guardrail_pages.rs `is_hidden`). */
private fun hiddenChar(cp: Int): Boolean =
    cp in 0x202A..0x202E || cp in 0x2066..0x2069 || cp == 0x200E || cp == 0x200F || cp == 0x061C ||
        cp in 0x200B..0x200D || cp in 0x2060..0x2064 || cp in 0x206A..0x206F || cp == 0xFEFF ||
        cp == 0x00AD || cp == 0x034F || cp == 0x115F || cp == 0x1160 || cp == 0x17B4 || cp == 0x17B5 || cp in 0x180B..0x180F ||
        cp == 0x2028 || cp == 0x2029 || cp == 0x3164 || cp == 0xFFA0 || cp in 0xFE00..0xFE0F || cp in 0xFFF9..0xFFFB ||
        cp in 0xE0000..0xE007F || cp in 0xE0100..0xE01EF || (Character.isISOControl(cp) && cp != '\n'.code)

/**
 * Agent-supplied text as it is to be approved: each hidden character shown as `<U+202E>`, so a
 * command cannot read differently from what it does. With how many there were.
 */
internal fun reveal(value: String): Pair<String, Int> {
    val out = StringBuilder(value.length)
    var hidden = 0
    value.codePoints().forEach { cp ->
        if (hiddenChar(cp)) {
            out.append("<U+%04X>".format(cp))
            hidden++
        } else {
            out.appendCodePoint(cp)
        }
    }
    return out.toString() to hidden
}
