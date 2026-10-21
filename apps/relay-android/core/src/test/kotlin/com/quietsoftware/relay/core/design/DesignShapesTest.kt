package com.quietsoftware.relay.core.design

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

/**
 * Shapes.kt against the PC's shapes.rs and bounds.rs: every case of `layout-cases.json` draws
 * its connectors, polygons and painted bounds where the PC does, then the pieces a painter
 * strokes, one at a time.
 */
class DesignShapesTest {
    private val cases = layoutCases()

    private fun node(id: String, kind: String, x: Double = 0.0, y: Double = 0.0, w: Double = 0.0, h: Double = 0.0) =
        DesignNode(id = id, parent = "p1", kind = kind, x = x, y = y, w = w, h = h)

    private fun near(want: Pt, got: Pt) = abs(want.x - got.x) < 1e-6 && abs(want.y - got.y) < 1e-6

    private fun case(name: String) = cases.single { it.name == name }

    @Test
    fun `every case draws its lines, polygons and bounds where the PC does`() {
        for (case in cases) {
            case.check("lines")
            case.check("polygons")
            case.check("bounds")
        }
    }

    /** B5: a connector an agent wrote without points was not drawn on the phone; it runs edge to edge, and follows its shapes. */
    @Test
    fun `a connector runs from edge to edge`() {
        val bare = case("a_connector_without_points")
        val c = bare.scene.node("c")!!
        val pts = linePoints(bare.scene, bare.placed, c)
        assertEquals(listOf(Pt(100.0, 50.0), Pt(300.0, 50.0)), pts.points)
        assertFalse(pts.curved)
        assertEquals(true to true, attachedEnds(bare.scene, bare.placed, c))
        val head = arrowheads(bare.scene, bare.placed, c, bare.placed.rect("c")!!).single()
        assertEquals(Pt(300.0, 50.0), head.tip)
        val moved = case("a_connector_follows_a_moved_shape")
        val end = linePoints(moved.scene, moved.placed, moved.scene.node("c")!!).points[1]
        assertTrue("$end", end.y > 200.0 && end.x < 350.0)
        val elbow = case("a_connector_follows_a_moved_shape_by_an_elbow")
        val square = linePoints(elbow.scene, elbow.placed, elbow.scene.node("c")!!).points
        assertEquals(4, square.size)
        assertTrue("elbows are square: $square", square.zipWithNext().all { (a, b) -> a.x == b.x || a.y == b.y })
    }

    /** E-board-22 (part b): two shapes beside each other whose outlines overlap across the run join straight through the overlap's middle. */
    @Test
    fun `near aligned elbows run straight`() {
        val side = case("near_aligned_elbows_run_straight_side_by_side")
        assertEquals(listOf(Pt(100.0, 32.0), Pt(300.0, 32.0)), linePoints(side.scene, side.placed, side.scene.node("c")!!).points)
        val apart = case("elbows_apart_keep_their_corners")
        assertEquals(4, linePoints(apart.scene, apart.placed, apart.scene.node("c")!!).points.size)
        // Free ends under a pixel apart across: no jog either.
        assertEquals(listOf(Pt(0.0, 400.0), Pt(200.0, 400.5)), linePoints(apart.scene, apart.placed, apart.scene.node("free")!!).points)
    }

    @Test
    fun `a connector meets the outline as it is drawn`() {
        val tri = case("a_connector_meets_a_triangle_on_its_edge")
        assertTrue(near(Pt(50.0 + 43.30127018922193 * 50.0 / 75.0, 50.0), linePoints(tri.scene, tri.placed, tri.scene.node("c")!!).points[0]))
        val diamond = case("a_connector_meets_a_diamond_at_its_corner")
        assertTrue(near(Pt(50.0 + 50.0 * Math.sqrt(2.0), 50.0), linePoints(diamond.scene, diamond.placed, diamond.scene.node("c")!!).points[0]))
        val turned = case("a_connector_meets_a_child_of_a_turned_frame")
        val pts = linePoints(turned.scene, turned.placed, turned.scene.node("c")!!).points
        assertTrue("$pts", near(Pt(100.0, 50.0), pts[0]) && near(Pt(100.0, 400.0), pts[1]))
        val inside = case("a_connector_inside_a_turned_frame_is_drawn_in_its_coordinates")
        val own = linePoints(inside.scene, inside.placed, inside.scene.node("c")!!).points
        assertTrue("$own", near(Pt(40.0, 20.0), own[0]) && near(Pt(150.0, 20.0), own[1]))
        val pill = case("a_connector_meets_a_pills_rounded_end")
        val a = linePoints(pill.scene, pill.placed, pill.scene.node("c")!!).points[0]
        assertTrue("on the corner's circle: $a", abs(Math.hypot(a.x - 80.0, a.y - 20.0) - 20.0) < 1e-6 && a.x < 100.0)
        assertTrue(near(Pt(68.0, 40.0), linePoints(pill.scene, pill.placed, pill.scene.node("e")!!).points[0]))
    }

    /** B57: a star without `sides` has five points, as on the PC; flips mirror a polygon's corners. */
    @Test
    fun `a star has five points unless it says`() {
        val r = Rect(0.0, 0.0, 100.0, 100.0)
        assertEquals(10, nodePolygon(node("s", "polygon").copy(star = 0.4), r).size)
        assertEquals(3, nodePolygon(node("t", "polygon"), r).size)
        assertEquals(16, nodePolygon(node("t", "polygon").copy(sides = 8, star = 0.5), r).size)
        val tri = polygon(r, 3, null)
        assertTrue(near(Pt(50.0, 0.0), tri[0]))
        assertTrue("the apex is at the bottom", near(Pt(50.0, 100.0), nodePolygon(node("t", "polygon").copy(flipY = true), r)[0]))
        assertEquals(24, polygon(r, 40, null).size)
    }

    @Test
    fun `arcs cut an ellipse and cap a ring`() {
        assertNull(arcOf(node("e", "ellipse")))
        assertNull("a whole ellipse is no arc", arcOf(node("e", "ellipse").copy(arc = DesignArc(sweep = 360.0))))
        assertNull("only an ellipse has an arc", arcOf(node("r", "rect").copy(arc = DesignArc(sweep = 90.0))))
        assertEquals(ArcSpec(10.0, -360.0, 0.99), arcOf(node("e", "ellipse").copy(arc = DesignArc(start = 10.0, sweep = -900.0, inner = 4.0))))
        val r = Rect(0.0, 0.0, 100.0, 100.0)
        val ring = node("e", "ellipse").copy(arc = DesignArc(sweep = 90.0, inner = 0.6, cap = "round"))
        val ends = arcEnds(ring, r)!!
        assertEquals("round", ends.cap)
        // The ring runs from 30 to 50 px out from the centre: its middle is 40 out, its caps 10 round.
        for (e in listOf(ends.start, ends.end)) {
            assertEquals(40.0, Math.hypot(e.mid.x - 50.0, e.mid.y - 50.0), 1e-9)
            assertEquals(10.0, e.capRadius, 1e-9)
        }
        assertTrue("the start at the top looks back left: ${ends.start.out}", ends.start.out.x < -0.99)
        assertTrue("the end at 3 o'clock runs on downward: ${ends.end.out}", ends.end.out.y > 0.99)
        assertEquals(10.0, capReach(ring.copy(arc = ring.arc!!.copy(cap = "square")), r), 1e-9)
        assertNull("butt ends are the plain outline", arcEnds(ring.copy(arc = ring.arc!!.copy(cap = "butt")), r))
        assertNull("a cap this phone does not know is butt", arcEnds(ring.copy(arc = ring.arc!!.copy(cap = "chamfer")), r))
        assertNull("a pie has no ends to shape", arcEnds(ring.copy(arc = ring.arc!!.copy(inner = 0.0)), r))
        assertEquals(0.0, capReach(node("e", "ellipse"), r), 0.0)
    }

    @Test
    fun `strokes are pieced as the PC paints them`() {
        // A pen path turns at each point between the midpoints (B118).
        val pen = node("v", "path").copy(points = listOf(listOf(0.0, 0.0), listOf(50.0, 100.0), listOf(100.0, 0.0), listOf(150.0, 100.0)))
        val path = pathSegments(pen, Pt(10.0, 0.0))!!
        assertEquals(Pt(10.0, 0.0), path.start)
        assertEquals(
            listOf(Seg.Quad(Pt(60.0, 100.0), Pt(85.0, 50.0)), Seg.Quad(Pt(110.0, 0.0), Pt(135.0, 50.0)), Seg.Line(Pt(160.0, 100.0))),
            path.segs,
        )
        // One point is a dot.
        assertEquals(listOf(Seg.Line(Pt(3.01, 4.0))), pathSegments(pen.copy(points = listOf(listOf(3.0, 4.0))), Pt(0.0, 0.0))!!.segs)
        val line = node("l", "line").copy(points = listOf(listOf(0.0, 0.0), listOf(10.0, 0.0), listOf(10.0, 10.0)), closed = true)
        assertTrue(closedShape(line) && fillableLine(line))
        assertEquals(listOf(Seg.Line(Pt(10.0, 0.0)), Seg.Line(Pt(10.0, 10.0)), Seg.Line(Pt(0.0, 0.0))), pathSegments(line, Pt(0.0, 0.0))!!.segs)
        assertNull(pathSegments(line.copy(points = null), Pt(0.0, 0.0)))
        assertTrue(strokeOutline(line.copy(points = null), Rect.ZERO).isEmpty())
        assertFalse("a connector is never closed", closedShape(line.copy(from = "a")))
        assertFalse("two points have no inside", fillableLine(line.copy(points = listOf(listOf(0.0, 0.0), listOf(1.0, 1.0)))))
        // A point that is not an x, y pair: no points at all, as the PC sets the value aside.
        assertNull(pointsOf(line.copy(points = listOf(listOf(0.0, 0.0), listOf(1.0)))))
        // A smooth curve passes through its points.
        val smooth = smoothSegments(listOf(Pt(0.0, 0.0), Pt(50.0, 40.0), Pt(100.0, 0.0)), false)
        assertEquals(listOf(Pt(50.0, 40.0), Pt(100.0, 0.0)), smooth.map { it.to })
        assertEquals(3, smoothSegments(listOf(Pt(0.0, 0.0), Pt(50.0, 40.0), Pt(100.0, 0.0)), true).size)
        assertTrue(smoothSegments(listOf(Pt(0.0, 0.0)), false).isEmpty())
        val curve = strokeOutline(line.copy(closed = null, smooth = true, points = listOf(listOf(0.0, 0.0), listOf(50.0, 40.0), listOf(100.0, 0.0))), Rect.ZERO)
        assertTrue("a curve through its points: $curve", curve.size > 12 && Pt(50.0, 40.0) in curve)
    }

    @Test
    fun `arrowheads sit on the end segments`() {
        val s = DesignScene(listOf(DesignNode(id = "p1", kind = "page")))
        val placed = place(s, "p1", Mono)
        val r = Rect(200.0, 200.0, 100.0, 0.0)
        val line = node("l", "line", 200.0, 200.0, 100.0).copy(points = listOf(listOf(0.0, 0.0), listOf(100.0, 0.0)), strokeWidth = 10.0)
        assertTrue(arrowheads(s, placed, line, r).isEmpty())
        val heads = arrowheads(s, placed, line.copy(arrowEnd = true, arrowStart = true), r)
        // 35 px long (10 × 3.5), its wings 0.55 × 35 to each side.
        assertEquals(Arrowhead(Pt(300.0, 200.0), Pt(265.0, 219.25), Pt(265.0, 180.75)), heads[0])
        assertEquals("the start's tip is the first point", Pt(200.0, 200.0), heads[1].tip)
        val noDirection = line.copy(arrowEnd = true, points = listOf(listOf(0.0, 0.0), listOf(0.0, 0.0)))
        assertTrue("no direction, no head", arrowheads(s, placed, noDirection, r).isEmpty())
        assertEquals(8.0, arrowSize(node("t", "line")), 0.0)
    }

    /** The icon set is the PC's copy (phone_fixtures.rs holds it to icons.rs), each path data of commands and numbers alone. */
    @Test
    fun `the icons are the PC's`() {
        assertEquals(155, DesignIcons.names.size)
        assertEquals(listOf("home", "search"), DesignIcons.names.take(2))
        assertEquals("M12 5v14M5 12h14", DesignIcons.path("plus"))
        assertNull(DesignIcons.path("no-such-icon"))
        assertNull(DesignIcons.path(null))
        val token = Regex("[MmLlHhVvCcSsQqTtAaZz]|-?(\\d+\\.?\\d*|\\.\\d+)")
        for (name in DesignIcons.names) {
            val data = DesignIcons.path(name)!!
            assertTrue(name, data.startsWith("M"))
            assertEquals("$name: only commands and numbers", "", token.replace(data, "").replace(Regex("[\\s,]"), ""))
        }
    }

    /** B55: what a page of groups alone paints has a box, and a turned group turns about its children's middle. */
    @Test
    fun `a page of groups has a box`() {
        val case = case("a_group_on_its_own_has_its_childrens_box")
        val page = paintedBounds(case.scene, case.placed, Mono, "p1")
        assertNotNull(page)
        assertEquals(Rect(110.0, 120.0, 50.0, 40.0), paintedBounds(case.scene, case.placed, Mono, "g"))
        // The bar 100 × 20 at (500, 100) turned a quarter about its middle (550, 110).
        val turned = paintedBounds(case.scene, case.placed, Mono, "turned")!!
        assertTrue("$turned", near(Pt(540.0, 60.0), Pt(turned.x, turned.y)) && near(Pt(20.0, 100.0), Pt(turned.w, turned.h)))
    }
}
