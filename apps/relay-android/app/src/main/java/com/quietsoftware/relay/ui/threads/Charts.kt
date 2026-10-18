package com.quietsoftware.relay.ui.threads

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay as RelayData
import com.quietsoftware.relay.ui.LocalNav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Fence
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.flow.flowOf
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToLong

/**
 * Charts in a thread (docs/THREADS.md "Charts", threads_chart.rs): an agent's ```` ```chart ````
 * block keeps its question, never its numbers. The card reads them through the phone's cache, so
 * a chart seen once draws again with the PC away, and redraws when the ledger moves.
 */
internal enum class ChartKind { Bar, Line, Donut, Price }

internal data class ChartSpec(
    val kind: ChartKind,
    val title: String,
    /** A `money.series` payload. */
    val query: JsonObject? = null,
    /** A `gym.series` payload: Avex's history instead of the ledger. */
    val gym: JsonObject? = null,
    /** `{"labels", "series": [{"name", "values"}]}` in minor units, drawn as written. */
    val data: JsonObject? = null,
    /** An `arbiter.series` payload, for a price chart. */
    val price: JsonObject? = null,
)

/** Read a chart block; null when it is not one Relay can draw, so it shows as code. */
internal fun chartSpec(text: String): ChartSpec? {
    val v = runCatching { Wire.json.parseToJsonElement(text.trim()) as? JsonObject }.getOrNull() ?: return null
    val title = v.s("title").orEmpty()
    if (v.s("type") == "price") {
        val q = (v["arbiter"] as? JsonObject)?.takeIf { it.s("product") != null } ?: return null
        return ChartSpec(ChartKind.Price, title, price = q)
    }
    val kind = when (v.s("type") ?: "bar") {
        "bar", "column" -> ChartKind.Bar
        "line", "area" -> ChartKind.Line
        "donut", "pie" -> ChartKind.Donut
        else -> return null
    }
    val query = v["query"] as? JsonObject
    val gym = v["gym"] as? JsonObject
    val data = (v["data"] as? JsonObject)?.takeIf { it["labels"] is JsonArray && it["series"] is JsonArray }
    if (query == null && gym == null && data == null) return null
    return ChartSpec(kind, title, query, gym, data)
}

/** The fence a thread's Markdown draws charts with, and an agent's ask for a file to import. */
internal val ChartFence = Fence { lang, body ->
    if (lang == "import") importCard(body)
    else if (lang != "chart") null else chartSpec(body)?.let { spec -> { ChartCard(spec) } }
}

/** Series colours, newest first: Relay's lime, then blue, orange, violet, teal, gold. */
private val PALETTE = listOf(0xFFC6F24E, 0xFF6A9FD8, 0xFFE08A5F, 0xFFC27BC0, 0xFF4FA9A0, 0xFFD9A441).map { Color(it) }
private val BUY = Color(0xFF2EC469)
private val PLOT_H = 184.dp
private val AXIS_W = 56.dp

private fun seriesColor(index: Int, count: Int): Color = PALETTE[(count - 1 - min(index, count - 1)) % PALETTE.size]

/** The fields each question takes: the PC refuses any other. */
private val MONEY_KEYS = setOf("measure", "by", "periods", "period_offset", "category", "cumulative", "today")
private val GYM_KEYS = setOf("measure", "by", "periods", "lift")
private val PRICE_KEYS = setOf("product", "granularity", "bars", "strategy_id")

private fun JsonObject.only(keys: Set<String>) = JsonObject(filterKeys { it in keys })

/** How a chart's values read: money in minor units, or a training figure in tenths of its unit. */
private sealed interface Fmt {
    val digits: Int
    fun format(v: Long): String
    fun whole(v: Long): String

    class Cash(val m: Money) : Fmt {
        override val digits get() = m.digits
        override fun format(v: Long) = m.format(v)
        override fun whole(v: Long) = m.whole(v)
    }

    class Unit(val unit: String) : Fmt {
        override val digits get() = 1
        override fun format(v: Long) = words(v, false)
        override fun whole(v: Long) = words(v, true)
        private fun words(tenths: Long, whole: Boolean): String {
            val n = if (whole || tenths % 10 == 0L) group(Math.round(tenths / 10.0)) else String.format(java.util.Locale.ROOT, "%.1f", tenths / 10.0)
            return when (unit) {
                "" -> n
                "sessions", "sets", "minutes" -> if (whole) n else "$n $unit"
                else -> "$n $unit"
            }
        }
    }

    /** Prices, in hundredths of their quote currency. */
    class Price(val quote: String) : Fmt {
        override val digits get() = 2
        override fun format(v: Long) = if (Money.known(quote)) Money(quote, 2).format(v) else "${trimmed(v / 100.0, 2)} $quote"
        override fun whole(v: Long) = if (Money.known(quote)) Money(quote, 2).whole(v) else "${group(Math.round(v / 100.0))} $quote"
    }
}

private class Numbers(
    val labels: List<String>,
    val colors: List<Long?>,
    val series: List<Pair<String, List<Long>>>,
    /** How many labels each series has reached (a period still running), else all of them. */
    val known: List<Int>,
    val fmt: Fmt,
    /** The value axis starts at zero; a lift's line starts near its lowest value instead. */
    val fromZero: Boolean = true,
    val cumulative: Boolean = false,
    /** Buys and sells on a price line: (label index, price, buy). */
    val marks: List<Triple<Int, Long, Boolean>> = emptyList(),
)

private fun readMoney(v: JsonObject, cumulative: Boolean): Numbers {
    val labels = v.arr("labels").map { (it as? JsonPrimitive)?.content.orEmpty() }
    val colors = v.arr("label_colors").let { c -> labels.indices.map { i -> (c.getOrNull(i) as? JsonPrimitive)?.longOrNull } }
    val lines = v.arr("series").mapNotNull { it as? JsonObject }
    val series = lines.map { s -> s.str("name") to labels.indices.map { i -> (s.arr("values").getOrNull(i) as? JsonPrimitive)?.longOrNull ?: 0L } }
    val known = lines.map { s -> (s["known"] as? JsonPrimitive)?.longOrNull?.toInt()?.coerceAtMost(labels.size) ?: labels.size }
    return Numbers(labels, colors, series, known, Fmt.Cash(Money.of(v)), cumulative = cumulative)
}

/** A `gym.series` reading in tenths of its unit. A lift's best has nothing to read in a week it was not done: such points are left out. */
private fun readGym(v: JsonObject, kind: ChartKind): Numbers {
    val labels = v.arr("labels").map { (it as? JsonPrimitive)?.content.orEmpty() }
    val lines = v.arr("series").mapNotNull { it as? JsonObject }.map { s -> s.str("name") to labels.indices.map { i -> (s.arr("values").getOrNull(i) as? JsonPrimitive)?.doubleOrNull } }
    val keep = labels.indices.filter { i -> lines.any { it.second[i] != null } }
    val series = lines.map { (name, values) -> name to keep.map { i -> ((values[i] ?: 0.0) * 10).roundToLong() } }
    val measure = v.str("measure")
    return Numbers(keep.map { labels[it] }, keep.map { null }, series, List(series.size) { keep.size }, Fmt.Unit(v.str("unit")), fromZero = !(kind == ChartKind.Line && measure in setOf("e1rm", "top_weight")))
}

private fun readWritten(d: JsonObject): Numbers {
    val labels = d.arr("labels").map { (it as? JsonPrimitive)?.content.orEmpty() }
    val series = d.arr("series").mapNotNull { it as? JsonObject }.map { s -> s.str("name") to labels.indices.map { i -> (s.arr("values").getOrNull(i) as? JsonPrimitive)?.let { p -> p.longOrNull ?: p.doubleOrNull?.roundToLong() } ?: 0L } }
    return Numbers(labels, labels.map { null }, series, List(series.size) { labels.size }, Fmt.Cash(Money.of(d)))
}

/** Closes in hundredths, buys and sells at their bar. */
private fun readPrice(v: JsonObject): Numbers {
    val candles = v.objs("candles")
    val quote = v.str("product").substringAfter('-', "")
    val closes = candles.map { ((it.d("close") ?: 0.0) * 100).roundToLong() }
    val starts = candles.map { it.l("start") ?: 0L }
    val labels = starts.map { java.time.Instant.ofEpochSecond(it).atZone(java.time.ZoneId.systemDefault()).toLocalDate().let { d -> "${d.dayOfMonth} ${d.month.name.take(3).lowercase().replaceFirstChar { c -> c.uppercase() }}" } }
    val marks = v.objs("markers").mapNotNull { m ->
        val at = m.l("at") ?: return@mapNotNull null
        val i = starts.indexOfLast { it <= at }.takeIf { it >= 0 } ?: return@mapNotNull null
        Triple(i, ((m.d("price") ?: 0.0) * 100).roundToLong(), m.s("side") == "buy")
    }
    return Numbers(labels, labels.map { null }, listOf(v.str("product") to closes), listOf(labels.size), Fmt.Price(quote), fromZero = false, marks = marks)
}

private fun JsonObject.l(key: String): Long? = (this[key] as? JsonPrimitive)?.longOrNull

/** Round axis steps for values up to [max] (relay_client::thread_view::ticks): the step, and how many reach it. */
private fun ticks(max: Long, digits: Int): Pair<Long, Int> {
    val unit = 10.0.pow(digits)
    val m = max(max, 1L)
    val rough = m / 3.0 / unit
    val power = 10.0.pow(floor(log10(max(rough, 1e-9))))
    val whole = { s: Double -> abs(s * unit - Math.round(s * unit)) < 1e-6 }
    val step = listOf(1.0, 2.0, 2.5, 5.0, 10.0).map { it * power }.firstOrNull { it >= rough && whole(it) } ?: (10.0 * power)
    val minor = max(Math.round(step * unit), 1L)
    return minor to max((m + minor - 1) / minor, 1L).toInt()
}

@Composable
internal fun ChartCard(spec: ChartSpec) {
    val nav = LocalNav.current
    val c = Relay.colors
    val (op, payload) = when {
        spec.price != null -> "arbiter.series" to spec.price.only(PRICE_KEYS)
        spec.gym != null -> "gym.series" to spec.gym.only(GYM_KEYS)
        spec.query != null -> "money.series" to spec.query.only(MONEY_KEYS)
        else -> null to null
    }
    val flow = remember(op, payload) { if (op != null && payload != null) nav.relay.live(op, payload) else flowOf(RelayData.Live()) }
    val live by flow.collectAsStateWithLifecycle(RelayData.Live())
    val data = remember(live.result, spec) {
        val r = live.result as? JsonObject
        when {
            spec.data != null -> readWritten(spec.data)
            r == null -> null
            spec.kind == ChartKind.Price -> readPrice(r)
            spec.gym != null -> readGym(r, spec.kind)
            else -> readMoney(r, spec.query?.get("cumulative")?.let { (it as? JsonPrimitive)?.content == "true" } == true)
        }
    }
    var picked by remember(data) { mutableStateOf<Int?>(null) }
    Column(
        Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        T(spec.title.ifBlank { if (spec.kind == ChartKind.Price) spec.price?.s("product") ?: "Price" else "Chart" }, style = Relay.type.ui.copy(fontSize = 13.sp, lineHeight = 18.sp), color = c.ink, weight = FontWeight.SemiBold)
        when {
            data == null && live.error != null -> T("This chart could not be read: ${said(live.error!!, "This engine cannot draw it yet.")}", style = Relay.type.caption, color = c.heldText)
            data == null -> T(
                when {
                    spec.gym != null -> "Reading Avex…"
                    spec.price != null -> "Reading prices…"
                    else -> "Reading the ledger…"
                },
                style = Relay.type.caption, color = c.ink3,
            )
            data.labels.isEmpty() || data.series.all { (_, v) -> v.all { it == 0L } } -> T("Nothing to draw yet: no entries match.", style = Relay.type.caption, color = c.ink3)
            spec.kind == ChartKind.Donut -> Donut(data)
            else -> {
                Legend(data, spec.kind)
                Plot(data, spec.kind, picked) { picked = if (picked == it) null else it }
                picked?.takeIf { it < data.labels.size }?.let { j -> Readout(data, j) }
            }
        }
        Foot(spec, live, data)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Legend(data: Numbers, kind: ChartKind) {
    val count = data.series.size
    if (count < 2 && kind != ChartKind.Line) return
    FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        data.series.forEachIndexed { i, (name, _) ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Dot(if (kind == ChartKind.Price) PALETTE[1] else seriesColor(i, count), 8.dp)
                T(name.ifBlank { "Total" }, style = Relay.type.caption, color = Relay.colors.ink2, maxLines = 1)
            }
        }
    }
}

/** What a tapped group holds, newest series first. */
@Composable
private fun Readout(data: Numbers, j: Int) {
    val c = Relay.colors
    val parts = data.series.asReversed().filter { (_, v) -> j < v.size }.joinToString("  ·  ") { (name, v) ->
        if (name.isBlank() || data.series.size == 1) data.fmt.format(v[j]) else "$name ${data.fmt.format(v[j])}"
    }
    Row(Modifier.fillMaxWidth().clip(Radii.key).background(c.wash).padding(horizontal = 10.dp, vertical = 6.dp)) {
        T(data.labels[j], style = Relay.type.caption, color = c.ink, weight = FontWeight.Medium, maxLines = 1)
        T("  ·  $parts", Modifier.weight(1f), Relay.type.mono.copy(fontSize = 12.sp), c.ink2, maxLines = 2)
    }
}

/** Bars or lines over a value axis, the group labels under them; a tap picks a group. */
@Composable
private fun Plot(data: Numbers, kind: ChartKind, picked: Int?, onPick: (Int) -> Unit) {
    val c = Relay.colors
    val pick by rememberUpdatedState(onPick)
    val measurer = rememberTextMeasurer()
    val axisStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 10.5.sp, color = c.ink3)
    val all = data.series.flatMap { it.second } + data.marks.map { it.second }
    val lo = min(all.minOrNull() ?: 0L, 0L)
    val hi = max(all.maxOrNull() ?: 0L, 0L)
    val least = all.minOrNull() ?: 0L
    val base = if (data.fromZero || least <= 0) 0L else {
        val (step, _) = ticks(max(hi - least, 1), data.fmt.digits)
        max(least / step - 1, 0L) * step
    }
    val (step, up) = ticks(max(hi - base, -lo), data.fmt.digits)
    val down = if (lo < 0) ((-lo + step - 1) / step).toInt() else 0
    val top = base + step * up
    val bottom = base - step * down
    val span = max(top - bottom, 1L).toDouble()
    val groups = data.labels.size
    val count = data.series.size
    val tickLabels = remember(data) { (-down..up).map { k -> base + k * step }.map { it to data.fmt.whole(it) } }
    Canvas(
        Modifier.fillMaxWidth().height(PLOT_H).pointerInput(data) {
            detectTapGestures { pos ->
                val left = AXIS_W.toPx()
                val gw = (size.width - left - 4.dp.toPx()) / groups
                val j = floor((pos.x - left) / gw).toInt()
                if (j in 0 until groups) pick(j)
            }
        },
    ) {
        val padT = 10.dp.toPx()
        val padB = 8.dp.toPx()
        val h = size.height
        val left = AXIS_W.toPx()
        val right = size.width - 4.dp.toPx()
        val gw = (right - left) / groups
        fun y(v: Long) = (padT + (top - v) / span * (h - padT - padB)).toFloat()
        picked?.let { j -> drawRoundRect(c.wash, Offset(left + j * gw, 0f), Size(gw, h), CornerRadius(4.dp.toPx())) }
        for ((value, text) in tickLabels) {
            val yy = y(value)
            drawLine(if (value == base) c.lineEmphasis else c.edge, Offset(left, yy), Offset(right, yy), strokeWidth = 1f)
            val layout = measurer.measure(text, axisStyle, maxLines = 1)
            drawText(layout, topLeft = Offset(0f, (yy - layout.size.height / 2f).coerceIn(0f, h - layout.size.height)))
        }
        val zero = y(base)
        when (kind) {
            ChartKind.Line, ChartKind.Price -> {
                data.series.forEachIndexed { i, (_, values) ->
                    val shown = values.take(max(1, min(data.known.getOrElse(i) { values.size }, values.size)))
                    val color = if (kind == ChartKind.Price) PALETTE[1] else seriesColor(i, count)
                    val path = Path()
                    shown.forEachIndexed { j, v ->
                        val x = left + (j + .5f) * gw
                        if (j == 0) path.moveTo(x, y(v)) else path.lineTo(x, y(v))
                    }
                    drawPath(path, color, style = Stroke(if (kind == ChartKind.Price) 1.75.dp.toPx() else 2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
                    if (groups <= 16 && kind == ChartKind.Line) shown.forEachIndexed { j, v -> drawCircle(color, 3.dp.toPx(), Offset(left + (j + .5f) * gw, y(v))) }
                }
                for ((j, price, buy) in data.marks) {
                    val o = Offset(left + (j + .5f) * gw, y(price))
                    if (buy) drawCircle(BUY, 4.dp.toPx(), o)
                    else {
                        drawCircle(c.slab, 4.dp.toPx(), o)
                        drawCircle(c.ink, 4.dp.toPx(), o, style = Stroke(1.5.dp.toPx()))
                    }
                }
            }
            else -> {
                val bw = (gw * .72f / count - 3.dp.toPx()).coerceIn(3.dp.toPx(), 26.dp.toPx())
                val inner = bw * count + 3.dp.toPx() * (count - 1)
                for (j in 0 until groups) {
                    val x0 = left + j * gw + (gw - inner) / 2
                    data.series.forEachIndexed { i, (_, values) ->
                        val v = values.getOrElse(j) { 0L }
                        if (v == 0L) return@forEachIndexed
                        val x = x0 + i * (bw + 3.dp.toPx())
                        val y1 = min(y(v), zero)
                        val y2 = max(y(v), zero)
                        val r = min(min(bw / 2, 3.dp.toPx()), y2 - y1)
                        val p = Path()
                        if (v > 0) {
                            p.moveTo(x, y2); p.lineTo(x, y1 + r)
                            p.quadraticTo(x, y1, x + r, y1); p.lineTo(x + bw - r, y1)
                            p.quadraticTo(x + bw, y1, x + bw, y1 + r); p.lineTo(x + bw, y2)
                        } else {
                            p.moveTo(x, y1); p.lineTo(x, y2 - r)
                            p.quadraticTo(x, y2, x + r, y2); p.lineTo(x + bw - r, y2)
                            p.quadraticTo(x + bw, y2, x + bw, y2 - r); p.lineTo(x + bw, y1)
                        }
                        p.close()
                        drawPath(p, seriesColor(i, count))
                    }
                }
            }
        }
    }
    // The group labels, thinned to about six on a phone.
    val every = max(1, ceil(groups / 6.0).toInt())
    Row(Modifier.fillMaxWidth().padding(start = AXIS_W, end = 4.dp)) {
        data.labels.forEachIndexed { j, name ->
            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                if (j % every == 0) T(name, style = Relay.type.mono.copy(fontSize = 11.sp, textAlign = TextAlign.Center), color = c.ink2, maxLines = 1)
            }
        }
    }
}

/** Where the money went: a ring of the newest series, and a list beside it (under it on a narrow phone). */
@Composable
private fun Donut(data: Numbers) {
    val c = Relay.colors
    val (name, values) = data.series.last()
    val parts = data.labels.indices.filter { values.getOrElse(it) { 0L } > 0 }.map { i ->
        Triple(data.labels[i], values[i], data.colors[i]?.let { hue(it) } ?: PALETTE[i % PALETTE.size])
    }
    val total = parts.sumOf { it.second }
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val narrow = maxWidth < 330.dp
        val ring = @Composable {
            Box(Modifier.size(140.dp), contentAlignment = Alignment.Center) {
                Canvas(Modifier.size(140.dp)) {
                    val stroke = 20.dp.toPx()
                    val radius = size.minDimension / 2 - stroke / 2 - 2.dp.toPx()
                    var at = -90f
                    val gap = if (parts.size > 1) .7f else 0f
                    for ((_, v, color) in parts) {
                        val sweep = v.toFloat() / max(total, 1L) * 360f
                        drawArc(color, at + gap, max(sweep - 2 * gap, .5f), false, Offset(center.x - radius, center.y - radius), Size(radius * 2, radius * 2), style = Stroke(stroke))
                        at += sweep
                    }
                }
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    T(data.fmt.whole(total), style = Relay.type.title.copy(fontSize = 17.sp, fontFeatureSettings = "tnum"), color = c.ink)
                    if (name.isNotBlank()) T(name, style = Relay.type.caption.copy(fontSize = 11.5.sp), color = c.ink3, maxLines = 1)
                }
            }
        }
        val list = @Composable { m: Modifier ->
            Column(m, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                for ((label, v, color) in parts) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Dot(color, 8.dp)
                        T(label, Modifier.weight(1f), Relay.type.caption, c.ink2, maxLines = 1)
                        T(data.fmt.whole(v), style = Relay.type.mono.copy(fontSize = 12.sp), color = c.ink)
                        T("${Math.round(v * 100.0 / max(total, 1L))}%", Modifier.padding(start = 2.dp), Relay.type.mono.copy(fontSize = 11.5.sp, textAlign = TextAlign.End), c.ink3)
                    }
                }
            }
        }
        if (narrow) Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) { ring(); list(Modifier.fillMaxWidth()) }
        else Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(20.dp)) { ring(); list(Modifier.weight(1f)) }
    }
}

/** Where the numbers come from and how fresh they are, and the newest series' total. */
@Composable
private fun Foot(spec: ChartSpec, live: RelayData.Live, data: Numbers?) {
    val c = Relay.colors
    val asked = spec.data == null
    val stale = asked && live.result != null && !live.fresh
    val note = when {
        !asked -> "Numbers as written in the reply"
        stale -> "Saved on this phone · ${ago(live.at)}"
        spec.gym != null -> "From Avex's last export"
        spec.price != null -> "Live from Arbiter"
        else -> "Live from Tally, redraws when the ledger changes"
    }
    val total = data?.takeIf { spec.kind != ChartKind.Donut && it.series.isNotEmpty() }?.let { d ->
        val i = d.series.lastIndex
        val values = d.series[i].second.take(d.known.getOrElse(i) { d.labels.size })
        when {
            values.isEmpty() -> null
            spec.kind == ChartKind.Price -> d.fmt.format(values.last())
            d.fmt is Fmt.Cash && d.cumulative -> d.fmt.whole(values.last())
            d.fmt is Fmt.Cash -> d.fmt.whole(values.sum())
            else -> null
        }
    }
    Column {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (asked) Dot(if (stale) c.waiting else c.live, 6.dp)
            T(note, Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 2)
            total?.let { T(it, style = Relay.type.title.copy(fontSize = 18.sp, lineHeight = 22.sp, fontFeatureSettings = "tnum"), color = c.ink, maxLines = 1) }
        }
    }
}
