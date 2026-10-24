package com.quietsoftware.relay.ui.threads

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** A streaming reply split into what is settled and the paragraph still being written. */
class TalkTest {
    @Test
    fun oneParagraphIsAllStillComing() {
        assertEquals("" to "You spent **$412** on", splitStream("You spent **$412** on"))
    }

    @Test
    fun settledEndsAtTheLastBlankLine() {
        val (done, now) = splitStream("First.\n\nSecond.\n\nThird is be")
        assertEquals("First.\n\nSecond.", done)
        assertEquals("Third is be", now)
    }

    @Test
    fun aBlankLineInsideAnOpenFenceIsNotACut() {
        val text = "Here:\n\n```kotlin\nval a = 1\n\nval b = 2"
        val (done, now) = splitStream(text)
        assertEquals("Here:", done)
        assertEquals("```kotlin\nval a = 1\n\nval b = 2", now)
    }

    @Test
    fun aClosedFenceSettles() {
        val (done, now) = splitStream("```\nx\n\ny\n```\n\nAfter")
        assertEquals("```\nx\n\ny\n```", done)
        assertEquals("After", now)
    }

    @Test
    fun aTrailingBlankLineSettlesEverything() {
        assertEquals("One." to "", splitStream("One.\n\n"))
    }

    /**
     * The proxy for the parse cost of a streaming reply: characters handed to the Markdown parser
     * over the whole stream, one redraw per chunk. Reading the whole reply every frame grows with
     * its square; reading the settled part once per paragraph and the tail every frame does not.
     */
    @Test
    fun streamingParsesFarLessThanTheWholeReplyEachFrame() {
        val reply = (1..12).joinToString("\n\n") { n -> "Paragraph $n " + "says something about the budget. ".repeat(8) }
        var whole = 0L
        var split = 0L
        var settled = ""
        var at = 0
        while (at < reply.length) {
            at = minOf(reply.length, at + CHUNK)
            val text = reply.substring(0, at)
            whole += text.length
            val (done, now) = splitStream(text)
            if (done != settled) {
                split += done.length
                settled = done
            }
            split += now.length
        }
        println("Streaming a ${reply.length}-char reply in $CHUNK-char frames: whole reply each frame $whole chars parsed, settled + tail $split")
        assertTrue("$split vs $whole", split * 5 < whole)
    }

    private companion object {
        const val CHUNK = 24
    }
}
