package com.quietsoftware.relay.ui.code

import com.quietsoftware.relay.core.design.DesignGot
import com.quietsoftware.relay.core.design.DesignNode
import com.quietsoftware.relay.core.design.DesignRow
import com.quietsoftware.relay.core.design.Omissions
import com.quietsoftware.relay.core.wire.Wire
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.code.DesignPlans.page
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode
import java.io.File

/** What a design's page shows besides its drawing: its frames, what it leaves out, and a design it cannot read. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class DesignPageTest {
    private fun frame(id: String, hidden: Boolean? = null, parent: String? = null) =
        DesignNode(id = id, parent = parent, kind = "frame", name = id, w = 390.0, h = 844.0, hidden = hidden)

    /** B127: the frames list leaves out a hidden frame, as the drawing does. */
    @Test
    fun `the frames list leaves out what is not drawn`() {
        val plan = page(
            frame("shown"),
            frame("hidden", hidden = true),
            frame("unhidden", hidden = false),
            DesignNode(id = "card", kind = "rect", w = 40.0, h = 40.0),
            // Frames inside frames are not listed: they are drawn inside their parent.
            frame("nested", parent = "shown"),
        )
        assertEquals(listOf("shown", "unhidden"), plan.frames.map { it.id })
        assertEquals("390 × 844", plan.frames[0].size)
    }

    /** B56: a frame laid out by its children reads its size as drawn, not its stored 0. */
    @Test
    fun `a frame reads the size it is drawn at`() {
        val plan = page(
            DesignNode(id = "stack", kind = "frame", name = "Stack", layout = "row", wSizing = "hug", hSizing = "hug", gap = 10.0),
            DesignNode(id = "a", parent = "stack", kind = "rect", w = 40.0, h = 30.0),
            DesignNode(id = "b", parent = "stack", kind = "rect", w = 50.0, h = 20.0),
        )
        assertEquals(listOf(FrameRow("stack", "Stack", 100.0, 30.0)), plan.frames)
    }

    /** B55: a page whose top level is only groups has something to draw. */
    @Test
    fun `a page of groups has a box`() {
        val plan = page(
            DesignNode(id = "g", kind = "group", x = 100.0, y = 100.0),
            DesignNode(id = "r", parent = "g", kind = "rect", x = 10.0, y = 20.0, w = 50.0, h = 40.0),
        )
        val b = plan.bounds!!
        assertEquals(listOf(110.0, 120.0, 50.0, 40.0), listOf(b.x, b.y, b.w, b.h))
    }

    @Test
    fun `an empty page has nothing to fit`() {
        assertNull(page().bounds)
        assertNull(page(DesignNode(id = "h", kind = "rect", w = 10.0, h = 10.0, hidden = true)).bounds)
    }

    @Test
    fun `the caption says what the phone leaves out, and only then`() {
        assertNull(omissionsText(Omissions()))
        assertEquals(
            "Not drawn on the phone: blur on 2 layers · the glow of 1 layer · 1 SVG picture · 3 layers from a newer Relay · 1 layer nested too deep.",
            omissionsText(Omissions(blur = 2, glow = 1, svg = 1, unknown = 3, deep = 1)),
        )
        assertEquals(
            "Some layers on this page move. The PC plays them; this outline shows them at rest.",
            omissionsText(Omissions(moving = 2)),
        )
        val plan = page(
            DesignNode(id = "b", kind = "rect", w = 10.0, h = 10.0, blur = 4.0),
            DesignNode(id = "x", kind = "component", w = 10.0, h = 10.0),
            DesignNode(id = "t", kind = "text", text = "Hi", shadow = com.quietsoftware.relay.core.design.DesignShadow(blur = 6.0, color = "#FF0000")),
        )
        assertEquals(Omissions(blur = 1, glow = 1, unknown = 1), plan.omissions)
    }

    @Test
    fun `what of a design the phone could not read is said once`() {
        val got = DesignGot(DesignRow(1), emptyList())
        assertNull(unreadText(got))
        assertEquals("1 layer from a newer Relay is left out.", unreadText(got.copy(unreadable = 1)))
        assertEquals("2 layers are drawn without a setting from a newer Relay.", unreadText(got.copy(partial = 2)))
    }

    /** An answer that is no design is said, not waited on for ever. */
    @Test
    fun `an answer the phone cannot read is unreadable, not loading`() {
        val plan = planDesign(Live(result = JsonObject(mapOf("nodes" to JsonPrimitive(3))), fresh = true), fontResolver)
        assertTrue(plan.unreadable)
        assertFalse(planDesign(Live(), fontResolver).unreadable)
    }

    /** An answer again with the same reply (only fresher) keeps its plan: nothing is laid out twice. */
    @Test
    fun `a fresher copy of the same reply keeps its plan`() = runBlocking {
        val reply = Wire.parse(fixture())!!
        val plans = designPlans(flowOf(Live(reply, at = 1, fresh = false), Live(reply, at = 2, fresh = true)), fontResolver).toList()
        assertEquals(2, plans.size)
        assertSame(plans[0].pages, plans[1].pages)
        assertTrue(plans[1].live.fresh)
    }

    /** The phone's fixture, every kind and field the PC has, plans without a page failing. */
    @Test
    fun `the fixture plans and draws`() {
        val plan = planDesign(Live(Wire.parse(fixture())!!, fresh = true), fontResolver)
        val page = plan.pages.single()
        assertFalse(page.failed)
        assertEquals(listOf("Settings", "Board", "Stack"), page.frames.map { it.name })
        assertTrue(page.bounds!!.w > 1000)
        DesignPlans.draw(page, 400, 300)
    }

    /** Words measure as the PC's do: none are 2 px wide, a line tall; more wrap onto lines of the line height. */
    @Test
    fun `words measure as the PC's do`() {
        val m = DesignPlans.measure()
        val empty = m.measure(DesignNode(id = "e", kind = "text"), null)
        assertEquals(2.0, empty.w, 0.0)
        assertEquals(21.0, empty.h, 0.0)
        val long = DesignNode(id = "l", kind = "text", text = "one two three four five six seven eight nine ten")
        val line = m.measure(long, null)
        val wrapped = m.measure(long, 80.0)
        assertTrue("$line", line.w > 200)
        assertTrue("$wrapped", wrapped.w <= 82 && wrapped.h >= 4 * 20.8)
        val cut = m.measure(long.copy(maxLines = 2), 80.0)
        assertEquals(42.0, cut.h, 1.0)
    }

    @Test
    fun `runs land on the letters they name, through case and control characters`() {
        val n = DesignNode(
            id = "t", kind = "text", text = "stra\u0000ße go", textCase = "upper",
            runs = listOf(com.quietsoftware.relay.core.design.DesignTextRun(start = 5, end = 7, fontWeight = 700)),
        )
        val words = wordsOf(n)
        assertEquals("STRASSE GO", words.text)
        // "ße" (characters 5 and 6 of the stored words) is "SSE" drawn.
        assertEquals(listOf(4 to 7), words.spanStyles.map { it.start to it.end })
    }

    private val fontResolver get() = androidx.compose.ui.text.font.createFontFamilyResolver(androidx.test.core.app.ApplicationProvider.getApplicationContext())

    /** The `design.get` reply the phone's :core tests read (the module's own directory is the working one). */
    private fun fixture(): String = File("../core/src/test/resources/design/get-reply.json").readText()
}
