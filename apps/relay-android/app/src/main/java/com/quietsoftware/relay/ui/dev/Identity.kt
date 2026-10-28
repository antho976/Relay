package com.quietsoftware.relay.ui.dev

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.core.model.Project
import com.quietsoftware.relay.core.model.Session
import com.quietsoftware.relay.core.model.Task
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay

/*
 * How an agent is told apart from the others at a glance, everywhere it is listed: a badge in a
 * colour of its own (from its name, so it never changes) with its name's two initials, a title
 * that says what it is doing, and a line that says where and when.
 */

/** The agent's own colour: one of the category hues, picked by its name. */
internal fun agentHue(name: String): Color {
    var h = 0
    for (ch in name) h = h * 31 + ch.code
    return Palette.HUES[Math.floorMod(h, Palette.HUES.size)]
}

/** "brisk-otter" → "BO", "worker" → "WO". */
internal fun agentInitials(name: String): String {
    val parts = name.split('-', '_', ' ').filter { it.isNotBlank() }
    return when {
        parts.size >= 2 -> "${parts[0][0]}${parts[1][0]}"
        parts.size == 1 -> parts[0].take(2)
        else -> "?"
    }.uppercase()
}

/** What the agent is doing, in words: its intent, else its task's title, else its name. A shell is a shell. */
internal fun agentTitle(session: Session, task: Task? = null): String =
    session.intent?.trim()?.ifBlank { null } ?: task?.title?.trim()?.ifBlank { null } ?: if (session.shell) "Shell" else session.name

/** What a session is, in a few words: "Shell" for the person's own shell, else its provider and role. */
internal fun kindWords(session: Session): String = if (session.shell) "Shell" else "${session.provider} · ${session.role}"

/** An agent's state as a word in a sentence: "Working", "Needs you", "Parked". */
internal fun stateWord(session: Session): String = when (session.state) {
    "running" -> if (session.shell) "Open" else "Working"
    "spawning" -> "Starting"
    "idle" -> if (session.shell) "Open" else "Ready"
    "blocked" -> "Needs you"
    else -> session.stateLabel.lowercase().replaceFirstChar(Char::uppercase)
}

/** When the agent last wrote anything, "5m ago"; empty when it never has. */
internal fun lastHeard(session: Session): String {
    val at = epoch(session.lastOutputAt).takeIf { it > 0 } ?: epoch(session.spawnedAt).takeIf { it > 0 } ?: epoch(session.updatedAt)
    return if (at > 0) ago(at) else ""
}

/**
 * "brisk-otter · RelayV4 · reviewer · #12": who it is and where, under its title. The name is left
 * out when it is the title already; the project when [project] is null; the role when it builds.
 */
internal fun agentWhere(session: Session, title: String, project: Project?, task: Task?): String = listOfNotNull(
    session.name.takeIf { it != title },
    project?.name,
    session.role.takeIf { it.isNotBlank() && it != "builder" },
    task?.num?.let { "#$it" },
).joinToString(" · ")

/** The agent's badge: its initials on a wash of its colour, its lamp at the corner. */
@Composable
internal fun AgentBadge(session: Session, size: Dp = 36.dp, lamp: Boolean = true, ring: Color = Relay.colors.slab, modifier: Modifier = Modifier) {
    val hue = agentHue(session.name)
    Box(modifier.size(size)) {
        Box(
            Modifier.size(size).clip(RoundedCornerShape(size * .3f)).background(hue.copy(alpha = .18f)).border(1.dp, hue.copy(alpha = .35f), RoundedCornerShape(size * .3f)),
            contentAlignment = Alignment.Center,
        ) {
            T(
                agentInitials(session.name),
                style = Relay.type.mono.copy(fontSize = (size.value * .34f).sp, textAlign = TextAlign.Center, letterSpacing = 0.sp),
                color = hue,
                weight = FontWeight.SemiBold,
            )
        }
        if (lamp) {
            val dot = (size.value * .3f).coerceAtLeast(8f).dp
            Box(
                Modifier.align(Alignment.BottomEnd).offset(x = 2.dp, y = 2.dp).size(dot + 4.dp).clip(CircleShape).background(ring),
                contentAlignment = Alignment.Center,
            ) { LampDot(session.lamp, dot) }
        }
    }
}
