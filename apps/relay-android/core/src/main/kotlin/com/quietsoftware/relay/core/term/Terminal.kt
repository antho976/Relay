package com.quietsoftware.relay.core.term

/**
 * A VT/xterm screen fed the raw output of a PTY on the PC: the agents' TUIs (Claude Code, Codex)
 * draw into it and the app renders [row] / [scrollbackRow].
 *
 * Not thread-safe: feed and read on one thread. Only [version] and [synchronizedOutput] may be
 * polled from another.
 *
 * Queries (DSR, DA, DECRQM) are answered by the PC's own terminal, so this one stays silent unless
 * [onReply] is set.
 */
class Terminal(cols: Int, rows: Int, scrollbackLimit: Int = 5000) {
    var cols: Int = cols.coerceAtLeast(1)
        private set
    var rows: Int = rows.coerceAtLeast(1)
        private set
    val scrollbackLimit: Int = scrollbackLimit.coerceAtLeast(0)

    /** Moves on every feed, resize and reset; the renderer redraws when it differs from what it last drew. */
    @Volatile var version: Long = 0L
        private set

    /** Receives what a real terminal would answer to DSR 5/6 and DA1. Null: answer nothing. */
    var onReply: ((String) -> Unit)? = null

    var cursorRow: Int = 0
        private set
    var cursorCol: Int = 0
        private set
    /** DECTCEM, `?25`. */
    var cursorVisible: Boolean = true
        private set
    /** `?2004`. */
    var bracketedPaste: Boolean = false
        private set
    /** DECCKM, `?1`: arrows are sent as `ESC O A` etc. */
    var applicationCursorKeys: Boolean = false
        private set
    /** DECKPAM / DECKPNM. */
    var applicationKeypad: Boolean = false
        private set
    /** OSC 0 / OSC 2. */
    var title: String = ""
        private set
    /**
     * DEC mode 2026: the program is mid-frame and the renderer should hold its last one. Released
     * by the program, or here once [SYNC_LIMIT] bytes arrive without a release.
     */
    @Volatile var synchronizedOutput: Boolean = false
        private set
    /** Mouse reporting the program asked for: 0, 9 (X10), 1000, 1002 or 1003. */
    var mouseTracking: Int = 0
        private set
    /** Mouse encoding: 0 (X10 bytes), 1005, 1006 (SGR) or 1015. */
    var mouseEncoding: Int = 0
        private set
    /** `?1004`. */
    var focusReporting: Boolean = false
        private set

    /** `?1049` / `?1047` / `?47`. */
    val altScreen: Boolean get() = screen === alt

    /** Lines in scrollback; only the main screen accumulates any. */
    var scrollbackSize: Int = 0
        private set

    /**
     * Lines that have left the top of a full scrollback for good. A row index (as [lineAt]
     * counts) taken earlier names the same line now once this has grown by as much is taken
     * off it: what keeps a selection on its text while output goes on.
     */
    var dropped: Long = 0
        private set

    /** Bumps when the scrollback is cleared or the terminal reset: row indices taken before name nothing now. */
    var cleared: Int = 0
        private set

    /** Throw instead of recovering when a bug surfaces mid-feed; tests turn it on. */
    internal var strict = false

    private class SavedCursor {
        var row = 0
        var col = 0
        var pendingWrap = false
        var pen = Style.DEFAULT
        var origin = false
        val g = IntArray(4)
        var gl = 0

        fun clear() {
            row = 0; col = 0; pendingWrap = false; pen = Style.DEFAULT; origin = false; g.fill(CS_ASCII); gl = 0
        }
    }

    private class Screen(rows: Int, cols: Int) {
        var lines: Array<Line> = Array(rows) { Line(cols) }
        val saved = SavedCursor()
    }

    private val main = Screen(this.rows, this.cols)
    private val alt = Screen(this.rows, this.cols)
    private var screen = main

    /** xterm's "wrap pending": the cursor sits on the last column and wraps when the next char arrives. */
    private var pendingWrap = false
    private var pen = Style.DEFAULT
    private var top = 0
    private var bottom = this.rows - 1

    private var autowrap = true
    private var originMode = false
    private var insertMode = false
    private var newlineMode = false

    private val g = IntArray(4)
    private var gl = 0
    private var charset = CS_ASCII

    private var tabs = defaultTabs(this.cols)
    private var lastPrinted = 0
    /** The last thing printed was a ZWJ: the next pictograph joins its cell. */
    private var joinNext = false
    private var syncBytes = 0L
    private var extUsed = 0

    private var u8Need = 0
    private var u8Cp = 0
    private var u8Lo = 0x80
    private var u8Hi = 0xBF

    private var sb = arrayOfNulls<Line>(0)
    private var sbStart = 0
    private var scratch = arrayOfNulls<Line>(this.rows)

    private val parser = Parser(object : ParserSink {
        override fun print(cp: Int) = printChar(cp)
        override fun execute(code: Int) = executeControl(code)
        override fun csi(p: Parser, final: Int) = dispatchCsi(p, final)
        override fun esc(final: Int, intermediates: Int) = dispatchEsc(final, intermediates)
        override fun osc(data: CharSequence) = dispatchOsc(data)
    })

    // ---- reading ----

    /** A visible row of the active screen, 0 until [rows]. */
    fun row(index: Int): Line = screen.lines[index]

    /** A scrollback line, 0 = oldest. */
    fun scrollbackRow(index: Int): Line {
        if (index !in 0 until scrollbackSize) throw IndexOutOfBoundsException("scrollback row $index of $scrollbackSize")
        return sb[(sbStart + index) % sb.size]!!
    }

    /** Scrollback and screen as one list, 0 = oldest scrollback line, [scrollbackSize] = the screen's first row. */
    fun lineAt(index: Int): Line = if (index < scrollbackSize) scrollbackRow(index) else screen.lines[index - scrollbackSize]

    /**
     * Plain text of the last [lines] lines of scrollback + screen, counted back from the last one
     * with text in it. Wrapped lines are joined; trailing spaces are trimmed.
     */
    fun plainText(lines: Int = 800): String {
        var end = scrollbackSize + rows - 1
        while (end >= 0 && isBlank(lineAt(end))) end--
        if (end < 0 || lines <= 0) return ""
        return text((end - lines + 1).coerceAtLeast(0), 0, end, Int.MAX_VALUE - 1)
    }

    /**
     * The text from ([startRow], [startCol]) to ([endRow], [endCol]) inclusive, in reading order,
     * rows indexed as in [lineAt]. Wrapped lines are joined; trailing spaces are trimmed.
     */
    fun textBetween(startRow: Int, startCol: Int, endRow: Int, endCol: Int): String {
        var sr = startRow; var sc = startCol; var er = endRow; var ec = endCol
        if (sr > er || (sr == er && sc > ec)) { sr = endRow; sc = endCol; er = startRow; ec = startCol }
        val last = scrollbackSize + rows - 1
        if (er < 0 || sr > last) return ""
        if (sr < 0) { sr = 0; sc = 0 }
        if (er > last) { er = last; ec = Int.MAX_VALUE - 1 }
        return text(sr, sc, er, ec)
    }

    private fun text(sr: Int, sc: Int, er: Int, ec: Int): String {
        val out = StringBuilder()
        for (i in sr..er) {
            val line = lineAt(i)
            val len = line.length
            var from = if (i == sr) sc.coerceIn(0, len) else 0
            if (from in 1 until len && line.cp[from] == Line.WIDE_TAIL) from--
            val to = minOf(if (i == er) ec.coerceAtLeast(-1) + 1 else len, line.contentEnd())
            for (c in from until to) {
                val v = line.cp[c]
                if (v == Line.WIDE_TAIL) continue
                if (v == 0) out.append(' ') else out.appendCodePoint(v)
                line.ex?.get(c)?.let(out::append)
            }
            if (i < er && !line.wrapped) {
                trimEnd(out)
                out.append('\n')
            }
        }
        trimEnd(out)
        return out.toString()
    }

    // ---- feeding ----

    /** Feeds raw PTY bytes. A UTF-8 sequence split across calls is carried over. */
    fun feed(bytes: ByteArray, offset: Int = 0, length: Int = bytes.size - offset) {
        if (offset < 0 || length < 0 || offset > bytes.size - length) {
            throw IndexOutOfBoundsException("offset $offset, length $length, size ${bytes.size}")
        }
        val end = offset + length
        val syncWasOpen = synchronizedOutput
        var i = offset
        while (i < end) {
            try {
                while (i < end) {
                    val b = bytes[i].toInt() and 0xFF
                    if (b < 0x80 && u8Need == 0) parser.advance(b) else decode(b)
                    i++
                }
            } catch (e: RuntimeException) {
                if (strict) throw e
                recover()
                i++
            }
        }
        if (synchronizedOutput && syncWasOpen) {
            syncBytes += length
            if (syncBytes > SYNC_LIMIT) synchronizedOutput = false
        }
        if (length > 0) version++
    }

    fun feed(text: String) {
        val b = text.toByteArray(Charsets.UTF_8)
        feed(b, 0, b.size)
    }

    /** Malformed input becomes U+FFFD, one per maximal invalid subpart (as WHATWG decoders do). */
    private fun decode(b: Int) {
        if (u8Need > 0) {
            if (b in u8Lo..u8Hi) {
                u8Cp = (u8Cp shl 6) or (b and 0x3F)
                u8Lo = 0x80; u8Hi = 0xBF
                if (--u8Need == 0) parser.advance(u8Cp)
                return
            }
            u8Need = 0
            parser.advance(0xFFFD)
        }
        when (b) {
            in 0x00..0x7F -> parser.advance(b)
            in 0xC2..0xDF -> { u8Need = 1; u8Cp = b and 0x1F; u8Lo = 0x80; u8Hi = 0xBF }
            in 0xE0..0xEF -> {
                u8Need = 2; u8Cp = b and 0x0F
                u8Lo = if (b == 0xE0) 0xA0 else 0x80; u8Hi = if (b == 0xED) 0x9F else 0xBF
            }
            in 0xF0..0xF4 -> {
                u8Need = 3; u8Cp = b and 0x07
                u8Lo = if (b == 0xF0) 0x90 else 0x80; u8Hi = if (b == 0xF4) 0x8F else 0xBF
            }
            else -> parser.advance(0xFFFD)
        }
    }

    private fun recover() {
        parser.reset()
        cursorRow = cursorRow.coerceIn(0, rows - 1)
        cursorCol = cursorCol.coerceIn(0, cols - 1)
        pendingWrap = false
        if (top !in 0 until rows || bottom !in top + 1 until rows) { top = 0; bottom = rows - 1 }
    }

    // ---- size and epochs ----

    /**
     * Resizes without reflow: rows and lines are truncated or padded. When the screen loses rows
     * below the cursor's, lines leave from the top (into scrollback on the main screen) so the
     * cursor's line stays put, as xterm and VTE do; the program then redraws.
     */
    fun resize(cols: Int, rows: Int) {
        val nc = cols.coerceAtLeast(1)
        val nr = rows.coerceAtLeast(1)
        if (nc == this.cols && nr == this.rows) return
        resizeScreen(main, nc, nr)
        resizeScreen(alt, nc, nr)
        val oldCols = this.cols
        this.cols = nc
        this.rows = nr
        cursorRow = cursorRow.coerceIn(0, nr - 1)
        cursorCol = cursorCol.coerceIn(0, nc - 1)
        pendingWrap = false
        top = 0
        bottom = nr - 1
        tabs = defaultTabs(nc).also { t -> tabs.copyInto(t, 0, 0, minOf(oldCols, nc)) }
        scratch = arrayOfNulls(nr)
        version++
    }

    private fun resizeScreen(s: Screen, nc: Int, nr: Int) {
        val old = s.lines
        val anchor = if (s === screen) cursorRow else s.saved.row
        val drop = (anchor - nr + 1).coerceIn(0, old.size)
        if (s === main) for (i in 0 until drop) pushScrollback(old[i])
        s.lines = Array(nr) { i ->
            val src = drop + i
            if (src < old.size) old[src].also { it.resize(nc) } else Line(nc)
        }
        if (s === screen) cursorRow -= drop
        s.saved.row = (s.saved.row - drop).coerceIn(0, nr - 1)
        s.saved.col = s.saved.col.coerceIn(0, nc - 1)
    }

    /** Forgets everything, scrollback and title included: a new PTY epoch. */
    fun reset() {
        resetState()
        clearScrollback()
        title = ""
        u8Need = 0
        parser.reset()
        version++
    }

    /** RIS: everything but scrollback and title, which xterm keeps too. */
    private fun resetState() {
        for (s in arrayOf(main, alt)) {
            for (l in s.lines) l.reset(cols, Style.DEFAULT)
            s.saved.clear()
        }
        screen = main
        cursorRow = 0; cursorCol = 0; pendingWrap = false
        pen = Style.DEFAULT
        top = 0; bottom = rows - 1
        cursorVisible = true; bracketedPaste = false; applicationCursorKeys = false; applicationKeypad = false
        synchronizedOutput = false; syncBytes = 0
        mouseTracking = 0; mouseEncoding = 0; focusReporting = false
        autowrap = true; originMode = false; insertMode = false; newlineMode = false
        g.fill(CS_ASCII); gl = 0; charset = CS_ASCII
        tabs = defaultTabs(cols)
        lastPrinted = 0; joinNext = false
    }

    private fun clearScrollback() {
        sb = arrayOfNulls(0)
        sbStart = 0
        scrollbackSize = 0
        cleared++
    }

    // ---- printing ----

    private fun printChar(c0: Int) {
        var c = c0
        if (charset != CS_ASCII && c < 0x7F) c = mapCharset(c)
        val w = if (c < 0x300) 1 else CharWidth.of(c)
        if (w == 0) {
            joinNext = attach(c) && c == ZWJ
            return
        }
        if (joinNext) {
            joinNext = false
            if (c >= 0x2190 && attach(c)) return
        }
        if (c in 0x1F3FB..0x1F3FF && prevIsEmoji() && attach(c)) return
        put(c, if (w > cols) 1 else w)
    }

    private fun put(c: Int, w: Int) {
        if (pendingWrap) {
            pendingWrap = false
            if (autowrap) {
                screen.lines[cursorRow].wrapped = true
                cursorCol = 0
                index()
            }
        }
        if (cursorCol + w > cols) {
            // A wide char at the last column: blank that column and wrap first.
            if (autowrap) {
                val line = screen.lines[cursorRow]
                line.erase(cursorCol, cols, Style.erased(pen))
                line.wrapped = true
                cursorCol = 0
                index()
            } else {
                cursorCol = cols - w
            }
        }
        val line = screen.lines[cursorRow]
        if (insertMode) line.insertBlanks(cursorCol, w, Style.erased(pen))
        if (w == 2) line.putWide(cursorCol, c, pen) else line.put(cursorCol, c, pen)
        lastPrinted = c
        val next = cursorCol + w
        if (next >= cols) {
            cursorCol = cols - 1
            pendingWrap = autowrap
        } else {
            cursorCol = next
        }
    }

    /** The cell a zero-width char joins: the last one printed, left of the cursor. -1 if none. */
    private fun prevCell(): Int {
        val line = screen.lines[cursorRow]
        var col = if (pendingWrap) cursorCol else cursorCol - 1
        if (col >= 0 && line.cp[col] == Line.WIDE_TAIL) col--
        return if (col >= 0 && line.cp[col] > 0) col else -1
    }

    private fun attach(c: Int): Boolean {
        val col = prevCell()
        if (col < 0) return false
        screen.lines[cursorRow].appendExtra(col, String(Character.toChars(c)))
        return true
    }

    private fun prevIsEmoji(): Boolean {
        val col = prevCell()
        if (col < 0) return false
        val line = screen.lines[cursorRow]
        val base = line.cp[col]
        return col + 1 < line.length && line.cp[col + 1] == Line.WIDE_TAIL && (base >= 0x1F000 || base in 0x2600..0x27BF)
    }

    private fun mapCharset(c: Int): Int = when (charset) {
        CS_DEC -> if (c in 0x5F..0x7E) DEC_GRAPHICS[c - 0x5F] else c
        CS_UK -> if (c == '#'.code) 0xA3 else c
        else -> c
    }

    private fun updateCharset() {
        charset = g[gl]
    }

    // ---- C0 / C1 ----

    private fun executeControl(code: Int) {
        joinNext = false
        when (code) {
            0x08 -> {
                pendingWrap = false
                if (cursorCol > 0) cursorCol--
            }
            0x09 -> tab(1)
            0x0A, 0x0B, 0x0C -> {
                index()
                if (newlineMode) cursorCol = 0
            }
            0x0D -> { cursorCol = 0; pendingWrap = false }
            0x0E -> { gl = 1; updateCharset() }
            0x0F -> { gl = 0; updateCharset() }
            0x84 -> index()
            0x85 -> { index(); cursorCol = 0 }
            0x88 -> tabs[cursorCol] = true
            0x8D -> reverseIndex()
        }
    }

    /** IND / LF: down a line, scrolling the region when on its bottom margin. */
    private fun index() {
        pendingWrap = false
        if (cursorRow == bottom) scrollUp(top, bottom, 1, screen === main && top == 0)
        else if (cursorRow < rows - 1) cursorRow++
    }

    /** RI: up a line, scrolling the region down when on its top margin. */
    private fun reverseIndex() {
        pendingWrap = false
        if (cursorRow == top) scrollDown(top, bottom, 1)
        else if (cursorRow > 0) cursorRow--
    }

    private fun tab(n: Int) {
        var c = cursorCol
        var k = n.coerceAtMost(cols)
        while (k-- > 0 && c < cols - 1) {
            c++
            while (c < cols - 1 && !tabs[c]) c++
        }
        if (c != cursorCol) { cursorCol = c; pendingWrap = false }
    }

    private fun backTab(n: Int) {
        var c = cursorCol
        var k = n.coerceAtMost(cols)
        while (k-- > 0 && c > 0) {
            c--
            while (c > 0 && !tabs[c]) c--
        }
        cursorCol = c
        pendingWrap = false
    }

    // ---- scrolling ----

    /**
     * Scrolls rows [t, b] up by [n0]. With [save] the rows leaving the top go to scrollback. xterm
     * (and VTE, xterm.js) save whenever the region's top is the screen's, not only for a full-screen
     * region; Codex relies on that, writing its history above its viewport through a `1;N` region.
     *
     * The row above the region no longer continues into it, so its wrap flag goes.
     */
    private fun scrollUp(t: Int, b: Int, n0: Int, save: Boolean) {
        val n = minOf(n0, b - t + 1)
        if (n <= 0) return
        val lines = screen.lines
        val blank = Style.erased(pen)
        if (t > 0) lines[t - 1].wrapped = false
        for (i in 0 until n) scratch[i] = lines[t + i]
        System.arraycopy(lines, t + n, lines, t, b - t + 1 - n)
        for (i in 0 until n) {
            var l = scratch[i]!!
            scratch[i] = null
            if (save) l = pushScrollback(l) ?: Line(cols)
            l.reset(cols, blank)
            lines[b - n + 1 + i] = l
        }
    }

    /** Scrolls rows [t, b] down by [n0]; the last row that moves loses the rows it continued into. */
    private fun scrollDown(t: Int, b: Int, n0: Int) {
        val n = minOf(n0, b - t + 1)
        if (n <= 0) return
        val lines = screen.lines
        val blank = Style.erased(pen)
        if (t > 0) lines[t - 1].wrapped = false
        if (b - n >= t) lines[b - n].wrapped = false
        for (i in 0 until n) scratch[i] = lines[b - n + 1 + i]
        System.arraycopy(lines, t, lines, t + n, b - t + 1 - n)
        for (i in 0 until n) {
            val l = scratch[i]!!
            scratch[i] = null
            l.reset(cols, blank)
            lines[t + i] = l
        }
    }

    /** Appends to the scrollback ring; returns the line it evicted (free for reuse), or null. */
    private fun pushScrollback(line: Line): Line? {
        val limit = scrollbackLimit
        if (limit == 0) {
            dropped++
            return line
        }
        if (scrollbackSize < limit) {
            if (scrollbackSize == sb.size) {
                val grown = arrayOfNulls<Line>(minOf(maxOf(64, sb.size * 2), limit))
                for (i in 0 until scrollbackSize) grown[i] = sb[(sbStart + i) % sb.size]
                sb = grown
                sbStart = 0
            }
            sb[(sbStart + scrollbackSize) % sb.size] = line
            scrollbackSize++
            return null
        }
        val evicted = sb[sbStart]
        sb[sbStart] = line
        sbStart = (sbStart + 1) % sb.size
        dropped++
        return evicted
    }

    // ---- ESC ----

    private fun dispatchEsc(final: Int, inter: Int) {
        joinNext = false
        when (inter) {
            0 -> when (final.toChar()) {
                '7' -> saveCursor()
                '8' -> restoreCursor()
                'D' -> index()
                'E' -> { index(); cursorCol = 0 }
                'H' -> tabs[cursorCol] = true
                'M' -> reverseIndex()
                'c' -> resetState()
                '=' -> applicationKeypad = true
                '>' -> applicationKeypad = false
                'Z' -> reply(DA1)
            }
            '#'.code -> if (final == '8'.code) alignmentTest()
            '('.code -> designate(0, final)
            ')'.code -> designate(1, final)
            '*'.code -> designate(2, final)
            '+'.code -> designate(3, final)
        }
    }

    private fun designate(slot: Int, final: Int) {
        g[slot] = when (final.toChar()) {
            '0' -> CS_DEC
            'A' -> CS_UK
            else -> CS_ASCII
        }
        updateCharset()
    }

    private fun saveCursor() {
        val s = screen.saved
        s.row = cursorRow; s.col = cursorCol; s.pendingWrap = pendingWrap
        s.pen = pen; s.origin = originMode
        g.copyInto(s.g); s.gl = gl
    }

    /** DECRC; with nothing saved it homes the cursor and resets the pen, as xterm does. */
    private fun restoreCursor() {
        val s = screen.saved
        cursorRow = s.row.coerceIn(0, rows - 1)
        cursorCol = s.col.coerceIn(0, cols - 1)
        pendingWrap = s.pendingWrap && autowrap && cursorCol == cols - 1
        pen = s.pen
        originMode = s.origin
        s.g.copyInto(g); gl = s.gl
        updateCharset()
    }

    /** DECALN: fill the screen with E, reset the margins, home. */
    private fun alignmentTest() {
        for (l in screen.lines) l.fill('E'.code, Style.DEFAULT)
        top = 0; bottom = rows - 1
        cursorRow = 0; cursorCol = 0; pendingWrap = false
    }

    // ---- OSC ----

    private fun dispatchOsc(data: CharSequence) {
        joinNext = false
        var i = 0
        var ps = 0
        while (i < data.length && data[i] in '0'..'9' && i < 6) { ps = ps * 10 + (data[i] - '0'); i++ }
        if (i == 0 || (i < data.length && data[i] != ';')) return
        if (ps == 0 || ps == 2) title = if (i < data.length) data.subSequence(i + 1, minOf(data.length, i + 1 + MAX_TITLE)).toString() else ""
    }

    // ---- CSI ----

    private fun dispatchCsi(p: Parser, final: Int) {
        joinNext = false
        if (p.intermediates != 0) {
            // DECSTR; the rest (DECSCUSR ` q`, DECRQM `$p`, …) change nothing on screen.
            if (p.intermediates == '!'.code && final == 'p'.code && p.prefix == 0) softReset()
            return
        }
        when (p.prefix) {
            0 -> {}
            '?'.code -> {
                when (final.toChar()) {
                    'h' -> decset(p, true)
                    'l' -> decset(p, false)
                    'J' -> eraseDisplay(p.param(0, 0))
                    'K' -> eraseLine(p.param(0, 0))
                    'n' -> if (p.raw(0) == 6) reply("\u001b[?${reportRow()};${cursorCol + 1}R")
                }
                return
            }
            // `>`, `=`, `<`: XTMODKEYS, DA2/DA3, the kitty keyboard protocol: nothing to draw.
            else -> return
        }
        val n = p.param(0, 1)
        when (final.toChar()) {
            '@' -> { screen.lines[cursorRow].insertBlanks(cursorCol, n, Style.erased(pen)); pendingWrap = false }
            'A' -> { cursorRow = maxOf(cursorRow - n, if (cursorRow >= top) top else 0); pendingWrap = false }
            'B' -> { cursorRow = minOf(cursorRow + n, if (cursorRow <= bottom) bottom else rows - 1); pendingWrap = false }
            'C' -> { cursorCol = minOf(cursorCol + n, cols - 1); pendingWrap = false }
            'D' -> { cursorCol = maxOf(cursorCol - n, 0); pendingWrap = false }
            'E' -> { cursorRow = minOf(cursorRow + n, if (cursorRow <= bottom) bottom else rows - 1); cursorCol = 0; pendingWrap = false }
            'F' -> { cursorRow = maxOf(cursorRow - n, if (cursorRow >= top) top else 0); cursorCol = 0; pendingWrap = false }
            'G', '`' -> { cursorCol = (n - 1).coerceIn(0, cols - 1); pendingWrap = false }
            'H', 'f' -> moveTo(p.param(0, 1) - 1, p.param(1, 1) - 1)
            'I' -> tab(n)
            'J' -> eraseDisplay(p.param(0, 0))
            'K' -> eraseLine(p.param(0, 0))
            'L' -> if (cursorRow in top..bottom) { scrollDown(cursorRow, bottom, n); cursorCol = 0; pendingWrap = false }
            'M' -> if (cursorRow in top..bottom) { scrollUp(cursorRow, bottom, n, false); cursorCol = 0; pendingWrap = false }
            'P' -> { screen.lines[cursorRow].deleteCells(cursorCol, n, Style.erased(pen)); pendingWrap = false }
            'S' -> scrollUp(top, bottom, n, screen === main && top == 0)
            'T' -> if (p.paramCount <= 1) scrollDown(top, bottom, n)
            'X' -> { screen.lines[cursorRow].erase(cursorCol, cursorCol + n, Style.erased(pen)); pendingWrap = false }
            'Z' -> backTab(n)
            'a' -> { cursorCol = minOf(cursorCol + n, cols - 1); pendingWrap = false }
            'b' -> repeatLast(n)
            'c' -> if (p.param(0, 0) == 0) reply(DA1)
            'd' -> moveTo(n - 1, cursorCol)
            'e' -> { cursorRow = minOf(cursorRow + n, if (cursorRow <= bottom) bottom else rows - 1); pendingWrap = false }
            'g' -> when (p.param(0, 0)) {
                0 -> tabs[cursorCol] = false
                3 -> tabs.fill(false)
            }
            'h' -> ansiMode(p, true)
            'l' -> ansiMode(p, false)
            'm' -> sgr(p)
            'n' -> when (p.raw(0)) {
                5 -> reply("\u001b[0n")
                6 -> reply("\u001b[${reportRow()};${cursorCol + 1}R")
            }
            'r' -> {
                val t = p.param(0, 1)
                val b = p.param(1, rows).coerceAtMost(rows)
                if (t < b) { top = t - 1; bottom = b - 1; home() }
            }
            's' -> saveCursor()
            'u' -> restoreCursor()
        }
    }

    /** CUP / HVP / VPA: to ([r], [c]) 0-based, relative to the top margin in origin mode. */
    private fun moveTo(r: Int, c: Int) {
        cursorRow = if (originMode) (top + r).coerceIn(top, bottom) else r.coerceIn(0, rows - 1)
        cursorCol = c.coerceIn(0, cols - 1)
        pendingWrap = false
    }

    private fun home() = moveTo(0, 0)

    private fun reportRow(): Int = if (originMode) cursorRow - top + 1 else cursorRow + 1

    private fun reply(s: String) {
        onReply?.invoke(s)
    }

    /**
     * ED and EL leave the cursor and a pending wrap alone. At the pending wrap the cursor is past
     * the last column, so erasing from it spares that column: `ESC[K` after a full line (grep and
     * GCC end coloured text with one) does not eat the last char. VTE, konsole and xterm.js agree;
     * xterm alone erases it.
     */
    private fun eraseDisplay(mode: Int) {
        val lines = screen.lines
        val blank = Style.erased(pen)
        when (mode) {
            0 -> {
                eraseLine(0)
                for (r in cursorRow + 1 until rows) lines[r].reset(cols, blank)
            }
            1 -> {
                for (r in 0 until cursorRow) lines[r].reset(cols, blank)
                lines[cursorRow].erase(0, cursorCol + 1, blank)
            }
            2 -> for (r in 0 until rows) lines[r].reset(cols, blank)
            3 -> clearScrollback()
        }
    }

    private fun eraseLine(mode: Int) {
        val line = screen.lines[cursorRow]
        val blank = Style.erased(pen)
        val from = when (mode) {
            0 -> if (pendingWrap) cols else cursorCol
            1 -> { line.erase(0, cursorCol + 1, blank); return }
            2 -> 0
            else -> return
        }
        line.erase(from, cols, blank)
        line.wrapped = false
        // A line erased whole is no longer the continuation of the one above.
        if (from == 0 && cursorRow > 0) screen.lines[cursorRow - 1].wrapped = false
    }

    private fun repeatLast(n: Int) {
        val c = lastPrinted
        if (c == 0) return
        val w = if (c < 0x300) 1 else CharWidth.of(c).coerceIn(1, minOf(2, cols))
        repeat(n) { put(c, w) }
    }

    private fun ansiMode(p: Parser, on: Boolean) {
        for (i in 0 until p.paramCount) when (p.raw(i)) {
            4 -> insertMode = on
            20 -> newlineMode = on
        }
    }

    private fun decset(p: Parser, on: Boolean) {
        for (i in 0 until p.paramCount) {
            when (val m = p.raw(i)) {
                1 -> applicationCursorKeys = on
                6 -> { originMode = on; home() }
                7 -> { autowrap = on; if (!on) pendingWrap = false }
                25 -> cursorVisible = on
                47 -> if (on) enterAlt(false) else leaveAlt()
                1047 -> if (on) enterAlt(false) else { if (altScreen) clearScreen(alt); leaveAlt() }
                1048 -> if (on) saveCursor() else restoreCursor()
                // As xterm: save and restore on whichever screen is current; clear only on the way in.
                1049 -> if (on) {
                    saveCursor()
                    if (!altScreen) enterAlt(true)
                } else {
                    leaveAlt()
                    restoreCursor()
                }
                9, 1000, 1002, 1003 -> mouseTracking = if (on) m else if (mouseTracking == m) 0 else mouseTracking
                1005, 1006, 1015 -> mouseEncoding = if (on) m else if (mouseEncoding == m) 0 else mouseEncoding
                1004 -> focusReporting = on
                2004 -> bracketedPaste = on
                2026 -> { synchronizedOutput = on; syncBytes = 0 }
            }
        }
    }

    private fun enterAlt(clear: Boolean) {
        screen = alt
        if (clear) clearScreen(alt)
        pendingWrap = false
    }

    private fun leaveAlt() {
        screen = main
        pendingWrap = false
    }

    private fun clearScreen(s: Screen) {
        val blank = Style.erased(pen)
        for (l in s.lines) l.reset(cols, blank)
    }

    /** DECSTR: modes, pen, margins and charsets back to their defaults; the screen is left alone. */
    private fun softReset() {
        cursorVisible = true; insertMode = false; originMode = false; autowrap = true
        applicationCursorKeys = false; applicationKeypad = false
        top = 0; bottom = rows - 1
        pen = Style.DEFAULT
        g.fill(CS_ASCII); gl = 0; updateCharset()
        main.saved.clear(); alt.saved.clear()
        pendingWrap = false
    }

    private fun sgr(p: Parser) {
        val n = p.paramCount
        if (n == 0) { pen = Style.DEFAULT; return }
        var s = pen
        var i = 0
        while (i < n) {
            val v = p.raw(i).coerceAtLeast(0)
            var subs = 0
            while (p.isSub(i + 1 + subs)) subs++
            var step = 1 + subs
            when (v) {
                0 -> s = Style.DEFAULT
                1 -> s = s or Style.BOLD
                2 -> s = s or Style.DIM
                3 -> s = s or Style.ITALIC
                // 4:0 is "no underline"; 4:1..4:5 (single, double, curly, dotted, dashed) all underline.
                4 -> s = if (subs > 0 && p.raw(i + 1) == 0) s and Style.UNDERLINE.inv() else s or Style.UNDERLINE
                5, 6 -> s = s or Style.BLINK
                7 -> s = s or Style.INVERSE
                8 -> s = s or Style.INVISIBLE
                9 -> s = s or Style.STRIKE
                21 -> s = s or Style.UNDERLINE
                22 -> s = s and (Style.BOLD or Style.DIM).inv()
                23 -> s = s and Style.ITALIC.inv()
                24 -> s = s and Style.UNDERLINE.inv()
                25 -> s = s and Style.BLINK.inv()
                27 -> s = s and Style.INVERSE.inv()
                28 -> s = s and Style.INVISIBLE.inv()
                29 -> s = s and Style.STRIKE.inv()
                in 30..37 -> s = Style.withFg(s, v - 30)
                39 -> s = Style.withFg(s, TermColor.DEFAULT_FG)
                in 40..47 -> s = Style.withBg(s, v - 40)
                49 -> s = Style.withBg(s, TermColor.DEFAULT_BG)
                in 90..97 -> s = Style.withFg(s, v - 90 + 8)
                in 100..107 -> s = Style.withBg(s, v - 100 + 8)
                38, 48, 58 -> {
                    val c = extColor(p, i, subs)
                    step = 1 + extUsed
                    if (c >= 0) {
                        if (v == 38) s = Style.withFg(s, c) else if (v == 48) s = Style.withBg(s, c)
                    }
                }
            }
            i += step
        }
        pen = s
    }

    /**
     * The colour of a 38/48/58 at [i], or -1. Sets [extUsed] to the parameters it consumed after
     * [i]. Takes `5;n`, `2;r;g;b`, `5:n`, `2:r:g:b` and `2:cs:r:g:b` (empty colour space included).
     */
    private fun extColor(p: Parser, i: Int, subs: Int): Int {
        val n = p.paramCount
        if (subs > 0) {
            extUsed = subs
            return when (p.raw(i + 1)) {
                5 -> if (subs >= 2) indexOrNone(p.raw(i + 2)) else -1
                2 -> when {
                    subs >= 5 -> rgb(p, i + 3)
                    subs == 4 -> rgb(p, i + 2)
                    else -> -1
                }
                else -> -1
            }
        }
        return when (p.raw(i + 1)) {
            5 -> { extUsed = minOf(2, n - 1 - i); if (i + 2 < n) indexOrNone(p.raw(i + 2)) else -1 }
            2 -> { extUsed = minOf(4, n - 1 - i); if (i + 4 < n) rgb(p, i + 2) else -1 }
            else -> { extUsed = minOf(1, n - 1 - i); -1 }
        }
    }

    private fun indexOrNone(v: Int): Int = if (v > 255) -1 else v.coerceAtLeast(0)

    private fun rgb(p: Parser, at: Int): Int {
        val r = p.raw(at); val g = p.raw(at + 1); val b = p.raw(at + 2)
        if (r > 255 || g > 255 || b > 255) return -1
        return TermColor.ofRgb(r.coerceAtLeast(0), g.coerceAtLeast(0), b.coerceAtLeast(0))
    }

    private companion object {
        const val CS_ASCII = 0
        const val CS_DEC = 1
        const val CS_UK = 2
        const val ZWJ = 0x200D
        const val MAX_TITLE = 1024
        const val DA1 = "\u001b[?62;22c"

        /** Bytes fed under an unreleased DEC 2026 before the renderer is let go regardless. */
        const val SYNC_LIMIT = 2L * 1024 * 1024

        /** DEC special graphics for 0x5F..0x7E: the line-drawing set behind `ESC ( 0`. */
        val DEC_GRAPHICS = intArrayOf(
            0x00A0, 0x25C6, 0x2592, 0x2409, 0x240C, 0x240D, 0x240A, 0x00B0, 0x00B1, 0x2424, 0x240B,
            0x2518, 0x2510, 0x250C, 0x2514, 0x253C, 0x23BA, 0x23BB, 0x2500, 0x23BC, 0x23BD, 0x251C,
            0x2524, 0x2534, 0x252C, 0x2502, 0x2264, 0x2265, 0x03C0, 0x2260, 0x00A3, 0x00B7,
        )

        fun defaultTabs(cols: Int) = BooleanArray(cols) { it > 0 && it % 8 == 0 }

        fun isBlank(line: Line): Boolean {
            for (c in 0 until line.length) {
                val v = line.cp[c]
                if (v != 0 && v != ' '.code) return false
            }
            return true
        }

        fun trimEnd(sb: StringBuilder) {
            var n = sb.length
            while (n > 0 && sb[n - 1] == ' ') n--
            sb.setLength(n)
        }
    }
}
