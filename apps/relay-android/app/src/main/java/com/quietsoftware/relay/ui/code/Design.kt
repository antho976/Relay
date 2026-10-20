package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.model.Lamp
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
import com.quietsoftware.relay.ui.kit.Radii
import com.quietsoftware.relay.ui.kit.SectionLabel
import com.quietsoftware.relay.ui.kit.Slab
import com.quietsoftware.relay.ui.kit.StaleNote
import com.quietsoftware.relay.ui.kit.T
import com.quietsoftware.relay.ui.kit.ago
import com.quietsoftware.relay.ui.kit.epoch
import com.quietsoftware.relay.ui.kit.plural
import com.quietsoftware.relay.ui.pages.ScrollBody
import com.quietsoftware.relay.ui.shell.Page
import com.quietsoftware.relay.ui.shell.PageBar
import com.quietsoftware.relay.ui.theme.Relay
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/** A design canvas as `design.list` and `design.get` give it (types.rs `Design`). */
@Serializable
private data class DesignRow(
    val id: Long,
    val title: String = "",
    val session: String? = null,
    val pages: Long = 0,
    val nodes: Long = 0,
    @SerialName("updated_by") val updatedBy: String = "",
    @SerialName("updated_at") val updatedAt: String = "",
    @SerialName("deleted_at") val deletedAt: String? = null,
)

@Serializable
private data class DesignList(val designs: List<DesignRow> = emptyList())

/** The fields of a node (types.rs `DesignNode`) this outline draws; the rest is left to the PC. */
@Serializable
internal data class DesignNode(
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
    val clip: Boolean? = null,
    val hidden: Boolean? = null,
    val layout: String? = null,
    val points: List<List<Double>>? = null,
    val rotation: Double? = null,
    val sides: Int? = null,
    val star: Double? = null,
)

@Serializable
private data class DesignGot(val design: DesignRow? = null, val nodes: List<DesignNode> = emptyList())

/**
 * A project's design canvases (design/gallery.rs), read only: each with the terminal it belongs
 * to, when it last changed and by whom, and its pages; the deleted ones apart. Drawing, editing,
 * Restore and Delete forever are on the PC.
 */
@Composable
fun DesignsScreen(projectId: Long, nav: Nav) {
    val c = Relay.colors
    val s by nav.shell.state.collectAsStateWithLifecycle()
    val project = s.projects.firstOrNull { it.num == projectId }
    val live = rememberLive(nav, "design.list", buildJsonObject { put("project_id", projectId); put("include_deleted", true) })
    val all = live.result?.decodeAs<DesignList>()?.designs
    val designs = all.orEmpty().filter { it.deletedAt == null }
    val deleted = all.orEmpty().filter { it.deletedAt != null }
    val lamps = remember(s.sessions) { s.sessions.associate { it.name to it.lamp } }
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar("Design", nav::back, subtitle = project?.name)
            ScrollBody {
                if (all == null) {
                    LiveWait(live, page = true)
                    return@ScrollBody
                }
                if (!live.fresh) StaleNote(live.at)
                if (designs.isEmpty()) {
                    Empty(
                        "layout",
                        "No designs yet",
                        "Sketch screens, flows and ideas on the PC's infinite canvas. Agents in this project make and change designs too: ask one, and watch it draw.",
                    )
                } else {
                    SectionLabel("Designs · ${designs.size}")
                    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                        designs.forEachIndexed { index, d ->
                            if (index > 0) Hairline()
                            DesignLine(d, d.session?.let { lamps[it] ?: Lamp.Off }, onClick = { nav.design(d.id) })
                        }
                    }
                }
                if (deleted.isNotEmpty()) {
                    SectionLabel("Deleted · ${deleted.size}")
                    T("Deleted designs wait here until they are deleted for good. Restore one on the PC.", style = Relay.type.caption, color = c.ink3)
                    Gap(6.dp)
                    Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 4.dp, vertical = 4.dp)) {
                        deleted.forEachIndexed { index, d ->
                            if (index > 0) Hairline()
                            DesignLine(d, d.session?.let { lamps[it] ?: Lamp.Off }, onClick = null)
                        }
                    }
                }
            }
        }
    }
}

/** A gallery card as a row: its title, then the terminal (or "Project only"), when, and its pages. */
@Composable
private fun DesignLine(d: DesignRow, lamp: Lamp?, onClick: (() -> Unit)?) {
    val c = Relay.colors
    val gone = d.deletedAt != null
    val whenText = if (gone) {
        "deleted ${ago(epoch(d.deletedAt))}"
    } else {
        ago(epoch(d.updatedAt)) + if (d.updatedBy.isNotEmpty() && d.updatedBy != "user") " by ${d.updatedBy}" else ""
    }
    ListRow(onClick = onClick, padding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            T(d.title.ifBlank { "Untitled" }, style = Relay.type.uiMedium, color = if (gone) c.ink3 else c.ink, maxLines = 1)
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (lamp != null) {
                    LampDot(lamp, 6.dp)
                    T(" " + d.session.orEmpty(), style = Relay.type.caption, color = c.ink2, maxLines = 1)
                } else {
                    T("Project only", style = Relay.type.caption, color = c.ink2, maxLines = 1)
                }
                T(" · $whenText · ${plural(d.pages, "page")} · ${plural(d.nodes, "layer")}", Modifier.weight(1f), Relay.type.caption, c.ink3, maxLines = 1)
            }
        }
    }
}

/**
 * One design, read only: each page as an outline of its frames and shapes where they sit, with
 * its frames by name and size. Text, pictures, icons and what an auto-layout frame holds are
 * laid out by the PC's canvas, so they are not drawn here.
 */
@Composable
fun DesignScreen(id: Long, nav: Nav) {
    val c = Relay.colors
    val live = rememberLive(nav, "design.get", buildJsonObject { put("design_id", id) })
    val got = live.result?.decodeAs<DesignGot>()
    val design = got?.design
    val kids = remember(got) { got?.nodes.orEmpty().groupBy { it.parent } }
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(design?.title?.ifBlank { null } ?: "Design", nav::back, subtitle = "Design")
            ScrollBody {
                if (got == null || design == null) {
                    LiveWait(live, page = true)
                    return@ScrollBody
                }
                if (!live.fresh) StaleNote(live.at)
                T(
                    listOf(design.session ?: "Project only", "changed ${ago(epoch(design.updatedAt))}", plural(design.pages, "page")).joinToString(" · "),
                    style = Relay.type.caption,
                    color = c.ink3,
                )
                T("An outline of each page: its frames and shapes where they sit. Text, pictures and what an auto-layout frame holds are drawn on the PC.", style = Relay.type.caption, color = c.ink3)
                val pages = got.nodes.filter { it.kind == "page" }
                if (pages.isEmpty()) Empty("layout", "Nothing drawn yet", "This design has no pages.")
                pages.forEachIndexed { index, page ->
                    val frames = pageFrames(page, kids)
                    SectionLabel(page.name?.ifBlank { null } ?: "Page ${index + 1}")
                    PageOutline(page, kids)
                    if (frames.isNotEmpty()) {
                        Gap(8.dp)
                        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
                            frames.forEachIndexed { i, f ->
                                if (i > 0) Hairline()
                                Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
                                    T(f.name?.ifBlank { null } ?: "Frame", Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                                    T("${f.w.toInt()} × ${f.h.toInt()}", style = Relay.type.mono, color = c.ink3)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Whether the outline draws [n] at all: a hidden layer is left out, and so is its row in the frames list. */
private fun shown(n: DesignNode) = n.hidden != true

/** The frames listed under a page's outline: its top-level ones that it draws. */
internal fun pageFrames(page: DesignNode, kids: Map<String?, List<DesignNode>>): List<DesignNode> =
    kids[page.id].orEmpty().filter { it.kind == "frame" && shown(it) }

/** The kinds this outline draws, and the containers it looks into. */
private val DRAWN = setOf("frame", "rect", "sticky", "ellipse", "polygon", "line", "path", "group")

/** The page's top-level nodes drawn to fit the column, on the page's own canvas colour. */
@Composable
private fun PageOutline(page: DesignNode, kids: Map<String?, List<DesignNode>>) {
    val c = Relay.colors
    val top = kids[page.id].orEmpty().filter { shown(it) && it.kind in DRAWN }
    val box = remember(top) { extent(top) }
    if (box == null) {
        T("Nothing on this page the phone can draw.", style = Relay.type.caption, color = c.ink3)
        return
    }
    val ground = hex(page.fill) ?: CANVAS
    val density = LocalDensity.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val pad = with(density) { 12.dp.toPx() }
        val wide = constraints.maxWidth.toFloat()
        val tallest = with(density) { 360.dp.toPx() }
        val zoom = min((wide - 2 * pad) / box.width, (tallest - 2 * pad) / box.height).coerceAtLeast(0.0001f)
        val height = with(density) { (box.height * zoom + 2 * pad).toDp() }
        Canvas(Modifier.fillMaxWidth().height(height).clip(Radii.key).background(ground).border(1.dp, c.edge, Radii.key)) {
            // Centred across; a hairline at any zoom.
            val dx = (size.width - box.width * zoom) / 2 - box.left * zoom
            val dy = pad - box.top * zoom
            withTransform({
                translate(dx, dy)
                scale(zoom, zoom, Offset.Zero)
            }) {
                for (n in top) drawNode(n, kids, 0f, 0f, 1f, 1f / zoom)
            }
        }
    }
}

/** The box around [nodes] in page units; null when there is nothing with a size. */
private fun extent(nodes: List<DesignNode>): androidx.compose.ui.geometry.Rect? {
    var l = Float.MAX_VALUE
    var t = Float.MAX_VALUE
    var r = -Float.MAX_VALUE
    var b = -Float.MAX_VALUE
    for (n in nodes) {
        val x = n.x.toFloat()
        val y = n.y.toFloat()
        val pts = n.points.orEmpty().filter { it.size >= 2 }
        if (pts.isNotEmpty()) {
            for (p in pts) {
                l = min(l, x + p[0].toFloat()); r = max(r, x + p[0].toFloat())
                t = min(t, y + p[1].toFloat()); b = max(b, y + p[1].toFloat())
            }
        }
        if (n.w > 0 && n.h > 0) {
            l = min(l, x); t = min(t, y)
            r = max(r, x + n.w.toFloat()); b = max(b, y + n.h.toFloat())
        }
    }
    if (r <= l || b <= t) return null
    return androidx.compose.ui.geometry.Rect(l, t, r, b)
}

/**
 * One node at its place under a parent whose top-left is ([ox], [oy]) in page units, and what
 * it holds where that is drawn at its own coordinates: a group, or a frame without auto layout.
 */
private fun DrawScope.drawNode(n: DesignNode, kids: Map<String?, List<DesignNode>>, ox: Float, oy: Float, alpha: Float, hair: Float) {
    if (!shown(n) || n.kind !in DRAWN) return
    val a = alpha * (n.opacity?.toFloat() ?: 1f).coerceIn(0f, 1f)
    val left = ox + n.x.toFloat()
    val top = oy + n.y.toFloat()
    val w = n.w.toFloat()
    val h = n.h.toFloat()
    val line = Stroke(max((n.strokeWidth ?: 1.0).toFloat(), hair))
    rotate(n.rotation?.toFloat() ?: 0f, Offset(left + w / 2, top + h / 2)) {
        when (n.kind) {
            "frame", "rect", "sticky" -> if (w > 0 && h > 0) {
                val r = CornerRadius((n.radius ?: if (n.kind == "sticky") 6.0 else 0.0).toFloat())
                drawRoundRect(hex(n.fill) ?: fillOf(n.kind), Offset(left, top), Size(w, h), r, alpha = a)
                hex(n.stroke)?.let { drawRoundRect(it, Offset(left, top), Size(w, h), r, style = line, alpha = a) }
                if (n.kind == "frame" && (n.layout == null || n.layout == "none")) {
                    val inside = kids[n.id].orEmpty()
                    if (n.clip != false) {
                        clipRect(left, top, left + w, top + h) { for (k in inside) drawNode(k, kids, left, top, a, hair) }
                    } else {
                        for (k in inside) drawNode(k, kids, left, top, a, hair)
                    }
                }
            }
            "ellipse" -> if (w > 0 && h > 0) {
                drawOval(hex(n.fill) ?: SHAPE_FILL, Offset(left, top), Size(w, h), alpha = a)
                hex(n.stroke)?.let { drawOval(it, Offset(left, top), Size(w, h), style = line, alpha = a) }
            }
            "polygon" -> if (w > 0 && h > 0) {
                val shape = polygon(left, top, w, h, (n.sides ?: 3).coerceIn(3, 24), n.star?.toFloat())
                drawPath(shape, hex(n.fill) ?: SHAPE_FILL, alpha = a)
                hex(n.stroke)?.let { drawPath(shape, it, style = line, alpha = a) }
            }
            "line", "path" -> {
                val pts = n.points.orEmpty().filter { it.size >= 2 }
                if (pts.size >= 2) {
                    val p = Path().apply {
                        moveTo(left + pts[0][0].toFloat(), top + pts[0][1].toFloat())
                        for (q in pts.drop(1)) lineTo(left + q[0].toFloat(), top + q[1].toFloat())
                    }
                    drawPath(p, hex(n.stroke) ?: STROKE, style = line, alpha = a)
                }
            }
            "group" -> {
                for (k in kids[n.id].orEmpty()) drawNode(k, kids, left, top, a, hair)
            }
        }
    }
}

/** A regular polygon or, with [star], a star, its first point at the top centre (types.rs `polygon`). */
private fun polygon(left: Float, top: Float, w: Float, h: Float, sides: Int, star: Float?): Path {
    val cx = left + w / 2
    val cy = top + h / 2
    val count = if (star != null) sides * 2 else sides
    return Path().apply {
        for (i in 0 until count) {
            val angle = -PI / 2 + i * 2 * PI / count
            val f = if (star != null && i % 2 == 1) star.coerceIn(0.05f, 0.95f) else 1f
            val x = cx + (w / 2) * f * cos(angle).toFloat()
            val y = cy + (h / 2) * f * sin(angle).toFloat()
            if (i == 0) moveTo(x, y) else lineTo(x, y)
        }
        close()
    }
}

// The PC's defaults (design/paint.rs): a white frame, a grey shape, a yellow sticky, a grey stroke.
private val FRAME_FILL = Color(0xFFFFFFFF)
private val SHAPE_FILL = Color(0xFFD9D9D9)
private val STICKY_FILL = Color(0xFFFFE58A)
private val STROKE = Color(0xFF8C8C8C)
private val CANVAS = Color(0xFF1E1D1B)

private fun fillOf(kind: String): Color = when (kind) {
    "frame" -> FRAME_FILL
    "sticky" -> STICKY_FILL
    else -> SHAPE_FILL
}

/** `#RGB`, `#RRGGBB` or `#RRGGBBAA` (design.rs `parse_color`); null when it is none of them. */
private fun hex(text: String?): Color? {
    val h = text?.removePrefix("#") ?: return null
    if (h.isEmpty() || !h.all { it in '0'..'9' || it.lowercaseChar() in 'a'..'f' }) return null
    val full = when (h.length) {
        3 -> h.map { "$it$it" }.joinToString("") + "ff"
        6 -> h + "ff"
        8 -> h
        else -> return null
    }
    val v = full.toLong(16)
    return Color(red = ((v shr 24) and 0xFF).toInt(), green = ((v shr 16) and 0xFF).toInt(), blue = ((v shr 8) and 0xFF).toInt(), alpha = (v and 0xFF).toInt())
}
