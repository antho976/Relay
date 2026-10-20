package com.quietsoftware.relay.core.design

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Layout.kt against the PC's layout.rs: every case of `layout-cases.json` (which
 * crates/relay-client/tests/phone_fixtures.rs holds to the PC's own code) places every node where
 * the PC places it, then the rules only the phone has, for what it cannot read.
 */
class DesignLayoutTest {
    private val cases = layoutCases()

    private fun node(id: String, parent: String?, kind: String, x: Double = 0.0, y: Double = 0.0, w: Double = 0.0, h: Double = 0.0) =
        DesignNode(id = id, parent = parent, kind = kind, x = x, y = y, w = w, h = h)

    private val page = node("p1", null, "page")

    @Test
    fun `every case is placed where the PC places it`() {
        assertTrue("the fixture holds cases", cases.size > 30)
        for (case in cases) {
            case.check("rects")
            case.check("origins")
            case.check("wraps")
            case.check("scrolls")
        }
    }

    /** B55: a page of groups alone had no box on the phone; each group is as big as what it holds, and turns about its middle. */
    @Test
    fun `a group is as big as its children`() {
        val case = cases.single { it.name == "a_group_on_its_own_has_its_childrens_box" }
        assertEquals(Rect(110.0, 120.0, 50.0, 40.0), case.placed.rect("g"))
        assertEquals(Pt(100.0, 100.0), case.placed.origin("g"))
        // Nested groups add their offsets.
        assertEquals(Rect(325.0, 35.0, 10.0, 10.0), case.placed.rect("dot"))
        assertNull("a hidden child is not placed", case.placed.rect("gone"))
        // A turned group turns about its children's middle, not its own point.
        assertEquals(Rect(500.0, 100.0, 100.0, 20.0), case.placed.rect("turned"))
    }

    /** B56: a frame that hugs its children was 0 high on the phone, or as high as a stale stored h. */
    @Test
    fun `a hugging frame takes its children's size`() {
        val case = cases.single { it.name == "a_hugging_frame_has_a_size" }
        // 16 + "Sign in" 20 + 8 + 44 + 8 + the button (12 + 20 + 12) + 16.
        assertEquals(Rect(20.0, 20.0, 390.0, 156.0), case.placed.rect("sheet"))
        assertEquals(Rect(36.0, 116.0, 112.0, 44.0), case.placed.rect("button"))
        // Its stored h of 999 is not what it shows: 10 + 30 + 10.
        assertEquals(50.0, case.placed.rect("stale")!!.h, 0.0)
    }

    @Test
    fun `wrap breaks its lines as the PC does`() {
        assertEquals(listOf(0..0, 1..1, 2..3), breakLines(listOf(50.0, 200.0, 30.0, 30.0), 100.0, 10.0))
        assertEquals("an exact fit stays on one line", listOf(0..1), breakLines(listOf(50.0, 40.0), 100.0, 10.0))
        assertEquals(listOf(0..0, 1..1), breakLines(listOf(50.0, 41.0), 100.0, 10.0))
        assertEquals(listOf(0..1), breakLines(listOf(500.0, 500.0), null, 10.0))
        assertEquals(emptyList<IntRange>(), breakLines(emptyList(), 10.0, 0.0))
    }

    @Test
    fun `fill shares keep bounded children and share the rest`() {
        assertEquals(listOf(50.0, 250.0), shares(300.0, listOf(null to 50.0, null to null)))
        assertEquals(listOf(10.0, 90.0), shares(100.0, listOf(null to 30.0, 90.0 to null)))
        assertEquals(listOf(80.0, 80.0), shares(100.0, listOf(80.0 to null, 80.0 to null)))
        assertEquals(listOf(0.0, 5.0), shares(-20.0, listOf(null to null, 5.0 to null)))
        assertEquals(listOf(30.0, 30.0, 30.0), shares(90.0, List(3) { null to null }))
        assertEquals(emptyList<Double>(), shares(10.0, emptyList()))
        // A room that is no number shares nothing, as Rust's f64::max reads it, and still ends.
        assertEquals(listOf(0.0, 5.0), shares(Double.NaN, listOf(null to 10.0, 5.0 to null)))
    }

    @Test
    fun `sizing, pins and layouts read as the PC reads them, an unknown value as left out`() {
        assertEquals(Sizing.Hug, wSizing(node("t", "p1", "text")))
        assertEquals(Sizing.Fixed, wSizing(node("t", "p1", "text", w = 100.0)))
        assertEquals(Sizing.Hug, hSizing(node("t", "p1", "text", w = 100.0)))
        assertEquals(Sizing.Fixed, wSizing(node("r", "p1", "rect").copy(wSizing = "stretchy")))
        assertEquals(LayoutMode.None, layoutMode(node("g", "p1", "group").copy(layout = "row")))
        assertEquals(LayoutMode.Unknown, layoutMode(node("f", "p1", "frame").copy(layout = "grid")))
        assertEquals(Pin.Both to Pin.Scale, pinsOf(node("r", "p1", "rect").copy(pinX = "left_right", pinY = "scale")))
        assertEquals(Pin.Start to Pin.Start, pinsOf(node("r", "p1", "rect").copy(pinX = "anchored")))
        assertEquals(60.0 to 20.0, followAxis(Pin.Scale, 30.0, 10.0, 100.0, 200.0))
        assertEquals(30.0 to 10.0, followAxis(Pin.Scale, 30.0, 10.0, 0.0, 200.0))
        assertEquals(30.0 to 0.0, followAxis(Pin.Both, 30.0, 10.0, 100.0, 50.0))
        // A minimum wins over a maximum, as in CSS.
        assertEquals(80.0, clampW(node("r", "p1", "rect").copy(minW = 80.0, maxW = 50.0), 10.0), 0.0)
    }

    /**
     * What the phone cannot read is not placed, and Omissions counts it: a kind it does not know,
     * what a frame with a layout it does not know holds (the frame keeps its own size), and a node
     * deeper than MAX_DEPTH.
     */
    @Test
    fun `what the phone cannot read is not placed`() {
        val nodes = mutableListOf(
            page,
            node("future", "p1", "component", 0.0, 0.0, 50.0, 50.0),
            node("inside", "future", "rect", 0.0, 0.0, 10.0, 10.0),
            node("grid", "p1", "frame", 100.0, 0.0, 200.0, 100.0).copy(layout = "grid", wSizing = "hug"),
            node("cell", "grid", "rect", 0.0, 0.0, 10.0, 10.0),
            node("g", "p1", "group", 400.0, 0.0),
            node("known", "g", "rect", 0.0, 0.0, 10.0, 10.0),
            node("unknown", "g", "hologram", 500.0, 500.0, 10.0, 10.0),
        )
        var parent = "p1"
        for (i in 1..DesignScene.MAX_DEPTH + 1) {
            nodes += node("d$i", parent, "frame", 1.0, 0.0, 10.0, 10.0)
            parent = "d$i"
        }
        val scene = DesignScene(nodes)
        val placed = place(scene, "p1", Mono)
        assertNull(placed.rect("future"))
        assertNull(placed.rect("inside"))
        assertEquals("a layout it does not know is a frame of its own size", Rect(100.0, 0.0, 200.0, 100.0), placed.rect("grid"))
        assertNull(placed.rect("cell"))
        assertEquals("a group bounds what it can read", Rect(400.0, 0.0, 10.0, 10.0), placed.rect("g"))
        assertNull(placed.rect("unknown"))
        assertEquals(Rect(64.0, 0.0, 10.0, 10.0), placed.rect("d${DesignScene.MAX_DEPTH}"))
        assertNull(placed.rect("d${DesignScene.MAX_DEPTH + 1}"))
        // The component (not what it holds), the grid's cell and the unknown child of g.
        assertEquals(Omissions(unknown = 3, deep = 1), omissions(scene, "p1"))
    }

    /** A text's words wrap where layout measured them, a stretched hugging one at the column's width (TM-7). */
    @Test
    fun `texts wrap where they were measured`() {
        val case = cases.single { it.name == "a_stretched_hugging_text_is_drawn_wrapped_where_it_was_measured" }
        val caption = case.scene.node("caption")!!
        val r = case.placed.rect("caption")!!
        assertEquals(180.0, case.placed.wrapWidth(caption, r)!!, 0.0)
        assertNull("the free rule cannot know", wrapWidth(caption, r))
        assertEquals(100.0, case.placed.wrapWidth(case.scene.node("bubble")!!, case.placed.rect("bubble")!!)!!, 0.0)
        assertNull(case.placed.wrapWidth(case.scene.node("free")!!, case.placed.rect("free")!!))
        assertEquals(168.0, wrapWidth(node("s", "p1", "sticky"), Rect(0.0, 0.0, 200.0, 200.0))!!, 0.0)
    }

    /** B45: a pass measures each text once however many rows ask for its size. */
    @Test
    fun `a deep chain of rows measures its text once`() {
        val nodes = mutableListOf(page)
        for (i in 0 until 20) nodes += node("r$i", if (i == 0) "p1" else "r${i - 1}", "frame", 0.0, 0.0, 1000.0, 50.0).copy(layout = "row", wSizing = "fixed")
        nodes += node("leaf", "r19", "text").copy(text = "Hi")
        var asked = 0
        val counted = Measure { n, wrap -> asked++; Mono.measure(n, wrap) }
        val placed = place(DesignScene(nodes), "p1", counted)
        assertTrue("$asked measures", asked <= 2)
        assertEquals(Rect(0.0, 0.0, 16.0, 20.0), placed.rect("leaf"))
    }
}
