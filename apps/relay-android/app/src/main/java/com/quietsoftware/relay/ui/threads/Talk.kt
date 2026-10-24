package com.quietsoftware.relay.ui.threads

import android.content.ClipData
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import com.quietsoftware.relay.ui.dev.MicGlyph
import com.quietsoftware.relay.ui.dev.ReturnGlyph
import com.quietsoftware.relay.ui.kit.Glyph
import com.quietsoftware.relay.ui.kit.Markdown
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.TOUCH
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.time.Duration
import java.time.LocalDate
import java.time.LocalDateTime

// ---- A conversation, as Claude's own app draws one ----
//
// The thread and an agent's conversation are one look: the person's words in a soft bubble on the
// right, the agent's as plain reading text across the column, quiet keys under a finished reply,
// Claude's spark while it works, and the bar at the foot: a rounded box with the words on top and
// a row of round keys under them, the model (or the agent's mode) and the send key on the right.

/** Claude's clay: the send key and the working spark. */
internal val CLAY = Color(0xFFD97757)

/** The person's bubble: rounder than a card, as Claude's app draws it. */
internal val BUBBLE = RoundedCornerShape(20.dp)

/** The bar's box. */
private val BAR = RoundedCornerShape(24.dp)

/** The agent's words: a serif at reading size, as Claude's app sets its replies. */
@Composable
internal fun replyStyle(): TextStyle = Relay.type.body.copy(fontFamily = FontFamily.Serif, fontSize = 16.sp, lineHeight = 25.sp)

/** The person's words in their bubble. */
@Composable
internal fun bubbleStyle(): TextStyle = Relay.type.body.copy(fontSize = 15.5.sp, lineHeight = 22.sp)

/** The person's bubble's fill. */
internal val bubbleFill: Color @Composable get() = Relay.colors.track

/**
 * Today, kept current: it moves on at midnight, and again whenever the screen comes back (a phone
 * asleep overnight does not count the hours a delay waits for), so "Today" never goes stale.
 */
@Composable
internal fun rememberToday(): LocalDate {
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val today by produceState(LocalDate.now(), lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                value = LocalDate.now()
                val now = LocalDateTime.now()
                delay(Duration.between(now, now.toLocalDate().plusDays(1).atStartOfDay()).toMillis() + 500)
            }
        }
    }
    return today
}

/**
 * A reply being written, as what is settled and what is still coming: the settled part ends at the
 * last blank line outside a code fence. Each frame then reads again only the paragraph being
 * written; the settled part is read once per paragraph.
 */
internal fun splitStream(text: String): Pair<String, String> {
    var fence = false
    var cut = -1
    var start = 0
    while (start < text.length) {
        val end = text.indexOf('\n', start).let { if (it < 0) text.length else it }
        val line = text.substring(start, end)
        if (line.trimStart().startsWith("```")) fence = !fence
        else if (!fence && start > 0 && line.isBlank()) cut = start
        start = end + 1
    }
    if (cut <= 0) return "" to text
    return text.substring(0, cut).trimEnd() to text.substring(cut).trimStart('\n', ' ')
}

/** A reply streaming in: the settled paragraphs, read once each, then the one being written. */
@Composable
internal fun StreamedReply(text: String, modifier: Modifier = Modifier) {
    val (done, now) = remember(text) { splitStream(text) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (done.isNotEmpty()) Markdown(done, style = replyStyle())
        if (now.isNotEmpty()) Markdown(now, style = replyStyle())
    }
}

/** Claude's spark, turning and breathing while the agent works. */
@Composable
internal fun Spark(size: Dp = 18.dp, tint: Color = CLAY, glyph: String = "claude") {
    val t = rememberInfiniteTransition(label = "spark")
    val turn by t.animateFloat(0f, 360f, infiniteRepeatable(tween(6_000, easing = LinearEasing)), label = "turn")
    val breath by t.animateFloat(.82f, 1.08f, infiniteRepeatable(tween(900, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "breath")
    Box(Modifier.size(size).graphicsLayer { rotationZ = turn; scaleX = breath; scaleY = breath }, contentAlignment = Alignment.Center) {
        Glyph(glyph, size, tint)
    }
}

/** The working line: the spark, and what the agent is doing in quiet words. */
@Composable
internal fun Working(text: String, modifier: Modifier = Modifier, glyph: String = "claude", tint: Color = CLAY, aside: (@Composable () -> Unit)? = null) {
    Row(modifier.heightIn(min = 28.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Spark(18.dp, tint, glyph)
        T(text, Modifier.weight(1f, fill = false), Relay.type.ui, Relay.colors.ink3, maxLines = 1)
        aside?.invoke()
    }
}

/** The keys under a finished reply: Copy its words, and Try again where the reply can be asked for again. */
@Composable
internal fun ReplyKeys(text: String, modifier: Modifier = Modifier, onRetry: (() -> Unit)? = null) {
    val c = Relay.colors
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    var copied by remember { mutableStateOf(false) }
    Row(modifier.padding(top = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        // The first key's glyph lines up with the reply's text.
        Box(
            Modifier.size(TOUCH).clip(CircleShape).clickable(role = Role.Button) {
                scope.launch {
                    clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("reply", text)))
                    copied = true
                    delay(1400)
                    copied = false
                }
            }.semantics { contentDescription = if (copied) "Copied" else "Copy reply" },
            contentAlignment = Alignment.CenterStart,
        ) { Glyph(if (copied) "check" else "copy", 16.dp, c.ink3) }
        onRetry?.let { retry ->
            Box(
                Modifier.size(TOUCH).clip(CircleShape).clickable(role = Role.Button, onClick = retry).semantics { contentDescription = "Try again" },
                contentAlignment = Alignment.CenterStart,
            ) { Glyph("refresh", 16.dp, c.ink3) }
        }
    }
}

// ---- The bar ----

/** What the bar's round key on the right does now. */
internal enum class BarKey { Send, Stop, Enter, Speak }

/**
 * The message bar, as Claude's app draws it: one rounded box, the words on top growing to about
 * eight lines, and under them round keys on the left ([leading]: attach, settings), a quiet
 * label on the right ([trailing]: the model, or an agent's mode) and the round key, which is
 * clay to send, a square to stop, the microphone with nothing typed, or Enter for a prompt.
 * [above] shows over the words (the pictures picked).
 */
@Composable
internal fun TalkBar(
    field: TextFieldValue,
    onField: (TextFieldValue) -> Unit,
    placeholder: String,
    key: BarKey,
    keyEnabled: Boolean,
    onKey: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    focus: FocusRequester? = null,
    lines: Int = 8,
    keyboard: KeyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
    above: @Composable () -> Unit = {},
    trailing: @Composable RowScope.() -> Unit = {},
    leading: @Composable RowScope.() -> Unit,
) {
    val c = Relay.colors
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Column(
        modifier.fillMaxWidth().clip(BAR).background(c.wash)
            .border(1.dp, if (focused) c.lineFocus else c.lineEmphasis, BAR)
            .padding(start = 8.dp, end = 8.dp, top = 12.dp, bottom = 8.dp),
    ) {
        Box(Modifier.padding(horizontal = 8.dp)) { above() }
        BasicTextField(
            value = field,
            onValueChange = onField,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp).heightIn(min = 26.dp).alpha(if (enabled) 1f else .6f)
                .then(if (focus != null) Modifier.focusRequester(focus) else Modifier).focusProperties { canFocus = enabled },
            textStyle = Relay.type.body.copy(fontSize = 16.sp, lineHeight = 23.sp, color = c.ink),
            cursorBrush = SolidColor(CLAY),
            minLines = 1,
            maxLines = lines,
            interactionSource = source,
            keyboardOptions = keyboard,
            decorationBox = { inner ->
                Box {
                    if (field.text.isEmpty()) T(placeholder, style = Relay.type.body.copy(fontSize = 16.sp, lineHeight = 23.sp), color = c.ink3, maxLines = 1)
                    inner()
                }
            },
        )
        Row(Modifier.fillMaxWidth().padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, content = leading)
            Row(verticalAlignment = Alignment.CenterVertically, content = trailing)
            ActionKey(key, keyEnabled, onKey)
        }
    }
}

/** A round key in the bar's lower row: a ring around a glyph, in a full-size target. Off, a tap says [whyOff] if it has one. */
@Composable
internal fun BarButton(spoken: String, enabled: Boolean = true, whyOff: (() -> Unit)? = null, onClick: () -> Unit, content: @Composable () -> Unit) {
    val c = Relay.colors
    val haptic = LocalHapticFeedback.current
    Box(
        Modifier.size(TOUCH).clip(CircleShape).clickable(enabled = enabled || whyOff != null, role = Role.Button) {
            if (enabled) {
                haptic.performHapticFeedback(HapticFeedbackType.KeyboardTap)
                onClick()
            } else {
                whyOff?.invoke()
            }
        }.semantics { contentDescription = spoken },
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(34.dp).alpha(if (enabled) 1f else .4f).clip(CircleShape).border(1.dp, c.lineFocus, CircleShape), contentAlignment = Alignment.Center) { content() }
    }
}

/** [BarButton] with a glyph. */
@Composable
internal fun BarButton(glyph: String, spoken: String, enabled: Boolean = true, whyOff: (() -> Unit)? = null, onClick: () -> Unit) =
    BarButton(spoken, enabled, whyOff, onClick) { Glyph(glyph, 16.dp, Relay.colors.ink2) }

/**
 * The bar's quiet label on the right: the model, or an agent's mode, with a chevron when it opens
 * a list. [content] holds what it opens (a popover anchored to it).
 */
@Composable
internal fun BarLabel(text: String, spoken: String, enabled: Boolean = true, chevron: Boolean = true, whyOff: (() -> Unit)? = null, onClick: () -> Unit, content: @Composable () -> Unit = {}) {
    val c = Relay.colors
    Box(
        Modifier.heightIn(min = TOUCH).clip(RoundedCornerShape(12.dp)).clickable(enabled = enabled || whyOff != null, role = Role.Button) {
            if (enabled) onClick() else whyOff?.invoke()
        }.semantics { contentDescription = spoken },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier.alpha(if (enabled) 1f else .45f).padding(start = 8.dp, end = if (chevron) 6.dp else 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            T(text, Modifier.widthIn(max = 150.dp), Relay.type.ui, c.ink2, maxLines = 1, weight = FontWeight.Medium)
            if (chevron) Glyph("chevron-down", 12.dp, c.ink3)
        }
        content()
    }
}

/** The bar's round key: clay to send, a stop square, the microphone, or Enter. */
@Composable
private fun ActionKey(key: BarKey, enabled: Boolean, onClick: () -> Unit) {
    val c = Relay.colors
    val haptic = LocalHapticFeedback.current
    val spoken = when (key) {
        BarKey.Send -> "Send"
        BarKey.Stop -> "Stop"
        BarKey.Enter -> "Enter"
        BarKey.Speak -> "Dictate a message"
    }
    Box(
        Modifier.size(TOUCH).clip(CircleShape).clickable(enabled = enabled, role = Role.Button) {
            if (key == BarKey.Send) haptic.performHapticFeedback(HapticFeedbackType.Confirm)
            onClick()
        }.semantics { contentDescription = spoken },
        contentAlignment = Alignment.Center,
    ) {
        val fill = when {
            key == BarKey.Send -> if (enabled) CLAY else CLAY.copy(alpha = .35f)
            key == BarKey.Stop -> if (enabled) c.ink else c.track
            else -> c.track
        }
        val tint = when (key) {
            BarKey.Send -> if (enabled) Color.White else Color.White.copy(alpha = .6f)
            BarKey.Stop -> if (enabled) c.wall else c.ink3
            else -> if (enabled) c.ink else c.ink3
        }
        Box(Modifier.size(36.dp).clip(CircleShape).background(fill), contentAlignment = Alignment.Center) {
            when (key) {
                BarKey.Send -> Glyph("arrow-up", 17.dp, tint)
                BarKey.Stop -> Box(Modifier.size(11.dp).clip(RoundedCornerShape(2.5.dp)).background(tint))
                BarKey.Enter -> ReturnGlyph(17.dp, tint)
                BarKey.Speak -> MicGlyph(18.dp, tint)
            }
        }
    }
}
