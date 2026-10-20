package com.quietsoftware.relay.core.design

import com.quietsoftware.relay.core.wire.Wire
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** The PC's look rules as the phone ports them (Look.kt). */
class DesignLookTest {
    private fun node(id: String, kind: String, parent: String? = "p") = DesignNode(id = id, parent = parent, kind = kind)

    @Test
    fun `colours parse as the PC parses them`() {
        assertEquals(0xFFAABBCCL, parseColor("#ABC"))
        assertEquals(0xFFAABBCCL, parseColor("#abc"))
        assertEquals(0xFF102030L, parseColor("#102030"))
        assertEquals(0x40102030L, parseColor("#10203040"))
        assertEquals(0x40, alphaOf(parseColor("#10203040")!!))
        for (bad in listOf(null, "", "#", "red", "\$accent", "#12345", "102030", "#GGHHII", "#1020304050")) assertNull(bad, parseColor(bad))
        assertEquals("accent", colorRef("\$accent"))
        assertNull(colorRef("\$"))
        assertNull(colorRef("#FFF"))
        assertEquals(1.0, luminance("#FFFFFF"), 1e-9)
        assertEquals(0.0, luminance("#000"), 1e-9)
        assertEquals(0.0, luminance("\$gone"), 1e-9)
    }

    @Test
    fun `fills and grounds default by kind`() {
        assertEquals(FRAME_FILL, fillOf(node("f", "frame")))
        assertEquals(SHAPE_FILL, fillOf(node("r", "polygon")))
        assertEquals(STICKY_FILL, fillOf(node("s", "sticky")))
        assertEquals(CANVAS, fillOf(node("p", "page", null)))
        assertNull(fillOf(node("t", "text")))
        val g = DesignGradient(stops = listOf(DesignStop(1.0, "#000000"), DesignStop(0.0, "#FFFFFF"), DesignStop(0.5, "#808080")))
        assertEquals("#808080", groundOf(node("r", "rect").copy(gradient = g)))
    }

    @Test
    fun `a text is dark on a light ground and light on a dark one`() {
        val page = node("p", "page", null)
        val frame = node("f", "frame").copy(fill = "#FFFFFF")
        val glass = node("g", "frame", "f").copy(fill = "#00000040")
        val inFrame = node("t1", "text", "g")
        val onCanvas = node("t2", "text")
        val label = node("r", "rect").copy(gradient = DesignGradient(stops = listOf(DesignStop(0.0, "#101010"), DesignStop(1.0, "#FFFFFF"))))
        val scene = DesignScene(listOf(page, frame, glass, inFrame, onCanvas, label))
        assertEquals(LIGHT_TEXT, textColor(scene, onCanvas))
        // The glass frame is under half opaque, so the white frame below it is the ground.
        assertEquals(DARK_TEXT, textColor(scene, inFrame))
        assertEquals(LIGHT_TEXT, textColor(scene, label))
        assertEquals(DARK_TEXT, textColor(scene, node("s", "sticky")))
        assertEquals("#123456", textColor(scene, onCanvas.copy(color = "#123456")))
    }

    @Test
    fun `fonts default as the schema says`() {
        val t = node("t", "text")
        assertEquals(listOf(16.0, 1.3), listOf(fontSize(t), lineHeight(t)))
        assertEquals("Geist", fontFamily(t))
        assertEquals(400, fontWeight(t))
        assertEquals("left", textAlign(t))
        assertEquals("center", textAlign(node("r", "rect")))
        assertEquals("right", textAlign(node("r", "rect").copy(align = "right")))
        assertEquals("left", textAlign(t.copy(align = "justify")))
    }

    /** design.rs `TextCase` tests, the same strings (B74). */
    @Test
    fun `text case draws as the PC draws it`() {
        assertEquals("Hello  Wide-World 2go", textCase("title", "hello  wide-world 2go"))
        assertEquals("STRASSE", textCase("upper", "straße"))
        assertEquals("Well-Known And/Or Don't ǅep", textCase("title", "well-known and/or don't ǆep"))
        assertEquals("3rd Place", textCase("title", "3rd place"))
        assertEquals("'Tis Rock'n'roll, (Quoted) ’Twas", textCase("title", "'tis rock'n'roll, (quoted) ’twas"))
        for ((digraph, title) in listOf('Ǆ' to 'ǅ', 'ǉ' to 'ǈ', 'Ǌ' to 'ǋ', 'ǳ' to 'ǲ')) {
            assertEquals("${title}x", textCase("title", "${digraph}x"))
        }
        assertEquals("McDonald IPHONE", textCase("title", "mcDonald iPHONE"))
        // One letter at a time: no final sigma.
        assertEquals("οδοσ", textCase("lower", "ΟΔΟΣ"))
        assertEquals("As Stored", textCase("small_caps", "As Stored"))
        assertEquals("x", textCase(null, "x"))
        // Outside the basic plane, a character is one, not two.
        assertEquals("𝐀 Bc", textCase("title", "𝐀 bc"))
    }

    @Test
    fun `title words start where the PC's editor starts them`() {
        val text = "jean-luc e-mail foo/bar foo.bar"
        assertEquals("jlemfbfb", titleStarts(text).map { text[it] }.joinToString(""))
        assertEquals(listOf(0, 6), titleStarts("don't 2go"))
        assertTrue(titleStarts("  -- ").isEmpty())
    }

    @Test
    fun `each character is found again in the cased text`() {
        val (upper, starts) = textCaseIndexed("upper", "straße km")
        assertEquals("STRASSE KM", upper)
        assertArrayEquals(intArrayOf(0, 1, 2, 3, 4, 6, 7, 8, 9, 10), starts)
        for (case in listOf("upper", "lower", "title", "none")) {
            for (text in listOf("", "Hello  wide-WORLD 2go", "straße İstanbul ǆep", "a\u0000b", "😀 smile")) {
                val (cased, at) = textCaseIndexed(case, text)
                assertEquals("$case $text", textCase(case, text), cased)
                assertEquals(text.codePointCount(0, text.length) + 1, at.size)
                assertEquals(cased.length, at.last())
                assertTrue((1 until at.size).all { at[it - 1] <= at[it] })
            }
        }
    }

    /** paint.rs `gradient`: CSS's angles across the box, long enough to reach its corners. */
    @Test
    fun `a linear gradient runs as CSS reads its angle`() {
        val r = Rect(0.0, 0.0, 200.0, 100.0)
        fun ends(angle: Double?) = linearEnds(angle, r).let { (a, b) -> listOf(a.x, a.y, b.x, b.y) }
        assertClose(listOf(100.0, 0.0, 100.0, 100.0), ends(null))
        assertClose(listOf(100.0, 0.0, 100.0, 100.0), ends(180.0))
        assertClose(listOf(100.0, 100.0, 100.0, 0.0), ends(0.0))
        assertClose(listOf(0.0, 50.0, 200.0, 50.0), ends(90.0))
        val h = (200 + 100) * Math.sqrt(0.5) / 2
        assertClose(listOf(100 - h * Math.sqrt(0.5), 50 + h * Math.sqrt(0.5), 100 + h * Math.sqrt(0.5), 50 - h * Math.sqrt(0.5)), ends(45.0))
    }

    @Test
    fun `a picture fits its box`() {
        val r = Rect(0.0, 0.0, 100.0, 50.0)
        assertEquals(Rect(0.0, -25.0, 100.0, 100.0), fitRect("cover", r, 200.0, 200.0))
        assertEquals(Rect(0.0, -25.0, 100.0, 100.0), fitRect(null, r, 200.0, 200.0))
        assertEquals(Rect(0.0, -25.0, 100.0, 100.0), fitRect("tile", r, 200.0, 200.0))
        assertEquals(Rect(25.0, 0.0, 50.0, 50.0), fitRect("contain", r, 200.0, 200.0))
        assertEquals(r, fitRect("stretch", r, 200.0, 200.0))
    }

    @Test
    fun `corners are capped at half the shorter side`() {
        val r = Rect(0.0, 0.0, 100.0, 40.0)
        assertArrayEquals(doubleArrayOf(20.0, 20.0, 20.0, 20.0), cornerRadii(node("a", "rect").copy(radius = 50.0), r), 0.0)
        assertArrayEquals(doubleArrayOf(6.0, 6.0, 6.0, 6.0), cornerRadii(node("s", "sticky"), r), 0.0)
        assertArrayEquals(doubleArrayOf(0.0, 0.0, 0.0, 0.0), cornerRadii(node("a", "rect"), r), 0.0)
        assertArrayEquals(doubleArrayOf(4.0, 20.0, 0.0, 8.0), cornerRadii(node("a", "rect").copy(radius = 3.0, radii = listOf(4.0, 90.0, 0.0, 8.0)), r), 0.0)
    }

    @Test
    fun `a shape's label sits inside its padding`() {
        val r = Rect(10.0, 20.0, 100.0, 60.0)
        val rect = node("a", "rect")
        assertArrayEquals(doubleArrayOf(0.0, 12.0, 0.0, 12.0), labelInset(rect), 0.0)
        assertArrayEquals(doubleArrayOf(4.0, 4.0, 4.0, 4.0), labelInset(rect.copy(padding = listOf(4.0))), 0.0)
        assertArrayEquals(doubleArrayOf(2.0, 6.0, 2.0, 6.0), labelInset(rect.copy(padding = listOf(2.0, 6.0))), 0.0)
        assertArrayEquals(doubleArrayOf(1.0, 2.0, 3.0, 4.0), labelInset(rect.copy(padding = listOf(1.0, 2.0, 3.0, 4.0))), 0.0)
        var wrapped = 0.0
        assertEquals(Rect(22.0, 40.0, 76.0, 20.0), labelBox(rect, r) { wrapped = it; 20.0 })
        assertEquals(76.0, wrapped, 0.0)
        // Inset 10 at the top and 30 at the bottom: centred in what is left.
        assertEquals(Rect(14.0, 35.0, 92.0, 10.0), labelBox(rect.copy(padding = listOf(10.0, 4.0, 30.0, 4.0)), r) { 10.0 })
    }

    @Test
    fun `strokes default by kind and dash`() {
        assertEquals(2.0, strokeWidth(node("l", "line")), 0.0)
        assertEquals(0.5, strokeWidth(node("l", "path").copy(strokeWidth = 0.1)), 0.0)
        assertEquals(2.0, strokeWidth(node("i", "icon")), 0.0)
        assertEquals(1.0, strokeWidth(node("r", "rect")), 0.0)
        assertEquals("round", strokeCap(node("l", "line")))
        assertEquals("butt", strokeCap(node("l", "line").copy(dash = listOf(6.0, 4.0))))
        assertEquals("round", strokeCap(node("l", "line").copy(dash = listOf(0.0, 4.0))))
        assertEquals("square", strokeCap(node("l", "line").copy(dash = listOf(6.0, 4.0), strokeCap = "square")))
        assertEquals("round", strokeJoin(node("l", "line")))
        assertEquals("bevel", strokeJoin(node("l", "line").copy(strokeJoin = "bevel")))
        assertEquals("round", strokeJoin(node("l", "line").copy(strokeJoin = "arched")))
        assertArrayEquals(doubleArrayOf(4.0, 2.0, 1.0, 4.0, 2.0, 1.0), dashIntervals(listOf(4.0, 2.0, 1.0)), 0.0)
        assertArrayEquals(doubleArrayOf(4.0, 2.0), dashIntervals(listOf(4.0, 2.0)), 0.0)
        assertNull(dashIntervals(null))
        assertTrue(dashDots(listOf(0.0, 4.0)))
        assertFalse(dashDots(listOf(4.0, 0.0)))
        // An odd list repeats, so its second entry is a dash the second time round.
        assertTrue(dashDots(listOf(4.0, 0.0, 4.0)))
    }

    /** MODEL-SPEC §1 #13 and B37. */
    @Test
    fun `shadows are the ones a node casts`() {
        val drop = DesignShadow(y = 2.0, blur = 4.0, color = "#00000040")
        val inner = DesignShadow(blur = 3.0, spread = 2.0, color = "#00000040", inset = true)
        val glow = DesignShadow(blur = 8.0, spread = 3.0, color = "#4AF0FF99")
        assertEquals(listOf(STICKY_SHADOW), effectiveShadows(node("s", "sticky")))
        assertEquals(emptyList<DesignShadow>(), effectiveShadows(node("s", "sticky").copy(shadow = DesignShadow(color = "#00000000"))))
        assertEquals(listOf(drop), effectiveShadows(node("s", "sticky").copy(shadow = drop)))
        assertEquals(emptyList<DesignShadow>(), effectiveShadows(node("r", "rect")))
        assertEquals(listOf(inner, drop), effectiveShadows(node("r", "rect").copy(shadow = glow, shadows = listOf(inner, drop))))
        // A text casts outer shadows only, without spread.
        assertEquals(listOf(glow.copy(spread = 0.0)), effectiveShadows(node("t", "text").copy(shadows = listOf(inner, glow))))
        assertEquals(16.0, shadowReach(node("t", "text").copy(shadow = glow)), 0.0)
        assertEquals(8.0 + 2.0 + 2.0, shadowReach(node("r", "rect").copy(shadows = listOf(inner, drop.copy(spread = 2.0)))), 0.0)
        assertEquals(10.0 * 2 + 3.0, shadowReach(node("s", "sticky")), 0.0)
        assertEquals(23.0 + 1.0 + 8.0, reach(node("s", "sticky").copy(blur = 4.0)), 0.0)
    }

    /** B57: a star without `sides` has 5 points on the phone, as on the PC and as the field's doc says. */
    @Test
    fun `a star without sides has five points, as its doc says`() {
        assertEquals(5, polygonSides(node("y", "polygon").copy(star = 0.4)))
        assertEquals(3, polygonSides(node("y", "polygon")))
        assertEquals(6, polygonSides(node("y", "polygon").copy(sides = 6, star = 0.4)))
        assertEquals(24, polygonSides(node("y", "polygon").copy(sides = 40)))
        val schema = File("../../../schema/bus.v1.json")
        assertTrue("${schema.absolutePath} not found: run from apps/relay-android/core in the Relay tree", schema.isFile)
        val doc = Wire.parse(schema.readText())!!["\$defs"]!!.jsonObject["DesignNode"]!!.jsonObject["properties"]!!
            .jsonObject["sides"]!!.jsonObject["description"]!!.jsonPrimitive.content
        assertTrue("the sides doc no longer says what polygonSides does: $doc", "default 3, or 5 points for a star" in doc)
    }

    @Test
    fun `the phone lays a style on the kinds the PC does`() {
        val table = Wire.parse(resource("style-kinds.json"))!!
        assertEquals(STYLE_FIELDS, table.keys.toList())
        for ((field, kinds) in table) {
            assertEquals(field, kinds.jsonArray.map { it.jsonPrimitive.content }.toSet(), STYLE_KINDS.getValue(field))
        }
        assertEquals(STYLE_FIELDS.toSet(), STYLE_KINDS.keys)
    }

    /** reuse §6 and style.rs `Styles::resolve`, on the PC's reply. */
    @Test
    fun `a node draws with its style laid under and its colours resolved`() {
        val got = decodeDesignGot(Wire.parse(resource("get-reply.json")))!!
        val scene = got.scene()
        // The card style under a frame that sets none of it.
        val profile = scene.node("profile")!!
        assertEquals(listOf("#FFFFFF", "#0000001A"), listOf(profile.fill, profile.stroke))
        assertEquals(16.0, profile.radius)
        assertEquals(DesignShadow(y = 2.0, blur = 8.0, color = "#0000001F"), profile.shadow)
        // Its own fill keeps the button's gradient off; the rest is laid, and names resolve.
        val cta = scene.node("cta")!!
        assertEquals("#4AF0FF", cta.fill)
        assertNull(cta.gradient)
        assertEquals(listOf(14.0, 14.0, 14.0, 14.0), cta.radii)
        assertEquals("#1A1A1A", cta.color)
        assertEquals(listOf("Sora", "upper", "center"), listOf(cta.font, cta.textCase, cta.align))
        assertEquals(DARK_TEXT, textColor(scene, cta))
        // A frame takes the outline's stroke gradient, dash and cap, not its join.
        val chip = scene.node("chip-2")!!
        assertEquals("conic", chip.strokeGradient!!.kind)
        assertEquals("#4AF0FF", chip.strokeGradient!!.stops[0].color)
        assertEquals(listOf(4.0, 2.0), chip.dash)
        assertEquals("round", chip.strokeCap)
        assertNull(chip.strokeJoin)
        // Named colours in a run and a gradient stop.
        assertEquals("#4AF0FF", scene.node("name")!!.runs!![1].color)
        assertEquals("#4AF0FF", scene.node("slide-1")!!.gradient!!.stops[0].color)
        // A node that names nothing is the very node stored.
        assertSame(scene.stored("avatar"), scene.node("avatar"))
    }

    @Test
    fun `a name the design does not hold leaves the kind's default`() {
        val looks = DesignLooks(colors = mapOf("ink" to "#111111"))
        val rect = node("r", "rect").copy(
            fill = "\$gone", stroke = "\$ink", style = "missing",
            gradient = DesignGradient(stops = listOf(DesignStop(0.0, "\$gone"), DesignStop(1.0, "#FFFFFF"))),
            strokeGradient = DesignGradient(stops = listOf(DesignStop(0.0, "\$gone"), DesignStop(0.5, "#000000"), DesignStop(1.0, "\$ink"))),
            shadows = listOf(DesignShadow(color = "\$gone"), DesignShadow(blur = 2.0, color = "\$ink")),
            runs = listOf(DesignTextRun(0, 1, color = "\$gone"), DesignTextRun(1, 2, color = "\$gone", italic = true)),
            text = "ab",
        )
        val drawn = looks.resolve(rect)
        assertNull(drawn.fill)
        assertEquals(SHAPE_FILL, fillOf(drawn))
        assertEquals("#111111", drawn.stroke)
        assertNull(drawn.style)
        assertNull(drawn.gradient)
        assertEquals(listOf("#000000", "#111111"), drawn.strokeGradient!!.stops.map { it.color })
        assertEquals(listOf(DesignShadow(blur = 2.0, color = "#111111")), drawn.shadows)
        assertEquals(listOf(DesignTextRun(1, 2, italic = true)), drawn.runs)
        assertEquals(drawn, resolveLook(rect, looks.colors, looks.styles))
    }

    @Test
    fun `a style lies under a node as its kind takes it`() {
        val glowing = DesignStyle(shadows = listOf(DesignShadow(blur = 300.0, spread = 4.0, color = "#FFFFFF"), DesignShadow(color = "#000000", inset = true)), radius = 8.0, strokeJoin = "bevel")
        val looks = DesignLooks(styles = mapOf("glow" to glowing, "dots" to DesignStyle(dash = listOf(0.0, 4.0), strokeCap = "round")))
        // A text casts outer shadows without spread, blur within 200, and has no corners.
        val text = looks.resolve(node("t", "text").copy(style = "glow"))
        assertEquals(listOf(DesignShadow(blur = 200.0, color = "#FFFFFF")), text.shadows)
        assertNull(text.radius)
        // A box keeps them as they are; an ellipse takes the join, a rect does not.
        assertEquals(glowing.shadows, looks.resolve(node("r", "rect").copy(style = "glow")).shadows)
        assertEquals(8.0, looks.resolve(node("r", "rect").copy(style = "glow")).radius)
        assertNull(looks.resolve(node("r", "rect").copy(style = "glow")).strokeJoin)
        assertEquals("bevel", looks.resolve(node("e", "ellipse").copy(style = "glow")).strokeJoin)
        // Its own shadow keeps the style's list off.
        assertNull(looks.resolve(node("r", "rect").copy(style = "glow", shadow = DesignShadow(color = "#000"))).shadows)
        // Its own butt ends and the style's dots are not combined: butt ends draw nothing of a dot.
        val butt = looks.resolve(node("l", "line").copy(style = "dots", strokeCap = "butt"))
        assertNull(butt.dash)
        assertEquals("butt", butt.strokeCap)
        val dotted = looks.resolve(node("l", "line").copy(style = "dots"))
        assertEquals(listOf(0.0, 4.0), dotted.dash)
        assertEquals("round", dotted.strokeCap)
    }

    private fun assertClose(expected: List<Double>, actual: List<Double>) {
        assertEquals(expected.size, actual.size)
        expected.zip(actual).forEach { (e, a) -> assertEquals("$expected vs $actual", e, a, 1e-9) }
    }
}

/** A file from `src/test/resources/design/`. */
internal fun resource(name: String): String =
    requireNotNull(DesignLookTest::class.java.getResource("/design/$name")) { "no test resource design/$name" }.readText()
