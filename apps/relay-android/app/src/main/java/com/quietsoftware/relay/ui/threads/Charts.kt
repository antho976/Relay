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
import androidx.compose.foundation.layout.wrapContentWidth
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
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
import com.quietsoftware.relay.core.wire.b
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
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull
import java.math.RoundingMode
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
    /**
     * The reply's own numbers, for what the ledger does not hold (a plan, a projection):
     * `{"labels", "series": [{"name", "values", "projected"?}], "unit"?}`, drawn as written
     * ([readWritten]).
     */
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
internal sealed interface Fmt {
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
                "%" -> "$n%"
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

/**
 * One line, or one set of bars: its name, a value for each label (null where it has none, which
 * draws nothing, not a zero), and whether it is a projection, drawn dashed or lighter.
 */
internal data class Series(val name: String, val values: List<Long?>, val projected: Boolean = false)

internal class Numbers(
    val labels: List<String>,
    val colors: List<Long?>,
    val series: List<Series>,
    /** How many labels each series has reached (a period still running), else all of them. */
    val known: List<Int>,
    val fmt: Fmt,
    /** The value axis starts at zero; a lift's line starts near its lowest value instead. */
    val fromZero: Boolean = true,
    val cumulative: Boolean = false,
    /** Buys and sells on a price line: (label index, price, buy). */
    val marks: List<Triple<Int, Long, Boolean>> = emptyList(),
    /** What a group is called in full, for the readout: a price bar's day and time under its hour. */
    val names: List<String> = labels,
    /** The labels are categories (`money.series` by category): bars read as rows. */
    val categories: Boolean = false,
)

private fun readMoney(v: JsonObject, cumulative: Boolean, categories: Boolean): Numbers {
    val labels = v.arr("labels").map { (it as? JsonPrimitive)?.content.orEmpty() }
    val colors = v.arr("label_colors").let { c -> labels.indices.map { i -> (c.getOrNull(i) as? JsonPrimitive)?.longOrNull } }
    val lines = v.arr("series").mapNotNull { it as? JsonObject }
    val series = lines.map { s -> Series(s.str("name"), labels.indices.map { i -> (s.arr("values").getOrNull(i) as? JsonPrimitive)?.longOrNull ?: 0L }) }
    val known = lines.map { s -> (s["known"] as? JsonPrimitive)?.longOrNull?.toInt()?.coerceAtMost(labels.size) ?: labels.size }
    return Numbers(labels, colors, series, known, Fmt.Cash(Money.of(v)), cumulative = cumulative, categories = categories)
}

/** A `gym.series` reading in tenths of its unit. A lift's best has nothing to read in a week it was not done: such points are left out. */
private fun readGym(v: JsonObject, kind: ChartKind): Numbers {
    val labels = v.arr("labels").map { (it as? JsonPrimitive)?.content.orEmpty() }
    val lines = v.arr("series").mapNotNull { it as? JsonObject }.map { s -> s.str("name") to labels.indices.map { i -> (s.arr("values").getOrNull(i) as? JsonPrimitive)?.doubleOrNull } }
    val keep = labels.indices.filter { i -> lines.any { it.second[i] != null } }
    val series = lines.map { (name, values) -> Series(name, keep.map { i -> values[i]?.let { (it * 10).roundToLong() } }) }
    val measure = v.str("measure")
    return Numbers(keep.map { labels[it] }, keep.map { null }, series, List(series.size) { keep.size }, Fmt.Unit(v.str("unit")), fromZero = !(kind == ChartKind.Line && measure in setOf("e1rm", "top_weight")))
}

/**
 * A chart's own numbers (`data`), for what the ledger does not hold, such as a plan or what is to
 * come. Without `unit` they are amounts in [money]'s currency as plain decimals (1234.56, or the
 * string "1234.56"), kept in its minor units; with one ("kg", "%", "km", "" for a count) they are
 * figures in that unit, kept in tenths as Avex's are. A point missing or not a number is left
 * out, not drawn as 0. A series marked `projected` draws dashed or lighter.
 */
internal fun readWritten(d: JsonObject, money: Money): Numbers {
    val labels = d.arr("labels").map { (it as? JsonPrimitive)?.content.orEmpty() }
    val unit = (d["unit"] as? JsonPrimitive)?.takeIf { it.isString }?.content
    val fmt = if (unit != null) Fmt.Unit(unit) else Fmt.Cash(money)
    val series = d.arr("series").mapNotNull { it as? JsonObject }.map { s ->
        val values = s.arr("values")
        Series(s.str("name"), labels.indices.map { i -> decimal(values.getOrNull(i), fmt.digits) }, projected = s.b("projected") == true)
    }
    return Numbers(labels, labels.map { null }, series, List(series.size) { labels.size }, fmt)
}

/** A decimal number, or a string holding one, as a whole count of 10^-[digits]; null for anything else. */
internal fun decimal(e: JsonElement?, digits: Int): Long? {
    val p = e as? JsonPrimitive ?: return null
    if (p is JsonNull) return null
    return runCatching { p.content.trim().toBigDecimal().movePointRight(digits).setScale(0, RoundingMode.HALF_EVEN).longValueExact() }.getOrNull()
}

/** Closes in hundredths, buys and sells at their bar. */
private fun readPrice(v: JsonObject): Numbers {
    val candles = v.objs("candles")
    val quote = v.str("product").substringAfter('-', "")
    val closes = candles.map { ((it.d("close") ?: 0.0) * 100).roundToLong() }
    val starts = candles.map { it.l("start") ?: 0L }
    val times = starts.map { java.time.Instant.ofEpochSecond(it).atZone(java.time.ZoneId.systemDefault()) }
    // Bars within three days are told apart by the hour, longer ones by the day (threads_chart.rs).
    val hourly = (starts.lastOrNull() ?: 0L) - (starts.firstOrNull() ?: 0L) <= 3 * 86_400L
    val labels = times.map { it.format(if (hourly) HOUR else DAY) }
    val marks = v.objs("markers").mapNotNull { m ->
        val at = m.l("at") ?: return@mapNotNull null
        val i = starts.indexOfLast { it <= at }.takeIf { it >= 0 } ?: return@mapNotNull null
        Triple(i, ((m.d("price") ?: 0.0) * 100).roundToLong(), m.s("side") == "buy")
    }
    return Numbers(labels, labels.map { null }, listOf(Series(v.str("product"), closes)), listOf(labels.size), Fmt.Price(quote), fromZero = false, marks = marks, names = times.map { it.format(WHEN) })
}

private val HOUR = java.time.format.DateTimeFormatter.ofPattern("HH:mm", java.util.Locale.ENGLISH)
private val DAY = java.time.format.DateTimeFormatter.ofPattern("d MMM", java.util.Locale.ENGLISH)
private val WHEN = java.time.format.DateTimeFormatter.ofPattern("EEE d MMM HH:mm", java.util.Locale.ENGLISH)

private val MONTH_NAMES = listOf("January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December")

/** "October 2026" as "Oct 2026": a month's name cut to three letters, for an axis with little room. */
private fun shortMonths(label: String): String = MONTH_NAMES.fold(label) { s, m -> s.replace(m, m.take(3)) }

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
    // The reply's own amounts are in the ledger's currency, as the phone last read it, unless the chart names one.
    val ledger by remember(spec.data != null) { if (spec.data != null) nav.relay.cached("money.summary") else flowOf(null) }.collectAsStateWithLifecycle(null)
    val cash = spec.data?.takeIf { it.s("currency") != null }?.let { Money.of(it) } ?: Money.of(ledger?.result as? JsonObject)
    val data = remember(live.result, spec, cash.currency, cash.digits) {
        val r = live.result as? JsonObject
        when {
            spec.data != null -> readWritten(spec.data, cash)
            r == null -> null
            spec.kind == ChartKind.Price -> readPrice(r)
            spec.gym != null -> readGym(r, spec.kind)
            // By category unless it asks otherwise, as `money.series` reads it.
            else -> readMoney(r, spec.query?.get("cumulative")?.let { (it as? JsonPrimitive)?.content == "true" } == true, (spec.query?.s("by") ?: "category") == "category")
        }
    }
    var picked by remember(data) { mutableStateOf<Int?>(null) }
    Column(
        Modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        T(spec.title.ifBlank { if (spec.kind == ChartKind.Price) spec.price?.s("product") ?: "Price" else "Chart" }, style = Relay.type.ui.copy(fontSize = 13.sp, lineHeight = 18.sp), color = c.ink, weight = FontWeight.SemiBold)
        when {
            // The PC away is a state, not a failure: said quietly, where red read as an error.
            data == null && live.error?.code == "link.down" -> T("Not on this phone yet: it draws once the PC is in reach.", style = Relay.type.caption, color = c.ink3)
            data == null && live.error != null -> T("This chart could not be read: ${said(live.error!!, "This engine cannot draw it yet.")}", style = Relay.type.caption, color = c.heldText)
            data == null -> T(
                when {
                    spec.gym != null -> "Reading Avex…"
                    spec.price != null -> "Reading prices…"
                    else -> "Reading the ledger…"
                },
                style = Relay.type.caption, color = c.ink3,
            )
            data.labels.isEmpty() || data.series.all { s -> s.values.all { it == null || it == 0L } } -> T(
                when {
                    spec.data != null -> "Nothing to draw: this chart came with no numbers."
                    // Of the ledger's and Avex's, only Avex's count in a unit: workouts, not entries.
                    data.fmt is Fmt.Unit -> "Nothing to draw yet: no workouts match."
                    else -> "Nothing to draw yet: no entries match."
                },
                style = Relay.type.caption, color = c.ink3,
            )
            spec.kind == ChartKind.Donut -> Donut(data)
            spec.kind == ChartKind.Bar && data.categories && data.series.all { s -> s.values.all { it == null || it >= 0L } } -> {
                Legend(data, spec.kind)
                CategoryRows(data)
            }
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
    // One series needs no key, unless it is a projection, which says so.
    if (count < 2 && kind != ChartKind.Line && data.series.none { it.projected }) return
    FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        data.series.forEachIndexed { i, s ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                val color = if (kind == ChartKind.Price) PALETTE[1] else seriesColor(i, count)
                Dot(if (s.projected) color.copy(alpha = PROJECTED_ALPHA) else color, 8.dp)
                T(s.title(), style = Relay.type.caption, color = Relay.colors.ink2, maxLines = 1)
            }
        }
    }
}

/** A series as its key names it: "Savings", "Total", "Savings (projected)". */
internal fun Series.title(): String = name.ifBlank { "Total" }.let { if (projected) "$it (projected)" else it }

/** How strong a projected series' bars and key are against the drawn ones. */
private const val PROJECTED_ALPHA = .45f

/** What group [j] holds, newest series first: "Groceries $412.30 · Dining $96.00"; a series with no value there is left out. */
private fun Numbers.parts(j: Int, between: String = "  ·  "): String =
    series.asReversed().mapNotNull { s -> s.values.getOrNull(j)?.let { v -> s to v } }.joinToString(between) { (s, v) ->
        if (series.size == 1 || (s.name.isBlank() && !s.projected)) fmt.format(v) else "${s.title()} ${fmt.format(v)}"
    }

/** What a tapped group holds, under its name in full. */
@Composable
private fun Readout(data: Numbers, j: Int) {
    val c = Relay.colors
    Row(Modifier.fillMaxWidth().clip(Radii.key).background(c.wash).padding(horizontal = 10.dp, vertical = 6.dp)) {
        T(data.names[j], style = Relay.type.caption, color = c.ink, weight = FontWeight.Medium, maxLines = 1)
        T("  ·  ${data.parts(j)}", Modifier.weight(1f), Relay.type.mono.copy(fontSize = 12.sp), c.ink2, maxLines = 2)
    }
}

/** Bars or lines over a value axis, the group labels under them; a tap picks a group. */
@Composable
private fun Plot(data: Numbers, kind: ChartKind, picked: Int?, onPick: (Int) -> Unit) {
    val c = Relay.colors
    val pick by rememberUpdatedState(onPick)
    val measurer = rememberTextMeasurer()
    val axisStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 10.5.sp, color = c.ink3)
    val all = data.series.flatMap { it.values.filterNotNull() } + data.marks.map { it.second }
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
    // A screen reader hears the newest group the chart has reached, as a tap on it reads.
    val spoken = remember(data) {
        val j = ((data.known.maxOrNull() ?: groups).coerceIn(1, groups)) - 1
        "Latest: ${data.names[j]}, ${data.parts(j, ", ")}"
    }
    Canvas(
        Modifier.fillMaxWidth().height(PLOT_H).pointerInput(data) {
            detectTapGestures { pos ->
                val left = AXIS_W.toPx()
                val gw = (size.width - left - 4.dp.toPx()) / groups
                val j = floor((pos.x - left) / gw).toInt()
                if (j in 0 until groups) pick(j)
            }
        }.semantics { contentDescription = spoken },
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
                data.series.forEachIndexed { i, s ->
                    val values = s.values
                    val shown = values.take(max(1, min(data.known.getOrElse(i) { values.size }, values.size)))
                    val color = if (kind == ChartKind.Price) PALETTE[1] else seriesColor(i, count)
                    // A point with no value is stepped over: the line joins the ones either side.
                    val path = Path()
                    var started = false
                    shown.forEachIndexed { j, v ->
                        if (v == null) return@forEachIndexed
                        val x = left + (j + .5f) * gw
                        if (started) path.lineTo(x, y(v)) else path.moveTo(x, y(v))
                        started = true
                    }
                    // A projection is dashed, its points open.
                    val dashes = if (s.projected) PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 5.dp.toPx())) else null
                    drawPath(path, color, style = Stroke(if (kind == ChartKind.Price) 1.75.dp.toPx() else 2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round, pathEffect = dashes))
                    if (groups <= 16 && kind == ChartKind.Line) shown.forEachIndexed { j, v ->
                        if (v == null) return@forEachIndexed
                        val o = Offset(left + (j + .5f) * gw, y(v))
                        if (s.projected) {
                            drawCircle(c.slab, 3.dp.toPx(), o)
                            drawCircle(color, 3.dp.toPx(), o, style = Stroke(1.5.dp.toPx()))
                        } else {
                            drawCircle(color, 3.dp.toPx(), o)
                        }
                    }
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
                    data.series.forEachIndexed { i, s ->
                        val v = s.values.getOrNull(j) ?: return@forEachIndexed
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
                        // A projection's bars are lighter than what was.
                        drawPath(p, seriesColor(i, count).let { if (s.projected) it.copy(alpha = PROJECTED_ALPHA) else it })
                    }
                }
            }
        }
    }
    // The group labels, thinned to about five on a phone, each free to run past its slot, where a
    // slot's width cut them to "Octo…"; on a narrow plot a month's name is three letters. The
    // readout keeps the label in full.
    val every = max(1, ceil(groups / 5.0).toInt())
    BoxWithConstraints(Modifier.fillMaxWidth().padding(start = AXIS_W, end = 4.dp)) {
        val narrow = maxWidth < 480.dp
        Row(Modifier.fillMaxWidth()) {
            data.labels.forEachIndexed { j, name ->
                Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                    if (j % every == 0) T(
                        if (narrow) shortMonths(name) else name,
                        Modifier.wrapContentWidth(unbounded = true),
                        Relay.type.mono.copy(fontSize = 11.sp, textAlign = TextAlign.Center), c.ink2, maxLines = 1,
                    )
                }
            }
        }
    }
}

/**
 * Bars by category as rows (name, bar, value), in the ledger's order, largest first: under a
 * bar a category's name had a slot's width and read as "Gro…". More than one period is a bar
 * each, newest on top in the legend's colours, and their values newest first.
 */
@Composable
private fun CategoryRows(data: Numbers) {
    val c = Relay.colors
    val count = data.series.size
    val hi = max(data.series.maxOf { s -> s.values.filterNotNull().maxOrNull() ?: 0L }, 1L)
    val shown = data.labels.indices.filter { i -> data.series.any { s -> (s.values.getOrNull(i) ?: 0L) > 0 } }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (i in shown) {
            val tint = data.colors[i]?.let { hue(it) } ?: PALETTE[i % PALETTE.size]
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dot(tint, 8.dp)
                T(data.labels[i], Modifier.weight(.42f), Relay.type.caption, c.ink2, maxLines = 1)
                Column(Modifier.weight(.58f)) {
                    for (k in data.series.indices.reversed()) {
                        PaceMeter((data.series[k].values.getOrNull(i) ?: 0L).toFloat() / hi, if (count == 1) tint else seriesColor(k, count))
                    }
                }
                T(data.series.asReversed().joinToString(" · ") { s -> data.fmt.whole(s.values.getOrNull(i) ?: 0L) }, style = Relay.type.mono.copy(fontSize = 12.sp), color = c.ink, maxLines = 1)
            }
        }
    }
}

/** Where the money went: a ring of the newest series, and a list beside it (under it on a narrow phone). */
@Composable
private fun Donut(data: Numbers) {
    val c = Relay.colors
    val (name, values) = data.series.last()
    val parts = data.labels.indices.filter { (values.getOrNull(it) ?: 0L) > 0 }.map { i ->
        Triple(data.labels[i], values[i] ?: 0L, data.colors[i]?.let { hue(it) } ?: PALETTE[i % PALETTE.size])
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
                        T(share(v * 10_000 / max(total, 1L)), Modifier.padding(start = 2.dp), Relay.type.mono.copy(fontSize = 11.5.sp, textAlign = TextAlign.End), c.ink3)
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
        !asked -> "Claude's numbers from this reply, not Tally's: it does not redraw"
        stale -> "Saved on this phone · ${ago(live.at)}"
        spec.gym != null -> "From Avex's last export, redraws when a newer one comes in"
        spec.price != null -> "Live from Arbiter, redraws when it trades"
        else -> "Live from Tally, redraws when the ledger changes"
    }
    // The figure says what it is: the newest series' name, or "Total". A reply's own numbers
    // have none: summed, a projection or a balance over months says nothing.
    val total = data?.takeIf { asked && spec.kind != ChartKind.Donut && it.series.isNotEmpty() }?.let { d ->
        val i = d.series.lastIndex
        val values = d.series[i].values.take(d.known.getOrElse(i) { d.labels.size }).filterNotNull()
        val figure = when {
            values.isEmpty() -> null
            spec.kind == ChartKind.Price -> d.fmt.format(values.last())
            d.fmt is Fmt.Cash && d.cumulative -> d.fmt.whole(values.last())
            d.fmt is Fmt.Cash -> d.fmt.whole(values.sum())
            else -> null
        }
        figure?.let { d.series[i].name.ifBlank { "Total" } to it }
    }
    Column {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (asked) Dot(if (stale) c.waiting else c.live, 6.dp)
            T(note, Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 2)
            total?.let { (name, figure) ->
                T(name, style = Relay.type.caption, color = c.ink3, maxLines = 1)
                T(figure, style = Relay.type.title.copy(fontSize = 18.sp, lineHeight = 22.sp, fontFeatureSettings = "tnum"), color = c.ink, maxLines = 1)
            }
        }
    }
}
