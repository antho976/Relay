package com.quietsoftware.relay.ui

import com.quietsoftware.relay.core.sync.EventRouter
import com.quietsoftware.relay.core.sync.QueryKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Every query a screen shows live (`Relay.live`) is read again when the PC says its domain moved,
 * which it can only say for events the phone subscribed to: the op's domain has `"<domain>.*"` in
 * [EventRouter.SUBSCRIBE], or is named in [NO_EVENTS] with why it needs none. Without it a screen
 * that stays open shows its first answer for good, as an open design did while an agent drew it (B4).
 */
class LiveDomainsTest {
    private val sources = File("src/main/java").walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()

    @Test
    fun `every domain a screen reads live is one the phone hears about`() {
        val reads = liveReads()
        assertTrue("no live reads found under ${File("src/main/java").absolutePath}", "design.get" in reads && "design.list" in reads)
        val unheard = reads.filterKeys { op -> QueryKey.domain(op).let { "$it.*" !in EventRouter.SUBSCRIBE && it !in NO_EVENTS } }
        assertEquals("add \"<domain>.*\" to EventRouter.SUBSCRIBE, or say in NO_EVENTS why the domain has no events", emptyMap<String, Set<String>>(), unheard)
        // An exclusion no screen needs any more, or one the phone now subscribes to, is stale.
        val domains = reads.keys.map(QueryKey::domain).toSet()
        assertEquals(emptyList<String>(), NO_EVENTS.keys.filter { it !in domains || "$it.*" in EventRouter.SUBSCRIBE })
    }

    /** A live read whose op is not written at the call escapes the scan; only these pass one through. */
    @Test
    fun `every live read names its op where the scan sees it`() {
        val through = sources.filter { PASSED.containsMatchIn(it.readText()) }.map { it.name }.toSet()
        assertEquals(PASS_THROUGH.keys, through)
        for ((file, ops) in PASS_THROUGH) {
            val text = sources.single { it.name == file }.readText()
            for (op in ops) assertTrue("$file no longer reads $op", "\"$op\"" in text)
        }
    }

    /** Each op read live, with the files that read it. */
    private fun liveReads(): Map<String, Set<String>> {
        val reads = sortedMapOf<String, MutableSet<String>>()
        for (f in sources) for (m in NAMED.findAll(f.readText())) reads.getOrPut(m.groupValues[1]) { sortedSetOf() } += f.name
        for ((file, ops) in PASS_THROUGH) for (op in ops) reads.getOrPut(op) { sortedSetOf() } += file
        return reads
    }

    companion object {
        /** `rememberLive(nav, "op"`, the Threads space's `rememberLive("op"`, and `.live("op"`. */
        private val NAMED = Regex("""(?:rememberLive\((?:nav,\s*)?|\.live\()"([a-z_]+(?:\.[a-z_]+)+)"""")

        /** The same calls with anything but a written op first (possessive: `nav, ` cannot be given back). */
        private val PASSED = Regex("""rememberLive\((?:nav,\s*)?+(?!")|\.live\((?!")""")

        /** The files that read live through a variable, and the ops that variable can hold. */
        private val PASS_THROUGH = mapOf(
            // Both `rememberLive`s; their callers name the op and are scanned.
            "CodeCommon.kt" to emptyList(),
            "Sources.kt" to emptyList(),
            // A chart in a thread reads one of these, picked by what the chart shows.
            "Charts.kt" to listOf("arbiter.series", "gym.series", "money.series"),
        )

        /** Domains read live that the PC sends no events for, and how their screens keep up. */
        private val NO_EVENTS = mapOf(
            "app" to "the PC sends no app events; the backups page reads its list again after each backup",
            "dashboard" to "the PC sends no dashboard events; the syncer reads it again on the events that move it",
        )
    }
}
