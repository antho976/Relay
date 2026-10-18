package com.quietsoftware.relay.core.term

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** Scrolling, scrollback, the alternate screen, resize and text extraction. */
class ScreenBufferTest {
    private fun lines(n: Int) = (1..n).joinToString("\r\n") { "L$it" }

    @Test fun `lines scrolled off a full-screen region go to scrollback`() {
        val t = term(rows = 3)
        t.feed(lines(5))
        assertEquals(listOf("L1", "L2"), t.scrollback())
        assertEquals(listOf("L3", "L4", "L5"), t.screen())
        t.assertCursor(2, 2)
    }

    @Test fun `scrollback is bounded and drops the oldest`() {
        val t = term(rows = 2, scrollback = 3)
        t.feed(lines(10))
        assertEquals(listOf("L6", "L7", "L8"), t.scrollback())
        assertEquals(listOf("L9", "L10"), t.screen())
    }

    @Test fun `recycled lines come back blank, in the current width`() {
        val t = term(cols = 6, rows = 2, scrollback = 2)
        t.feed("${CSI}41m")
        t.feed(lines(6))
        t.resize(8, 2)
        t.feed("\r\n\r\n\r\n")
        for (r in 0 until 2) {
            assertEquals(8, t.row(r).length)
        }
        assertEquals(listOf("", ""), t.screen())
        assertEquals("BCE: the new lines take the pen's background", 1, Style.bg(t.row(1).style(7)))
    }

    @Test fun `a zero scrollback limit keeps nothing`() {
        val t = term(rows = 2, scrollback = 0)
        t.feed(lines(5))
        assertEquals(0, t.scrollbackSize)
        assertEquals(listOf("L4", "L5"), t.screen())
    }

    @Test fun `a region below the top scrolls without feeding scrollback`() {
        val t = term(rows = 4)
        t.feed(lines(4))
        t.feed("${CSI}2;3r${CSI}3;1H\n\nX")
        assertEquals(0, t.scrollbackSize)
        assertEquals(listOf("L1", "", "X", "L4"), t.screen())
    }

    @Test fun `a region at the top feeds scrollback, leaving the rows below it alone`() {
        // Codex writes history this way: a 1;N region above its viewport, then CR LF at its bottom.
        val t = term(rows = 5)
        t.feed("a\r\nb\r\nc${CSI}5;1Hviewport${CSI}1;3r${CSI}3;1H")
        t.feed("\r\nh1\r\nh2${CSI}r")
        assertEquals(listOf("a", "b"), t.scrollback())
        assertEquals(listOf("c", "h1", "h2", "", "viewport"), t.screen())
    }

    @Test fun `the alternate screen never feeds scrollback`() {
        val t = term(rows = 3)
        t.feed("main")
        t.feed("${CSI}?1049h")
        t.feed(lines(10))
        assertEquals(0, t.scrollbackSize)
        assertEquals(listOf("L8", "L9", "L10"), t.screen())
        t.feed("${CSI}?1049l")
        assertEquals("main", t.line(0))
    }

    @Test fun `SU feeds scrollback and SD scrolls down`() {
        val t = term(rows = 3)
        t.feed(lines(3))
        t.feed("${CSI}2S")
        assertEquals(listOf("L1", "L2"), t.scrollback())
        assertEquals(listOf("L3", "", ""), t.screen())
        t.feed("${CSI}T")
        assertEquals(listOf("", "L3", ""), t.screen())
    }

    @Test fun `DL at the top row deletes without feeding scrollback`() {
        val t = term(rows = 3)
        t.feed(lines(3))
        t.feed("${CSI}H${CSI}M")
        assertEquals(0, t.scrollbackSize)
        assertEquals(listOf("L2", "L3", ""), t.screen())
    }

    @Test fun `ED 3 clears scrollback and leaves the screen`() {
        val t = term(rows = 2)
        t.feed(lines(5))
        assertEquals(3, t.scrollbackSize)
        t.feed("${CSI}3J")
        assertEquals(0, t.scrollbackSize)
        assertEquals(listOf("L4", "L5"), t.screen())
        t.feed("\r\nnext")
        assertEquals(listOf("L4"), t.scrollback())
    }

    @Test fun `1049 saves the cursor, clears the alt screen and restores on exit`() {
        val t = term(cols = 10, rows = 3)
        t.feed("one\r\ntwo${CSI}1;31m")
        t.assertCursor(1, 3)
        t.feed("${CSI}?1049h")
        assertTrue(t.altScreen)
        assertEquals(listOf("", "", ""), t.screen())
        t.feed("${CSI}0m${CSI}3;1Halt")
        t.feed("${CSI}?1049l")
        assertFalse(t.altScreen)
        assertEquals(listOf("one", "two", ""), t.screen())
        t.assertCursor(1, 3)
        t.feed("!")
        assertEquals("the pen came back with the cursor", 1, Style.fg(t.row(1).style(3)))
        t.feed("${CSI}?1049h")
        assertEquals("entering again starts from a clear screen", listOf("", "", ""), t.screen())
    }

    @Test fun `1049 set again inside the alt screen does not clear it`() {
        val t = term(rows = 2)
        t.feed("${CSI}?1049hx${CSI}?1049h")
        assertEquals("x", t.line(0))
    }

    @Test fun `1049 reset restores the cursor even outside the alt screen`() {
        val t = term()
        t.feed("${CSI}2;3H${ESC}7${CSI}5;5H${CSI}?1049l")
        t.assertCursor(1, 2)
    }

    @Test fun `47 switches without clearing and 1047 clears on exit`() {
        val t = term(rows = 2)
        t.feed("${CSI}?47hfirst${CSI}?47l")
        t.feed("${CSI}?47h")
        assertEquals("first", t.line(0))
        t.feed("${CSI}?47l${CSI}?1047h")
        assertEquals("first", t.line(0))
        t.feed("${CSI}?1047l${CSI}?1047h")
        assertEquals("", t.line(0))
    }

    @Test fun `scrollback rows keep their width after a resize`() {
        val t = term(cols = 4, rows = 2)
        t.feed("abcd\r\nefgh\r\nijkl")
        t.resize(6, 2)
        assertEquals(4, t.scrollbackRow(0).length)
        assertEquals("abcd", t.scrollbackRow(0).str())
        assertEquals(6, t.row(0).length)
    }

    @Test fun `resize truncates and pads columns`() {
        val t = term(cols = 6, rows = 2)
        t.feed("abcdef")
        t.resize(3, 2)
        assertEquals("abc", t.line(0))
        t.assertCursor(0, 2)
        t.resize(8, 2)
        assertEquals("abc", t.line(0))
        assertEquals(8, t.row(0).length)
        t.feed("${CSI}1;4Hx")
        assertEquals("abcx", t.line(0))
    }

    @Test fun `resize keeps the cursor's line, moving the top into scrollback`() {
        val t = term(rows = 4)
        t.feed(lines(4))
        t.resize(10, 2)
        assertEquals(listOf("L3", "L4"), t.screen())
        assertEquals(listOf("L1", "L2"), t.scrollback())
        t.assertCursor(1, 2)
        t.resize(10, 4)
        assertEquals(listOf("L3", "L4", "", ""), t.screen())
        t.assertCursor(1, 2)
    }

    @Test fun `resize with the cursor high truncates the bottom`() {
        val t = term(rows = 4)
        t.feed(lines(4))
        t.feed("${CSI}H")
        t.resize(10, 2)
        assertEquals(listOf("L1", "L2"), t.screen())
        assertEquals(0, t.scrollbackSize)
    }

    @Test fun `resize resets the scroll region and clamps the cursor`() {
        val t = term(cols = 10, rows = 6)
        t.feed("${CSI}2;4r${CSI}6;10H")
        t.resize(5, 6)
        t.assertCursor(5, 4)
        t.feed("${CSI}H\n\n\n\n\n\nX")
        assertEquals("X", t.line(5))
        assertEquals(1, t.scrollbackSize)
    }

    @Test fun `resize drops a wide char cut in half`() {
        val t = term(cols = 6, rows = 1)
        t.feed("abc中")
        t.resize(4, 1)
        assertEquals(0, t.row(0).char(3))
        t.assertInvariants()
    }

    @Test fun `resize on the alt screen keeps the main screen for later`() {
        val t = term(cols = 10, rows = 4)
        t.feed(lines(4))
        t.feed("${CSI}?1049h${CSI}H")
        t.resize(10, 2)
        t.feed("${CSI}?1049l")
        assertEquals(listOf("L3", "L4"), t.screen())
        t.assertCursor(1, 2)
        t.assertInvariants()
    }

    @Test fun `plainText joins wrapped lines across scrollback and trims`() {
        val t = term(cols = 5, rows = 3)
        t.feed("hello world!\r\nab   \r\n\r\nlast  ")
        assertEquals("hello world!\nab\n\nlast", t.plainText())
        assertEquals("ab\n\nlast", t.plainText(3))
    }

    @Test fun `auto-wrapping at the bottom keeps the whole chain of wrapped lines`() {
        val t = term(cols = 2, rows = 2)
        t.feed("abcdefgh")
        assertEquals(listOf("ab", "cd"), t.scrollback())
        assertTrue(t.scrollbackRow(0).wrapped && t.scrollbackRow(1).wrapped && t.row(0).wrapped)
        assertEquals("abcdefgh", t.plainText())
    }

    @Test fun `inserting or erasing a line breaks the wrap into it`() {
        val t = term(cols = 4, rows = 4)
        t.feed("abcdef${CSI}2;1H${CSI}L")
        assertFalse(t.row(0).wrapped)
        assertEquals("abcd\n\nef", t.plainText())
        val u = term(cols = 4, rows = 4)
        u.feed("abcdef${CSI}2K")
        assertFalse(u.row(0).wrapped)
        val v = term(cols = 4, rows = 3)
        v.feed("abcdefghij${CSI}1;1H${CSI}T")
        assertEquals(listOf("", "abcd", "efgh"), v.screen())
        assertFalse("the row pushed against the region's end no longer continues", v.row(2).wrapped)
        assertTrue("rows that moved together stay joined", v.row(1).wrapped)
    }

    @Test fun `plainText keeps spaces inside a wrapped line`() {
        val t = term(cols = 4, rows = 3)
        t.feed("ab  cd")
        assertEquals("ab  cd", t.plainText())
    }

    @Test fun `plainText skips the gap a wide char left at the margin`() {
        val t = term(cols = 5, rows = 2)
        t.feed("abcd中")
        assertEquals("abcd中", t.plainText())
    }

    @Test fun `plainText of an empty terminal is empty`() {
        assertEquals("", term().plainText())
    }

    @Test fun `textBetween selects in reading order across rows`() {
        val t = term(cols = 6, rows = 3)
        t.feed("abcdef\r\nghi\r\njklmno")
        assertEquals("cdef\nghi\njk", t.textBetween(0, 2, 2, 1))
        assertEquals("selection may run backwards", "cdef\nghi\njk", t.textBetween(2, 1, 0, 2))
        assertEquals("h", t.textBetween(1, 1, 1, 1))
        assertEquals("ghi", t.textBetween(1, 0, 1, 99))
    }

    @Test fun `textBetween indexes scrollback first and includes a wide char from its tail`() {
        val t = term(cols = 6, rows = 2)
        t.feed("old\r\n中文x\r\nnew")
        assertEquals(1, t.scrollbackSize)
        assertEquals("old", t.textBetween(0, 0, 0, 5))
        assertEquals("文x", t.textBetween(1, 3, 1, 4))
        assertEquals("old\n中文x\nnew", t.textBetween(0, 0, 2, 5))
        assertNotSame(t.lineAt(0), t.lineAt(1))
        assertEquals("new", t.lineAt(2).str())
    }

    @Test fun `combining marks survive into text`() {
        val t = term()
        t.feed("é!")
        assertEquals("é!", t.plainText())
    }

    @Test fun `lines leaving a full scrollback are counted, and a clear is told apart`() {
        val t = Terminal(10, 2, scrollbackLimit = 3)
        t.feed((1..4).joinToString("") { "l$it\r\n" })
        // Four lines up, two rows: l1..l3 went to scrollback, none dropped yet.
        assertEquals(0L, t.dropped)
        t.feed("l5\r\nl6\r\n")
        assertEquals(2L, t.dropped)
        // What was row 2 before the two dropped is row 0 now.
        assertEquals("l3", t.textBetween(0, 0, 0, 9))
        val cleared = t.cleared
        t.feed("\u001b[3J")
        assertEquals(cleared + 1, t.cleared)
    }
}
