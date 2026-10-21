package com.quietsoftware.relay.ui.code

import com.quietsoftware.relay.core.design.DesignArc
import com.quietsoftware.relay.core.design.DesignGradient
import com.quietsoftware.relay.core.design.DesignNode
import com.quietsoftware.relay.core.design.DesignShadow
import com.quietsoftware.relay.core.design.DesignStop
import com.quietsoftware.relay.core.design.DesignTextRun
import com.quietsoftware.relay.ui.code.DesignPlans.draw
import com.quietsoftware.relay.ui.code.DesignPlans.page
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode

/**
 * What the phone draws, pixel by pixel, against what the PC's painter draws there (paint.rs):
 * each bug the old outline had, and each rule of the paint order the phone now follows.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class DesignDrawTest {
    private fun rect(id: String, x: Double, y: Double, w: Double, h: Double, fill: String? = null) =
        DesignNode(id = id, kind = "rect", x = x, y = y, w = w, h = h, fill = fill)

    /** B5: a connector an agent writes without points (from, to, route) was not drawn at all; it runs from edge to edge. */
    @Test
    fun `a connector without points is drawn between its shapes`() {
        val plan = page(
            rect("a", 0.0, 0.0, 100.0, 100.0, "#FFFFFF"),
            rect("b", 140.0, 0.0, 100.0, 100.0, "#FFFFFF"),
            DesignNode(id = "c", kind = "line", from = "a", to = "b", stroke = "#FF0000", strokeWidth = 4.0),
        )
        val out = draw(plan)
        assertTrue(hex(out.rgb(120, 50)), out.near(120, 50, RED))
        assertTrue(hex(out.rgb(120, 60)), out.near(120, 60, GROUND))
    }

    /** B5, the elbow: a routed connector turns square corners where the PC's does. */
    @Test
    fun `an elbow connector turns where the PC turns it`() {
        val plan = page(
            rect("a", 0.0, 0.0, 60.0, 60.0, "#FFFFFF"),
            rect("b", 160.0, 120.0, 60.0, 60.0, "#FFFFFF"),
            DesignNode(id = "c", kind = "line", from = "a", to = "b", route = "elbow", stroke = "#FF0000", strokeWidth = 4.0),
        )
        val out = draw(plan)
        // From a's edge towards b's centre, (60, 52.5), across to the halfway line, down, and across into b at (160, 127.5).
        assertTrue(hex(out.rgb(85, 52)), out.near(85, 52, RED))
        assertTrue(hex(out.rgb(110, 90)), out.near(110, 90, RED))
        assertTrue(hex(out.rgb(135, 127)), out.near(135, 127, RED))
        assertTrue(hex(out.rgb(135, 52)), out.near(135, 52, GROUND))
    }

    /** B54: an ellipse with an arc is a pie, not a whole oval. */
    @Test
    fun `an arc is drawn as its slice`() {
        val plan = page(DesignNode(id = "pie", kind = "ellipse", w = 100.0, h = 100.0, fill = "#FF0000", arc = DesignArc(start = 0.0, sweep = 90.0)))
        val out = draw(plan)
        assertTrue(hex(out.rgb(75, 25)), out.near(75, 25, RED))
        assertTrue(hex(out.rgb(25, 75)), out.near(25, 75, GROUND))
        assertTrue(hex(out.rgb(25, 25)), out.near(25, 25, GROUND))
    }

    /** B54: a whole ring keeps its hole, and a ring segment's round cap reaches past its end. */
    @Test
    fun `a ring has its hole and its round ends`() {
        val ring = draw(page(DesignNode(id = "o", kind = "ellipse", w = 100.0, h = 100.0, fill = "#FF0000", arc = DesignArc(inner = 0.5))))
        assertTrue(hex(ring.rgb(50, 50)), ring.near(50, 50, GROUND))
        assertTrue(hex(ring.rgb(50, 12)), ring.near(50, 12, RED))
        val capped = page(DesignNode(id = "o", kind = "ellipse", x = 20.0, y = 20.0, w = 100.0, h = 100.0, fill = "#FF0000", arc = DesignArc(start = 0.0, sweep = 90.0, inner = 0.6, cap = "round")))
        val out = draw(capped)
        // The start cap bulges left of twelve o'clock, half the ring's thickness (10 px) round.
        assertTrue(hex(out.rgb(65, 30)), out.near(65, 30, RED))
        val butt = draw(page(DesignNode(id = "o", kind = "ellipse", x = 20.0, y = 20.0, w = 100.0, h = 100.0, fill = "#FF0000", arc = DesignArc(start = 0.0, sweep = 90.0, inner = 0.6))))
        assertTrue(hex(butt.rgb(65, 30)), butt.near(65, 30, GROUND))
    }

    /** B54: a gradient fills, top to bottom by default, where the old outline drew the kind's grey. */
    @Test
    fun `a linear gradient runs from its first stop to its last`() {
        val g = DesignGradient(stops = listOf(DesignStop(0.0, "#FF0000"), DesignStop(1.0, "#0000FF")))
        val out = draw(page(DesignNode(id = "g", kind = "rect", w = 100.0, h = 100.0, gradient = g)))
        assertTrue(hex(out.rgb(50, 1)), out.near(50, 1, RED, slack = 12))
        assertTrue(hex(out.rgb(50, 98)), out.near(50, 98, BLUE, slack = 12))
        val across = draw(page(DesignNode(id = "g", kind = "rect", w = 100.0, h = 100.0, gradient = g.copy(angle = 90.0))))
        assertTrue(hex(across.rgb(1, 50)), across.near(1, 50, RED, slack = 12))
        assertTrue(hex(across.rgb(98, 50)), across.near(98, 50, BLUE, slack = 12))
    }

    /** B54: a radial gradient fills the box as an ellipse; a conic one turns from the top, clockwise. */
    @Test
    fun `radial and conic gradients`() {
        val stops = listOf(DesignStop(0.0, "#FF0000"), DesignStop(1.0, "#0000FF"))
        val radial = draw(page(DesignNode(id = "g", kind = "rect", w = 200.0, h = 100.0, gradient = DesignGradient("radial", stops = stops))))
        assertTrue(hex(radial.rgb(100, 50)), radial.near(100, 50, RED, slack = 12))
        // Elliptical: the box's side edges are as far round as its top and bottom.
        assertTrue(hex(radial.rgb(199, 50)), radial.near(199, 50, BLUE, slack = 16))
        assertTrue(hex(radial.rgb(100, 99)), radial.near(100, 99, BLUE, slack = 16))
        val conic = draw(page(DesignNode(id = "g", kind = "rect", w = 100.0, h = 100.0, gradient = DesignGradient("conic", stops = stops))))
        // Just right of twelve o'clock it starts red; just left of it, a turn round, it ends blue.
        assertTrue(hex(conic.rgb(53, 10)), conic.near(53, 10, RED, slack = 24))
        assertTrue(hex(conic.rgb(47, 10)), conic.near(47, 10, BLUE, slack = 24))
    }

    /** A radial gradient moved to the top of its box (`center`) and reaching half way (`radius`): a glow kept inside its frame. */
    @Test
    fun `a radial gradient is centred where its center says`() {
        val stops = listOf(DesignStop(0.0, "#FF0000"), DesignStop(1.0, "#0000FF"))
        val glow = DesignGradient("radial", stops = stops, center = listOf(0.5, 0.0), radius = 0.5)
        val out = draw(page(DesignNode(id = "g", kind = "rect", w = 200.0, h = 200.0, gradient = glow)))
        assertTrue(hex(out.rgb(100, 1)), out.near(100, 1, RED, slack = 16))
        assertTrue(hex(out.rgb(100, 100)), out.near(100, 100, BLUE, slack = 16))
    }

    /** B54: `radii` round each corner on its own. */
    @Test
    fun `radii round each corner`() {
        val out = draw(page(DesignNode(id = "r", kind = "rect", w = 100.0, h = 100.0, fill = "#FF0000", radii = listOf(50.0, 0.0, 0.0, 0.0))))
        assertTrue(hex(out.rgb(4, 4)), out.near(4, 4, GROUND))
        assertTrue(hex(out.rgb(96, 4)), out.near(96, 4, RED))
        assertTrue(hex(out.rgb(96, 96)), out.near(96, 96, RED))
    }

    /** B54: a closed line of three or more points fills, and an open one ends in its arrowhead. */
    @Test
    fun `closed lines fill and open ones carry their arrows`() {
        val closed = draw(page(DesignNode(id = "t", kind = "line", w = 100.0, h = 100.0, fill = "#FF0000", points = listOf(listOf(0.0, 0.0), listOf(100.0, 0.0), listOf(50.0, 100.0)), closed = true)))
        assertTrue(hex(closed.rgb(50, 30)), closed.near(50, 30, RED))
        val arrow = draw(page(DesignNode(id = "a", kind = "line", y = 50.0, w = 100.0, stroke = "#FF0000", points = listOf(listOf(0.0, 0.0), listOf(100.0, 0.0)), arrowEnd = true)))
        // 5 px back from the tip the head is wider than the 2 px line.
        assertTrue(hex(arrow.rgb(94, 51)), arrow.near(94, 51, RED))
        assertTrue(hex(arrow.rgb(40, 51)), arrow.near(40, 51, GROUND))
    }

    /** B55: a page whose top level is all groups had no box and drew nothing; a turned group turns about its children's middle. */
    @Test
    fun `a page of groups is drawn and a group turns about its children`() {
        val plan = page(
            DesignNode(id = "g", kind = "group", rotation = 90.0),
            rect("r", 100.0, 0.0, 100.0, 20.0, "#FF0000").copy(parent = "g"),
        )
        assertNotNull(plan.bounds)
        // Its child spans (100, 0) to (200, 20), so it turns about (150, 10): upright from y -40 to 60.
        val out = draw(plan, dy = 50f)
        assertTrue(hex(out.rgb(150, 100)), out.near(150, 100, RED))
        assertTrue(hex(out.rgb(110, 60)), out.near(110, 60, GROUND))
    }

    /** B56: a frame that hugs its children is drawn at the size they give it, not its stored 0 by 0. */
    @Test
    fun `a hugging frame is drawn at the size its children give it`() {
        val plan = page(
            DesignNode(id = "f", kind = "frame", name = "Card", x = 10.0, y = 10.0, fill = "#00FF00", layout = "column", wSizing = "hug", hSizing = "hug", padding = listOf(10.0)),
            rect("r", 0.0, 0.0, 100.0, 50.0, "#FF0000").copy(parent = "f"),
        )
        assertEquals(listOf("120 × 70"), plan.frames.map { it.size })
        val out = draw(plan)
        assertTrue(hex(out.rgb(15, 15)), out.near(15, 15, GREEN))
        assertTrue(hex(out.rgb(60, 40)), out.near(60, 40, RED))
        assertTrue(hex(out.rgb(125, 75)), out.near(125, 75, GREEN))
        assertTrue(hex(out.rgb(135, 85)), out.near(135, 85, GROUND))
    }

    /** B57: a star without `sides` has five points, as on the PC, not three. */
    @Test
    fun `a star has five points unless it says`() {
        val out = draw(page(DesignNode(id = "s", kind = "polygon", w = 200.0, h = 200.0, fill = "#FF0000", star = 0.5)))
        // 85 px out along the point at 18 degrees above three o'clock: inside a five-point star, outside a three-point one.
        assertTrue(hex(out.rgb(181, 74)), out.near(181, 74, RED))
        assertTrue(hex(out.rgb(100, 10)), out.near(100, 10, RED))
    }

    /** B128: a box's stroke is drawn inside its edge; a width of 0 draws none; a line is 2 px by default. */
    @Test
    fun `strokes are inside the edge, none at 0, lines 2 px`() {
        val inside = draw(page(DesignNode(id = "b", kind = "rect", x = 20.0, y = 20.0, w = 100.0, h = 100.0, fill = "#00000000", stroke = "#0000FF", strokeWidth = 4.0)))
        assertTrue(hex(inside.rgb(21, 70)), inside.near(21, 70, BLUE))
        assertTrue(hex(inside.rgb(18, 70)), inside.near(18, 70, GROUND))
        val none = draw(page(DesignNode(id = "b", kind = "rect", x = 20.0, y = 20.0, w = 100.0, h = 100.0, fill = "#00000000", stroke = "#0000FF", strokeWidth = 0.0)))
        assertTrue(none.rgbs(15, 15, 125, 125).all { it == GROUND })
        val line = draw(page(DesignNode(id = "l", kind = "line", y = 50.0, w = 200.0, stroke = "#FF0000", points = listOf(listOf(0.0, 0.0), listOf(200.0, 0.0)))))
        assertTrue(hex(line.rgb(100, 49)), line.near(100, 49, RED, slack = 8))
        assertTrue(hex(line.rgb(100, 50)), line.near(100, 50, RED, slack = 8))
        // A polygon's and an arc's stroke is inside too.
        val tri = draw(page(DesignNode(id = "t", kind = "polygon", x = 20.0, y = 20.0, w = 100.0, h = 100.0, fill = "#00000000", stroke = "#0000FF", strokeWidth = 4.0, sides = 4)))
        assertTrue(hex(tri.rgb(22, 70)), tri.near(22, 70, BLUE))
        assertTrue(hex(tri.rgb(18, 70)), tri.near(18, 70, GROUND))
    }

    /** B128: a flipped polygon is mirrored, a triangle pointing down. */
    @Test
    fun `a flipped polygon is mirrored`() {
        val out = draw(page(DesignNode(id = "t", kind = "polygon", w = 100.0, h = 100.0, fill = "#FF0000", sides = 3, flipY = true)))
        assertTrue(hex(out.rgb(50, 95)), out.near(50, 95, RED))
        assertTrue(hex(out.rgb(50, 30)), out.near(50, 30, RED))
        assertTrue(hex(out.rgb(50, 10)), out.near(50, 10, GROUND))
    }

    /** B93: a frame's inner shadow lies over its fill and under its children. */
    @Test
    fun `a frame's inner shadow is over its fill and under its children`() {
        val plan = page(
            DesignNode(id = "f", kind = "frame", w = 100.0, h = 100.0, fill = "#FFFFFF", shadows = listOf(DesignShadow(spread = 30.0, color = "#000000", inset = true))),
            rect("r", 0.0, 0.0, 40.0, 40.0, "#FF0000").copy(parent = "f"),
        )
        val out = draw(plan)
        assertTrue(hex(out.rgb(10, 10)), out.near(10, 10, RED))
        assertTrue(hex(out.rgb(90, 90)), out.near(90, 90, BLACK))
        assertTrue(hex(out.rgb(50, 50)), out.near(50, 50, WHITE))
    }

    /** B31: a picture's drop shadow is outside its box, its inner shadow over the picture inside it. */
    @Test
    fun `a picture's shadows are outside and over it`() {
        val plan = page(
            DesignNode(
                id = "i", kind = "image", x = 20.0, y = 20.0, w = 100.0, h = 100.0, src = "0123456789abcdef0123456789abcdef.png",
                shadows = listOf(DesignShadow(spread = 5.0, color = "#FF0000"), DesignShadow(spread = 10.0, color = "#0000FF", inset = true)),
            ),
        )
        val out = draw(plan)
        assertTrue(hex(out.rgb(17, 70)), out.near(17, 70, RED))
        assertTrue(hex(out.rgb(25, 70)), out.near(25, 70, BLUE))
        assertFalse(hex(out.rgb(70, 70)), out.near(70, 70, BLUE, slack = 60))
    }

    /** B120: a flip mirrors a picture, never its shadow, which stays where the light puts it. */
    @Test
    fun `a flip does not move a shadow`() {
        val out = draw(page(DesignNode(id = "i", kind = "image", x = 20.0, y = 20.0, w = 50.0, h = 50.0, flipX = true, shadow = DesignShadow(x = 10.0, color = "#FF0000"))))
        assertTrue(hex(out.rgb(75, 45)), out.near(75, 45, RED))
        assertTrue(hex(out.rgb(15, 45)), out.near(15, 45, GROUND))
    }

    /** B37: a sticky casts its own soft shadow unless it names one, and one wholly clear casts nothing. */
    @Test
    fun `a sticky's shadow is its own unless it names one`() {
        val own = draw(page(DesignNode(id = "s", kind = "sticky", x = 20.0, y = 20.0, w = 100.0, h = 100.0), fill = "#FFFFFF"))
        assertTrue(hex(own.rgb(70, 124)), own.rgb(70, 124) != WHITE)
        val clear = draw(page(DesignNode(id = "s", kind = "sticky", x = 20.0, y = 20.0, w = 100.0, h = 100.0, shadow = DesignShadow(color = "#00000000")), fill = "#FFFFFF"))
        assertTrue(hex(clear.rgb(70, 124)), clear.near(70, 124, WHITE, slack = 0))
    }

    /** A see-through group is one piece: where its children overlap is no darker than where they do not. */
    @Test
    fun `opacity takes a node and its children as one`() {
        val plan = page(
            DesignNode(id = "g", kind = "group", opacity = 0.5),
            rect("a", 0.0, 0.0, 100.0, 100.0, "#FF0000").copy(parent = "g"),
            rect("b", 50.0, 0.0, 100.0, 100.0, "#FF0000").copy(parent = "g"),
            fill = "#FFFFFF",
        )
        val out = draw(plan)
        assertEquals(hex(out.rgb(25, 50)), hex(out.rgb(75, 50)))
        assertTrue(hex(out.rgb(75, 50)), out.near(75, 50, 0xFF8080, slack = 8))
    }

    /** A blending node mixes with what is under it, its page's ground included. */
    @Test
    fun `a blend mixes with what is under it`() {
        val plan = page(
            rect("a", 0.0, 0.0, 100.0, 100.0, "#FF0000"),
            rect("b", 50.0, 0.0, 100.0, 100.0, "#0000FF").copy(blend = "multiply"),
            fill = "#FFFFFF",
        )
        val out = draw(plan)
        assertTrue(hex(out.rgb(75, 50)), out.near(75, 50, BLACK))
        assertTrue(hex(out.rgb(125, 50)), out.near(125, 50, BLUE))
        assertTrue(hex(out.rgb(25, 50)), out.near(25, 50, RED))
    }

    /**
     * A turned frame or group holding a blend is drawn whole: the layer its children blend in is
     * bounded in its own frame, not as it sits turned on the page, which cut them to a strip.
     */
    @Test
    fun `a turned frame or group holding a blend is drawn end to end`() {
        fun kids(parent: String, long: Double) = arrayOf(
            rect("a", 0.0, 0.0, long, 40.0, "#FF0000").copy(parent = parent),
            rect("b", 0.0, 0.0, 40.0, 40.0, "#0000FF").copy(parent = parent, blend = "multiply"),
        )
        // A 200 by 40 frame turned a quarter about (100, 20) stands from y -80 to 120 (20 to 220
        // drawn): its left end at the top, the red its first 160 px, its green ground the rest.
        val frame = DesignNode(id = "f", kind = "frame", w = 200.0, h = 40.0, rotation = 90.0, fill = "#00FF00")
        val f = draw(page(frame, *kids("f", 160.0), fill = "#FFFFFF"), h = 260, dy = 100f)
        assertTrue(hex(f.rgb(100, 40)), f.near(100, 40, BLACK))
        assertTrue(hex(f.rgb(100, 150)), f.near(100, 150, RED))
        assertTrue(hex(f.rgb(100, 210)), f.near(100, 210, GREEN))
        assertTrue(hex(f.rgb(70, 120)), f.near(70, 120, WHITE))
        val g = draw(page(DesignNode(id = "g", kind = "group", rotation = 90.0), *kids("g", 200.0), fill = "#FFFFFF"), h = 260, dy = 100f)
        assertTrue(hex(g.rgb(100, 40)), g.near(100, 40, BLACK))
        assertTrue(hex(g.rgb(100, 200)), g.near(100, 200, RED))
        // See-through as well: its own layer sits on the page turned, the one inside it unturned.
        val faint = draw(page(frame.copy(opacity = 0.5), *kids("f", 160.0), fill = "#FFFFFF"), h = 260, dy = 100f)
        assertTrue(hex(faint.rgb(100, 40)), faint.near(100, 40, 0x808080, slack = 8))
        assertTrue(hex(faint.rgb(100, 150)), faint.near(100, 150, 0xFF8080, slack = 8))
        assertTrue(hex(faint.rgb(100, 210)), faint.near(100, 210, 0x80FF80, slack = 8))
    }

    /** A spread grows a rounded corner with it and keeps a square one square, as GSK's outset shadow does. */
    @Test
    fun `a spread shadow keeps a square corner square`() {
        val square = draw(page(rect("s", 40.0, 40.0, 100.0, 100.0, "#FFFFFF").copy(shadow = DesignShadow(spread = 20.0, color = "#FF0000")), fill = "#FFFFFF"))
        // The cast spans 20 to 160; 2 px inside its corner.
        assertTrue(hex(square.rgb(22, 22)), square.near(22, 22, RED))
        assertTrue(hex(square.rgb(157, 157)), square.near(157, 157, RED))
        val round = draw(page(rect("s", 40.0, 40.0, 100.0, 100.0, "#FFFFFF").copy(radius = 10.0, shadow = DesignShadow(spread = 20.0, color = "#FF0000")), fill = "#FFFFFF"))
        // Its corner, 10 grown to 30, is round about (50, 50).
        assertTrue(hex(round.rgb(22, 22)), round.near(22, 22, WHITE))
        assertTrue(hex(round.rgb(30, 30)), round.near(30, 30, RED))
    }

    /**
     * A polygon's shadow with a spread is the shape grown or shrunk by it, blurred once, as the
     * PC blurs it: the same as the shadow, with no spread, of a diamond that much larger or smaller.
     */
    @Test
    fun `a shape's spread shadow is blurred once`() {
        // A diamond 100 across about (120, 120); a spread of 10 moves its sides 10 px, its corners 10√2.
        val corner = 10.0 * kotlin.math.sqrt(2.0)
        fun diamond(grow: Double, shadow: DesignShadow) = page(
            DesignNode(id = "d", kind = "polygon", x = 70.0 - grow, y = 70.0 - grow, w = 100.0 + 2 * grow, h = 100.0 + 2 * grow, fill = "#00000000", sides = 4, shadow = shadow),
            fill = "#FFFFFF",
        )
        fun same(label: String, a: PagePlan, b: PagePlan) {
            val x = draw(a)
            val y = draw(b)
            // Along the diagonal from the centre out through the middle of the top-left side.
            for (t in 0..55 step 3) {
                val p = 120 - t
                assertTrue("$label at $t: ${hex(x.rgb(p, p))} against ${hex(y.rgb(p, p))}", x.near(p, p, y.rgb(p, p), slack = 4))
            }
        }
        val soft = DesignShadow(blur = 20.0, color = "#000000")
        same("drop -10", diamond(0.0, soft.copy(spread = -10.0)), diamond(-corner, soft))
        same("drop 10", diamond(0.0, soft.copy(spread = 10.0)), diamond(corner, soft))
        // An inner shadow is kept inside the shape, so only inside the smaller of the two is alike.
        val inner = soft.copy(inset = true)
        fun inside(label: String, a: PagePlan, b: PagePlan, upTo: Int) {
            val x = draw(a)
            val y = draw(b)
            for (t in 0..upTo step 3) {
                val p = 120 - t
                assertTrue("$label at $t: ${hex(x.rgb(p, p))} against ${hex(y.rgb(p, p))}", x.near(p, p, y.rgb(p, p), slack = 4))
            }
        }
        inside("inner 10", diamond(0.0, inner.copy(spread = 10.0)), diamond(-corner, inner), upTo = 16)
        inside("inner -10", diamond(0.0, inner.copy(spread = -10.0)), diamond(corner, inner), upTo = 23)
    }

    /** Words are set and drawn in their colour, a run in its own. */
    @Test
    fun `words are drawn, a run in its own colour`() {
        val plan = page(
            DesignNode(
                id = "t", kind = "text", x = 10.0, y = 10.0, text = "MMMM WWWW", color = "#0000FF", fontSize = 40.0,
                runs = listOf(DesignTextRun(start = 0, end = 4, color = "#FF0000")),
            ),
            fill = "#FFFFFF",
        )
        val box = plan.placed.rect("t")!!
        assertTrue("$box", box.w > 150 && box.h > 40)
        val out = draw(plan, w = 400, h = 120)
        val left = out.rgbs(10, 10, (10 + box.w / 2).toInt() - 20, 70)
        val right = out.rgbs((10 + box.w / 2).toInt() + 20, 10, (10 + box.w).toInt(), 70)
        assertTrue(left.any { it == RED } && left.none { it == BLUE })
        assertTrue(right.any { it == BLUE } && right.none { it == RED })
    }

    /** A shape's label sits centred in it, light on a dark shape. */
    @Test
    fun `a label is drawn inside its shape`() {
        val plan = page(rect("r", 0.0, 0.0, 200.0, 80.0, "#000000").copy(text = "Save", fontSize = 30.0), fill = "#FFFFFF")
        val out = draw(plan)
        val inside = out.rgbs(60, 20, 140, 60)
        assertTrue(inside.any { (it and 0xFF) > 0xC0 })
        assertTrue(out.rgbs(0, 0, 40, 80).all { it == BLACK })
    }

    /** An icon is stroked from the set in its colour; a name the phone lacks shows a grey tile, as on the PC. */
    @Test
    fun `icons are stroked, a missing one is a tile`() {
        val out = draw(page(DesignNode(id = "i", kind = "icon", w = 96.0, h = 96.0, stroke = "#FF0000", icon = "zap"), fill = "#FFFFFF"))
        assertTrue(out.rgbs(0, 0, 96, 96).any { it == RED })
        val missing = draw(page(DesignNode(id = "i", kind = "icon", w = 96.0, h = 96.0, icon = "no-such-icon"), fill = "#FFFFFF"))
        assertTrue(hex(missing.rgb(48, 48)), missing.rgb(48, 48) != WHITE && missing.near(48, 48, 0xD0D0D0, slack = 40))
    }
}
