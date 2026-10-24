package com.quietsoftware.relay.core.term

/** What to send for the keys the input bar offers, honouring the terminal's modes. */
object Keys {
    const val ESC = "\u001b"
    const val TAB = "\t"
    const val SHIFT_TAB = "\u001b[Z"
    const val ENTER = "\r"
    const val CTRL_C = "\u0003"
    const val CTRL_D = "\u0004"
    const val BACKSPACE = "\u007f"
    const val DELETE = "\u001b[3~"
    const val PAGE_UP = "\u001b[5~"
    const val PAGE_DOWN = "\u001b[6~"

    private const val PASTE_START = "\u001b[200~"
    private const val PASTE_END = "\u001b[201~"

    fun arrowUp(t: Terminal): String = cursorKey(t, 'A')
    fun arrowDown(t: Terminal): String = cursorKey(t, 'B')
    fun arrowRight(t: Terminal): String = cursorKey(t, 'C')
    fun arrowLeft(t: Terminal): String = cursorKey(t, 'D')
    fun home(t: Terminal): String = cursorKey(t, 'H')
    fun end(t: Terminal): String = cursorKey(t, 'F')

    /** `ESC [ x`, or `ESC O x` under DECCKM. */
    private fun cursorKey(t: Terminal, final: Char): String =
        if (t.applicationCursorKeys) "\u001bO$final" else "\u001b[$final"

    /**
     * The wheel turned [notches] times ([up] goes back into the history) over the screen cell at
     * [col], [row] (from 0), as the program asked to hear it: a mouse report per notch in its
     * encoding while it tracks the mouse; otherwise, in the alternate screen, the arrow keys
     * xterm's alternate scroll sends. Empty on the main screen with no mouse asked for, which
     * scrolls the phone's own scrollback instead.
     */
    fun wheel(t: Terminal, up: Boolean, col: Int, row: Int, notches: Int = 1): String {
        if (notches <= 0) return ""
        val one = when {
            t.mouseTracking != 0 -> wheelReport(t.mouseEncoding, if (up) 64 else 65, col.coerceIn(0, t.cols - 1) + 1, row.coerceIn(0, t.rows - 1) + 1)
            t.altScreen -> if (up) arrowUp(t) else arrowDown(t)
            else -> return ""
        }
        return one.repeat(notches)
    }

    /** One report of button [b] at [x], [y] (from 1). */
    private fun wheelReport(encoding: Int, b: Int, x: Int, y: Int): String = when (encoding) {
        1006 -> "\u001b[<$b;$x;${y}M"
        1015 -> "\u001b[${b + 32};$x;${y}M"
        // UTF-8: each value a character, up to 2015.
        1005 -> "\u001b[M" + (32 + b).toChar() + (32 + x.coerceAtMost(2015)).toChar() + (32 + y.coerceAtMost(2015)).toChar()
        // X10 bytes. The PC writes the input as UTF-8, so a value past 127 would arrive as two
        // bytes: positions stop at 95, the last that stays one byte.
        else -> "\u001b[M" + (32 + b).toChar() + (32 + x.coerceAtMost(95)).toChar() + (32 + y.coerceAtMost(95)).toChar()
    }

    /** Ctrl with a letter (either case) or one of `@[\]^_?` and space; anything else is sent as is. */
    fun ctrl(c: Char): String = when (c) {
        in 'a'..'z' -> (c - 'a' + 1).toChar().toString()
        in 'A'..'Z' -> (c - 'A' + 1).toChar().toString()
        '@', ' ' -> "\u0000"
        '[' -> "\u001b"
        '\\' -> "\u001c"
        ']' -> "\u001d"
        '^' -> "\u001e"
        '_' -> "\u001f"
        '?' -> "\u007f"
        else -> c.toString()
    }

    /**
     * What the keyboard typed, as the terminal should receive it: a line break is Enter (CR), and
     * a held Ctrl or Alt applies to the first character only, as a sticky modifier does (Alt is
     * `ESC` before it). The rest goes as typed.
     */
    fun typed(text: String, ctrl: Boolean = false, alt: Boolean = false): String {
        if (text.isEmpty()) return ""
        val body = text.replace("\r\n", "\r").replace('\n', '\r')
        val first = body[0].let { if (ctrl) ctrl(it) else it.toString() }
        return (if (alt) ESC else "") + first + body.substring(1)
    }

    /**
     * [text] as the program should receive a paste: line breaks as CR (what Enter sends), wrapped
     * in `ESC[200~ … ESC[201~` under bracketed paste. An end marker inside the text is removed, so a
     * paste cannot close the bracket early and have the rest run as typed keys.
     */
    fun paste(t: Terminal, text: String): String {
        val body = text.replace("\r\n", "\r").replace('\n', '\r')
        if (!t.bracketedPaste) return body
        var clean = body
        while (true) {
            val stripped = clean.replace(PASTE_END, "").replace(PASTE_START, "")
            if (stripped == clean) break
            clean = stripped
        }
        return PASTE_START + clean + PASTE_END
    }
}
