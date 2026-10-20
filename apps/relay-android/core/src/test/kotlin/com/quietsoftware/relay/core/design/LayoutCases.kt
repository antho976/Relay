package com.quietsoftware.relay.core.design

import com.quietsoftware.relay.core.wire.Wire
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor

/*
 * The cases of `layout-cases.json`, which crates/relay-client/tests/phone_fixtures.rs lays out
 * with the PC's code and this module with its port: each holds what the PC computes, and the
 * phone must compute the same (feature-phone §8.1).
 */

/** 8 px a character (code points, as Rust counts `chars`), 20 px a line, wrapping whole characters: layout.rs's tests' `Mono`. */
internal object Mono : Measure {
    override fun measure(n: DesignNode, wrap: Double?): TextSize {
        val text = n.text.orEmpty()
        val chars = text.codePointCount(0, text.length).toDouble()
        val natural = chars * 8.0
        if (wrap != null && natural > wrap && wrap >= 8.0) {
            val perLine = floor(wrap / 8.0)
            return TextSize(perLine * 8.0, ceil(chars / perLine) * 20.0)
        }
        return TextSize(natural, 20.0)
    }
}

/** One case: its scene laid out by the phone, and what the PC computes for it. */
internal class LayoutCase(val name: String, val page: String, val ids: List<String>, val scene: DesignScene, val want: JsonObject) {
    val placed: Placed by lazy { place(scene, page, Mono) }

    /** What the phone computes for section [section], in the file's shape: each id the PC lists one for, in the case's order. */
    fun got(section: String): JsonObject {
        val out = LinkedHashMap<String, JsonElement>()
        for (id in ids) {
            val r = placed.rect(id)
            val n = scene.node(id)
            val v: JsonElement? = when (section) {
                "rects" -> r?.let(::rectJson)
                "origins" -> placed.origins[id]?.let(::pointJson)
                "wraps" -> placed.wraps[id]?.let(::JsonPrimitive)
                "scrolls" -> placed.scrolls[id]?.let(::rectJson)
                "lines" -> if (r != null && n?.kind == "line") {
                    val lp = linePoints(scene, placed, n)
                    JsonObject(mapOf("points" to JsonArray(lp.points.map(::pointJson)), "curved" to JsonPrimitive(lp.curved)))
                } else null
                "polygons" -> if (r != null && n?.kind == "polygon") JsonArray(nodePolygon(n, r).map(::pointJson)) else null
                "bounds" -> if (r != null) paintedBounds(scene, placed, Mono, id)?.let(::rectJson) ?: JsonNull else null
                else -> error("no section $section")
            }
            if (v != null) out[id] = v
        }
        return JsonObject(out)
    }

    /** Asserts the phone computes [section] as the PC does, every id and every number within 1e-6. */
    fun check(section: String) {
        val want = want[section]?.jsonObject ?: JsonObject(emptyMap())
        val got = got(section)
        assertEquals("$name: which ids $section holds", want.keys, got.keys)
        for ((id, w) in want) {
            assertTrue("$name: $section of $id is ${got[id]} on the phone, $w on the PC", close(w, got.getValue(id)))
        }
    }
}

private fun rectJson(r: Rect) = JsonArray(listOf(r.x, r.y, r.w, r.h).map(::JsonPrimitive))

private fun pointJson(p: Pt) = JsonArray(listOf(JsonPrimitive(p.x), JsonPrimitive(p.y)))

/** Whether two values agree as phone_fixtures.rs's `close` says: numbers within 1e-6, lists and objects entry by entry. */
internal fun close(a: JsonElement, b: JsonElement): Boolean = when {
    a is JsonArray && b is JsonArray -> a.size == b.size && a.indices.all { close(a[it], b[it]) }
    a is JsonObject && b is JsonObject -> a.keys == b.keys && a.keys.all { close(a.getValue(it), b.getValue(it)) }
    a is JsonPrimitive && b is JsonPrimitive && !a.isString && !b.isString && a.doubleOrNull != null && b.doubleOrNull != null ->
        abs(a.doubleOrNull!! - b.doubleOrNull!!) <= 1e-6
    a is JsonPrimitive && b is JsonPrimitive && a.booleanOrNull != null -> a.booleanOrNull == b.booleanOrNull
    else -> a == b
}

/** Every case of `layout-cases.json`, its nodes read as the phone reads a reply. */
internal fun layoutCases(): List<LayoutCase> {
    val file = Wire.parse(resource("layout-cases.json")) ?: error("layout-cases.json is not a JSON object")
    return file.getValue("cases").jsonArray.map { element ->
        val case = element.jsonObject
        val name = case.getValue("name").jsonPrimitive.content
        val nodes = case.getValue("nodes").jsonArray.map { n ->
            val read = readNode(n) ?: error("$name: $n is not a node")
            assertEquals("$name: ${read.first.id} reads whole", false, read.second)
            read.first
        }
        val colors = case["colors"]?.jsonObject.orEmpty().mapValues { it.value.jsonPrimitive.content }
        val styles = case["styles"]?.jsonObject.orEmpty().mapValues { Wire.lenient.decodeFromJsonElement(DesignStyle.serializer(), it.value) }
        LayoutCase(name, case.getValue("page").jsonPrimitive.content, nodes.map { it.id }, DesignScene(nodes, DesignLooks(colors, styles)), case)
    }
}
