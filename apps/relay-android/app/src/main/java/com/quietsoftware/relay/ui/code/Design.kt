package com.quietsoftware.relay.ui.code

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import com.quietsoftware.relay.core.design.CANVAS
import com.quietsoftware.relay.core.design.decodeDesignGot
import com.quietsoftware.relay.ui.kit.Pill
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.quietsoftware.relay.core.design.DesignGot
import com.quietsoftware.relay.core.design.DesignList
import com.quietsoftware.relay.core.design.DesignRow
import com.quietsoftware.relay.core.design.Omissions
import com.quietsoftware.relay.core.model.Lamp
import com.quietsoftware.relay.core.model.decodeAs
import com.quietsoftware.relay.data.Relay.Live
import com.quietsoftware.relay.ui.Nav
import com.quietsoftware.relay.ui.kit.Empty
import com.quietsoftware.relay.ui.kit.Gap
import com.quietsoftware.relay.ui.kit.Hairline
import com.quietsoftware.relay.ui.kit.LampDot
import com.quietsoftware.relay.ui.kit.ListRow
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
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

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
                        "Sketch screens, flows and ideas on the PC's infinite canvas. Agents in this project make and change designs too: ask one, and they show up here as it draws.",
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
 * One design, read only, a page at a time: its pages as chips (read alone, a small answer), and
 * the page chosen drawn as the PC draws it (DesignCanvas.kt), with its frames by name and size and
 * a line for what the phone leaves out. A tap on the page opens it full screen. It reads again as
 * the design changes (`design.*` events), so an agent's drawing shows while the screen stays open.
 * A PC too old to read the pages alone is read whole, every page in turn, as before.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DesignScreen(id: Long, nav: Nav) {
    val c = Relay.colors
    val app = LocalContext.current.applicationContext
    val pictures = remember(id) { DesignPictures.of(app, nav.relay, id) }
    val pagesLive = rememberLive(nav, "design.get", remember(id) { buildJsonObject { put("design_id", id); put("pages_only", true) } })
    // An older PC refuses the flag it does not know: it is read whole.
    val whole = pagesLive.error?.code == "bus.schema"
    val listed = remember(pagesLive.result) { decodeDesignGot(pagesLive.result) }
    val pages = listed?.nodes?.filter { it.kind == "page" }.orEmpty()
    var chosen by rememberSaveable(id) { mutableStateOf<String?>(null) }
    val page = chosen?.takeIf { want -> pages.any { it.id == want } } ?: pages.firstOrNull()?.id
    val payload = remember(id, page, whole) {
        when {
            whole -> buildJsonObject { put("design_id", id) }
            page != null -> buildJsonObject { put("design_id", id); put("page", page) }
            else -> null
        }
    }
    val plan = rememberDesignPlan(nav, payload)
    val got = plan?.got
    val design = got?.design ?: listed?.design
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(design?.title?.ifBlank { null } ?: "Design", nav::back, subtitle = "Design")
            ScrollBody {
                if (!whole && listed == null) {
                    LiveWait(pagesLive, page = true)
                    return@ScrollBody
                }
                if (plan == null || plan.live.result == null) {
                    LiveWait(plan?.live ?: Live(), page = true)
                    return@ScrollBody
                }
                if (got == null || design == null) {
                    // An answer came that this phone cannot read: say so, rather than wait for ever.
                    Empty("layout", "This design could not be read here", "Open it on the PC.")
                    return@ScrollBody
                }
                if (!plan.live.fresh) StaleNote(plan.live.at)
                T(
                    listOf(design.session ?: "Project only", "changed ${ago(epoch(design.updatedAt))}", plural(design.pages, "page")).joinToString(" · "),
                    style = Relay.type.caption,
                    color = c.ink3,
                )
                unreadText(got)?.let { T(it, style = Relay.type.caption, color = c.ink3) }
                if (!whole && pages.size > 1) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        pages.forEachIndexed { i, p ->
                            Pill(p.name?.ifBlank { null } ?: "Page ${i + 1}", selected = p.id == page, onClick = { chosen = p.id })
                        }
                    }
                }
                if (plan.pages.isEmpty()) Empty("layout", "Nothing drawn yet", "This design has no pages.")
                for (p in plan.pages) DesignPageBlock(p, pictures, onOpen = { nav.designPage(id, p.id) })
            }
        }
    }
}

/** A page: its name, its canvas (a tap opens it full screen), what the phone leaves out of it, and its frames. */
@Composable
private fun DesignPageBlock(page: PagePlan, pictures: DesignPictures, onOpen: () -> Unit) {
    val c = Relay.colors
    // Its pictures, read as each answer for the page comes (one that failed is tried again).
    LaunchedEffect(page) { pictures.load(pictureSrcs(page)) }
    SectionLabel(page.name)
    when {
        page.failed -> T("This page could not be drawn here. Open it on the PC.", style = Relay.type.caption, color = c.ink3)
        page.bounds == null -> if (page.omissions.none) T("Nothing on this page yet.", style = Relay.type.caption, color = c.ink3)
        else -> PageCanvas(page, Modifier.clickable(onClickLabel = "Open the page full screen", onClick = onOpen), picture = pictures::picture)
    }
    omissionsText(page.omissions)?.let {
        Gap(6.dp)
        T(it, style = Relay.type.caption, color = c.ink3)
    }
    if (page.frames.isNotEmpty()) {
        Gap(8.dp)
        Slab(Modifier.fillMaxWidth(), padding = PaddingValues(horizontal = 14.dp, vertical = 4.dp)) {
            page.frames.forEachIndexed { i, f ->
                if (i > 0) Hairline()
                Row(Modifier.fillMaxWidth().padding(vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
                    T(f.name, Modifier.weight(1f), Relay.type.ui, c.ink, maxLines = 1)
                    T(f.size, style = Relay.type.mono, color = c.ink3)
                }
            }
        }
    }
}

/** The most a page is zoomed in past fitting the screen. */
private const val MOST_PAGE_ZOOM = 8f

/**
 * One page full screen (feature-phone §4.7): fitted at first, then pinched up to 8 times and
 * panned; a double tap fits it again. The page is drawn once at fit and the layer scaled, so its
 * words stay sharp (the display list replays at the layer's scale).
 */
@Composable
fun DesignPageScreen(id: Long, page: String, nav: Nav) {
    val c = Relay.colors
    val app = LocalContext.current.applicationContext
    val pictures = remember(id) { DesignPictures.of(app, nav.relay, id) }
    val plan = rememberDesignPlan(nav, remember(id, page) { buildJsonObject { put("design_id", id); put("page", page) } })
    val shown = plan?.pages?.firstOrNull()
    var zoom by remember(id, page) { mutableFloatStateOf(1f) }
    var pan by remember(id, page) { mutableStateOf(Offset.Zero) }
    Page {
        Column(Modifier.fillMaxSize()) {
            PageBar(shown?.name ?: "Page", nav::back, subtitle = plan?.got?.design?.title?.ifBlank { null } ?: "Design")
            if (shown?.bounds == null) {
                ScrollBody {
                    when {
                        plan == null || plan.live.result == null -> LiveWait(plan?.live ?: Live(), page = true)
                        shown == null -> Empty("layout", "This page is not in the design any more", "It may have been deleted on the PC.")
                        shown.failed -> Empty("layout", "This page could not be drawn here", "Open it on the PC.")
                        else -> Empty("layout", "Nothing on this page yet", "")
                    }
                }
                return@Column
            }
            LaunchedEffect(shown) { pictures.load(pictureSrcs(shown)) }
            val box = shown.bounds
            Box(
                Modifier.fillMaxSize().clipToBounds().background(colorOf(CANVAS)!!)
                    .pointerInput(id, page) {
                        detectTransformGestures { _, move, scale, _ ->
                            zoom = (zoom * scale).coerceIn(1f, MOST_PAGE_ZOOM)
                            pan = if (zoom == 1f) Offset.Zero else pan + move
                        }
                    }
                    .pointerInput(id, page) { detectTapGestures(onDoubleTap = { zoom = 1f; pan = Offset.Zero }) }
                    .semantics { contentDescription = "${shown.name}, full screen; pinch to zoom, double tap to fit" },
            ) {
                Canvas(Modifier.fillMaxSize().graphicsLayer(scaleX = zoom, scaleY = zoom, translationX = pan.x, translationY = pan.y).background(shown.ground)) {
                    val pad = 16.dp.toPx()
                    val fit = fitZoom(box, size.width - 2 * pad, size.height - 2 * pad)
                    val dx = (size.width - box.w.toFloat() * fit) / 2 - box.x.toFloat() * fit
                    val dy = (size.height - box.h.toFloat() * fit) / 2 - box.y.toFloat() * fit
                    withTransform({
                        translate(dx, dy)
                        scale(fit, fit, Offset.Zero)
                    }) {
                        drawPage(shown, 1f / (fit * zoom), pictures::picture)
                    }
                }
                omissionsText(shown.omissions)?.let {
                    T(it, Modifier.align(Alignment.BottomStart).padding(12.dp), Relay.type.caption, c.ink3)
                }
            }
        }
    }
}

/**
 * The line under a page for what the PC shows of it and the phone does not (feature-phone §7):
 * "Not drawn on the phone: blur on 2 layers · 1 SVG picture", and a sentence when layers move.
 * Null when the phone draws the page as the PC does at rest.
 */
internal fun omissionsText(o: Omissions): String? {
    val left = buildList {
        if (o.blur > 0) add("blur on ${plural(o.blur, "layer")}")
        if (o.glow > 0) add("the glow of ${plural(o.glow, "layer")}")
        if (o.svg > 0) add(plural(o.svg, "SVG picture"))
        if (o.unknown > 0) add("${plural(o.unknown, "layer")} from a newer Relay")
        if (o.deep > 0) add("${plural(o.deep, "layer")} nested too deep")
    }
    val lines = listOfNotNull(
        left.takeIf { it.isNotEmpty() }?.joinToString(" · ", prefix = "Not drawn on the phone: ", postfix = "."),
        "Some layers on this page move. The PC plays them; this outline shows them at rest.".takeIf { o.moving > 0 },
    )
    return lines.takeIf { it.isNotEmpty() }?.joinToString("\n")
}

/** What of a design's answer the phone could not read at all, or read without a setting, said once for the design; null when it read it all. */
internal fun unreadText(got: DesignGot): String? = listOfNotNull(
    "${plural(got.unreadable, "layer")} from a newer Relay ${if (got.unreadable == 1) "is" else "are"} left out.".takeIf { got.unreadable > 0 },
    "${plural(got.partial, "layer")} ${if (got.partial == 1) "is" else "are"} drawn without a setting from a newer Relay.".takeIf { got.partial > 0 },
).takeIf { it.isNotEmpty() }?.joinToString(" ")
