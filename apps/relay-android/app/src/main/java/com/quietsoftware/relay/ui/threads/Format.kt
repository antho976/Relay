package com.quietsoftware.relay.ui.threads

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import com.quietsoftware.relay.core.wire.BusError
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull
import java.math.RoundingMode
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToLong

// ---- JSON ----

/** A number, or a decimal string as Arbiter sends its amounts. */
internal fun JsonObject.d(key: String): Double? = (this[key] as? JsonPrimitive)?.let { it.doubleOrNull ?: it.content.trim().toDoubleOrNull() }

/**
 * An amount from a tool's answer at [digits] decimals: the bus's integer, or the decimal string the
 * PC's MCP door gives a model instead ("157.99"; relay-cli's units.rs). Threads stored before the
 * door converted hold the integer.
 */
internal fun JsonObject.minor(key: String, digits: Int): Long? {
    val p = this[key] as? JsonPrimitive ?: return null
    p.longOrNull?.let { return it }
    if (!p.isString) return null
    return runCatching {
        p.content.trim().toBigDecimal().movePointRight(digits).setScale(0, RoundingMode.HALF_EVEN).longValueExact()
    }.getOrNull()
}

internal fun JsonObject.objs(key: String): List<JsonObject> = arr(key).mapNotNull { it as? JsonObject }

/** A field as text, `""` when it is missing; a number reads as its digits ([s]). */
internal fun JsonObject.str(key: String): String = s(key).orEmpty()

internal val JsonElement?.o: JsonObject? get() = this as? JsonObject

// ---- Money ----

/**
 * Amounts as Tally writes them (relay_money::money::MoneyFormatter, en-CA): minor units, the
 * currency's own symbol at home, a true minus sign, whole units rounded half-even.
 */
internal class Money(val currency: String, val digits: Int) {
    private val scale = pow10(digits)
    private val symbol = symbolOf(currency)

    /** "$1,284.50", "−$96.00". */
    fun format(minor: Long): String {
        val a = abs(minor)
        val frac = if (digits > 0) "." + (a % scale).toString().padStart(digits, '0') else ""
        return signed(minor, dress(group(a / scale) + frac))
    }

    /** "$1,285": whole units, for figures where cents are noise. */
    fun whole(minor: Long): String = signed(minor, dress(group(halfEven(abs(minor), scale))))

    /** Income reads "+$1,200.00"; anything else as [format]. */
    fun plus(minor: Long): String = if (minor > 0) "+" + format(minor) else format(minor)

    /** A decimal amount in major units (Arbiter's strings), as [format]. */
    fun major(value: Double): String = format((value * scale).roundToLong())

    private fun dress(body: String) = symbol + body

    private fun signed(minor: Long, body: String) = if (minor < 0) "−$body" else body

    companion object {
        /** The currency and its digits from a reading (`money.summary`, `money.series`). */
        fun of(o: JsonObject?): Money {
            val code = o?.s("currency")?.takeIf { it.isNotBlank() } ?: "CAD"
            val digits = (o?.get("fraction_digits") as? JsonPrimitive)?.content?.toIntOrNull() ?: digitsOf(code)
            return Money(code, digits)
        }

        fun digitsOf(code: String): Int = when (code.uppercase()) {
            "JPY", "KRW", "CLP", "ISK", "VND" -> 0
            "BHD", "KWD", "OMR", "JOD", "TND" -> 3
            else -> 2
        }

        /** Whether [code] is a currency Tally knows; anything else (a coin) is shown as a number and its code. */
        fun known(code: String): Boolean = code.uppercase() in FIAT

        private val FIAT = setOf("CAD", "USD", "EUR", "GBP", "AUD", "NZD", "MXN", "JPY", "CHF", "SEK", "NOK", "DKK", "INR", "CNY", "HKD", "SGD", "KRW", "BRL", "ZAR")

        private fun symbolOf(code: String): String {
            val home = when (Locale.getDefault().country) {
                "US" -> "USD"
                "AU" -> "AUD"
                "NZ" -> "NZD"
                "MX" -> "MXN"
                else -> "CAD"
            }
            return when (val c = code.uppercase()) {
                home -> "$"
                "CAD" -> "CA$"
                "USD" -> "US$"
                "AUD" -> "A$"
                "NZD" -> "NZ$"
                "MXN" -> "MX$"
                "EUR" -> "€"
                "GBP" -> "£"
                "JPY" -> "JP¥"
                "INR" -> "₹"
                else -> "$c "
            }
        }

        private fun pow10(n: Int): Long {
            var x = 1L
            repeat(n) { x *= 10 }
            return x
        }

        private fun halfEven(a: Long, scale: Long): Long {
            val q = a / scale
            val r = a % scale
            return when {
                r * 2 > scale -> q + 1
                r * 2 < scale -> q
                else -> if (q % 2 == 0L) q else q + 1
            }
        }
    }
}

/** `12,500`, `−3`. */
internal fun group(n: Long): String {
    val digits = abs(n).toString()
    val out = StringBuilder()
    digits.forEachIndexed { i, ch ->
        if (i > 0 && (digits.length - i) % 3 == 0) out.append(',')
        out.append(ch)
    }
    return if (n < 0) "−$out" else out.toString()
}

/** `x` with at most [places] decimals, trailing zeros dropped: 0.01230000 reads "0.0123". */
internal fun trimmed(x: Double, places: Int): String {
    val s = String.format(Locale.ROOT, "%.${places}f", x)
    return if (s.contains('.')) s.trimEnd('0').trimEnd('.') else s
}

/** "+4.2%", "−3.0%". */
internal fun pct(v: Double): String {
    val s = String.format(Locale.ROOT, "%.1f%%", abs(v))
    return when {
        s == "0.0%" -> s
        v > 0 -> "+$s"
        else -> "−$s"
    }
}

/** Basis points as a percentage: 1234 reads "+12.3%". */
internal fun bps(v: Long): String = pct(v / 100.0)

/** A share in basis points, in whole percent: "52%", or "<1%" for a sliver (money_invest.rs `share`). */
internal fun share(bps: Long): String = if (bps in 1..49) "<1%" else "${(bps + 50) / 100}%"

// ---- Dates ----

/** The local day of a `YYYY-MM-DD` date or an RFC 3339 time. */
internal fun dayOf(value: String?): LocalDate? {
    if (value.isNullOrBlank()) return null
    if (value.length == 10) return runCatching { LocalDate.parse(value) }.getOrNull()
    return runCatching { Instant.parse(value).atZone(ZoneId.systemDefault()).toLocalDate() }.getOrNull()
}

private val SAME_YEAR = DateTimeFormatter.ofPattern("EEE d MMM", Locale.ENGLISH)
private val OTHER_YEAR = DateTimeFormatter.ofPattern("EEE d MMM yyyy", Locale.ENGLISH)
private val SHORT = DateTimeFormatter.ofPattern("d MMM", Locale.ENGLISH)
private val MONTH = DateTimeFormatter.ofPattern("MMMM", Locale.ENGLISH)
private val MONTH_YEAR = DateTimeFormatter.ofPattern("MMMM yyyy", Locale.ENGLISH)

/** "Today", "Yesterday", "Tue 7 Oct", "Tue 7 Oct 2025" (money_pages.rs `human_date`). */
internal fun humanDay(day: LocalDate, today: LocalDate = LocalDate.now()): String = when {
    day == today -> "Today"
    day == today.minusDays(1) -> "Yesterday"
    day.year == today.year -> day.format(SAME_YEAR)
    else -> day.format(OTHER_YEAR)
}

internal fun humanDate(value: String?): String = dayOf(value)?.let { humanDay(it) } ?: value.orEmpty()

/** [humanDate] inside a sentence: "today", "yesterday", "Tue 7 Oct" (money_invest.rs `as_of`). */
internal fun inlineDate(value: String?): String = humanDate(value).let { if (it == "Today" || it == "Yesterday") it.lowercase() else it }

/** "Thu 8 Oct", never "Today": a week's name (avex_pages.rs `day_words`). */
internal fun dayWords(value: String?): String {
    val day = dayOf(value) ?: return value.orEmpty()
    return day.format(if (day.year == LocalDate.now().year) SAME_YEAR else OTHER_YEAR)
}

/** "October", or "15 Oct to 14 Nov" for a period that does not start on the 1st. */
internal fun periodName(period: JsonObject?): String {
    val start = dayOf(period?.s("start")) ?: return "This period"
    val end = dayOf(period?.s("end_exclusive")) ?: return "This period"
    val last = end.minusDays(1)
    return if (start.dayOfMonth == 1 && last.month == start.month) {
        start.format(if (start.year == LocalDate.now().year) MONTH else MONTH_YEAR)
    } else {
        "${start.format(SHORT)} to ${last.format(SHORT)}"
    }
}

/** "5m ago" from an RFC 3339 time; empty when it does not parse. */
internal fun agoOf(ts: String?): String = com.quietsoftware.relay.ui.kit.ago(com.quietsoftware.relay.ui.kit.epoch(ts))

/** What a refused read says on a page: the engine's own sentence, and plainly when this engine lacks the op. */
internal fun said(e: BusError, missing: String): String = when {
    e.code == "bus.unknown_op" || e.code == "bus.not_implemented" -> missing
    e.code == "link.down" -> "The PC is out of reach, and this was never read on this phone."
    else -> e.message.ifBlank { e.code }
}

// ---- Colour ----

/** Tally's category hue for a stored colour index. */
internal fun hue(index: Long?): Color = Palette.HUES[((index ?: 10L) % Palette.HUES.size + Palette.HUES.size).toInt() % Palette.HUES.size]

// ---- Small pieces ----

/** A category's badge: its hue on a 15% wash of it, and its initial, as Tally's badges sit. */
@Composable
internal fun HueBadge(name: String, color: Color, size: Dp = 24.dp, glyph: String? = null) {
    Box(
        Modifier.size(size).clip(RoundedCornerShape(if (size >= 30.dp) 10.dp else 8.dp)).background(color.copy(alpha = .15f)),
        contentAlignment = Alignment.Center,
    ) {
        if (glyph != null) Glyph(glyph, size * .55f, color)
        else T(name.trim().take(1).uppercase().ifEmpty { "·" }, style = Relay.type.caption.copy(fontSize = (size.value * .46f).sp, textAlign = TextAlign.Center), color = color, weight = FontWeight.SemiBold)
    }
}

/** A thin meter with an optional pace tick (money_pages.rs `meter_in`): track, fill, and where an even spend would be. */
@Composable
internal fun PaceMeter(fraction: Float, fill: Color, modifier: Modifier = Modifier, tick: Float? = null, height: Dp = 4.dp) {
    val c = Relay.colors
    Canvas(modifier.fillMaxWidth().height(height + 6.dp)) {
        val h = height.toPx()
        val top = 3.dp.toPx()
        val r = CornerRadius(h / 2)
        drawRoundRect(c.track, Offset(0f, top), Size(size.width, h), r)
        val f = fraction.coerceIn(0f, 1f)
        if (f > 0f) drawRoundRect(fill, Offset(0f, top), Size(maxOf(size.width * f, h), h), r)
        tick?.takeIf { it in 0f..1f }?.let { t ->
            val x = (size.width * t).coerceIn(1f, size.width - 1f)
            drawLine(c.ink3, Offset(x, 0f), Offset(x, size.height), strokeWidth = 1.5.dp.toPx(), cap = StrokeCap.Round)
        }
    }
}

/** A turning arc for what is still running. */
@Composable
internal fun Spinner(size: Dp = 12.dp, color: Color = Relay.colors.ink3) {
    val turn by rememberInfiniteTransition(label = "spin").animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "spin")
    Canvas(Modifier.size(size)) {
        val w = 1.5.dp.toPx()
        drawArc(color, turn, 270f, false, Offset(w / 2, w / 2), Size(this.size.width - w, this.size.height - w), style = Stroke(w, cap = StrokeCap.Round))
    }
}

/** The live lamp, breathing while an agent works. */
@Composable
internal fun WorkingLamp(size: Dp = 7.dp) {
    val a by rememberInfiniteTransition(label = "lamp").animateFloat(.35f, 1f, infiniteRepeatable(tween(800), RepeatMode.Reverse), label = "lamp")
    Box(Modifier.size(size).alpha(a).clip(Radii.pill).background(Relay.colors.live))
}

/** A titled card of rows, as Tally's pages and the PC's panel draw them. */
@Composable
internal fun Card(title: String, modifier: Modifier = Modifier, aside: String? = null, padding: PaddingValues = PaddingValues(start = 14.dp, end = 14.dp, top = 12.dp, bottom = 8.dp), content: @Composable ColumnScope.() -> Unit) {
    val c = Relay.colors
    Column(modifier.fillMaxWidth().clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).padding(padding)) {
        Row(Modifier.fillMaxWidth().padding(bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            T(title, Modifier.weight(1f), Relay.type.section.copy(fontWeight = FontWeight.SemiBold), c.ink)
            aside?.let { T(it, style = Relay.type.mono, color = c.ink3) }
        }
        content()
    }
}

/** A row in a card: a lead, a title over a detail, and a figure on the right. */
@Composable
internal fun FigureRow(
    title: String,
    detail: String,
    figure: String = "",
    modifier: Modifier = Modifier,
    figureColor: Color = Relay.colors.ink2,
    detailColor: Color = Relay.colors.ink3,
    divider: Boolean = true,
    onClick: (() -> Unit)? = null,
    lead: (@Composable () -> Unit)? = null,
) {
    val c = Relay.colors
    Column(modifier.fillMaxWidth()) {
        if (divider) Box(Modifier.fillMaxWidth().height(1.dp).background(c.lineSubtle))
        Row(
            Modifier.fillMaxWidth().heightIn(min = TOUCH).then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            lead?.invoke()
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
                T(title, style = Relay.type.ui, color = c.ink, maxLines = 1)
                if (detail.isNotEmpty()) T(detail, style = Relay.type.caption, color = detailColor, maxLines = 2)
            }
            if (figure.isNotEmpty()) T(figure, style = Relay.type.mono.copy(fontSize = 12.5.sp, fontFeatureSettings = "tnum"), color = figureColor, maxLines = 1)
        }
    }
}

/** A quiet sentence inside a card. */
@Composable
internal fun Quiet(text: String, modifier: Modifier = Modifier) =
    T(text, modifier.padding(vertical = 8.dp), Relay.type.caption, Relay.colors.ink3)

/** A pill of state: green live, amber waiting or paper, red held or halted, plain otherwise. */
@Composable
internal fun StatePill(text: String, tone: Color?) {
    val c = Relay.colors
    val ink = tone ?: c.ink2
    Row(
        Modifier.clip(Radii.pill).background((tone ?: c.ink3).copy(alpha = .14f)).padding(horizontal = 8.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        if (tone != null) Box(Modifier.size(6.dp).clip(Radii.pill).background(tone))
        T(text, style = Relay.type.caption.copy(fontSize = 11.5.sp), color = ink, maxLines = 1, weight = FontWeight.Medium)
    }
}

/**
 * A short list of choices under a key (the model and effort chips, a thread's menu): a slab
 * popover, a check by the chosen one. [groups] puts a small heading over the option it names
 * ("On this PC" over the PC's own models).
 */
@Composable
internal fun <V> Choices(
    open: Boolean,
    onDismiss: () -> Unit,
    options: List<Pair<V, String>>,
    selected: V?,
    onPick: (V) -> Unit,
    above: Boolean = true,
    anchor: Dp = 30.dp,
    glyphs: Map<V, String> = emptyMap(),
    danger: Set<V> = emptySet(),
    groups: Map<V, String> = emptyMap(),
) {
    if (!open) return
    val c = Relay.colors
    val shift = with(androidx.compose.ui.platform.LocalDensity.current) { (anchor + 6.dp).roundToPx() }
    Popup(
        alignment = if (above) Alignment.BottomStart else Alignment.TopEnd,
        offset = androidx.compose.ui.unit.IntOffset(0, if (above) -shift else shift),
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true),
    ) {
        Column(
            Modifier.widthIn(min = 180.dp, max = 280.dp)
                .clip(Radii.popover).background(c.slab).border(1.dp, c.strong, Radii.popover).padding(4.dp),
        ) {
            for ((value, label) in options) {
                val on = value == selected
                groups[value]?.let { T(it, Modifier.padding(start = 12.dp, end = 12.dp, top = 10.dp, bottom = 4.dp), Relay.type.caption, c.ink3, maxLines = 1) }
                Row(
                    Modifier.fillMaxWidth().heightIn(min = TOUCH).clip(Radii.key).clickable { onPick(value); onDismiss() }.padding(horizontal = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    glyphs[value]?.let { Glyph(it, 15.dp, if (value in danger) c.heldText else c.ink3) }
                    T(label, Modifier.weight(1f), Relay.type.ui, if (value in danger) c.heldText else if (on) c.ink else c.ink2, maxLines = 1)
                    if (on) Glyph("check", 14.dp, c.ink)
                }
            }
        }
    }
}
