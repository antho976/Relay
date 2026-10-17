package com.quietsoftware.relay.core.term

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** A tap on a printed web address finds all of it; a long press finds the word under it. */
class LinksTest {
    @Test fun `a tap anywhere on an address finds it, without the full stop after it`() {
        val t = Terminal(60, 4)
        t.feed("PR opened: https://github.com/antho976/RelayV4/pull/110. Done")
        assertEquals("https://github.com/antho976/RelayV4/pull/110", t.urlAt(0, 11))
        assertEquals("https://github.com/antho976/RelayV4/pull/110", t.urlAt(0, 54))
        assertNull(t.urlAt(0, 3))
        assertNull(t.urlAt(0, 57))
    }

    @Test fun `an address the terminal wrapped is followed onto the next row`() {
        val t = Terminal(20, 4)
        t.feed("see https://example.com/a/very/long/path ok")
        // Row 0 holds "see https://example.c", row 1 the rest of it.
        assertEquals("https://example.com/a/very/long/path", t.urlAt(0, 6))
        assertEquals("https://example.com/a/very/long/path", t.urlAt(1, 3))
        assertNull(t.urlAt(2, 0))
    }

    @Test fun `a long press takes the run of non-blank cells around it`() {
        val t = Terminal(40, 2)
        t.feed("cargo test -p relay-core")
        assertEquals(14..23, t.wordAt(0, 18))
        assertEquals(0..4, t.wordAt(0, 0))
        assertNull(t.wordAt(0, 5))
        assertNull(t.wordAt(5, 0))
    }
}
