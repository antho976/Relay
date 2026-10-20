package com.quietsoftware.relay.core.design

import com.quietsoftware.relay.core.wire.Wire
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * The icons an `icon` node draws: outline geometry on a 24-unit grid as SVG path data, stroked
 * with round caps and joins in the node's stroke. A copy of crates/relay-client/src/design/icons.rs
 * in `design/icons.json` (this module's resources), which crates/relay-client/tests/phone_fixtures.rs
 * holds to the PC's set, name for name and path for path, so an icon the PC gains fails `cargo
 * test` until the phone has it too.
 */
object DesignIcons {
    private val paths: Map<String, String> by lazy { load() }

    /** Every icon's name, in the PC's order (design.rs `ICONS`). */
    val names: List<String> get() = paths.keys.toList()

    /** An icon's path data; null for a name not in the set, which draws nothing (the PC's rule). */
    fun path(name: String?): String? = name?.let { paths[it] }

    private fun load(): Map<String, String> {
        val text = DesignIcons::class.java.getResourceAsStream("/design/icons.json")?.use { it.readBytes().decodeToString() }
            ?: error("design/icons.json is missing from the phone's resources")
        val o: JsonObject = Wire.parse(text) ?: error("design/icons.json is not a JSON object")
        val out = LinkedHashMap<String, String>(o.size * 2)
        for ((name, value) in o) {
            val path = (value as? JsonPrimitive)?.takeIf { it.isString }?.content ?: error("design/icons.json: $name is not path data")
            out[name] = path
        }
        return out
    }
}
