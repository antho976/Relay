package com.quietsoftware.relay.core.design

/**
 * What a page shows on the PC that the phone leaves out or shows otherwise, counted over the
 * page's visible nodes, for the one-line caption under it (feature-phone §7):
 *
 * - [blur]: nodes with a layer `blur`, drawn sharp.
 * - [glow]: texts, lines, paths and icons casting a shadow (a glow), drawn without it.
 * - [svg]: SVG pictures, shown as a placeholder (Android has no SVG decoder).
 * - [unknown]: nodes of a kind this phone does not know, what a frame with a layout it does not
 *   know holds, and nodes it could not read at all ([DesignGot.unreadable]).
 * - [deep]: nodes more than [DesignScene.MAX_DEPTH] below their page.
 * - [moving]: nodes with `motion`: the PC plays them, the phone shows them at rest.
 */
data class Omissions(
    val blur: Int = 0,
    val glow: Int = 0,
    val svg: Int = 0,
    val unknown: Int = 0,
    val deep: Int = 0,
    val moving: Int = 0,
) {
    /** Whether the phone draws the page as the PC does, at rest. */
    val none: Boolean get() = this == Omissions()
}

/**
 * [Omissions] of [page]'s subtree in [scene]: a hidden node and what it holds count for nothing,
 * as neither side draws them. [unreadable] is the reply's count of nodes the phone could not
 * read, which are counted unknown. A walk with a list, not recursion.
 */
fun omissions(scene: DesignScene, page: String, unreadable: Int = 0): Omissions {
    var blur = 0
    var glow = 0
    var svg = 0
    var unknown = unreadable
    var deep = 0
    var moving = 0
    // Each id with whether it is left out with what holds it (too deep, or under an unknown layout).
    val todo = ArrayDeque<Pair<String, Int>>()
    scene.children(page).forEach { todo.addLast(it to KEEP) }
    while (todo.isNotEmpty()) {
        val (id, why) = todo.removeLast()
        val n = scene.node(id) ?: continue
        if (n.isHidden) continue
        when {
            why == UNDER_UNKNOWN -> unknown++
            why == TOO_DEEP || scene.tooDeep(id) -> deep++
            n.kind !in DesignValues.kinds -> unknown++
            else -> {
                if ((n.blur ?: 0.0) > 0.0) blur++
                if (castsOuterOnly(n.kind) && effectiveShadows(n).isNotEmpty()) glow++
                if (n.kind == "image" && n.src.orEmpty().endsWith(".svg", ignoreCase = true)) svg++
                if (n.motion != null) moving++
            }
        }
        val inner = when {
            why != KEEP -> why
            scene.tooDeep(id) -> TOO_DEEP
            n.kind !in DesignValues.kinds -> continue
            n.kind == "frame" && n.layout != null && n.layout !in DesignValues.layouts -> UNDER_UNKNOWN
            else -> KEEP
        }
        scene.children(id).forEach { todo.addLast(it to inner) }
    }
    return Omissions(blur, glow, svg, unknown, deep, moving)
}

private const val KEEP = 0
private const val TOO_DEEP = 1
private const val UNDER_UNKNOWN = 2
