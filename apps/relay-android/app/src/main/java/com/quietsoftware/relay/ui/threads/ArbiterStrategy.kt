package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Key
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToLong

/** A bar's width as a sentence says it (arbiter_pages.rs `GRANULARITIES`). */
private val BARS = mapOf(
    "ONE_MINUTE" to "1m", "FIVE_MINUTE" to "5m", "FIFTEEN_MINUTE" to "15m", "THIRTY_MINUTE" to "30m",
    "ONE_HOUR" to "1h", "TWO_HOUR" to "2h", "SIX_HOUR" to "6h", "ONE_DAY" to "1d",
)

/** What each way of deciding means, in a line (arbiter_pages.rs `MODES`). */
private val MODE_ABOUT = mapOf(
    "rules" to "The rule places orders within the limits. A thread's agent may read, backtest and propose; what it proposes waits for you.",
    "agent" to "A thread's agent may place orders for this strategy itself. Each still passes the limits, and only you can change them.",
    "ask" to "Each order, from the rule or an agent, waits on a card for your yes, and expires unanswered.",
)

/** A strategy's limits (arbiter_pages.rs `LIMITS`): key, caption, unit. Money is the strategy's quote currency. */
private val STRATEGY_LIMITS = listOf(
    Triple("max_order", "Most per order", "money"),
    Triple("max_position", "Most held", "money"),
    Triple("daily_loss", "Daily loss, then halt", "money"),
    Triple("orders_per_hour", "Orders per hour", "count"),
    Triple("cooldown_minutes", "Cooldown after a loss", "minutes"),
)

private val RANGE_DAY = DateTimeFormatter.ofPattern("d MMM", Locale.ENGLISH)

/** "3 Mar–8 May": a backtest's first and last bar, from Unix seconds (arbiter_pages.rs `day_of`). */
private fun rangeOf(stats: JsonObject): String? {
    val start = stats.l("start") ?: return null
    val end = stats.l("end") ?: return null
    fun day(unix: Long) = Instant.ofEpochSecond(unix).atZone(ZoneId.systemDefault()).format(RANGE_DAY)
    return "${day(start)}–${day(end)}"
}

/** A fall from a high: "−4.2%", and "0.0%" when there was none to speak of (arbiter_pages.rs `drawdown`). */
private fun drawdown(x: Double) = if (abs(x) < .05) "0.0%" else "−${String.format(Locale.ROOT, "%.1f", abs(x))}%"

/**
 * One strategy, read only (arbiter_pages.rs `strategy_page`): where it runs and what it is doing,
 * its rule in words, its price with its trades marked, its backtest beside its paper and live
 * records, who decides, its limits, and its own decisions and orders. A halted one restarts here;
 * starting, stopping and editing it are on the PC.
 */
@Composable
fun ArbiterStrategyScreen(id: Long, nav: Nav) {
    val live = rememberLive("arbiter.strategy.get", remember(id) { buildJsonObject { put("id", id) } })
    val r = (live.result as? JsonObject)?.o("row")
    DetailPage(
        nav,
        r?.s("name") ?: "Strategy",
        r?.let { listOfNotNull("Version ${it.l("version") ?: 1}", BARS[it.s("granularity")]?.let { b -> "$b bars" }, it.s("product")).joinToString(" · ") },
    ) {
        Reading(live, "This engine has no Arbiter yet. Update Relay's engine to use it.") { StrategyPage(it, nav) }
    }
}

@Composable
private fun StrategyPage(d: JsonObject, nav: Nav) {
    val c = Relay.colors
    val r = d.o("row") ?: return
    val id = r.l("id") ?: return
    val state = r.s("state")
    val product = r.str("product")
    val (base, quote) = pair(product)
    val currency = r.s("currency")?.takeIf { it.isNotBlank() } ?: quote
    val bars = BARS[r.s("granularity")] ?: "1h"
    val mode = r.s("mode") ?: "rules"

    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        when {
            state == "halted" -> StatePill("Halted", c.held)
            r.s("venue") == "live" -> StatePill("Live", c.live)
            else -> StatePill("Paper", c.waiting)
        }
        if (state == "stopped") StatePill("Stopped", null)
        T(MODES[mode] ?: MODES.getValue("rules"), Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
    }
    if (state == "halted") {
        Column(
            Modifier.fillMaxWidth().clip(Radii.card).background(c.held.copy(alpha = .12f)).border(1.dp, c.held.copy(alpha = .45f), Radii.card).padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            T("Halted: ${r.s("halt_reason") ?: "a limit was reached"}. It places no order until you restart it.", style = Relay.type.ui, color = c.ink)
            Key("Restart", { nav.shell.act("arbiter.restart", buildJsonObject { put("strategy_id", id) }, done = "Restarted. It runs again from its next bar.") })
        }
    } else if (state == "running") {
        r.s("doing")?.takeIf { it.isNotBlank() }?.let { T(it, style = Relay.type.ui, color = c.ink2) }
    }

    val pnl = (r.d("pnl_realized") ?: 0.0) + (r.d("pnl_open") ?: 0.0)
    val held = r.d("held_base") ?: 0.0
    StatGrid(listOf(
        { m -> Stat("Since start, after fees", coinSigned(pnl, currency), "today ${coinSigned(r.d("pnl_today") ?: 0.0, currency)}", m, figureColor = gainColor(pnl)) },
        { m ->
            if (held > 0) Stat("Held", "${trimmed(held, 8)} $base", "at a cost of ${coin(r.d("held_cost") ?: 0.0, currency)}", m)
            else Stat("Held", "Nothing", plural(r.l("trades") ?: 0L, "trade"), m)
        },
    ))

    // The rule, in sentences, and the move it needs to break even.
    Card("The rule") {
        Column(Modifier.padding(bottom = 6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            for (line in r.arr("sentences").mapNotNull { (it as? JsonPrimitive)?.content }) Row {
                T("•", Modifier.padding(end = 8.dp), Relay.type.ui, c.ink3)
                T(line, Modifier.weight(1f), Relay.type.ui, c.ink2)
            }
            d.d("break_even_pct")?.let {
                T("A round trip costs about ${String.format(Locale.ROOT, "%.2f", it)}% at your fee tier, so this rule needs a move larger than that to make money.", Modifier.padding(top = 4.dp), Relay.type.caption, c.ink3)
            }
        }
    }

    ChartCard(ChartSpec(
        ChartKind.Price, "$product, $bars bars, with this strategy's trades",
        price = remember(id, product, r.s("granularity")) {
            buildJsonObject {
                put("product", product)
                r.s("granularity")?.let { put("granularity", it) }
                put("bars", 300)
                put("strategy_id", id)
            }
        },
    ))

    Results(d, currency)

    Card("Who decides") {
        Column(Modifier.padding(bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            T(MODES[mode] ?: MODES.getValue("rules"), style = Relay.type.ui, color = c.ink, weight = FontWeight.Medium)
            T(MODE_ABOUT[mode] ?: MODE_ABOUT.getValue("rules"), style = Relay.type.caption, color = c.ink3)
        }
    }

    val limits = d.o("limits")
    Card("Limits") {
        STRATEGY_LIMITS.forEachIndexed { i, (key, caption, unit) ->
            val v = limits?.d(key)
            val shown = when {
                v == null -> "No limit"
                unit == "money" -> coin(v, currency)
                unit == "minutes" -> "${v.roundToLong()} min"
                else -> "${v.roundToLong()} an hour"
            }
            FigureRow(caption, "", shown, figureColor = if (v == null) c.ink3 else c.ink2, divider = i > 0)
        }
    }

    val decisions = d.objs("decisions")
    Card("Decisions") {
        if (decisions.isEmpty()) Quiet("Nothing decided yet.")
        decisions.take(12).forEachIndexed { i, x -> DecisionRow(x, divider = i > 0) }
    }
    val orders = d.objs("orders")
    Card("Orders") {
        if (orders.isEmpty()) Quiet("No orders yet.")
        orders.take(10).forEachIndexed { i, o -> OrderLine(o, divider = i > 0) }
    }
    T("Read only on this phone: starting, stopping, editing it and its limits are on the PC.", Modifier.fillMaxWidth(), Relay.type.caption.copy(textAlign = TextAlign.Center), c.ink3)
}

/**
 * The latest backtest of this version beside the paper and live records (arbiter_pages.rs
 * `results_card`): the range it was judged on, which it was not tuned on, then each venue's days,
 * trades and return, and buying and holding over the whole range.
 */
@Composable
private fun Results(d: JsonObject, currency: String) {
    val c = Relay.colors
    val report = d.o("backtest")
    Card("Backtest, paper, live") {
        val judged = report?.o("judged")
        if (judged != null) {
            val ret = judged.d("return_pct")
            FigureRow(
                "Backtest, judged on",
                listOfNotNull(
                    rangeOf(judged),
                    plural(judged.l("trades") ?: 0L, "trade"),
                    judged.d("won_pct")?.let { "${it.roundToLong()}% won" },
                    judged.d("max_drawdown_pct")?.let { "worst drawdown ${drawdown(it)}" },
                ).joinToString(" · "),
                ret?.let(::pct).orEmpty(), figureColor = ret?.let { gainColor(it) } ?: c.ink2, divider = false,
            )
        }
        for ((name, record) in listOf("Paper" to d.o("paper"), "Live" to d.o("live"))) {
            val trades = record?.l("trades") ?: 0L
            val days = record?.d("days") ?: 0.0
            // A return is said once there are trades to say it of.
            val ret = record?.d("return_pct")?.takeIf { trades > 0 }
            FigureRow(
                name,
                if (days > 0) listOfNotNull("${days.roundToLong()} days", plural(trades, "trade"), record?.d("won_pct")?.let { "${it.roundToLong()}% won" }, record?.d("pnl")?.let { coinSigned(it, currency) }).joinToString(" · ")
                else "not yet",
                ret?.let(::pct).orEmpty(), figureColor = ret?.let { gainColor(it) } ?: c.ink2, divider = judged != null || name == "Live",
            )
        }
        report?.o("whole")?.let { whole ->
            whole.d("buy_hold_pct")?.let { hold -> FigureRow("Buy and hold", rangeOf(whole).orEmpty(), pct(hold), figureColor = gainColor(hold)) }
        }
        if (report == null) {
            Quiet("No backtest of this version yet. A backtest fills each signal at the next bar's open, at your fee tier with slippage, and judges the rule on the last third of the range, which it was not tuned on. Run one from the strategy on the PC.")
        } else {
            for (w in report.arr("warnings").mapNotNull { (it as? JsonPrimitive)?.content }) T(w, Modifier.padding(vertical = 4.dp), Relay.type.caption, c.waiting)
            val tried = d.l("variants_tried") ?: 0L
            if (tried > 1) T("$tried versions of this rule have been backtested. The more versions tried, the likelier the best one only fits the past.", Modifier.padding(vertical = 4.dp), Relay.type.caption, c.waiting)
            // Profit after fees over the range.
            (report["equity"] as? JsonArray)?.takeIf { it.size > 1 }?.let {
                Sparkline(it)
                T("Profit after fees over the backtest's range.", Modifier.padding(bottom = 6.dp), Relay.type.caption, c.ink3)
            }
        }
    }
}
