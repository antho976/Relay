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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.quietsoftware.relay.core.model.Hold
import com.quietsoftware.relay.core.wire.BusException
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.kit.Field
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
 * (`guardrail.hold.get`); Allow once, or an exception request's Approve keys, wake only after it is
 * shown, and half a second later, so a tap meant for something else cannot approve it. Over a page
 * that lists the holds itself, [show] is false and the tray stays out of the way.
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HoldCard(
    hold: Hold,
    more: Int,
    model: ShellModel,
    onLater: (() -> Unit)?,
    onOpen: (() -> Unit)?,
    modifier: Modifier = Modifier,
    openLabel: String = "Open inbox",
) {
    val c = Relay.colors
    val s by model.state.collectAsStateWithLifecycle()
    // The model's scope, not the card's: the card goes as soon as the hold is answered, often
    // before the answer to the key that answered it, whose toast would go with it.
    val scope = model.viewModelScope
    var detail by remember(hold.id) { mutableStateOf<JsonObject?>(null) }
    var armed by remember(hold.id) { mutableStateOf(false) }
    var busy by remember(hold.id) { mutableStateOf(false) }
    // Deny… opens a reason the agent reads, in the card, as on the PC; null while the keys show.
    var denial by remember(hold.id) { mutableStateOf<String?>(null) }
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
    val read = detail?.o("hold")
    val (policy, details) = effective(read?.s("policy") ?: hold.policy, read?.o("details") ?: hold.details as? JsonObject)
    val request = hold.op == "guardrail.request"
    // A gate names its command or path; an exception request names its value (guardrail_pages.rs `subject`).
    val raw = payload?.s("command") ?: payload?.s("path") ?: payload?.s("value")
        ?: details?.s("command") ?: details?.s("path") ?: details?.s("value") ?: payload?.s("message") ?: hold.op
    val (subject, hidden) = remember(raw) { reveal(raw) }
    val why = payload?.s("reason")?.let { reveal(it).first }
    // Allow once waits for the read to say the hold is open; Deny… goes on the phone's copy until then.
    val open = (read?.s("state") ?: hold.state) == "open"
    val kind = payload?.s("kind") ?: if (request) details?.s("kind") else null
    // What the agent asked for: its key is the primary one, and either answer says how long.
    val asked = details?.s("requested_scope") ?: "once"
    val ready = armed && open && s.online && !busy

    fun failed(e: BusException) = if (e.error.code == "link.down") "Needs the PC, which is out of reach" else e.error.message.ifBlank { e.error.code }

    fun allow() {
        busy = true
        scope.launch {
            try {
                val outcome = (model.relay.call("guardrail.confirm", buildJsonObject { put("hold_id", hold.id) }) as? JsonObject)?.o("outcome")
                // Allowed is not done: the held action runs now, and can still fail on its own.
                if (outcome?.b("ok") == false) {
                    val error = outcome?.o("error")
                    val message = error?.s("message")?.ifBlank { null } ?: error?.s("code").orEmpty()
                    model.toast(reveal("Allowed, but the action then failed: ${message.take(400)}").first)
                } else {
                    model.toast("Allowed once")
                }
            } catch (e: BusException) {
                model.toast(failed(e))
            }
            busy = false
        }
    }

    fun approve(lasting: String) {
        busy = true
        scope.launch {
            try {
                model.relay.call("guardrail.confirm", buildJsonObject { put("hold_id", hold.id); put("scope", lasting) })
                model.toast(approvedNotice(hold.session ?: "The agent", kind, details?.s("value") ?: payload?.s("value").orEmpty(), lasting))
            } catch (e: BusException) {
                model.toast(failed(e))
            }
            busy = false
        }
    }

    fun deny(reason: String) {
        busy = true
        scope.launch {
            try {
                val text = reason.trim()
                model.relay.call("guardrail.reject", buildJsonObject { put("hold_id", hold.id); if (text.isNotEmpty()) put("reason", text) })
                model.toast("Denied")
                denial = null
            } catch (e: BusException) {
                model.toast(failed(e))
            }
            busy = false
        }
    }

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
                T("  ${policyTitle(policy)} · ${ago(epoch(hold.createdAt))}", style = Relay.type.caption, color = c.ink3, maxLines = 1)
                Fill()
                onLater?.let { IconKey("close", it, size = 14.dp, label = "Answer later") }
            }
            // The person's own action, held for a second look, is not an agent's (guardrail_pages.rs `hold_row`).
            val own = !request && (hold.session.isNullOrBlank() || hold.actor == "user")
            T(
                if (own) "Your own action is held: ${verb(hold.op, kind)}" else "${hold.session ?: "An agent"} wants to ${verb(hold.op, kind)}",
                Modifier.padding(end = 8.dp),
                Relay.type.uiMedium,
                c.ink,
            )
            if (hidden > 0) {
                T("Contains hidden characters, shown as <U+…>; what was sent may read differently from what it does.", Modifier.padding(end = 8.dp), Relay.type.caption, c.heldText)
            }
            // The whole request, scrolled rather than cut, so nothing approved is out of sight.
            Box(Modifier.padding(end = 8.dp).clip(com.quietsoftware.relay.ui.kit.Radii.keycap).background(c.screen).heightIn(max = 160.dp).verticalScroll(rememberScrollState()).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 8.dp)) {
                T(subject, style = Relay.type.code, color = c.ink2)
            }
            why?.let { T("Why: $it", Modifier.padding(end = 8.dp), Relay.type.caption, c.ink3, maxLines = 3) }
            if (request) {
                T(
                    "Asked for ${if (asked == "session") "the rest of the session" else "one use"}. Only this rule is lifted, and only for ${hold.session ?: "this agent"}.",
                    Modifier.padding(end = 8.dp),
                    Relay.type.caption,
                    c.ink3,
                )
            }
            if (!s.online) T("Answer once the PC is in reach.", Modifier.padding(end = 8.dp), Relay.type.caption, c.ink3)
            val reason = denial
            if (reason != null) {
                Field(reason, { denial = it }, Modifier.fillMaxWidth().padding(end = 8.dp, top = 4.dp), placeholder = "Why not? The agent reads this")
                Row(Modifier.padding(end = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Key("Send denial", { deny(reason) }, kind = KeyKind.Primary, enabled = open && s.online && !busy, compact = true)
                    Key("Cancel", { denial = null }, kind = KeyKind.Quiet, compact = true)
                }
            } else {
                // Keys wrap rather than squeeze: an exception request has three, and a phone is narrow.
                FlowRow(
                    Modifier.padding(end = 8.dp, top = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (request) {
                        // The desktop's two answers (guardrail_pages.rs `exception_card`), each saying how long.
                        Key("Approve once", { approve("once") }, kind = if (asked == "session") KeyKind.Plain else KeyKind.Primary, enabled = ready, compact = true)
                        Key("Approve for this session", { approve("session") }, kind = if (asked == "session") KeyKind.Primary else KeyKind.Plain, enabled = ready, compact = true)
                    } else {
                        Key("Allow once", { allow() }, kind = KeyKind.Primary, enabled = ready, compact = true)
                    }
                    Key("Deny…", { denial = "" }, kind = KeyKind.Plain, enabled = open && s.online && !busy, compact = true)
                    if (more > 0 || onOpen != null) {
                        Row(Modifier.heightIn(min = 34.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            if (more > 0) T("$more more", style = Relay.type.caption, color = c.ink3)
                            onOpen?.let { Key(openLabel, it, kind = KeyKind.Quiet, compact = true) }
                        }
                    }
                }
            }
        }
    }
}

/** What a policy is called where a person reads it (guardrail_pages.rs `policy_title`). */
internal fun policyTitle(policy: String) = when (policy) {
    "exception" -> "Exception request"
    "protected_path" -> "Protected path"
    "destructive_write" -> "Large rewrite"
    "shape_gate" -> "File shape check"
    "write_root" -> "Write outside the worktree"
    "denied_command" -> "Denied command"
    "cap" -> "Commit over the size caps"
    else -> "Guardrail hold"
}

/**
 * The policy that actually stopped the action, and its details, from a hold's [policy] and
 * [details]: a hold the person tripped wraps the original (guardrail_pages.rs `effective`).
 */
internal fun effective(policy: String, details: JsonObject?): Pair<String, JsonObject?> {
    val original = details?.o("original")
    return if (details != null && original != null) details.s("policy").orEmpty() to original else policy to details
}

/** What the toast says once a request is approved: who may now do what, and for how long (`approved_notice`). */
private fun approvedNotice(session: String, kind: String?, value: String, lasting: String): String {
    val what = when {
        value.isEmpty() -> ""
        value.length > 120 -> " “${value.take(120)}…”"
        else -> " “$value”"
    }
    val does = when (kind) {
        "command" -> "run"
        "path" -> "write to"
        else -> "commit past the change caps"
    }
    val long = if (lasting == "session") "for the rest of the session" else "once"
    return reveal("Approved: $session may $does$what $long. The agent has been told to retry.").first
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
