package com.quietsoftware.relay.ui.threads

import android.os.SystemClock
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.Hub
import com.quietsoftware.relay.core.sync.Optimistic
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Pill
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

// Asking from Tally's, Arbiter's and Avex's pages (threads_view.rs `ask`, money_pages.rs
// `ask_key`): a question in the person's words starts a thread, which opens. The phone shows no
// thread beside a page to draft into, so every question asks; the words are the desktop's.

// ---- The questions ----

/** Plan's key (money_pages.rs `plan_head`). */
internal const val SUGGEST_BUDGETS = "Suggest monthly budgets from my last 3 months of spending"

/** The head of Investments (money_invest.rs `head`). */
internal const val INVESTMENTS_QUESTION = "How are my investments doing?"

/** An empty Invest view's Connect Wealthsimple (threads_view.rs `CONNECT`). */
internal const val CONNECT_WEALTHSIMPLE = "Help me connect my Wealthsimple accounts"

/** Avex's Volume by week and Lifts (avex_pages.rs `home`, `lifts_page`). */
internal const val VOLUME_QUESTION = "How has my training volume changed over the last few weeks?"
internal const val LIFTS_QUESTION = "Which of my lifts are progressing and which have stalled?"

/** The Budgets card's question, the one the month raises (money_pages.rs `home`). */
internal fun budgetsQuestion(status: String?): String = when (status) {
    "OVER_BUDGET" -> "I'm over my budget this month. Which categories put me there, and what can I still do?"
    "OVER_PACE" -> "I'm spending ahead of pace this month. Which budgets put me there, and how do I get back on pace?"
    else -> "Which of my budgets are ahead of pace this month, and why?"
}

/** Entries' question, about the period showing and its filters, "Dining · Visa" (money_pages.rs `transactions`). */
internal fun entriesQuestion(period: String, filtered: String): String =
    if (filtered.isEmpty()) "Where did my money go in $period?" else "What stands out in my $filtered entries for $period?"

/** "Can I cover Hydro-Québec on Sun 18 Oct?": a bill from `money.summary` (threads_view.rs `cover_question`). */
internal fun coverQuestion(bill: JsonObject): String {
    val day = when (bill.l("days_until") ?: 0L) {
        0L -> "today"
        1L -> "tomorrow"
        else -> "on ${humanDate(bill.s("next_date"))}"
    }
    return "Can I cover ${bill.s("name") ?: "this bill"} $day?"
}

/** "Tell me about this entry: Burger night, Dining, $14.98 on Thu 8 Oct" (money_pages.rs `ask_about`). */
internal fun entryQuestion(tx: JsonObject, money: Money): String {
    val amount = money.format(tx.minor("amount", money.digits) ?: 0L)
    val note = tx.str("note").trim()
    val parts = when (tx.s("type")) {
        "TRANSFER" -> listOf(note, "a transfer of $amount from ${tx.str("account")} to ${tx.str("to_account")}")
        "INCOME" -> listOf(note, tx.str("category"), "$amount in")
        else -> listOf(note, tx.str("category"), amount)
    }
    return "Tell me about this entry: ${parts.filter { it.isNotEmpty() }.joinToString(", ")} on ${dayWords(tx.s("date"))}"
}

/** An investment account's and a holding's rows (threads_view.rs `panel_invest`). */
internal fun accountQuestion(name: String) = "How is my $name account doing?"
internal fun holdingQuestion(symbol: String) = "How is $symbol doing?"

/** What the engine could not square in the investments (money_invest.rs, Needs a look). */
internal fun issuesQuestion(issues: List<String>) = "Tally says: ${issues.joinToString("; ")}. What does that mean, and how do I fix it?"

/** A workout's exercises (avex_pages.rs `workout_page`). */
internal fun planQuestion(title: String) = if (title == "Freestyle") "Plan my next workout from this one" else "Plan my next $title"

/** A lift's estimated max (avex_pages.rs `lift_page`): a stalled one asks how to move it again. */
internal fun liftQuestion(name: String, stalled: Boolean) =
    if (stalled) "How do I break this plateau on $name?" else "How is my $name progressing, and what should I change?"

// ---- Asking ----

/**
 * Starts a thread with [question] and opens it, with the newest thread's model and effort as a
 * new thread starts. Through the outbox, so it starts with the PC away too; its agent answers
 * once the PC has it.
 */
internal fun Nav.askInThread(question: String) {
    val last = shell.state.value.threads.firstOrNull()
    val payload = buildJsonObject {
        put("text", question)
        last?.model?.takeIf { it.isNotEmpty() }?.let { put("model", it) }
        last?.effort?.takeIf { it.isNotEmpty() }?.let { put("effort", it) }
    }
    shell.change("thread.create", payload, "New thread") { change ->
        when (change) {
            is Hub.Change.Queued -> thread(Optimistic.tempId(change.entry.id))
            is Hub.Change.Now -> (change.result as? JsonObject)?.l("id")?.let { thread(it.toString()) }
        }
    }
}

/** [askInThread], once: a second tap while the first makes its thread would make two. */
@Composable
internal fun rememberAsk(): (String) -> Unit {
    val nav = LocalNav.current
    var at by remember { mutableLongStateOf(0L) }
    return { question ->
        val now = SystemClock.uptimeMillis()
        if (now - at > 1_500) {
            at = now
            nav.askInThread(question)
        }
    }
}

/** The key that asks [question] in a new thread: Claude's mark and [label] (money_pages.rs `ask_key`). */
@Composable
internal fun AskKey(question: String, label: String = "Ask about this") {
    val ask = rememberAsk()
    Pill(label, glyph = "claude", onClick = { ask(question) })
}

/**
 * [Card] with the page's way to ask in its head, when [question] is given (money_pages.rs
 * `card_asking`).
 */
@Composable
internal fun AskCard(title: String, question: String?, aside: String? = null, content: @Composable ColumnScope.() -> Unit) {
    val c = Relay.colors
    Column(Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(start = 14.dp, end = 14.dp, top = 12.dp, bottom = 8.dp)) {
        Row(Modifier.fillMaxWidth().padding(bottom = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            T(title, Modifier.weight(1f), Relay.type.section.copy(fontWeight = FontWeight.SemiBold), c.ink, maxLines = 1)
            aside?.let { T(it, style = Relay.type.mono, color = c.ink3) }
            question?.let { AskKey(it) }
        }
        content()
    }
}

/**
 * A row that opens, on a tap, to the question it asks (the panel's rows, threads_view.rs
 * `row_key`): the words, and the key that asks them. On the PC a click drafts them into the
 * thread beside the panel; here they are read before they go. Without [question] the row takes
 * no tap.
 */
@Composable
internal fun Askable(question: String?, row: @Composable (onClick: (() -> Unit)?) -> Unit) {
    var open by rememberSaveable(question) { mutableStateOf(false) }
    row(if (question != null) ({ open = !open }) else null)
    if (question != null) AnimatedVisibility(open) {
        val c = Relay.colors
        Row(Modifier.fillMaxWidth().padding(start = 34.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            T("“$question”", Modifier.weight(1f), Relay.type.caption, c.ink2)
            AskKey(question, "Ask")
        }
    }
}
