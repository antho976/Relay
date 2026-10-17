package com.forge.app.service.wear

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.flow.updateAndGet
import java.net.URLDecoder
import java.net.URLEncoder
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Session-scoped workout marks Room never stores: which slots the user declared "done with this
 * exercise" early (DayUiEvent.FinishExerciseEarly; done < planned in Room forever), and how many
 * sets "+ add a set" put on top of a slot's plan. The day screen restores both from here, and the
 * wrist mirror's current-slot pick scans PAST the early-done slots instead of pinning the wrist to
 * an exercise the phone already filed under DONE.
 *
 * Kept for the session, not for the screen: both used to live only in the day screen's ViewModel,
 * so stepping back to Home and reopening the workout, or the system reclaiming the app between
 * sets, put a finished exercise back in the queue and dropped the added sets. They are written
 * through to a small preferences file and read back on first use; marks for another session are
 * replaced, never merged.
 */
@Singleton
class WearFocusHolder @Inject constructor(@ApplicationContext private val context: Context) {

    /** Slot ids (plan-slot ids, not swapped exercise ids) finished early, for [sessionId]. */
    data class EarlyDone(val sessionId: Long, val slotIds: Set<String>)

    /** Everything marked for one session: early-done slots and bonus sets per slot. */
    data class Marks(
        val sessionId: Long,
        val earlyDone: Set<String> = emptySet(),
        val bonusSets: Map<String, Int> = emptyMap()
    )

    private val prefs by lazy { context.getSharedPreferences(PREFS, Context.MODE_PRIVATE) }

    private val marks = MutableStateFlow<Marks?>(null)
    @Volatile private var loaded = false

    private val _earlyDone = MutableStateFlow<EarlyDone?>(null)
    val earlyDone: StateFlow<EarlyDone?> = _earlyDone

    /** Read the stored marks once. The first call reads a file, so callers on the main thread hop off it. */
    fun load() {
        if (loaded) return
        synchronized(this) {
            if (loaded) return
            val stored = runCatching { decodeMarks(prefs.getString(KEY, null)) }.getOrNull()
            marks.update { it ?: stored }
            loaded = true
            publish()
        }
    }

    /**
     * Mark a slot finished early. Called from the ViewModel's scope and, via `WearSyncService`'s
     * `runBlocking`, from a Binder thread — so the mutation is an atomic `update {}` rather than a
     * read-modify-write. The old form could drop a slot mark when a FinishExerciseEarly landed
     * while the wrist was reading, pinning the watch to an exercise the phone had already filed
     * under done.
     */
    fun markEarlyDone(sessionId: Long, slotId: String) = mutate(sessionId) { it.copy(earlyDone = it.earlyDone + slotId) }

    /** Record [count] sets added beyond [slotId]'s plan this session (0 clears it). */
    fun setBonusSets(sessionId: Long, slotId: String, count: Int) = mutate(sessionId) {
        it.copy(bonusSets = if (count > 0) it.bonusSets + (slotId to count) else it.bonusSets - slotId)
    }

    /** The early-done slot ids for [sessionId] (empty when the marks belong to another session).
     *  Read CROSS-THREAD — from the Binder thread inside `SetLogUseCase` / `WatchSessionMirror`. */
    fun earlyDoneFor(sessionId: Long): Set<String> {
        load()
        return marks.value?.takeIf { it.sessionId == sessionId }?.earlyDone ?: emptySet()
    }

    /** The bonus sets per slot for [sessionId] (empty when the marks belong to another session). */
    fun bonusSetsFor(sessionId: Long): Map<String, Int> {
        load()
        return marks.value?.takeIf { it.sessionId == sessionId }?.bonusSets ?: emptyMap()
    }

    private fun mutate(sessionId: Long, change: (Marks) -> Marks) {
        load()
        val next = marks.updateAndGet { current ->
            change(current?.takeIf { it.sessionId == sessionId } ?: Marks(sessionId))
        } ?: return
        publish()
        runCatching { prefs.edit().putString(KEY, encodeMarks(next)).apply() }
    }

    private fun publish() {
        _earlyDone.value = marks.value?.let { EarlyDone(it.sessionId, it.earlyDone) }
    }

    private companion object {
        const val PREFS = "session_marks"
        const val KEY = "marks"
    }
}

/**
 * One line per field: the session id, the early-done slots, then `slot=count` pairs. Ids are
 * URL-encoded, so a custom exercise's id can hold any character without breaking the line.
 */
internal fun encodeMarks(m: WearFocusHolder.Marks): String {
    fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
    return listOf(
        m.sessionId.toString(),
        m.earlyDone.joinToString(",") { enc(it) },
        m.bonusSets.entries.joinToString(",") { (k, v) -> "${enc(k)}=$v" }
    ).joinToString("\n")
}

/** [encodeMarks]' inverse; null for nothing stored or anything it cannot read. */
internal fun decodeMarks(text: String?): WearFocusHolder.Marks? {
    if (text.isNullOrBlank()) return null
    fun dec(s: String) = URLDecoder.decode(s, "UTF-8")
    val lines = text.split("\n")
    val id = lines.getOrNull(0)?.toLongOrNull() ?: return null
    val early = lines.getOrNull(1).orEmpty().split(",").filter { it.isNotEmpty() }.map(::dec).toSet()
    val bonus = lines.getOrNull(2).orEmpty().split(",").filter { it.isNotEmpty() }.mapNotNull { pair ->
        val k = pair.substringBefore('=', "")
        val v = pair.substringAfter('=', "").toIntOrNull()
        if (k.isEmpty() || v == null || v <= 0) null else dec(k) to v
    }.toMap()
    return WearFocusHolder.Marks(id, early, bonus)
}
