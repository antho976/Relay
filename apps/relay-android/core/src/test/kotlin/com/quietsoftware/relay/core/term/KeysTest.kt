package com.quietsoftware.relay.core.term

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The input side: modes the program sets and the bytes [Keys] sends under them. */
class KeysTest {
    @Test fun `arrows follow DECCKM`() {
        val t = term()
        assertEquals("$CSI${"A"}", Keys.arrowUp(t))
        assertEquals("${CSI}D", Keys.arrowLeft(t))
        assertEquals("${CSI}H", Keys.home(t))
        t.feed("$CSI?1h")
        assertTrue(t.applicationCursorKeys)
        assertEquals("${ESC}OA", Keys.arrowUp(t))
        assertEquals("${ESC}OB", Keys.arrowDown(t))
        assertEquals("${ESC}OC", Keys.arrowRight(t))
        assertEquals("${ESC}OD", Keys.arrowLeft(t))
        assertEquals("${ESC}OH", Keys.home(t))
        assertEquals("${ESC}OF", Keys.end(t))
        t.feed("$CSI?1l")
        assertEquals("${CSI}F", Keys.end(t))
    }

    @Test fun `ctrl maps letters and the punctuation row`() {
        assertEquals("\u0003", Keys.ctrl('c'))
        assertEquals("\u0003", Keys.ctrl('C'))
        assertEquals("\u001a", Keys.ctrl('z'))
        assertEquals("\u001b", Keys.ctrl('['))
        assertEquals("\u0000", Keys.ctrl('@'))
        assertEquals("\u007f", Keys.ctrl('?'))
        assertEquals("1", Keys.ctrl('1'))
        assertEquals(Keys.CTRL_C, Keys.ctrl('c'))
        assertEquals(Keys.CTRL_D, Keys.ctrl('d'))
    }

    @Test fun `bracketed paste follows mode 2004`() {
        val t = term()
        assertFalse(t.bracketedPaste)
        t.feed("$CSI?2004h")
        assertTrue(t.bracketedPaste)
        t.feed("$CSI?2004l")
        assertFalse(t.bracketedPaste)
    }

    @Test fun `paste turns line breaks into CR`() {
        val t = term()
        assertEquals("a\rb\rc", Keys.paste(t, "a\nb\r\nc"))
    }

    @Test fun `bracketed paste wraps the text and strips end markers inside it`() {
        val t = term()
        t.feed("$CSI?2004h")
        assertEquals("${CSI}200~a\rb${CSI}201~", Keys.paste(t, "a\nb"))
        assertEquals("${CSI}200~xy${CSI}201~", Keys.paste(t, "x${CSI}201~y"))
        assertEquals("a marker rebuilt by stripping is stripped too", "${CSI}200~ab${CSI}201~", Keys.paste(t, "a$ESC[20${CSI}201~1~b"))
    }

    @Test fun `synchronized output opens and closes with mode 2026`() {
        val t = term()
        assertFalse(t.synchronizedOutput)
        t.feed("$CSI?2026h")
        assertTrue(t.synchronizedOutput)
        t.feed("frame")
        assertTrue(t.synchronizedOutput)
        t.feed("$CSI?2026l")
        assertFalse(t.synchronizedOutput)
    }

    @Test fun `synchronized output lets go after 2 MB without a release`() {
        val t = term()
        t.feed("$CSI?2026h")
        val chunk = ByteArray(64 * 1024) { 'x'.code.toByte() }
        repeat(32) { t.feed(chunk) }
        assertTrue("exactly 2 MB is still within the cap", t.synchronizedOutput)
        t.feed("x")
        assertFalse(t.synchronizedOutput)
        t.feed("$CSI?2026h")
        assertTrue("a new frame starts a new count", t.synchronizedOutput)
    }

    @Test fun `typed keys send Enter for a line break and apply a sticky modifier to the first only`() {
        assertEquals("ls\r", Keys.typed("ls\n"))
        assertEquals("a\rb", Keys.typed("a\r\nb"))
        assertEquals("\u0003", Keys.typed("c", ctrl = true))
        assertEquals("\u0003c", Keys.typed("cc", ctrl = true))
        assertEquals("\u001bb", Keys.typed("b", alt = true))
        assertEquals("\u001b\u0018", Keys.typed("x", ctrl = true, alt = true))
        assertEquals("", Keys.typed("", ctrl = true))
    }

    @Test fun `the wheel reports in SGR at the finger's cell, as Claude Code asks`() {
        val t = term(cols = 80, rows = 24)
        // What Claude Code sets on start: the alternate screen, button and any-motion tracking, SGR.
        t.feed("$CSI?1049h$CSI?1000h$CSI?1002h$CSI?1003h$CSI?1006h")
        assertEquals("$CSI<64;5;3M", Keys.wheel(t, up = true, col = 4, row = 2))
        assertEquals("$CSI<65;80;24M$CSI<65;80;24M", Keys.wheel(t, up = false, col = 200, row = 99, notches = 2))
        assertEquals("", Keys.wheel(t, up = true, col = 0, row = 0, notches = 0))
    }

    @Test fun `the wheel uses the encoding the program set`() {
        val t = term(cols = 200, rows = 24)
        t.feed("$CSI?1049h$CSI?1000h")
        assertEquals("X10 bytes: 32 + button, then 32 + column and row from 1", "${CSI}M`&#", Keys.wheel(t, up = true, col = 5, row = 2))
        assertEquals("X10 positions stop where a byte would need two", "${CSI}Ma\u007f\u0021", Keys.wheel(t, up = false, col = 150, row = 0))
        t.feed("$CSI?1015h")
        assertEquals("${CSI}96;6;3M", Keys.wheel(t, up = true, col = 5, row = 2))
        t.feed("$CSI?1005h")
        assertEquals("${CSI}Ma" + (32 + 151).toChar() + "!", Keys.wheel(t, up = false, col = 150, row = 0))
        t.feed("$CSI?1006h")
        assertEquals("$CSI<65;151;1M", Keys.wheel(t, up = false, col = 150, row = 0))
    }

    @Test fun `with no mouse asked for the alternate screen scrolls by arrows and the main screen by itself`() {
        val t = term()
        assertEquals("the main screen keeps its own scrollback", "", Keys.wheel(t, up = true, col = 0, row = 0))
        t.feed("$CSI?1049h")
        assertEquals("${CSI}A${CSI}A${CSI}A", Keys.wheel(t, up = true, col = 0, row = 0, notches = 3))
        t.feed("$CSI?1h")
        assertEquals("${ESC}OB", Keys.wheel(t, up = false, col = 0, row = 0))
        t.feed("$CSI?1000h")
        assertEquals("tracking turned on wins over arrows", "${CSI}M`!!", Keys.wheel(t, up = true, col = 0, row = 0))
        t.feed("$CSI?1000l$CSI?1049l")
        assertEquals("", Keys.wheel(t, up = true, col = 0, row = 0))
    }
}
