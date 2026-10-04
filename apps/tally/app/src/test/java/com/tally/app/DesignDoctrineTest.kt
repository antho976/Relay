package com.tally.app

import com.tally.app.ui.common.CategoryIcons
import com.tally.core.Defaults
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.File

/**
 * The design laws a static read can hold, checked over every Kotlin file under ui/. Code rules
 * read the source with comments and string literals blanked out, so a comment that names a banned
 * API never trips them; copy rules read only the decoded string literals, escapes included.
 * A failure lists every offending file:line.
 */
class DesignDoctrineTest {

    // ── Code rules ───────────────────────────────────────────────────────────

    @Test fun typeSizesComeFromTheTypeScaleOnly() {
        assertNoCode("a fontSize at a call site (use MaterialTheme.typography)", Regex("""\bfontSize\s*="""), outsideTheme = true)
    }

    @Test fun coloursComeFromTheSchemeOnly() {
        assertNoCode("a raw Color(0x...) outside ui/theme (use MaterialTheme.colorScheme or categoryColor)", Regex("""\bColor\s*\(\s*0x"""), outsideTheme = true)
    }

    @Test fun noHairlineDividers() {
        assertNoCode("a divider (sections are separated by space and panels)", Regex("""\b(HorizontalDivider|VerticalDivider)\b|\bDivider\s*\("""))
    }

    @Test fun screensNameThemselvesWithPageTitleNotAnAppBar() {
        assertNoCode("a Material top app bar (use TopBar then PageTitle)", Regex("""TopAppBar\s*\("""))
    }

    @Test fun noGlobalScope() {
        assertNoCode("GlobalScope (work belongs to viewModelScope or the injected app scope)", Regex("""\bGlobalScope\b"""))
    }

    @Test fun userContentIsNeverCutToOneLine() {
        // The hub's tab labels are the app's own one-word copy at a fixed size, not user content.
        val allowed = setOf("nav/Hub.kt")
        assertNoCode("maxLines = 1 (user content wraps; it must survive 200%)", Regex("""\bmaxLines\s*=\s*1\b"""), skip = allowed)
    }

    @Test fun layoutUsesStartAndEndNeverLeftAndRight() {
        val offences = ArrayList<String>()
        sources.forEach { src ->
            val code = src.lexed.code
            // Modifier.padding(left = ..) / (right = ..), however the call is spread over lines.
            Regex("""\bpadding\s*\(""").findAll(code).forEach { m ->
                val args = balancedArgs(code, m.range.last)
                if (Regex("""\b(left|right)\s*=(?!=)""").containsMatchIn(args)) offences += src.at(m.range.first, "padding(left/right =)")
            }
            Regex("""\babsolutePadding\b|\bPaddingValues\.Absolute\b|\bAbsoluteAlignment\b|\bArrangement\.Absolute\b|\bTextAlign\.(Left|Right)\b""")
                .findAll(code).forEach { m -> offences += src.at(m.range.first, m.value) }
        }
        report("left/right where start/end mirrors for right-to-left", offences)
    }

    // ── Copy rules (string literals only) ────────────────────────────────────

    @Test fun copyHasNoExclamationMarks() {
        assertNoCopy("an exclamation mark in copy") { it.contains('!') }
    }

    @Test fun copyHasNoEmDashes() {
        assertNoCopy("an em dash in copy (use a comma, a colon or \" · \")") { it.contains('—') }
    }

    @Test fun copyNeverSaysOops() {
        val oops = Regex("""\boops\b""", RegexOption.IGNORE_CASE)
        assertNoCopy("\"Oops\" in copy (name the problem instead)") { oops.containsMatchIn(it) }
    }

    // ── Icon contract ────────────────────────────────────────────────────────

    @Test fun everyIconKeyHasAGlyph() {
        val drawn = CategoryIcons.all.keys
        val missing = (Defaults.iconKeys + Defaults.categories.map { it.icon }).distinct().filterNot { it in drawn }
        assertTrue("Icon keys with no glyph in CategoryIcons.all: $missing", missing.isEmpty())
    }

    @Test fun noTwoIconKeysDrawTheSameGlyph() {
        val byGlyph = CategoryIcons.all.entries.groupBy({ it.value.name }, { it.key }).filterValues { it.size > 1 }
        assertTrue("Keys sharing one glyph, so a picker shows twins: $byGlyph", byGlyph.isEmpty())
    }

    // ── The lexer is itself under test, so a rule cannot pass by reading nothing ──

    @Test fun theScanReadsTheWholeUiTree() {
        assertTrue("Only ${sources.size} ui files found under ${uiRoot.path}", sources.size >= 20)
        assertTrue(sources.any { it.rel == "home/HomeScreen.kt" })
        assertTrue(sources.any { it.rel.startsWith("theme/") })
    }

    @Test fun theLexerSeparatesCodeCommentsAndLiterals() {
        val text = """
            |// Never write TopAppBar( in a screen!
            |/* A block /* nested */ comment with Divider( and "quotes" */
            |val a = "Saved ${'$'}{if (!done) "late" else "on time"} today"
            |val b = '"'
            |val c = ""${'"'}Raw ${'$'}name with "quotes" inside""${'"'}
            |val d = "Escaped — dash \"and\" ${'$'} sign"
            |fun e() = padding(left = 4.dp)
        """.trimMargin()

        val lexed = DoctrineLexer(text).run()

        assertTrue("comments are blanked", "TopAppBar" !in lexed.code && "Divider" !in lexed.code)
        assertTrue("template code stays code", "!done" in lexed.code)
        assertTrue("code stays code", "padding(left = 4.dp)" in lexed.code)
        val literals = lexed.literals.map { it.text }
        assertEquals(
            listOf("Saved ", "late", "on time", " today", "\"", "Raw \$name with \"quotes\" inside", "Escaped — dash \"and\" \$ sign"),
            literals,
        )
        assertEquals(listOf(3, 3, 3, 3, 4, 5, 6), lexed.literals.map { lexed.lineOf(it.start) })
        assertTrue("no literal text leaks into code", "Saved" !in lexed.code && "Escaped" !in lexed.code)
    }

    // ── Machinery ────────────────────────────────────────────────────────────

    private class Source(val file: File, val rel: String) {
        val lexed: DoctrineLexer by lazy { DoctrineLexer(file.readText()).run() }
        fun at(index: Int, what: String) = "ui/$rel:${lexed.lineOf(index)}  $what"
    }

    private fun assertNoCode(rule: String, pattern: Regex, outsideTheme: Boolean = false, skip: Set<String> = emptySet()) {
        val offences = ArrayList<String>()
        sources
            .filter { !(outsideTheme && it.rel.startsWith("theme/")) && it.rel !in skip }
            .forEach { src -> pattern.findAll(src.lexed.code).forEach { m -> offences += src.at(m.range.first, m.value.trim()) } }
        report(rule, offences)
    }

    private fun assertNoCopy(rule: String, bad: (String) -> Boolean) {
        val offences = ArrayList<String>()
        sources.forEach { src ->
            src.lexed.literals.filter { bad(it.text) }.forEach { offences += src.at(it.start, "\"${it.text}\"") }
        }
        report(rule, offences)
    }

    private fun report(rule: String, offences: List<String>) {
        if (offences.isNotEmpty()) fail("Found $rule:\n" + offences.joinToString("\n"))
    }

    /** The text between the parenthesis opened at [open] and its match, nested calls included. */
    private fun balancedArgs(code: String, open: Int): String {
        var depth = 0
        for (k in open until code.length) {
            when (code[k]) {
                '(' -> depth++
                ')' -> { depth--; if (depth == 0) return code.substring(open + 1, k) }
            }
        }
        return code.substring(open + 1)
    }

    private companion object {
        /** Gradle runs unit tests from the module directory; an IDE may run them from the root. */
        val uiRoot: File by lazy {
            listOf("src/main/java/com/tally/app/ui", "app/src/main/java/com/tally/app/ui")
                .map(::File)
                .firstOrNull { it.isDirectory }
                ?: error("ui sources not found from ${File("").absolutePath}")
        }

        val sources: List<Source> by lazy {
            uiRoot.walkTopDown()
                .filter { it.isFile && it.extension == "kt" }
                .map { Source(it, it.relativeTo(uiRoot).invariantSeparatorsPath) }
                .sortedBy { it.rel }
                .toList()
        }
    }
}

/** A literal's decoded text and the source index where it starts. */
internal data class DoctrineLiteral(val start: Int, val text: String)

/**
 * Just enough of a Kotlin lexer for the doctrine: comments (nested), strings with escapes and
 * templates, raw strings, char literals and backticked names. [code] is the source with comments
 * and literal text replaced by spaces, newlines kept so offsets and lines still line up;
 * [literals] holds the decoded text of every literal segment, one segment per source line.
 */
internal class DoctrineLexer(private val src: String) {

    private val out = src.toCharArray()
    private val found = ArrayList<DoctrineLiteral>()
    private val lineStarts: IntArray = buildList {
        add(0)
        src.forEachIndexed { k, c -> if (c == '\n') add(k + 1) }
    }.toIntArray()
    private var i = 0

    lateinit var code: String
        private set
    val literals: List<DoctrineLiteral> get() = found

    fun run(): DoctrineLexer {
        scanCode(untilBrace = false)
        code = String(out)
        return this
    }

    fun lineOf(index: Int): Int {
        var lo = 0
        var hi = lineStarts.size - 1
        while (lo < hi) {
            val mid = (lo + hi + 1) / 2
            if (lineStarts[mid] <= index) lo = mid else hi = mid - 1
        }
        return lo + 1
    }

    private fun peek(ahead: Int): Char = if (i + ahead < src.length) src[i + ahead] else '\u0000'

    private fun blank(k: Int) { if (out[k] != '\n') out[k] = ' ' }

    private fun scanCode(untilBrace: Boolean) {
        var depth = 0
        while (i < src.length) {
            val c = src[i]
            when {
                c == '/' && peek(1) == '/' -> lineComment()
                c == '/' && peek(1) == '*' -> blockComment()
                c == '"' && peek(1) == '"' && peek(2) == '"' -> rawString()
                c == '"' -> string()
                c == '\'' -> charLiteral()
                c == '`' -> backticked()
                c == '{' -> { depth++; i++ }
                c == '}' -> {
                    if (untilBrace && depth == 0) { i++; return }
                    depth--
                    i++
                }
                else -> i++
            }
        }
    }

    private fun lineComment() {
        while (i < src.length && src[i] != '\n') { blank(i); i++ }
    }

    private fun blockComment() {
        var depth = 0
        while (i < src.length) {
            if (src[i] == '/' && peek(1) == '*') { depth++; blank(i); blank(i + 1); i += 2; continue }
            if (src[i] == '*' && peek(1) == '/') {
                depth--; blank(i); blank(i + 1); i += 2
                if (depth == 0) return
                continue
            }
            blank(i); i++
        }
    }

    private fun backticked() {
        i++
        while (i < src.length && src[i] != '`' && src[i] != '\n') i++
        if (i < src.length) i++
    }

    /** Appends one escape's decoded char to [sb], blanks it, and moves past it. */
    private fun escape(sb: StringBuilder) {
        val n = peek(1)
        val (decoded, length) = when (n) {
            'n' -> '\n' to 2
            't' -> '\t' to 2
            'r' -> '\r' to 2
            'b' -> '\b' to 2
            'u' -> (src.substring(i + 2, minOf(i + 6, src.length)).toIntOrNull(16)?.toChar() ?: 'u') to 6
            else -> n to 2
        }
        sb.append(decoded)
        for (k in i until minOf(i + length, src.length)) blank(k)
        i += length
    }

    private fun string() {
        blank(i); i++
        var sb = StringBuilder()
        var start = i
        fun flush() {
            if (sb.isNotEmpty()) found += DoctrineLiteral(start, sb.toString())
            sb = StringBuilder()
        }
        while (i < src.length) {
            val c = src[i]
            when {
                c == '\\' -> escape(sb)
                c == '$' && peek(1) == '{' -> {
                    flush()
                    blank(i); blank(i + 1); i += 2
                    scanCode(untilBrace = true)
                    start = i
                }
                c == '"' -> { blank(i); i++; flush(); return }
                c == '\n' -> { flush(); return }
                else -> { sb.append(c); blank(i); i++ }
            }
        }
        flush()
    }

    private fun rawString() {
        blank(i); blank(i + 1); blank(i + 2); i += 3
        var sb = StringBuilder()
        var start = i
        fun flush() {
            if (sb.isNotEmpty()) found += DoctrineLiteral(start, sb.toString())
            sb = StringBuilder()
        }
        while (i < src.length) {
            val c = src[i]
            when {
                c == '"' && peek(1) == '"' && peek(2) == '"' -> {
                    var run = 0
                    while (i + run < src.length && src[i + run] == '"') run++
                    repeat(run - 3) { sb.append('"') }
                    for (k in i until i + run) blank(k)
                    i += run
                    flush()
                    return
                }
                c == '$' && peek(1) == '{' -> {
                    flush()
                    blank(i); blank(i + 1); i += 2
                    scanCode(untilBrace = true)
                    start = i
                }
                c == '\n' -> { flush(); i++; start = i }
                else -> { sb.append(c); blank(i); i++ }
            }
        }
        flush()
    }

    private fun charLiteral() {
        blank(i); i++
        val sb = StringBuilder()
        val start = i
        if (i < src.length && src[i] == '\\') escape(sb) else if (i < src.length) { sb.append(src[i]); blank(i); i++ }
        if (i < src.length && src[i] == '\'') { blank(i); i++ }
        if (sb.isNotEmpty()) found += DoctrineLiteral(start, sb.toString())
    }
}
