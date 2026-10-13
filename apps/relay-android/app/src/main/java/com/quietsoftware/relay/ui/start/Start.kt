package com.quietsoftware.relay.ui.start

import androidx.compose.animation.core.FastOutSlowInEasing
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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.RelayMark
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay as Theme
import kotlinx.serialization.json.JsonObject
import java.text.NumberFormat
import java.time.LocalTime
import java.util.Currency

/**
 * The start screen (start.rs): the r mark and wordmark on a field of dots, a greeting, and a
 * card per space with one live line each. It has its own colours, as on the PC.
 */
@Composable
fun StartScreen(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    // Remembered: a new flow each recomposition (every frame of the fade-in) would ask the PC again.
    val dashboard by remember { nav.relay.live("dashboard.get") }.collectAsStateWithLifecycle(Relay.Live())
    val money by remember { nav.relay.live("money.summary") }.collectAsStateWithLifecycle(Relay.Live())
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { shown = true }
    val intro by animateFloatAsState(shown)

    val devLine = (dashboard.result as? JsonObject)?.let(::devReading)
        ?: s.sessions.count { it.state in setOf("running", "idle", "blocked") }.let { if (it == 0) null else "$it agents running" }
    val moneyLine = (money.result as? JsonObject)?.let(::moneyReading)
    val first = s.prefs.name.trim().substringBefore(' ')
    val greeting = greetingFor(LocalTime.now().hour) + if (first.isNotEmpty()) ", $first" else ""

    Box(Modifier.fillMaxSize().background(Palette.START_GROUND)) {
        DotField(Modifier.fillMaxSize())
        Column(
            Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 22.dp).alpha(intro),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column(Modifier.widthIn(max = 520.dp).fillMaxWidth().padding(top = 56.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    RelayMark(58.dp, ink = START_INK)
                    T("relay", Modifier.padding(start = 12.dp), TextStyle(fontFamily = Fonts.Sora, fontSize = 40.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-1.4).sp), START_INK)
                }
                T(greeting, Modifier.padding(top = 40.dp), TextStyle(fontFamily = Fonts.Sora, fontSize = 25.sp, fontWeight = FontWeight.SemiBold, lineHeight = 31.sp), START_INK)
                T("Where do you want to start?", Modifier.padding(top = 6.dp, bottom = 26.dp), Theme.type.body, Palette.START_TEXT2)
                SpaceCard(
                    glyph = "terminal",
                    title = "Dev",
                    description = "Agents, tasks and code across your projects",
                    line = devLine ?: if (s.online) "No agents running" else offlineLine(s.lastSeen),
                    live = s.online && devLine != null,
                    onClick = { nav.space("dev") },
                )
                SpaceCard(
                    glyph = "threads",
                    title = "Threads",
                    description = "Talk it through with agents that know your data",
                    line = moneyLine ?: "Start with your budget",
                    live = s.online && moneyLine != null,
                    onClick = { nav.space("threads") },
                )
                T(
                    if (s.online) "Connected to ${s.pc?.name?.ifBlank { null } ?: "your PC"}" else "Your PC is out of reach. Everything here is the phone's copy; changes wait and go when it is back.",
                    Modifier.padding(top = 18.dp, bottom = 32.dp).clickable { nav.pc() },
                    Theme.type.caption,
                    Palette.START_DIM,
                )
            }
        }
    }
}

private val START_INK = Color(0xFFEDEEF0)

@Composable
private fun animateFloatAsState(shown: Boolean) =
    androidx.compose.animation.core.animateFloatAsState(if (shown) 1f else 0f, tween(900, easing = FastOutSlowInEasing), label = "intro")

@Composable
private fun SpaceCard(glyph: String, title: String, description: String, line: String, live: Boolean, onClick: () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(bottom = 12.dp).clip(Radii.card).background(Palette.START_CARD)
            .border(1.dp, Palette.START_EDGE, Radii.card).clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 18.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Glyph(glyph, 22.dp, START_INK)
            T(title, Modifier.weight(1f).padding(start = 10.dp), TextStyle(fontFamily = Fonts.Sora, fontSize = 19.sp, fontWeight = FontWeight.SemiBold), START_INK)
            Glyph("chevron-right", 16.dp, Palette.START_DIM)
        }
        T(description, style = Theme.type.caption.copy(fontSize = 13.5.sp), color = Palette.START_TEXT2)
        Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dot(if (live) Palette.SIGNAL else Color(0xFF55585E), 6.dp)
            T(line, style = Theme.type.mono, color = if (live) START_INK else Palette.START_TEXT2, maxLines = 1)
        }
    }
}

/** The field of dots with a slow signal glow (start.rs's background, quieter on a phone). */
@Composable
private fun DotField(modifier: Modifier) {
    val pulse = rememberInfiniteTransition(label = "field")
    val glow by pulse.animateFloat(0.10f, 0.18f, infiniteRepeatable(tween(4800, easing = LinearEasing), RepeatMode.Reverse), label = "glow")
    Canvas(modifier) {
        val step = 26.dp.toPx()
        val r = 1.dp.toPx()
        var y = step / 2
        while (y < size.height) {
            var x = step / 2
            while (x < size.width) {
                drawCircle(Color.White.copy(alpha = 0.05f), r, Offset(x, y))
                x += step
            }
            y += step
        }
        val center = Offset(size.width * 0.82f, size.height * 0.12f)
        drawCircle(Brush.radialGradient(listOf(Palette.SIGNAL.copy(alpha = glow), Color.Transparent), center, size.minDimension * 0.7f), size.minDimension * 0.7f, center)
    }
}

private fun greetingFor(hour: Int) = when (hour) {
    in 5..11 -> "Good morning"
    in 12..17 -> "Good afternoon"
    in 18..22 -> "Good evening"
    else -> "Good night"
}

private fun offlineLine(lastSeen: Long) = if (lastSeen > 0) "PC away · last seen ${com.quietsoftware.relay.ui.kit.ago(lastSeen)}" else "PC away"

/** "3 agents running · 2 in review" (money.rs `dev_reading`). */
fun devReading(d: JsonObject): String {
    val live = d.arr("sessions_live").size
    val review = d.arr("in_review").size
    val agents = when (live) {
        0 -> "No agents running"
        1 -> "1 agent running"
        else -> "$live agents running"
    }
    return if (review == 0) agents else "$agents · $review in review"
}

/** "Budget · $412 left · on pace" (money.rs `money_reading`). */
fun moneyReading(s: JsonObject): String {
    if ((s["empty"] as? kotlinx.serialization.json.JsonPrimitive)?.content == "true") return "Start with your budget"
    val pace = s.o("pace") ?: return "Budget"
    val status = pace.s("status").orEmpty()
    if (status !in setOf("ON_PACE", "UNDER_PACE", "OVER_PACE")) return s.o("lines")?.s("margin")?.let { "Budget · $it" } ?: "Budget"
    val left = whole(pace.l("remaining") ?: 0, s.s("currency") ?: "CAD", (s.l("fraction_digits") ?: 2).toInt())
    val word = when (status) {
        "OVER_PACE" -> "over pace"
        "UNDER_PACE" -> "under pace"
        else -> "on pace"
    }
    return "Budget · $left left · $word"
}

/** Minor units as a whole amount in the ledger's currency: 41250 → "$412". */
fun whole(minor: Long, currency: String, digits: Int): String {
    val f = NumberFormat.getCurrencyInstance()
    runCatching { f.currency = Currency.getInstance(currency) }
    f.maximumFractionDigits = 0
    var div = 1.0
    repeat(digits) { div *= 10 }
    return f.format(minor / div)
}
