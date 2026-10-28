package com.quietsoftware.relay.ui.start

import android.content.Context
import android.provider.Settings
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.wire.arr
import com.quietsoftware.relay.core.wire.l
import com.quietsoftware.relay.core.wire.o
import com.quietsoftware.relay.core.wire.s
import com.quietsoftware.relay.data.Relay
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Dot
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Mark
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.RelayMark
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay as Theme
import java.text.NumberFormat
import java.time.LocalTime
import java.util.Currency
import kotlin.math.PI
import kotlin.math.exp
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject

/** Whether this process has played the opening: coming back to the start screen shows it settled. */
private var opened = false

/**
 * The start screen (start.rs): the mark and wordmark on a field of dots, a greeting, and a card
 * per space with one live line each. It has its own colours, as on the PC, and the PC's opening
 * (`Mark`): the lanes draw in, the signal breaks away and sends a wave across the field, the
 * wordmark spells in, then the words and the cards one after the other. After that the signal
 * beats and sends a fainter wave every few seconds, and the field answers a finger. Everything
 * moving reads the timeline in the draw phase, so a frame of it redraws without recomposing the
 * screen.
 */
@Composable
fun StartScreen(nav: Nav) {
    val s by nav.shell.state.collectAsStateWithLifecycle()
    // Remembered: a new flow each recomposition would ask the PC again.
    val dashboard by remember { nav.relay.live("dashboard.get") }.collectAsStateWithLifecycle(Relay.Live())
    val money by remember { nav.relay.live("money.summary") }.collectAsStateWithLifecycle(Relay.Live())
    val context = LocalContext.current
    val moving = remember { animates(context) }
    val opening = remember { Animatable(if (opened || !moving) 1f else 0f) }
    // The current wave's progress after the opening; 1 is none.
    val pulse = remember { Animatable(1f) }
    LaunchedEffect(Unit) {
        opened = true
        if (opening.value < 1f) opening.animateTo(1f, tween(Mark.OPENING_MS, easing = LinearEasing))
        if (!moving) return@LaunchedEffect
        delay(900)
        while (true) {
            pulse.snapTo(0f)
            pulse.animateTo(1f, tween(PULSE_MS, easing = LinearEasing))
            delay((PULSE_EVERY_MS - PULSE_MS).toLong())
        }
    }
    val t = { opening.value }
    val pulsing = { pulse.value }
    // The signal swells and eases back as each wave leaves it.
    val beat = {
        val into = pulse.value * PULSE_MS
        if (pulse.value < 1f && into < 420f) sin(PI.toFloat() * into / 420f) else 0f
    }
    var markAt by remember { mutableStateOf(Rect.Zero) }
    // The field answers a finger (or a mouse): the dots under it light and lean away, and a tap
    // sends a ripple out from where it landed. In the field's pixels.
    val scope = rememberCoroutineScope()
    var touchAt by remember { mutableStateOf(Offset.Zero) }
    val touch = remember { Animatable(0f) }
    var rippleAt by remember { mutableStateOf(Offset.Zero) }
    val ripple = remember { Animatable(1f) }
    val answer = if (!moving) Modifier else Modifier.pointerInput(Unit) {
        val slop = viewConfiguration.touchSlop
        awaitPointerEventScope {
            var down: Offset? = null
            var downAt = 0L
            while (true) {
                // Watched on the way in and never consumed: scrolling and the cards work as before.
                val event = awaitPointerEvent(PointerEventPass.Initial)
                val change = event.changes.firstOrNull() ?: continue
                when (event.type) {
                    PointerEventType.Press -> {
                        down = change.position
                        downAt = change.uptimeMillis
                        touchAt = change.position
                        scope.launch { touch.animateTo(1f, tween(180)) }
                    }
                    PointerEventType.Move, PointerEventType.Enter -> {
                        touchAt = change.position
                        if (touch.targetValue < 1f) scope.launch { touch.animateTo(1f, tween(180)) }
                    }
                    PointerEventType.Release -> {
                        val from = down
                        if (from != null && (change.position - from).getDistance() < slop && change.uptimeMillis - downAt < 350) {
                            rippleAt = change.position
                            scope.launch {
                                ripple.snapTo(0f)
                                ripple.animateTo(1f, tween(RIPPLE_MS, easing = LinearEasing))
                            }
                        }
                        down = null
                        // A finger lifts away; a mouse is still there.
                        if (change.type == PointerType.Touch) scope.launch { touch.animateTo(0f, tween(600)) }
                    }
                    PointerEventType.Exit -> scope.launch { touch.animateTo(0f, tween(600)) }
                }
            }
        }
    }

    val dash = dashboard.result as? JsonObject
    // Who works is this phone's copy, as Home and the drawer count it; what waits on the person and
    // what is in review come from the dashboard once it is read.
    val working = s.working
    val waiting = dash?.let(::waitingOnYou) ?: s.needsYou
    val devLine = dash?.let { devReading(it, working) } ?: if (working == 0 && waiting == 0) null else agentsLine(working, waiting, 0)
    val summary = money.result as? JsonObject
    val moneyLine = summary?.let(::moneyReading)
    val first = s.prefs.name.trim().substringBefore(' ')
    val greeting = greetingFor(LocalTime.now().hour) + if (first.isNotEmpty()) ", $first" else ""

    Box(Modifier.fillMaxSize().background(Palette.START_GROUND).then(answer)) {
        DotField(Modifier.fillMaxSize(), t, pulsing, { touchAt to touch.value }, { rippleAt to ripple.value }) { markAt }
        Column(
            Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 22.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column(Modifier.widthIn(max = 520.dp).fillMaxWidth().padding(top = 56.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    RelayMark(58.dp, Modifier.onGloballyPositioned { markAt = it.boundsInRoot() }, ink = START_INK, t = t, beat = beat)
                    T("relay", Modifier.padding(start = 12.dp).spell(t), TextStyle(fontFamily = Fonts.Sora, fontSize = 40.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-1.4).sp), START_INK)
                }
                T(greeting, Modifier.padding(top = 40.dp).reveal(t, Mark.GREETING, 12.dp), TextStyle(fontFamily = Fonts.Sora, fontSize = 25.sp, fontWeight = FontWeight.SemiBold, lineHeight = 31.sp), START_INK)
                T("Where do you want to start?", Modifier.padding(top = 6.dp, bottom = 26.dp).reveal(t, Mark.QUESTION, 8.dp), Theme.type.body, Palette.START_TEXT2)
                SpaceCard(
                    Modifier.reveal(t, Mark.CARD_DEV, 22.dp),
                    glyph = "terminal",
                    title = "Dev",
                    description = "Agents, tasks and code across your projects",
                    line = devLine ?: if (s.online) "No agents working" else offlineLine(s.lastSeen),
                    live = s.online && working > 0,
                    attention = s.online && waiting > 0,
                    onClick = { nav.space("dev") },
                )
                SpaceCard(
                    Modifier.reveal(t, Mark.CARD_THREADS, 22.dp),
                    glyph = "threads",
                    title = "Threads",
                    description = "Talk it through with agents that know your data",
                    // A budget never read is not an empty one: with the PC away it says so, as Dev does.
                    line = moneyLine ?: when {
                        !s.online -> offlineLine(s.lastSeen)
                        else -> money.error?.takeIf { it.code != "link.down" }?.message?.ifBlank { null } ?: "Reading…"
                    },
                    live = s.online && summary != null && !emptyLedger(summary),
                    onClick = { nav.space("threads") },
                )
                T(
                    if (s.online) "Connected to ${s.pc?.name?.ifBlank { null } ?: "your PC"}" else "Your PC is out of reach. Everything here is the phone's copy; changes wait and go when it is back.",
                    Modifier.reveal(t, Mark.FOOT, 0.dp).clickable { nav.pc() }.padding(top = 18.dp, bottom = 32.dp),
                    Theme.type.caption,
                    Palette.START_TEXT2,
                )
            }
        }
    }
}

private val START_INK = Color(0xFFEDEEF0)

/** The reading's lamp with nothing live, and with something waiting on you (start.css `.start-live`). */
private val START_IDLE = Color(0xFF55585E)
private val START_ATTENTION = Color(0xFFF0A828)

@Composable
private fun SpaceCard(modifier: Modifier, glyph: String, title: String, description: String, line: String, live: Boolean, attention: Boolean = false, onClick: () -> Unit) {
    Column(
        modifier.fillMaxWidth().padding(bottom = 12.dp).clip(Radii.card).background(Palette.START_CARD)
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
            Dot(if (attention) START_ATTENTION else if (live) Palette.SIGNAL else START_IDLE, 6.dp)
            T(line, style = Theme.type.mono, color = if (live || attention) START_INK else Palette.START_TEXT2, maxLines = 1)
        }
    }
}

/** The field of dots behind the lockup (start.rs `draw_field`, its dots a little quieter on a
 *  phone): a signal glow around the mark and a grid of dots that the opening uncovers from the
 *  signal outward, lit lime where a wave passes. Under a finger the dots light and lean away
 *  from it (`touch`: where, how present), and a tap's ripple runs out from where it landed
 *  (`ripple`: where, how far, 1 none). `mark` is the mark's bounds in the root. */
@Composable
private fun DotField(modifier: Modifier, t: () -> Float, pulse: () -> Float, touch: () -> Pair<Offset, Float>, ripple: () -> Pair<Offset, Float>, mark: () -> Rect) {
    var origin by remember { mutableStateOf(Offset.Zero) }
    Canvas(modifier.onGloballyPositioned { origin = it.positionInRoot() }) {
        val m = mark()
        val (signal, glow) = if (m.width > 0f) {
            Offset(m.left + m.width * Mark.SIGNAL_X / 120f, m.top + m.height * Mark.SIGNAL_Y / 120f) - origin to m.center - origin
        } else {
            Offset(size.width / 2, size.height * .15f).let { it to it }
        }
        drawField(t(), pulse(), signal, glow, touch(), ripple())
    }
}

/** One wave over the field: where it starts, how far out it is (0..1), how strong, how far it goes. */
private class Wave(val from: Offset, val progress: Float, val strength: Float, val far: Float)

private fun DrawScope.farthest(p: Offset) =
    listOf(Offset.Zero, Offset(size.width, 0f), Offset(0f, size.height), Offset(size.width, size.height))
        .maxOf { (it - p).getDistance() }.coerceAtLeast(1f)

private fun DrawScope.drawField(t: Float, pulse: Float, signal: Offset, glow: Offset, touch: Pair<Offset, Float>, ripple: Pair<Offset, Float>) {
    val far = farthest(signal)
    // The landing's wave is stronger than the pulses after it; a tap's ripple starts where it landed.
    val landing = ((t - Mark.PING.from) / (Mark.PING.to - Mark.PING.from)).coerceIn(0f, 1f)
    val waves = buildList {
        if (landing > 0f && landing < 1f) add(Wave(signal, landing, .9f, far))
        if (pulse < 1f) add(Wave(signal, pulse, .55f, far))
        if (ripple.second < 1f) add(Wave(ripple.first, ripple.second, .7f, farthest(ripple.first)))
    }
    val lit = Mark.phase(t, Mark.WAVE)
    val (touchAt, present) = touch

    val strongest = waves.filter { it.from == signal }.maxOfOrNull { (1f - it.progress) * it.strength } ?: 0f
    val glowAlpha = .085f * lit + .03f * strongest
    val reach = 230.dp.toPx()
    drawCircle(
        Brush.radialGradient(0f to Palette.SIGNAL.copy(alpha = glowAlpha), .45f to Palette.SIGNAL.copy(alpha = glowAlpha * .32f), 1f to Color.Transparent, center = glow, radius = reach),
        reach,
        glow,
    )
    val touchReach = 110.dp.toPx()
    if (present > .01f) {
        drawCircle(Brush.radialGradient(listOf(Palette.SIGNAL.copy(alpha = .07f * present), Color.Transparent), touchAt, touchReach * 1.6f), touchReach * 1.6f, touchAt)
    }
    val push = 11.dp.toPx()

    val step = 26.dp.toPx()
    val dot = 1.dp.toPx()
    val hot = 1.25.dp.toPx()
    val front = lit * (far + 160.dp.toPx())
    val band = 110.dp.toPx()
    val spread = 70.dp.toPx()
    var y = signal.y.mod(step)
    while (y < size.height) {
        var x = signal.x.mod(step)
        while (x < size.width) {
            val d = hypot(x - signal.x, y - signal.y)
            val fade = max(0f, 1f - (d / far).pow(1.15f))
            val shown = ((front - d) / band).coerceIn(0f, 1f)
            // The finger: lime under it, and each dot leaning away along the line from it.
            var px = x
            var py = y
            var near = 0f
            if (present > .01f) {
                val vx = x - touchAt.x
                val vy = y - touchAt.y
                val dp = hypot(vx, vy)
                near = present * exp(-(dp / touchReach).pow(2))
                if (dp > .5f) {
                    px += vx / dp * push * near
                    py += vy / dp * push * near
                }
            }
            val a = BASE_DOT * shown * max(fade, near)
            if (a > .004f) drawRect(Color.White.copy(alpha = a), Offset(px - dot, py - dot), Size(2 * dot, 2 * dot))
            var heat = 0f
            for (wave in waves) {
                val dw = hypot(x - wave.from.x, y - wave.from.y)
                val at = wave.progress * wave.far * .85f
                heat = max(heat, wave.strength * (1f - wave.progress) * exp(-((dw - at) / spread).pow(2)))
            }
            heat *= .35f + .65f * fade
            heat = max(heat, near * max(shown, .3f))
            if (heat > .02f) drawRect(Palette.SIGNAL.copy(alpha = .8f * min(heat, 1f)), Offset(px - hot, py - hot), Size(2 * hot, 2 * hot))
            x += step
        }
        y += step
    }
    // Each wave starts as a ring around where it began.
    for (wave in waves) {
        val p = min(wave.progress * 2.2f, 1f)
        if (p >= 1f) continue
        val eased = 1f - (1f - p).pow(3)
        drawCircle(Palette.SIGNAL.copy(alpha = .6f * (1f - eased) * wave.strength), 6.dp.toPx() + 40.dp.toPx() * eased, wave.from, style = Stroke(1.5.dp.toPx()))
    }
}

/** A part's place on the opening: it fades in over `span` and rises `rise` into place. */
private fun Modifier.reveal(t: () -> Float, span: Mark.Span, rise: Dp) = graphicsLayer {
    val p = Mark.phase(t(), span)
    alpha = p
    translationY = (1f - p) * rise.toPx()
}

/** The wordmark spelt in: it slides 14dp into place while a soft edge uncovers it letter by
 *  letter, left to right (start.rs `spell`). */
private fun Modifier.spell(t: () -> Float) = graphicsLayer {
    val s = Mark.WORDMARK
    alpha = Mark.phase(t(), Mark.Span(s.from, s.from + .08f))
    translationX = (1f - Mark.phase(t(), s)) * 14.dp.toPx()
    compositingStrategy = CompositingStrategy.Offscreen
}.drawWithContent {
    drawContent()
    val x = ((t() - Mark.WORDMARK.from) / (Mark.WORDMARK.to - Mark.WORDMARK.from)).coerceIn(0f, 1f)
    if (x >= 1f) return@drawWithContent
    val edge = x * 1.35f * size.width
    drawRect(Brush.horizontalGradient(listOf(Color.Black, Color.Transparent), edge - .35f * size.width, edge), blendMode = BlendMode.DstIn)
}

/** Whether the system animates at all (Developer options' animator scale, or Remove animations). */
private fun animates(context: Context) =
    Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) > 0f

private const val PULSE_EVERY_MS = 4_800
private const val PULSE_MS = 2_600
private const val BASE_DOT = .08f
private const val RIPPLE_MS = 1_800

private fun greetingFor(hour: Int) = when (hour) {
    in 5..11 -> "Good morning"
    in 12..17 -> "Good afternoon"
    in 18..22 -> "Good evening"
    else -> "Good night"
}

private fun offlineLine(lastSeen: Long) = if (lastSeen > 0) "PC away · last seen ${com.quietsoftware.relay.ui.kit.ago(lastSeen)}" else "PC away"

/** "3 agents working · 2 need you · 1 in review", after money.rs `dev_reading`, with this phone's count of who works. */
fun devReading(d: JsonObject, working: Int): String = agentsLine(working, waitingOnYou(d), d.arr("in_review").size)

/** Held actions waiting for a decision, and agents stopped on a question (money.rs `waiting_on_you`). */
fun waitingOnYou(d: JsonObject): Int =
    d.arr("holds_open").size + d.arr("projects").sumOf { ((it as? JsonObject)?.l("blocked_sessions") ?: 0L).toInt() }

private fun agentsLine(working: Int, waiting: Int, review: Int): String = buildList {
    add(if (working == 0) "No agents working" else "${plural(working, "agent")} working")
    if (waiting > 0) add(if (waiting == 1) "1 needs you" else "$waiting need you")
    if (review > 0) add("$review in review")
}.joinToString(" · ")

/** A ledger with nothing in it yet: its reading invites a budget, and its lamp stays dark. */
private fun emptyLedger(s: JsonObject) = (s["empty"] as? kotlinx.serialization.json.JsonPrimitive)?.content == "true"

/** "Budget · $412 left · on pace" (money.rs `money_reading`). */
fun moneyReading(s: JsonObject): String {
    if (emptyLedger(s)) return "Start with your budget"
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
