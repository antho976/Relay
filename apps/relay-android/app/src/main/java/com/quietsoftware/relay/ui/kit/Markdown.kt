package com.quietsoftware.relay.ui.kit

import android.content.ClipData
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Palette
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.launch

/**
 * A fenced block drawn as something other than code: a thread's `chart` block becomes a chart
 * card. [draw] runs during composition and answers null to leave the block as code.
 */
fun interface Fence {
    fun draw(lang: String, body: String): (@Composable () -> Unit)?
}

/** The fences the Markdown under it draws; none by default, so every fenced block is code. */
val LocalFence = staticCompositionLocalOf<Fence?> { null }

/**
 * Markdown as the PC draws a thread's reply (relay_client::thread_view): paragraphs, headings,
 * bullet and numbered lists, block quotes, fenced code, GFM tables, rules, and inline bold,
 * italics, code and links. Parsed once per text; selectable as one run.
 */
@Composable
fun Markdown(text: String, modifier: Modifier = Modifier, style: TextStyle = Relay.type.body, color: Color = Relay.colors.ink) {
    val c = Relay.colors
    val blocks = remember(text, c) { parse(text, Inline(c)) }
    SelectionContainer(modifier) {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            for (b in blocks) MdBlock(b, style, color)
        }
    }
}

// ---- Blocks ----

private sealed interface Block {
    data class Para(val text: AnnotatedString) : Block
    data class Heading(val level: Int, val text: AnnotatedString) : Block
    /** Items in order; each knows its depth and, in a numbered run, its number. */
    data class Items(val items: List<Item>) : Block
    data class Quote(val blocks: List<Block>) : Block
    data class Code(val lang: String, val text: String) : Block
    /** The header row first. [numeric] marks the columns of figures, which align right. */
    data class Table(val rows: List<List<AnnotatedString>>, val numeric: List<Boolean>) : Block
    data object Rule : Block
}

private data class Item(val depth: Int, val marker: String, val ordered: Boolean, val text: AnnotatedString)

@Composable
private fun MdBlock(b: Block, style: TextStyle, color: Color) {
    val c = Relay.colors
    when (b) {
        is Block.Para -> BasicText(b.text, style = style.copy(color = color))
        is Block.Heading -> {
            val big = b.level <= 2
            BasicText(
                b.text,
                Modifier.padding(top = if (big) 6.dp else 2.dp),
                style = style.copy(color = color, fontSize = if (big) 17.sp else 15.sp, lineHeight = if (big) 23.sp else 21.sp, fontWeight = FontWeight.SemiBold),
            )
        }
        is Block.Items -> Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
            for (item in b.items) {
                Row(Modifier.padding(start = (item.depth * 18).dp)) {
                    BasicText(
                        item.marker,
                        Modifier.widthIn(min = if (item.ordered) 22.dp else 16.dp).padding(end = 4.dp),
                        style = style.copy(color = c.ink3, fontFeatureSettings = "tnum"),
                    )
                    BasicText(item.text, Modifier.weight(1f), style = style.copy(color = color))
                }
            }
        }
        // The bar is drawn, not measured: an intrinsic height would crash on a chart (a
        // SubcomposeLayout) quoted inside.
        is Block.Quote -> Column(
            Modifier.drawBehind { drawRoundRect(c.strong, size = Size(2.dp.toPx(), size.height), cornerRadius = CornerRadius(1.dp.toPx())) }.padding(start = 14.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            for (inner in b.blocks) MdBlock(inner, style, c.ink2)
        }
        is Block.Code -> {
            val custom = LocalFence.current?.draw(b.lang, b.text)
            if (custom != null) custom() else CodeBlock(b)
        }
        is Block.Table -> TableBlock(b, style, color)
        Block.Rule -> Hairline(Modifier.padding(vertical = 4.dp), c.edge)
    }
}

@Composable
private fun CodeBlock(b: Block.Code) {
    val c = Relay.colors
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    var copied by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxWidth().clip(Radii.key).background(c.raised).border(1.dp, c.edge, Radii.key)) {
        Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 2.dp), verticalAlignment = Alignment.CenterVertically) {
            T(b.lang.ifBlank { "code" }, Modifier.weight(1f), Relay.type.mono, c.ink3, maxLines = 1)
            Box(
                Modifier.size(TOUCH).clip(Radii.icon).clickable(role = Role.Button) {
                    scope.launch {
                        clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("code", b.text)))
                        copied = true
                        kotlinx.coroutines.delay(1400)
                        copied = false
                    }
                }.semantics { contentDescription = if (copied) "Copied" else "Copy the code" },
                contentAlignment = Alignment.Center,
            ) { Glyph(if (copied) "check" else "copy", 14.dp, c.ink3) }
        }
        Box(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(start = 12.dp, end = 12.dp, bottom = 11.dp)) {
            BasicText(b.text, style = Relay.type.code.copy(color = c.ink2), softWrap = false)
        }
    }
}

/**
 * A GFM table on a slab card: the header over a rule, figures right-aligned in Geist Mono, and a
 * scroller of its own when it is wider than the column. Long cells wrap at about 260dp.
 */
@Composable
private fun TableBlock(b: Block.Table, style: TextStyle, color: Color) {
    val c = Relay.colors
    val head = Relay.type.caption.copy(color = c.ink3, fontWeight = FontWeight.SemiBold)
    val cell = style.copy(color = color, fontSize = 14.sp, lineHeight = 20.sp)
    val figure = Relay.type.mono.copy(color = color, fontSize = 13.sp, lineHeight = 20.sp, fontFeatureSettings = "tnum")
    val columns = b.rows.maxOf { it.size }
    val rows = b.rows.size
    Box(Modifier.clip(Radii.card).background(c.slab).border(1.dp, c.edge, Radii.card).horizontalScroll(rememberScrollState())) {
        Layout(
            content = {
                b.rows.forEachIndexed { r, row ->
                    for (col in 0 until columns) {
                        val num = b.numeric.getOrElse(col) { false }
                        BasicText(row.getOrNull(col) ?: AnnotatedString(""), style = if (r == 0) head else if (num) figure else cell)
                    }
                }
                Box(Modifier.background(c.lineSubtle))
            },
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
        ) { measurables, _ ->
            val gap = 18.dp.roundToPx()
            val rowGap = 7.dp.roundToPx()
            val cap = 260.dp.roundToPx()
            val cellCount = rows * columns
            val widths = IntArray(columns) { col ->
                (0 until rows).maxOf { r -> measurables[r * columns + col].maxIntrinsicWidth(Constraints.Infinity) }.coerceIn(1, cap)
            }
            val placeables = (0 until cellCount).map { i -> measurables[i].measure(Constraints(maxWidth = widths[i % columns])) }
            val heights = IntArray(rows) { r -> (0 until columns).maxOf { col -> placeables[r * columns + col].height } }
            val width = widths.sum() + gap * (columns - 1)
            val ruled = rows > 1
            val rule = measurables[cellCount].measure(Constraints.fixed(width, if (ruled) 1.dp.roundToPx() else 0))
            val height = heights.sum() + rowGap * (rows - 1) + (if (ruled) rowGap + rule.height else 0)
            layout(width, height) {
                var y = 0
                for (r in 0 until rows) {
                    var x = 0
                    for (col in 0 until columns) {
                        val p = placeables[r * columns + col]
                        p.placeRelative(if (b.numeric.getOrElse(col) { false }) x + widths[col] - p.width else x, y)
                        x += widths[col] + gap
                    }
                    y += heights[r] + rowGap
                    if (r == 0 && ruled) {
                        rule.placeRelative(0, y - (rowGap + 1) / 2)
                        y += rowGap + rule.height
                    }
                }
            }
        }
    }
}

// ---- Parsing ----

/** The inline styles, from the palette: code on a wash, links underlined. */
private class Inline(c: Palette) {
    val code = SpanStyle(fontFamily = Fonts.GeistMono, fontSize = 0.88.em, background = c.wash)
    val bold = SpanStyle(fontWeight = FontWeight.SemiBold)
    val italic = SpanStyle(fontStyle = FontStyle.Italic)
    val strike = SpanStyle(textDecoration = TextDecoration.LineThrough)
    val link = TextLinkStyles(style = SpanStyle(textDecoration = TextDecoration.Underline))
}

private fun parse(text: String, st: Inline): List<Block> = blocks(text.replace("\r\n", "\n").lines(), st)

private fun blocks(lines: List<String>, st: Inline): List<Block> {
    val out = ArrayList<Block>()
    val para = ArrayList<String>()
    fun flush() {
        if (para.isNotEmpty()) {
            out += Block.Para(inline(para.joinToString("\n") { it.trim() }, st))
            para.clear()
        }
    }
    var i = 0
    while (i < lines.size) {
        val line = lines[i]
        val t = line.trim()
        val fence = fenceOf(t)
        if (fence != null) {
            flush()
            val lang = t.removePrefix(fence).trim().substringBefore(' ')
            val code = ArrayList<String>()
            i++
            while (i < lines.size && !lines[i].trimStart().startsWith(fence)) {
                code += lines[i]
                i++
            }
            out += Block.Code(lang, code.joinToString("\n").trimIndent())
            i++
            continue
        }
        if (t.isEmpty()) {
            flush()
            i++
            continue
        }
        if (t.startsWith('#')) {
            val level = t.takeWhile { it == '#' }.length
            if (level <= 6 && t.length > level && t[level] == ' ') {
                flush()
                out += Block.Heading(level, inline(t.substring(level).trim().trimEnd('#').trim(), st))
                i++
                continue
            }
        }
        if (isRule(t)) {
            flush()
            out += Block.Rule
            i++
            continue
        }
        if (t.startsWith('>')) {
            flush()
            val quoted = ArrayList<String>()
            while (i < lines.size && lines[i].trim().startsWith('>')) {
                quoted += lines[i].trim().removePrefix(">").removePrefix(" ")
                i++
            }
            out += Block.Quote(blocks(quoted, st))
            continue
        }
        if (t.contains('|') && i + 1 < lines.size && isTableRule(lines[i + 1])) {
            flush()
            val raw = arrayListOf(cells(line))
            i += 2
            while (i < lines.size && lines[i].contains('|') && lines[i].isNotBlank()) {
                raw += cells(lines[i])
                i++
            }
            val width = raw.maxOf { it.size }
            val numeric = List(width) { col ->
                val body = raw.drop(1).mapNotNull { it.getOrNull(col) }.filter { it.isNotBlank() }
                body.isNotEmpty() && body.all(::isFigure)
            }
            out += Block.Table(raw.map { row -> row.map { inline(it, st) } }, numeric)
            continue
        }
        if (bullet(line) != null || numbered(line) != null) {
            flush()
            val items = ArrayList<Item>()
            val indents = ArrayList<Int>()
            val texts = ArrayList<StringBuilder>()
            val markers = ArrayList<Triple<Int, String, Boolean>>()
            // A numbered run counts up from its first number, as the PC's does, where "1." on
            // every item (as models often write them) read 1, 1, 1.
            val next = HashMap<Int, Int>()
            while (i < lines.size) {
                val l = lines[i]
                val b = bullet(l)
                val n = numbered(l)
                if (b != null || n != null) {
                    val indent = l.length - l.trimStart().length
                    while (indents.isNotEmpty() && indents.last() > indent) indents.removeAt(indents.lastIndex)
                    if (indents.isEmpty() || indents.last() < indent) indents += indent
                    val depth = (indents.size - 1).coerceAtMost(3)
                    // Back at this depth, the lists nested deeper are done: the next one counts afresh.
                    next.keys.removeAll { it > depth }
                    markers += if (n != null) {
                        val k = next[depth] ?: n.first
                        next[depth] = k + 1
                        Triple(depth, "$k.", true)
                    } else {
                        next.remove(depth)
                        Triple(depth, BULLETS[depth], false)
                    }
                    texts += StringBuilder((n?.second ?: b).orEmpty().trim())
                } else if (l.isNotBlank() && (l.startsWith("  ") || l.startsWith("\t")) && texts.isNotEmpty()) {
                    texts.last().append(' ').append(l.trim())
                } else if (l.isBlank() && i + 1 < lines.size && (bullet(lines[i + 1]) != null || numbered(lines[i + 1]) != null) && (lines[i + 1].startsWith(" ") || lines[i + 1].startsWith("\t"))) {
                    // A blank line between an item and its indented sub-items keeps the list going.
                } else {
                    break
                }
                i++
            }
            markers.forEachIndexed { k, (depth, marker, ordered) -> items += Item(depth, marker, ordered, inline(texts[k].toString(), st)) }
            out += Block.Items(items)
            continue
        }
        para += line
        i++
    }
    flush()
    return out
}

private val BULLETS = listOf("•", "◦", "▪", "▫")

/** The fence a code block opens with, ``` or ~~~; the same fence closes it. */
internal fun fenceOf(t: String): String? = when {
    t.startsWith("```") -> "```"
    t.startsWith("~~~") -> "~~~"
    else -> null
}

private fun isRule(t: String): Boolean {
    val compact = t.replace(" ", "")
    return compact.length >= 3 && (compact.all { it == '-' } || compact.all { it == '*' } || compact.all { it == '_' })
}

private fun bullet(line: String): String? {
    val t = line.trimStart()
    for (m in listOf("- ", "* ", "+ ", "• ")) if (t.startsWith(m)) return t.substring(m.length).let { s ->
        // A task list's box reads as a mark.
        when {
            s.startsWith("[ ] ") -> "☐ " + s.substring(4)
            s.startsWith("[x] ") || s.startsWith("[X] ") -> "☑ " + s.substring(4)
            else -> s
        }
    }
    return null
}

private fun numbered(line: String): Pair<Int, String>? {
    val t = line.trimStart()
    val digits = t.takeWhile { it.isDigit() }.length
    if (digits == 0 || digits > 4) return null
    val rest = t.substring(digits)
    val text = when {
        rest.startsWith(". ") -> rest.substring(2)
        rest.startsWith(") ") -> rest.substring(2)
        else -> return null
    }
    return t.substring(0, digits).toInt() to text
}

private fun cells(line: String): List<String> =
    line.trim().removePrefix("|").removeSuffix("|").split('|').map { it.trim() }

private fun isTableRule(line: String): Boolean {
    val t = line.trim()
    return t.contains('-') && t.contains('|') && t.all { it == '|' || it == '-' || it == ':' || it == ' ' }
}

/** Whether a cell is a figure ("$1,284.50", "−$96", "45%", "12"): it aligns right. */
private fun isFigure(cell: String): Boolean {
    val plain = cell.replace("**", "").replace("`", "").trim()
    var t = plain.trimStart('−', '-', '+', '(').trimEnd(')')
    t = t.trimStart('$', '€', '£').trimEnd('%', '$', '€').trim()
    t = t.removePrefix("CA$").removePrefix("US$").trim()
    return t.isNotEmpty() && t[0].isDigit() && t.all { it.isDigit() || it == ',' || it == '.' || it == ' ' || it == ' ' || it == ' ' }
}

private const val ESCAPABLE = "\\`*_{}[]()#+-.!|~>"

/** Inline Markdown: `**bold**`, `*italic*` or `_italic_`, `~~struck~~`, `` `code` ``, `[label](https://…)` and bare links. */
private fun inline(src: String, st: Inline): AnnotatedString = buildAnnotatedString { inlineInto(src, st) }

private fun AnnotatedString.Builder.inlineInto(src: String, st: Inline) {
    var i = 0
    val n = src.length
    while (i < n) {
        val ch = src[i]
        if (ch == '\\' && i + 1 < n && src[i + 1] in ESCAPABLE) {
            append(src[i + 1])
            i += 2
            continue
        }
        if (ch == '`') {
            val end = src.indexOf('`', i + 1)
            if (end > i) {
                withStyle(st.code) { append(" " + src.substring(i + 1, end) + " ") }
                i = end + 1
                continue
            }
        }
        if ((ch == '*' || ch == '_') && i + 1 < n && src[i + 1] == ch) {
            val pair = "$ch$ch"
            val end = src.indexOf(pair, i + 2)
            if (end > i + 2) {
                withStyle(st.bold) { inlineInto(src.substring(i + 2, end), st) }
                i = end + 2
                continue
            }
        }
        if (ch == '~' && i + 1 < n && src[i + 1] == '~') {
            val end = src.indexOf("~~", i + 2)
            if (end > i + 2) {
                withStyle(st.strike) { inlineInto(src.substring(i + 2, end), st) }
                i = end + 2
                continue
            }
        }
        if ((ch == '*' || ch == '_') && i + 1 < n && !src[i + 1].isWhitespace()) {
            // `_` inside a word (snake_case) is not emphasis.
            val wordBefore = i > 0 && src[i - 1].isLetterOrDigit()
            if (!(ch == '_' && wordBefore)) {
                val end = src.indexOf(ch, i + 1)
                if (end > i + 1 && !src[end - 1].isWhitespace()) {
                    val wordAfter = end + 1 < n && src[end + 1].isLetterOrDigit()
                    if (!(ch == '_' && wordAfter)) {
                        withStyle(st.italic) { inlineInto(src.substring(i + 1, end), st) }
                        i = end + 1
                        continue
                    }
                }
            }
        }
        if (ch == '[') {
            val close = src.indexOf("](", i + 1)
            if (close > i) {
                val end = src.indexOf(')', close + 2)
                if (end > close) {
                    val url = src.substring(close + 2, end).trim()
                    if (url.startsWith("https://") || url.startsWith("http://")) {
                        withLink(LinkAnnotation.Url(url, st.link)) { inlineInto(src.substring(i + 1, close), st) }
                        i = end + 1
                        continue
                    }
                }
            }
        }
        if ((src.startsWith("https://", i) || src.startsWith("http://", i)) && (i == 0 || !src[i - 1].isLetterOrDigit())) {
            var end = i
            while (end < n && !src[end].isWhitespace() && src[end] != '<' && src[end] != '>') end++
            while (end > i && src[end - 1] in ".,;:!?)'\"") end--
            val url = src.substring(i, end)
            withLink(LinkAnnotation.Url(url, st.link)) { append(url) }
            i = end
            continue
        }
        append(ch)
        i++
    }
}
