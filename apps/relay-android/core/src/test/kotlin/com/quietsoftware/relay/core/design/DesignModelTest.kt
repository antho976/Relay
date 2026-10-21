package com.quietsoftware.relay.core.design

import com.quietsoftware.relay.core.wire.Wire
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Reading `design.get` (Model.kt) and its tree (Scene.kt), against the PC's reply in `get-reply.json`. */
class DesignModelTest {
    private val reply: JsonObject = Wire.parse(resource("get-reply.json"))!!

    private fun got(json: String): DesignGot? = decodeDesignGot(Wire.parse(json))

    private val design = """"design": {"id": 7, "title": "T", "rev": 3, "pages": 1, "nodes": 2}"""

    @Test
    fun `the PC's reply reads whole`() {
        val got = decodeDesignGot(reply)!!
        assertEquals(reply["nodes"]!!.jsonArray.size, got.nodes.size)
        assertEquals(0, got.unreadable)
        assertEquals(0, got.partial)
        assertEquals(DesignValues.kinds, got.nodes.map { it.kind }.toSet())
        assertEquals("Phone fixture", got.design.title)
        assertEquals(42L, got.design.rev)
        assertEquals(got.nodes.size.toLong(), got.design.nodes)
        assertEquals(setOf("accent", "ink", "surface"), got.colors.keys)
        assertEquals(setOf("button", "card", "glass", "outline"), got.styles.keys)
        assertEquals("J. Doe", got.credits.getValue("0123456789abcdef0123456789abcdef.png").creator)
        assertTrue(got.assetsDir.endsWith("design-assets"))
        // A connector without points, by its ends.
        val link = got.nodes.single { it.id == "link" }
        assertEquals(listOf("step-a", "step-b", "elbow"), listOf(link.from, link.to, link.route))
        assertNull(link.points)
        // Round two's fields arrive.
        val name = got.nodes.single { it.id == "name" }
        assertEquals(DesignTextRun(start = 15, end = 21, color = "\$accent", font = "Geist Mono", fontSize = 14.0, italic = true, underline = true, strike = false, rise = 2.0, letterSpacing = 1.0), name.runs!![1])
        assertEquals(1, name.maxLines)
        val progress = got.nodes.single { it.id == "progress" }
        assertEquals(DesignArc(start = 0.0, sweep = 270.0, inner = 0.8, cap = "round"), progress.arc)
        assertEquals(listOf(0.0, 800.0), progress.motion!!.keys.map { it.atMs })
        assertEquals(true, progress.motion!!.looped)
        assertEquals(DesignTimeline(durationMs = 3000.0, looped = true), got.nodes.first().timeline)
    }

    /**
     * Every key of every node, style and credit in the fixture lands in a field of the phone's
     * model with its value: one the model did not name would be dropped without a word, as the
     * wire ignores keys it does not know. phone_fixture.rs makes the fixture hold every field the
     * PC has, so a new one fails here until Model.kt reads it.
     */
    @Test
    fun `every field the PC writes reaches the model`() {
        val all = Json(Wire.lenient) { encodeDefaults = true; explicitNulls = false }
        fun <T> covers(serializer: KSerializer<T>, element: JsonElement, at: String) {
            val read = all.encodeToJsonElement(serializer, Wire.lenient.decodeFromJsonElement(serializer, element))
            assertCovered(element, read, at)
        }
        for (node in reply["nodes"]!!.jsonArray) covers(DesignNode.serializer(), node, "node ${node.jsonObject["id"]}")
        for ((name, style) in reply["styles"]!!.jsonObject) covers(DesignStyle.serializer(), style, "style $name")
        for ((src, credit) in reply["credits"]!!.jsonObject) covers(DesignCredit.serializer(), credit, "credit $src")
        covers(DesignRow.serializer(), reply["design"]!!, "design")
    }

    private fun assertCovered(written: JsonElement, read: JsonElement?, at: String) {
        when (written) {
            is JsonObject -> {
                assertTrue("$at: not an object as read", read is JsonObject)
                for ((key, value) in written) {
                    if (value is JsonNull) continue
                    assertTrue("$at: `$key` is not read (Model.kt names no field for it)", key in (read as JsonObject))
                    assertCovered(value, read[key], "$at.$key")
                }
            }
            is JsonArray -> {
                assertTrue("$at: not a list as read", read is JsonArray && read.size == written.size)
                written.forEachIndexed { i, v -> assertCovered(v, (read as JsonArray)[i], "$at[$i]") }
            }
            is JsonPrimitive -> {
                val r = read as? JsonPrimitive
                val same = when {
                    written.isString -> r?.isString == true && r.content == written.content
                    written.booleanOrNull != null -> r?.booleanOrNull == written.booleanOrNull
                    else -> r?.doubleOrNull == written.doubleOrNull
                }
                assertTrue("$at: wrote $written, read $read", same)
            }
            JsonNull -> {}
        }
    }

    /** Each value of an enum field in the fixture, which holds every value the PC has, is one the phone knows. */
    @Test
    fun `every value the PC writes is one the phone knows`() {
        val got = decodeDesignGot(reply)!!
        val known = mapOf(
            "kind" to DesignValues.kinds, "layout" to DesignValues.layouts, "sizing" to DesignValues.sizings,
            "justify" to DesignValues.justify, "items" to DesignValues.items, "align" to DesignValues.align,
            "fit" to DesignValues.fits, "route" to DesignValues.routes, "blend" to DesignValues.blends,
            "text_case" to DesignValues.cases, "pin_x" to DesignValues.pinX, "pin_y" to DesignValues.pinY,
            "scroll" to DesignValues.scrolls, "cap" to DesignValues.caps, "join" to DesignValues.joins,
            "gradient" to DesignValues.gradients,
        )
        val used = mutableMapOf<String, MutableSet<String>>()
        fun use(field: String, value: String?) { if (value != null) used.getOrPut(field) { mutableSetOf() } += value }
        for (n in got.nodes) {
            use("kind", n.kind); use("layout", n.layout); use("sizing", n.wSizing); use("sizing", n.hSizing)
            use("justify", n.justify); use("items", n.items); use("align", n.align); use("fit", n.fit); use("route", n.route)
            use("blend", n.blend); use("text_case", n.textCase); use("pin_x", n.pinX); use("pin_y", n.pinY); use("scroll", n.scroll)
            use("cap", n.strokeCap); use("cap", n.arc?.cap); use("join", n.strokeJoin)
            use("gradient", n.gradient?.kind); use("gradient", n.strokeGradient?.kind)
        }
        for (s in got.styles.values) {
            use("align", s.align); use("blend", s.blend); use("text_case", s.textCase); use("cap", s.strokeCap); use("join", s.strokeJoin)
            use("gradient", s.gradient?.kind); use("gradient", s.strokeGradient?.kind)
        }
        for ((field, values) in used) {
            assertEquals("$field: values the phone does not know; add them to DesignValues and say how it draws them", emptySet<String>(), values - known.getValue(field))
        }
        // The fixture holds every value of these (phone_fixture.rs), so the phone knows all of them.
        for (field in listOf("kind", "layout", "sizing", "justify", "items", "align", "fit", "route", "pin_x", "pin_y", "scroll", "cap", "join", "text_case", "gradient")) {
            assertEquals(field, known.getValue(field), used.getValue(field))
        }
    }

    @Test
    fun `a node of a kind the phone does not know is read and counted`() {
        val got = got("""{$design, "nodes": [
            {"id": "p", "kind": "page"},
            {"id": "c", "parent": "p", "kind": "instance", "of": "card", "x": 10}
        ]}""")!!
        assertEquals(2, got.nodes.size)
        assertEquals(0, got.unreadable)
        assertEquals("instance", got.nodes[1].kind)
        assertEquals(1, omissions(got.scene(), "p", got.unreadable).unknown)
    }

    /** As the PC's `Node::read`: a value the phone cannot read is left out of its node, and the node drawn without it. */
    @Test
    fun `a field the phone cannot read is left out and the node kept`() {
        val got = got("""{$design, "nodes": [
            {"id": "p", "kind": "page"},
            {"id": "a", "parent": "p", "kind": "rect", "x": "oops", "y": 20, "w": 40, "h": 30, "padding": {"all": 4}},
            {"id": "b", "parent": "p", "kind": "rect", "x": 5, "w": 10, "h": 10}
        ]}""")!!
        assertEquals(listOf("p", "a", "b"), got.nodes.map { it.id })
        assertEquals(0, got.unreadable)
        assertEquals(1, got.partial)
        val a = got.nodes[1]
        assertEquals(listOf(0.0, 20.0, 40.0, 30.0), listOf(a.x, a.y, a.w, a.h))
        assertNull(a.padding)
        assertEquals(5.0, got.nodes[2].x, 0.0)
    }

    @Test
    fun `what is no node is left out and counted`() {
        val got = got("""{$design, "nodes": [
            {"id": "p", "kind": "page"},
            7,
            {"parent": "p", "kind": "rect"},
            {"id": "x", "parent": "p"},
            {"id": "y", "parent": {"id": "p"}, "kind": "rect"},
            {"id": "z", "kind": "rect"},
            {"id": "ok", "parent": "p", "kind": "ellipse"}
        ]}""")!!
        assertEquals(listOf("p", "ok"), got.nodes.map { it.id })
        assertEquals(5, got.unreadable)
        assertEquals(5, omissions(got.scene(), "p", got.unreadable).unknown)
    }

    @Test
    fun `a style the phone cannot read part of keeps the rest`() {
        val got = got("""{$design, "nodes": [], "styles": {"card": {"fill": "#FFF", "radius": "big", "glow": true}, "bad": 3}, "colors": {"a": "#000", "b": 5}}""")!!
        assertEquals(mapOf("card" to DesignStyle(fill = "#FFF")), got.styles)
        assertEquals(mapOf("a" to "#000"), got.colors)
    }

    @Test
    fun `a reply of the pages alone reads`() {
        val got = got("""{$design, "nodes": [{"id": "page-1", "kind": "page", "name": "One"}, {"id": "page-2", "kind": "page", "name": "Two"}], "assets_dir": "/a"}""")!!
        assertEquals(listOf("page-1", "page-2"), got.scene().pages)
        assertEquals("/a", got.assetsDir)
    }

    @Test
    fun `no design, no reply`() {
        assertNull(decodeDesignGot(null))
        assertNull(got("""{"nodes": []}"""))
        assertNull(got("""{"design": 5}"""))
    }

    @Test
    fun `the tree reads as design get gives it`() {
        val scene = decodeDesignGot(reply)!!.scene()
        assertEquals(listOf("page-1"), scene.pages)
        assertEquals(listOf("avatar", "name", "chevron"), scene.children("profile"))
        assertEquals(listOf("profile", "screen", "page-1"), scene.ancestors("name"))
        assertEquals(0, scene.depthOf("page-1"))
        assertEquals(3, scene.depthOf("name"))
        assertEquals("page-1", scene.pageOf("chip-2-label"))
        assertEquals("page-1", scene.pageOf("page-1"))
        assertTrue(scene.isInside("chip-1-label", "toggles"))
        assertFalse(scene.isInside("toggles", "chip-1"))
        // Drawn with its style and colours; stored as written.
        assertEquals("\$surface", scene.stored("screen")!!.fill)
        assertEquals("#FAF7F2", scene.node("screen")!!.fill)
        assertNull(scene.stored("profile")!!.fill)
        assertEquals("#FFFFFF", scene.node("profile")!!.fill)
    }

    @Test
    fun `a node too deep is neither placed nor drawn, and nothing recurses`() {
        val chain = (1..200).map { DesignNode(id = "f$it", parent = if (it == 1) "p" else "f${it - 1}", kind = "frame") }
        val scene = DesignScene(listOf(DesignNode(id = "p", kind = "page")) + chain)
        assertEquals(DesignScene.MAX_DEPTH, scene.depthOf("f64"))
        assertFalse(scene.tooDeep("f64"))
        assertTrue(scene.tooDeep("f65"))
        assertEquals(200, scene.ancestors("f200").size)
        assertEquals("p", scene.pageOf("f200"))
        assertEquals(Omissions(deep = 136), omissions(scene, "p"))
        // Built from the deepest first, too.
        assertEquals(200, DesignScene(chain.reversed() + DesignNode(id = "p", kind = "page")).depthOf("f200"))
    }

    @Test
    fun `a node whose parents never reach a page hangs nowhere`() {
        val scene = DesignScene(
            listOf(
                DesignNode(id = "p", kind = "page"),
                DesignNode(id = "a", parent = "b", kind = "frame"),
                DesignNode(id = "b", parent = "a", kind = "frame"),
                DesignNode(id = "c", parent = "gone", kind = "rect"),
                DesignNode(id = "d", parent = "a", kind = "rect"),
                DesignNode(id = "r", kind = "rect"),
            ),
        )
        assertEquals(listOf("p"), scene.pages)
        for (id in listOf("a", "b", "c", "d", "r")) {
            assertNull(id, scene.depthOf(id))
            assertNull(id, scene.pageOf(id))
            assertEquals(id, emptyList<String>(), scene.ancestors(id))
        }
        assertNotNull(scene.node("a"))
        assertEquals(Omissions(), omissions(scene, "p"))
    }

    @Test
    fun `the fixture's page leaves out only what the phone cannot draw yet`() {
        val got = decodeDesignGot(reply)!!
        // Blurred: slide-2, and header-bar's frosted ground. Glowing: the text, icon and line with shadows. SVG: diagram.
        // Moving: title, progress, fab, pen.
        assertEquals(Omissions(blur = 2, glow = 3, svg = 1, moving = 4), omissions(got.scene(), "page-1", got.unreadable))
        // A frame of a layout the phone does not know: drawn, and what it holds counted.
        val scene = DesignScene(
            listOf(
                DesignNode(id = "p", kind = "page"),
                DesignNode(id = "g", parent = "p", kind = "frame", layout = "grid"),
                DesignNode(id = "g1", parent = "g", kind = "rect"),
                DesignNode(id = "g2", parent = "g", kind = "frame"),
                DesignNode(id = "g3", parent = "g2", kind = "text", text = "x"),
                DesignNode(id = "h", parent = "g", kind = "rect", hidden = true),
            ),
        )
        assertEquals(Omissions(unknown = 3), omissions(scene, "p"))
        assertTrue(Omissions().none)
    }
}
