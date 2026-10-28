package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.b
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.KeyKind
import com.quietsoftware.relay.ui.kit.Segmented
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.shell.SpaceFrame
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Aide on the phone (docs/AIDE.md), as the PC's Aide pages: what waits on you first, what runs now
 * and what Aide said; its jobs, to run or pause; and what it learned, to confirm or forget. Its
 * settings (models, your servers, privacy, quiet hours) stay on the PC: a phone that is lost
 * should not be able to point Aide somewhere else.
 */
@Composable
fun AideScreen(nav: Nav) {
    SpaceFrame(nav) {
        val summary = rememberLive("aide.summary")
        val s = summary.result as? JsonObject
        var tab by rememberSaveable { mutableStateOf("now") }
        val missing = "This engine has no Aide yet. Update Relay's engine to use it."
        SourcePage("Aide", context = s?.let { { AideContext(it) } }) {
            Segmented(listOf("now" to "Now", "jobs" to "Jobs", "memory" to "Memory"), tab, { tab = it }, Modifier.fillMaxWidth(), fill = true)
            when (tab) {
                "jobs" -> Reading(rememberLive("aide.job.list"), missing) { Jobs(it, nav) }
                "memory" -> Reading(rememberLive("aide.fact.list"), missing) { Facts(it, nav) }
                else -> Reading(summary, missing) { Now(it, nav, missing) }
            }
        }
    }
}

/** "Paused", or the next wake and the day's interrupts. */
@Composable
private fun AideContext(s: JsonObject) {
    val c = Relay.colors
    val words = when {
        s.b("paused") == true -> "Paused: nothing runs, nothing is said"
        s.s("next_job") != null -> "Next: ${s.str("next_job")} ${untilOf(s.s("next_at"))}"
        else -> "No job is scheduled"
    }
    T("$words · ${s.l("interrupts_today") ?: 0} of ${s.l("interrupt_cap") ?: 0} interrupts today", style = Relay.type.caption, color = c.ink3, maxLines = 2)
}

/** "in 3h", "in 12m": how far off an RFC 3339 time is. */
internal fun untilOf(ts: String?): String {
    val at = com.quietsoftware.relay.ui.kit.epoch(ts)
    if (at <= 0) return ""
    val mins = (at - System.currentTimeMillis()) / 60_000
    return when {
        mins < 1 -> "now"
        mins < 60 -> "in ${mins}m"
        mins < 48 * 60 -> "in ${mins / 60}h"
        else -> "in ${mins / (24 * 60)}d"
    }
}

@Composable
private fun Now(s: JsonObject, nav: Nav, missing: String) {
    val running = s.objs("running")
    Reading(rememberLive("aide.errand.list"), missing) { e ->
        val errands = e.objs("errands")
        val waiting = errands.filter { it.s("state") == "waiting" || it.b("proposed_done") == true }
        Card("Waits on you", aside = waiting.size.takeIf { it > 0 }?.toString()) {
            if (waiting.isEmpty()) Quiet("Nothing waits on you.")
            waiting.forEachIndexed { i, errand -> ErrandRow(errand, nav, divider = i > 0) }
        }
        val open = errands.filter { it !in waiting }
        if (open.isNotEmpty()) Card("Aide is on") {
            open.forEachIndexed { i, errand -> ErrandRow(errand, nav, divider = i > 0) }
        }
    }
    if (running.isNotEmpty()) Card("Running now") {
        running.forEachIndexed { i, r ->
            FigureRow(r.str("job_name"), r.str("trigger"), r.str("state"), divider = i > 0)
            Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.End) {
                Key("Stop", { nav.shell.act("aide.run.stop", buildJsonObject { put("id", r.l("id") ?: 0) }, done = "Stopped") }, kind = KeyKind.Quiet)
            }
        }
    }
    val limit = remember { buildJsonObject { put("limit", 30) } }
    Reading(rememberLive("aide.message.list", limit), missing) { m ->
        val messages = m.objs("messages")
        Card("What Aide said") {
            if (messages.isEmpty()) Quiet("Aide has said nothing yet.")
            messages.forEachIndexed { i, msg -> MessageRow(msg, divider = i > 0) }
        }
    }
}

@Composable
private fun ErrandRow(e: JsonObject, nav: Nav, divider: Boolean) {
    val c = Relay.colors
    val asks = e.b("proposed_done") == true
    val detail = listOfNotNull(
        if (asks) "Aide thinks it is done" else if (e.s("state") == "waiting") "Waits on you" else null,
        e.s("due_at")?.let { "due ${untilOf(it).ifEmpty { agoOf(it) }}" },
        e.str("detail").takeIf { it.isNotBlank() },
    ).joinToString(" · ")
    FigureRow(e.str("title"), detail, divider = divider, detailColor = if (asks) c.waiting else c.ink3)
    Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
        val id = e.l("id") ?: 0
        if (asks) Key("Not yet", { nav.shell.act("aide.errand.update", buildJsonObject { put("id", id); put("state", "open") }) }, kind = KeyKind.Quiet)
        Key("Drop", { nav.shell.act("aide.errand.update", buildJsonObject { put("id", id); put("state", "dropped") }, done = "Dropped") }, kind = KeyKind.Quiet)
        Key("Done", { nav.shell.act("aide.errand.update", buildJsonObject { put("id", id); put("state", "done") }, done = "Closed") }, kind = KeyKind.Plain)
    }
}

@Composable
private fun MessageRow(m: JsonObject, divider: Boolean) {
    val c = Relay.colors
    val tone = when (m.s("level")) {
        "interrupt" -> c.held
        "brief" -> c.waiting
        else -> c.ink3
    }
    val how = when {
        m.b("notified") == true -> "told you"
        m.b("held") == true -> "held for the brief"
        m.s("briefed_at") != null -> "in a brief"
        else -> m.str("level")
    }
    FigureRow(m.str("title"), listOf(m.str("body").take(160), "${agoOf(m.s("created_at"))} · $how").filter { it.isNotBlank() }.joinToString("\n"), divider = divider,
        lead = { com.quietsoftware.relay.ui.kit.Dot(tone, 7.dp) })
}

@Composable
private fun Jobs(j: JsonObject, nav: Nav) {
    val c = Relay.colors
    val jobs = j.objs("jobs")
    Card("Jobs", aside = jobs.size.toString()) {
        if (jobs.isEmpty()) Quiet("No jobs yet. Make one in Aide on the PC, or ask Aide for one.")
        jobs.forEachIndexed { i, job ->
            val state = job.str("state")
            val when_ = (job.arr("when").mapNotNull { (it as? kotlinx.serialization.json.JsonPrimitive)?.content }).joinToString(" · ")
            val model = job.s("model") ?: "Aide's default model"
            FigureRow(job.str("name"), listOf(when_.ifEmpty { "Run by hand" }, model).joinToString(" · "), state, divider = i > 0,
                figureColor = when (state) { "active" -> c.live; "proposed" -> c.waiting; else -> c.ink3 })
            Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                val id = job.l("id") ?: 0
                when (state) {
                    "proposed" -> Key("Turn on", { nav.shell.act("aide.job.set", buildJsonObject { put("id", id); put("state", "active") }, done = "Turned on") }, kind = KeyKind.Primary)
                    "active" -> {
                        Key("Pause", { nav.shell.act("aide.job.set", buildJsonObject { put("id", id); put("state", "paused") }, done = "Paused") }, kind = KeyKind.Quiet)
                        Key("Run now", { nav.shell.act("aide.run.now", buildJsonObject { put("job_id", id) }, done = "Started") }, kind = KeyKind.Plain)
                    }
                    else -> Key("Turn on", { nav.shell.act("aide.job.set", buildJsonObject { put("id", id); put("state", "active") }, done = "Turned on") }, kind = KeyKind.Plain)
                }
            }
        }
    }
}

@Composable
private fun Facts(f: JsonObject, nav: Nav) {
    val c = Relay.colors
    val facts = f.objs("facts")
    Card("What Aide knows", aside = facts.size.toString()) {
        if (facts.isEmpty()) Quiet("Aide has learned nothing yet.")
        facts.forEachIndexed { i, fact ->
            val confirmed = fact.b("confirmed") == true
            val detail = listOfNotNull(fact.str("source"), if (fact.b("pinned") == true) "pinned" else null, if (!confirmed) "not confirmed: runs are not given it" else null).joinToString(" · ")
            FigureRow(fact.str("text"), detail, divider = i > 0, detailColor = if (confirmed) c.ink3 else c.waiting)
            Row(Modifier.fillMaxWidth().padding(bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                val id = fact.l("id") ?: 0
                Key("Forget", { nav.shell.act("aide.fact.forget", buildJsonObject { put("id", id) }, done = "Forgotten") }, kind = KeyKind.Quiet)
                if (!confirmed) Key("Confirm", { nav.shell.act("aide.fact.update", buildJsonObject { put("id", id); put("confirmed", true) }, done = "Confirmed") }, kind = KeyKind.Plain)
            }
        }
    }
    Column(Modifier.padding(horizontal = 4.dp)) {
        T("Aide's models, your servers, privacy and quiet hours are set on the PC.", style = Relay.type.caption, color = c.ink3)
    }
}
