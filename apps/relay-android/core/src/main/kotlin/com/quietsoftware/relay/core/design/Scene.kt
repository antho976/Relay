package com.quietsoftware.relay.core.design

/**
 * A design's nodes as a tree (crates/relay-client/src/design/scene.rs `Scene`), from
 * `design.get`'s order: pages first, every other node after its parent, siblings bottom to top.
 *
 * [node] is a node as it draws, its style laid under it and its `$name` colours resolved
 * ([DesignLooks]); [stored] is it as the PC stores it. Everything that places or paints reads
 * [node] (MODEL-SPEC §5 #1).
 *
 * A node more than [MAX_DEPTH] below its page is neither placed nor drawn, and is counted
 * (`Omissions`): the PC refuses to nest deeper (W14), a design an older PC let grow deeper is
 * still read, and the recursions that place and paint stay within a thread's stack. A node whose
 * chain of parents does not reach a page (a parent missing from the reply, or a loop) hangs
 * nowhere and is not drawn either. Nothing here recurses.
 */
class DesignScene(nodes: List<DesignNode>, val looks: DesignLooks = DesignLooks.NONE) {
    private val stored = LinkedHashMap<String, DesignNode>(nodes.size * 2)
    private val drawn = HashMap<String, DesignNode>(nodes.size * 2)
    private val kids = HashMap<String, MutableList<String>>()
    private val depths = HashMap<String, Int>(nodes.size * 2)

    /** The pages, in their order. */
    val pages: List<String>

    init {
        val roots = ArrayList<String>()
        for (n in nodes) {
            // The PC never sends an id twice; were one to come, the first stands.
            if (stored.putIfAbsent(n.id, n) != null) continue
            when (val parent = n.parent) {
                null -> if (n.kind == "page") roots += n.id
                else -> kids.getOrPut(parent) { ArrayList() } += n.id
            }
            // A name the design does not hold resolves too: it is taken out, so the kind's default shows.
            drawn[n.id] = if (DesignLooks.uses(n)) looks.resolve(n) else n
        }
        pages = roots
        for (id in stored.keys) depthAt(id)
    }

    val size: Int get() = stored.size

    /** Every node as it draws, in the reply's order. */
    val nodes: List<DesignNode> get() = stored.keys.map { drawn.getValue(it) }

    /** [id] as it draws: its style laid under it and its colours resolved. */
    fun node(id: String): DesignNode? = drawn[id]

    /** [id] as the PC stores it. */
    fun stored(id: String): DesignNode? = stored[id]

    /** [id]'s children, bottom to top. */
    fun children(id: String): List<String> = kids[id].orEmpty()

    fun parent(id: String): String? = stored[id]?.parent

    /** How many ancestors [id] has, its page included: 0 for a page, 1 for what sits on one; null when it hangs from no page. */
    fun depthOf(id: String): Int? = depths[id]?.takeIf { it != UNHUNG }

    /** Whether [id] is too deep to place or draw (deeper than [MAX_DEPTH]). */
    fun tooDeep(id: String): Boolean = (depthOf(id) ?: 0) > MAX_DEPTH

    /** [id]'s ancestors, nearest first, up to and including its page; empty for a page or a node that hangs nowhere. */
    fun ancestors(id: String): List<String> {
        val depth = depthOf(id) ?: return emptyList()
        val out = ArrayList<String>(depth)
        var at = parent(id)
        while (at != null && out.size < depth) {
            out += at
            at = parent(at)
        }
        return out
    }

    /** The page [id] is on (itself, for a page); null when it hangs from none. */
    fun pageOf(id: String): String? {
        if (depthOf(id) == null) return null
        return ancestors(id).lastOrNull() ?: id
    }

    /** Whether [id] is somewhere inside [ancestor]. */
    fun isInside(id: String, ancestor: String): Boolean = ancestor in ancestors(id)

    /** The depth of [id], working it out up its chain without recursion, each node once. */
    private fun depthAt(id: String): Int {
        depths[id]?.let { return it }
        // The nodes from `id` up whose depth is not known yet, and the depth of the one above the last.
        val chain = ArrayList<String>()
        val inChain = HashSet<String>()
        var above: Int
        var at = id
        while (true) {
            val known = depths[at]
            if (known != null) {
                above = known
                break
            }
            val n = stored[at]
            if (n == null || !inChain.add(at)) {
                above = UNHUNG
                break
            }
            chain += at
            val parent = n.parent
            if (parent == null) {
                above = if (n.kind == "page") -1 else UNHUNG
                break
            }
            at = parent
        }
        for (i in chain.indices.reversed()) {
            above = if (above == UNHUNG) UNHUNG else above + 1
            depths[chain[i]] = above
        }
        return depths[id] ?: UNHUNG
    }

    companion object {
        /** Most ancestors a node the phone draws has, its page included (design.rs `MAX_DEPTH`). */
        const val MAX_DEPTH = 64

        /** The kinds of node the phone knows; a node of another is left out and counted. */
        val KNOWN_KINDS: Set<String> get() = DesignValues.kinds

        /** The layouts the phone lays out; a frame with another is drawn, and what it holds is left out and counted. */
        val KNOWN_LAYOUTS: Set<String> get() = DesignValues.layouts

        private const val UNHUNG = Int.MIN_VALUE
    }
}
