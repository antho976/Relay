package com.quietsoftware.relay.ui.code

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.theme.Fonts
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Geist Mono at the desktop editor's 12.5, with a line of air. */
val codeTextStyle: TextStyle = TextStyle(fontFamily = Fonts.GeistMono, fontSize = 12.5.sp, lineHeight = 18.sp)

/** How a file's comments and words are read for colour. Light touch: a handful of token classes. */
enum class Lang { CLike, Hash, Plain }

fun languageOf(name: String): Lang {
    val lower = name.lowercase()
    val ext = lower.substringAfterLast('.', "")
    return when {
        ext in C_LIKE -> Lang.CLike
        ext in HASH_COMMENTS || lower in HASH_NAMES || lower.startsWith(".env") -> Lang.Hash
        else -> Lang.Plain
    }
}

private val C_LIKE = setOf(
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "kt", "kts", "java", "go", "c", "h", "cc", "cpp", "hpp",
    "cs", "swift", "scala", "dart", "json", "jsonc", "css", "scss", "zig", "php", "gradle",
)
private val HASH_COMMENTS = setOf("py", "pyi", "sh", "bash", "zsh", "fish", "toml", "yaml", "yml", "ini", "conf", "cfg", "rb", "env", "properties", "nu", "ps1")
private val HASH_NAMES = setOf("dockerfile", "makefile", ".gitignore")

private val KEYWORDS = setOf(
    "fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "return", "if", "else", "match", "for",
    "while", "loop", "const", "static", "trait", "where", "as", "in", "self", "Self", "super", "crate", "async",
    "await", "move", "ref", "dyn", "unsafe", "type", "val", "var", "fun", "class", "object", "interface",
    "import", "package", "export", "default", "function", "new", "this", "null", "true", "false", "void",
    "def", "from", "try", "catch", "throw", "finally", "break", "continue", "yield", "switch", "case",
    "extends", "implements", "private", "protected", "public", "override", "is", "not", "and", "or", "elif",
    "lambda", "pass", "with", "when", "sealed", "companion", "init", "suspend", "inline", "None", "True", "False",
)

private val KEYWORD_COLOR = Color(0xFFC979D6)
private val TYPE_COLOR = Color(0xFF3EC5CF)
private val STRING_COLOR = Color(0xFFE0B04A)
private val NUMBER_COLOR = Color(0xFFF2CF6B)
private val COMMENT_COLOR = Color(0xFF6E6E70)
private val FUNCTION_COLOR = Color(0xFF5B9CF6)

/** One line, coloured. Stateless on purpose: a multi-line string or comment is not carried across lines. */
fun highlight(line: String, lang: Lang): AnnotatedString {
    if (lang == Lang.Plain || line.isEmpty()) return AnnotatedString(line)
    return buildAnnotatedString {
        val n = line.length
        var i = 0
        while (i < n) {
            val ch = line[i]
            val next = if (i + 1 < n) line[i + 1] else ' '
            if ((ch == '/' && next == '/') || (lang == Lang.Hash && ch == '#')) {
                withStyle(SpanStyle(color = COMMENT_COLOR)) { append(line.substring(i)) }
                break
            }
            if (ch == '"' || ch == '\'' || ch == '`') {
                var j = i + 1
                while (j < n && line[j] != ch) {
                    if (line[j] == '\\') j++
                    j++
                }
                val end = minOf(j + 1, n)
                withStyle(SpanStyle(color = STRING_COLOR)) { append(line.substring(i, end)) }
                i = end
                continue
            }
            if (ch.isDigit() && (i == 0 || !isWordChar(line[i - 1]))) {
                var j = i
                while (j < n && (line[j].isLetterOrDigit() || line[j] == '.' || line[j] == '_')) j++
                withStyle(SpanStyle(color = NUMBER_COLOR)) { append(line.substring(i, j)) }
                i = j
                continue
            }
            if (ch.isLetter() || ch == '_') {
                var j = i
                while (j < n && isWordChar(line[j])) j++
                val word = line.substring(i, j)
                val color = when {
                    word in KEYWORDS -> KEYWORD_COLOR
                    word[0].isUpperCase() -> TYPE_COLOR
                    j < n && line[j] == '(' -> FUNCTION_COLOR
                    else -> null
                }
                if (color != null) withStyle(SpanStyle(color = color)) { append(word) } else append(word)
                i = j
                continue
            }
            append(ch)
            i++
        }
    }
}

private fun isWordChar(c: Char) = c.isLetterOrDigit() || c == '_'

/**
 * A text file, with line numbers that stay on the left while the lines scroll sideways. The
 * text is laid out in a lazy column of fixed width, so a long file costs what it shows. With
 * [mark] (a search hit's line) it opens a few lines above that line, and the line is marked.
 */
@Composable
fun CodeView(text: String, fileName: String, modifier: Modifier = Modifier, mark: Int? = null) {
    val c = Relay.colors
    // Nothing past MAX_COLUMNS can be scrolled to, so a minified file's one huge line is not laid out whole.
    val lines = remember(text) { text.removeSuffix("\n").split('\n').map { it.replace("\t", "    ").trimEnd('\r').take(MAX_COLUMNS + 3) } }
    val lang = remember(fileName) { languageOf(fileName) }
    val cell = monoCellPx(codeTextStyle)
    val density = LocalDensity.current
    val digits = lines.size.toString().length
    val longest = remember(lines) { lines.maxOf { it.length }.coerceAtMost(MAX_COLUMNS) }
    val gutter: Dp = with(density) { (cell * (digits + 2)).toDp() }
    val contentWidth: Dp = with(density) { (cell * (longest + 3)).toDp() } + gutter
    val hs = rememberScrollState()
    val list = rememberLazyListState(initialFirstVisibleItemIndex = mark?.let { (it - 1 - MARK_ABOVE).coerceIn(0, lines.lastIndex) } ?: 0)
    Box(modifier.fillMaxWidth().background(c.screen).horizontalScroll(hs)) {
        LazyColumn(Modifier.width(contentWidth).fillMaxHeight(), state = list, contentPadding = PaddingValues(vertical = 8.dp)) {
            items(lines.size) { i ->
                CodeLine(i + 1, lines[i], lang, gutter, hs, marked = i + 1 == mark)
            }
        }
    }
}

private const val MAX_COLUMNS = 2000

/** How many lines show above a marked line when the file opens at it. */
private const val MARK_ABOVE = 4

private const val IMAGE_MAX_SIDE = 4096

@Composable
private fun CodeLine(number: Int, text: String, lang: Lang, gutter: Dp, hs: ScrollState, marked: Boolean) {
    val c = Relay.colors
    val shown = remember(text, lang) { highlight(text, lang) }
    // A marked line is washed from edge to edge, gutter too, and its number is in ink.
    val ground = if (marked) c.wash else c.screen
    Box(Modifier.fillMaxWidth().then(if (marked) Modifier.background(ground) else Modifier)) {
        BasicText(
            text = shown,
            modifier = Modifier.padding(start = gutter + 8.dp, end = 16.dp),
            style = codeTextStyle.copy(color = c.ink),
            softWrap = false,
        )
        Box(
            Modifier.matchParentSize().width(gutter).offset { IntOffset(hs.value, 0) }.background(ground).padding(end = 8.dp),
            contentAlignment = Alignment.CenterEnd,
        ) {
            BasicText(number.toString(), style = codeTextStyle.copy(color = if (marked) c.ink else c.ink3))
        }
    }
}

/** An image the PC sent as bytes. Formats the phone cannot draw (SVG) say so. */
@Composable
fun ImageFile(base64: String, modifier: Modifier = Modifier) {
    val c = Relay.colors
    val bitmap by produceState<ImageBitmap?>(initialValue = null, base64) {
        value = withContext(Dispatchers.Default) {
            runCatching {
                val bytes = Base64.decode(base64, Base64.DEFAULT)
                // A small file can hold a huge image; drawn whole it overruns the canvas limit
                // (100 MB) and crashes, so it is read at most IMAGE_MAX_SIDE on its longer side.
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
                var sample = 1
                while (maxOf(bounds.outWidth, bounds.outHeight) / sample > IMAGE_MAX_SIDE) sample *= 2
                val options = BitmapFactory.Options().apply { inSampleSize = sample }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
            }.getOrNull()
        }
    }
    val bmp = bitmap
    Box(modifier.fillMaxSize().padding(16.dp), contentAlignment = Alignment.Center) {
        if (bmp != null) {
            Image(bitmap = bmp, contentDescription = null, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize())
        } else {
            Empty("file-image", "Can't draw this image", "The phone cannot show this kind of image. Open it on the PC.")
        }
    }
}
