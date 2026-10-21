package com.quietsoftware.relay.core.design

import com.quietsoftware.relay.core.wire.Wire
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/**
 * A design as `design.list` and `design.get` give it (crates/relay-bus/src/design.rs `Node`,
 * ops/design.rs `GetOut`), read only. Every field of a node is here, round two's included, so
 * the phone draws what the PC draws; `get-reply.json` in the tests holds every one, and
 * crates/relay-bus/tests/phone_fixture.rs fails when the PC gains a field the fixture lacks.
 *
 * Every enum is read as a string (`kind`, `layout`, the sizings, `pin_x`, `stroke_cap`…): a value
 * a newer PC writes would fail a whole node as an enum, or turn silently into the default with
 * `coerceInputValues`. As a string it reaches the rule that says what this phone does with it
 * ([DesignValues], `Look.kt`, `Omissions.kt`).
 */
@Serializable
data class DesignRow(
    val id: Long,
    @SerialName("project_id") val projectId: Long = 0,
    val title: String = "",
    val session: String? = null,
    val rev: Long = 0,
    val pages: Long = 0,
    val nodes: Long = 0,
    @SerialName("created_by") val createdBy: String = "",
    @SerialName("updated_by") val updatedBy: String = "",
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
    @SerialName("deleted_at") val deletedAt: String? = null,
)

@Serializable
data class DesignList(val designs: List<DesignRow> = emptyList())

/** One colour of a gradient: [at] 0 at its start to 1 at its end. */
@Serializable
data class DesignStop(val at: Double = 0.0, val color: String = "")

/** `linear` (along [angle], CSS's reading, 180 top to bottom), `radial` or `conic` (round the centre from [angle], 0 the top). */
@Serializable
data class DesignGradient(val kind: String = "linear", val angle: Double? = null, val stops: List<DesignStop> = emptyList())

@Serializable
data class DesignShadow(
    val x: Double = 0.0,
    val y: Double = 0.0,
    val blur: Double = 0.0,
    val spread: Double = 0.0,
    val color: String = "",
    val inset: Boolean = false,
)

/** Part of an ellipse: from [start] degrees clockwise from the top, [sweep] round, a hole of [inner] of the radius, ends shaped by [cap]. */
@Serializable
data class DesignArc(val start: Double = 0.0, val sweep: Double = 360.0, val inner: Double = 0.0, val cap: String? = null)

/** Characters [start] to [end] of a text (Unicode scalar values, as Rust counts `chars`), styled on their own. */
@Serializable
data class DesignTextRun(
    val start: Int = 0,
    val end: Int = 0,
    val color: String? = null,
    val font: String? = null,
    @SerialName("font_size") val fontSize: Double? = null,
    @SerialName("font_weight") val fontWeight: Int? = null,
    val italic: Boolean? = null,
    val underline: Boolean? = null,
    val strike: Boolean? = null,
    val rise: Double? = null,
)

/** One moment of a node's motion. The phone never plays: it shows a node at rest, its own fields. */
@Serializable
data class DesignKey(
    @SerialName("at_ms") val atMs: Double = 0.0,
    val ease: String? = null,
    val dx: Double? = null,
    val dy: Double? = null,
    val rotate: Double? = null,
    val scale: Double? = null,
    @SerialName("scale_x") val scaleX: Double? = null,
    @SerialName("scale_y") val scaleY: Double? = null,
    val opacity: Double? = null,
    val fill: String? = null,
    val stroke: String? = null,
    val color: String? = null,
    val blur: Double? = null,
    val sweep: Double? = null,
    val angle: Double? = null,
    val trim: Double? = null,
)

@Serializable
data class DesignMotion(
    val keys: List<DesignKey> = emptyList(),
    val ease: String? = null,
    val origin: List<Double>? = null,
    @SerialName("loop") val looped: Boolean? = null,
)

@Serializable
data class DesignTimeline(@SerialName("duration_ms") val durationMs: Double? = null, @SerialName("loop") val looped: Boolean? = null)

/** A node of a design, every field as the PC stores it (design.rs `Node`). */
@Serializable
data class DesignNode(
    val id: String = "",
    val parent: String? = null,
    val kind: String = "",
    val name: String? = null,
    val x: Double = 0.0,
    val y: Double = 0.0,
    val w: Double = 0.0,
    val h: Double = 0.0,
    val fill: String? = null,
    val stroke: String? = null,
    @SerialName("stroke_width") val strokeWidth: Double? = null,
    val radius: Double? = null,
    val opacity: Double? = null,
    val shadow: DesignShadow? = null,
    val clip: Boolean? = null,
    val hidden: Boolean? = null,
    val locked: Boolean? = null,
    /** The PC's layout and contrast checks pass over it; nothing drawn changes. */
    @SerialName("skip_checks") val skipChecks: Boolean? = null,
    /** A main component; its instances are real nodes the PC keeps in step, drawn as any are. */
    val component: Boolean? = null,
    /** On an instance's node, the main's node it follows; nothing drawn changes. */
    @SerialName("instance_of") val instanceOf: String? = null,
    /** The fields an instance's node keeps as its own; nothing drawn changes. */
    val overrides: List<String>? = null,
    /** A frame's chart, as data: the PC draws it as the frame's `generated` children, which the phone draws as any. */
    val chart: JsonObject? = null,
    /** Drawn by its frame's chart; nothing drawn changes. */
    val generated: Boolean? = null,
    val text: String? = null,
    val color: String? = null,
    val font: String? = null,
    @SerialName("font_size") val fontSize: Double? = null,
    @SerialName("font_weight") val fontWeight: Int? = null,
    val italic: Boolean? = null,
    val underline: Boolean? = null,
    val strike: Boolean? = null,
    val align: String? = null,
    @SerialName("line_height") val lineHeight: Double? = null,
    @SerialName("letter_spacing") val letterSpacing: Double? = null,
    @SerialName("w_sizing") val wSizing: String? = null,
    @SerialName("h_sizing") val hSizing: String? = null,
    val layout: String? = null,
    val gap: Double? = null,
    val padding: List<Double>? = null,
    val justify: String? = null,
    val items: String? = null,
    val points: List<List<Double>>? = null,
    @SerialName("arrow_start") val arrowStart: Boolean? = null,
    @SerialName("arrow_end") val arrowEnd: Boolean? = null,
    val src: String? = null,
    val fit: String? = null,
    val rotation: Double? = null,
    val sides: Int? = null,
    val star: Double? = null,
    val gradient: DesignGradient? = null,
    val radii: List<Double>? = null,
    /** A box's border width on each side, top, right, bottom, left, instead of [strokeWidth]. */
    @SerialName("stroke_widths") val strokeWidths: List<Double>? = null,
    val dash: List<Double>? = null,
    val blur: Double? = null,
    /** Frosted glass: the PC blurs what is drawn under it; the phone draws it without ([Omissions]). */
    @SerialName("backdrop_blur") val backdropBlur: Double? = null,
    val icon: String? = null,
    /** An icon's own outline (a saved icon's), drawn instead of [icon]'s: path data on the 24-unit grid. */
    @SerialName("icon_path") val iconPath: String? = null,
    val from: String? = null,
    val to: String? = null,
    val route: String? = null,
    val closed: Boolean? = null,
    val smooth: Boolean? = null,
    @SerialName("flip_x") val flipX: Boolean? = null,
    @SerialName("flip_y") val flipY: Boolean? = null,
    val arc: DesignArc? = null,
    val blend: String? = null,
    @SerialName("text_case") val textCase: String? = null,
    @SerialName("pin_x") val pinX: String? = null,
    @SerialName("pin_y") val pinY: String? = null,
    val absolute: Boolean? = null,
    @SerialName("min_w") val minW: Double? = null,
    @SerialName("max_w") val maxW: Double? = null,
    @SerialName("min_h") val minH: Double? = null,
    @SerialName("max_h") val maxH: Double? = null,
    @SerialName("row_gap") val rowGap: Double? = null,
    val scroll: String? = null,
    @SerialName("stroke_cap") val strokeCap: String? = null,
    @SerialName("stroke_join") val strokeJoin: String? = null,
    @SerialName("stroke_gradient") val strokeGradient: DesignGradient? = null,
    val shadows: List<DesignShadow>? = null,
    val runs: List<DesignTextRun>? = null,
    @SerialName("max_lines") val maxLines: Int? = null,
    val style: String? = null,
    val motion: DesignMotion? = null,
    val timeline: DesignTimeline? = null,
) {
    val isHidden: Boolean get() = hidden == true
}

/**
 * A named look nodes share (style.rs `Style`): the 26 style fields, each as a node holds it. A
 * node takes each one it does not set itself ([resolveLook]).
 */
@Serializable
data class DesignStyle(
    val fill: String? = null,
    val gradient: DesignGradient? = null,
    val stroke: String? = null,
    @SerialName("stroke_width") val strokeWidth: Double? = null,
    val dash: List<Double>? = null,
    val radius: Double? = null,
    val radii: List<Double>? = null,
    val shadow: DesignShadow? = null,
    val shadows: List<DesignShadow>? = null,
    val opacity: Double? = null,
    val blur: Double? = null,
    val blend: String? = null,
    val color: String? = null,
    val font: String? = null,
    @SerialName("font_size") val fontSize: Double? = null,
    @SerialName("font_weight") val fontWeight: Int? = null,
    val italic: Boolean? = null,
    val underline: Boolean? = null,
    val strike: Boolean? = null,
    val align: String? = null,
    @SerialName("line_height") val lineHeight: Double? = null,
    @SerialName("letter_spacing") val letterSpacing: Double? = null,
    @SerialName("text_case") val textCase: String? = null,
    @SerialName("stroke_cap") val strokeCap: String? = null,
    @SerialName("stroke_join") val strokeJoin: String? = null,
    @SerialName("stroke_gradient") val strokeGradient: DesignGradient? = null,
)

/** Where a stock photo came from (ops/picture.rs `PictureCredit`), by the image's `src`. */
@Serializable
data class DesignCredit(
    val source: String = "",
    val id: String = "",
    val title: String = "",
    val creator: String? = null,
    @SerialName("creator_url") val creatorUrl: String? = null,
    val licence: String = "",
    @SerialName("licence_url") val licenceUrl: String? = null,
    @SerialName("source_url") val sourceUrl: String? = null,
    val attribution: String = "",
    val caption: String = "",
)

/**
 * A `design.get` reply as the phone reads it. [unreadable]: nodes left out because they are not
 * a node at all (not an object, or no id or kind it can read), which `Omissions` counts.
 * [partial]: nodes drawn without a field whose value this phone could not read, as the PC reads
 * them (design/read.rs `Node::read` keeps such a value aside and draws the rest).
 */
data class DesignGot(
    val design: DesignRow,
    val nodes: List<DesignNode>,
    val unreadable: Int = 0,
    val partial: Int = 0,
    val colors: Map<String, String> = emptyMap(),
    val styles: Map<String, DesignStyle> = emptyMap(),
    val credits: Map<String, DesignCredit> = emptyMap(),
    val assetsDir: String = "",
) {
    /** The design's named colours and styles, to resolve a node's look with. */
    val looks: DesignLooks get() = DesignLooks(colors, styles)

    /** Its nodes as a tree, each drawn with its style and colours resolved. */
    fun scene(): DesignScene = DesignScene(nodes, looks)
}

/** Null when there is no `design` object; each node, style and credit read on its own (see [DesignGot]). */
fun decodeDesignGot(result: JsonElement?): DesignGot? {
    val o = result as? JsonObject ?: return null
    val design = (o["design"] as? JsonObject)?.let { runCatching { Wire.lenient.decodeFromJsonElement(DesignRow.serializer(), it) }.getOrNull() }
        ?: return null
    var unreadable = 0
    var partial = 0
    val nodes = ArrayList<DesignNode>()
    for (element in o["nodes"] as? JsonArray ?: JsonArray(emptyList())) {
        when (val read = readNode(element)) {
            null -> unreadable++
            else -> {
                nodes += read.first
                if (read.second) partial++
            }
        }
    }
    val colors = (o["colors"] as? JsonObject).orEmpty()
        .mapNotNull { (name, value) -> (value as? JsonPrimitive)?.takeIf { it.isString }?.let { name to it.content } }
        .toMap()
    val styles = (o["styles"] as? JsonObject).orEmpty()
        .mapNotNull { (name, value) -> readWhatReads(value, DesignStyle.serializer(), required = emptySet())?.let { name to it.first } }
        .toMap()
    val credits = (o["credits"] as? JsonObject).orEmpty()
        .mapNotNull { (src, value) -> runCatching { Wire.lenient.decodeFromJsonElement(DesignCredit.serializer(), value) }.getOrNull()?.let { src to it } }
        .toMap()
    val assetsDir = (o["assets_dir"] as? JsonPrimitive)?.contentOrNull.orEmpty()
    return DesignGot(design, nodes, unreadable, partial, colors, styles, credits, assetsDir)
}

/** The keys a node is nothing without: read them or leave the node out. */
private val NODE_REQUIRED = setOf("id", "kind", "parent")

/** One node of a reply, and whether a field of it was left out; null when it is no node. */
internal fun readNode(element: JsonElement): Pair<DesignNode, Boolean>? {
    val read = readWhatReads(element, DesignNode.serializer(), NODE_REQUIRED) ?: return null
    val node = read.first
    if (node.id.isEmpty() || node.kind.isEmpty() || (node.parent == null && node.kind != "page")) return null
    return read
}

/**
 * [element] read as a [T], and whether a key was left out: when the whole does not read, each key
 * is tried alone (beside `kind`, which a node's other fields go with) and the ones that do not
 * read are dropped, as the PC's `Node::read` sets them aside. Null when it is not an object, or a
 * key of [required] does not read.
 */
private fun <T> readWhatReads(element: JsonElement, serializer: KSerializer<T>, required: Set<String>): Pair<T, Boolean>? {
    val o = element as? JsonObject ?: return null
    runCatching { Wire.lenient.decodeFromJsonElement(serializer, o) }.onSuccess { return it to false }
    val kind = o["kind"]
    val kept = o.filter { (key, value) ->
        val alone = if (kind != null && key != "kind") mapOf("kind" to kind, key to value) else mapOf(key to value)
        runCatching { Wire.lenient.decodeFromJsonElement(serializer, JsonObject(alone)) }.isSuccess
    }
    if (required.any { it in o && it !in kept }) return null
    return runCatching { Wire.lenient.decodeFromJsonElement(serializer, JsonObject(kept)) }.getOrNull()?.let { it to true }
}

/**
 * The values of each enum field this phone knows how to draw. Anything else takes the fallback
 * its reader documents: an unknown kind or layout is left out and counted (`Omissions.kt`), an
 * unknown fit is `cover`, an unknown case leaves the words as stored, an unknown cap or join is
 * the default one. `DesignModelTest` holds `get-reply.json`'s values to these sets, and
 * phone_fixture.rs holds the fixture to every value the PC has, so a new value reaches both.
 */
object DesignValues {
    val kinds = setOf("page", "frame", "group", "rect", "ellipse", "line", "text", "sticky", "path", "image", "polygon", "icon")
    val layouts = setOf("none", "row", "column", "wrap")
    val sizings = setOf("fixed", "hug", "fill")
    val justify = setOf("start", "center", "end", "space_between")
    val items = setOf("start", "center", "end", "stretch")
    val align = setOf("left", "center", "right")
    val fits = setOf("cover", "contain", "stretch")
    val routes = setOf("straight", "elbow", "curve")
    val gradients = setOf("linear", "radial", "conic")
    val pinX = setOf("left", "right", "left_right", "center", "scale")
    val pinY = setOf("top", "bottom", "top_bottom", "center", "scale")
    val scrolls = setOf("x", "y", "both")
    val caps = setOf("butt", "round", "square")
    val joins = setOf("miter", "round", "bevel")
    val cases = setOf("upper", "lower", "title")
    val blends = setOf(
        "normal", "multiply", "screen", "overlay", "darken", "lighten", "color_dodge", "color_burn",
        "hard_light", "soft_light", "difference", "exclusion", "hue", "saturation", "color", "luminosity",
    )
}
