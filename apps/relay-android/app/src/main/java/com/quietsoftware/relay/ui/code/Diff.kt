package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.ui.theme.Relay

enum class DiffKind { Head, Add, Del, Ctx, Note }

class DiffLine(val kind: DiffKind, val text: String, val number: Int?)

private val HUNK_HEADER = Regex("""^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@""")

/**
 * The lines of a set of hunks, numbered: the engine's hunk text carries its own `@@` headers, and
 * a hunk without one gets a header from its starts (the old phone app's rule, kept).
 */
fun diffLines(hunks: List<Hunk>): List<DiffLine> {
    val out = ArrayList<DiffLine>()
    for (hunk in hunks) {
        if (hunk.text.isEmpty()) continue
        val body = hunk.text.removeSuffix("\n").split('\n')
        var oldLine = hunk.oldStart
        var newLine = hunk.newStart
        if (!body[0].startsWith("@@")) out.add(DiffLine(DiffKind.Head, "@@ -${hunk.oldStart} +${hunk.newStart} @@", null))
        for (text in body) {
            val header = HUNK_HEADER.find(text)
            when {
                header != null -> {
                    oldLine = header.groupValues[1].toInt()
                    newLine = header.groupValues[2].toInt()
                    out.add(DiffLine(DiffKind.Head, text, null))
                }
                text.startsWith("+") -> out.add(DiffLine(DiffKind.Add, text, newLine++))
                text.startsWith("-") -> out.add(DiffLine(DiffKind.Del, text, oldLine++))
                text.startsWith("\\") -> out.add(DiffLine(DiffKind.Note, text, null))
                else -> {
                    out.add(DiffLine(DiffKind.Ctx, text, newLine))
                    oldLine++
                    newLine++
                }
            }
        }
    }
    return out
}

/** Past this many lines a diff is cut and says so; the rest is one tap away on the PC. */
private const val MAX_DIFF_LINES = 1500

/**
 * A file's hunks on the console's black: numbered lines, additions in green on a faint green
 * wash, removals in red on a faint red wash, Geist Mono 12. Long lines scroll sideways.
 */
@Composable
fun DiffBlock(hunks: List<Hunk>, modifier: Modifier = Modifier) {
    val c = Relay.colors
    val all = remember(hunks) { diffLines(hunks) }
    val lines = if (all.size > MAX_DIFF_LINES) all.subList(0, MAX_DIFF_LINES) else all
    val style = codeTextStyle.copy(fontSize = 12.sp, lineHeight = 16.sp)
    val cell = monoCellPx(style)
    val density = LocalDensity.current
    val longest = lines.maxOfOrNull { it.text.length }?.coerceAtMost(MAX_COLUMNS) ?: 0
    val width: Dp = with(density) { (cell * (longest + 8)).toDp() } + 52.dp
    val hs = rememberScrollState()
    Column(modifier.fillMaxWidth().background(c.screen)) {
        Box(Modifier.horizontalScroll(hs)) {
            Column(Modifier.width(width)) {
                for (line in lines) DiffRow(line, style, width)
            }
        }
        if (all.size > lines.size) {
            BasicText(
                "${all.size - lines.size} more lines. Open the diff on the PC to see them.",
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                style = Relay.type.caption.copy(color = c.ink3),
            )
        }
    }
}

private const val MAX_COLUMNS = 2000

internal val ADD_TEXT = Color(0xFF73C991)
internal val DEL_TEXT = Color(0xFFE5675B)
internal val ADD_WASH = Color(0x1F73C991)
internal val DEL_WASH = Color(0x1FE5675B)

@Composable
private fun DiffRow(line: DiffLine, style: TextStyle, width: Dp) {
    val c = Relay.colors
    val wash = when (line.kind) {
        DiffKind.Add -> ADD_WASH
        DiffKind.Del -> DEL_WASH
        else -> Color.Transparent
    }
    val text = when (line.kind) {
        DiffKind.Add -> ADD_TEXT
        DiffKind.Del -> DEL_TEXT
        DiffKind.Head, DiffKind.Note -> c.ink3
        DiffKind.Ctx -> c.ink2
    }
    Row(Modifier.width(width).background(wash).padding(horizontal = 10.dp, vertical = 1.dp)) {
        BasicText(
            line.number?.toString().orEmpty(),
            modifier = Modifier.width(34.dp),
            style = style.copy(color = c.ink3, textAlign = TextAlign.End),
        )
        BasicText(
            text = AnnotatedString(line.text),
            modifier = Modifier.padding(start = 10.dp),
            style = style.copy(color = text),
            softWrap = false,
        )
    }
}
