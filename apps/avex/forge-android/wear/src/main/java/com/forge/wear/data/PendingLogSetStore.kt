package com.forge.wear.data

import java.io.File

/**
 * A "Log set" the phone has not answered yet: the command id it went out under, the set it was for
 * and exactly what it said (W01).
 *
 * [setKey] names the set the watch was showing (exercise, set index, target), and [payload] what
 * the user sent for it. A re-tap reuses [commandId] only when both still match: the phone dedupes
 * by command id, so the same id is a replay it recognises, and anything else is a different set.
 */
data class PendingLogSet(
    val commandId: String,
    val sessionId: Long,
    val setKey: String,
    val payload: String
)

/**
 * The pending Log set, on disk (W01).
 *
 * Its identity used to live in SetView's `remember`: the id in flight, the id that timed out and the
 * payload it was sent with. A recreated screen, or a process Wear reclaimed while the watch was out
 * of range, forgot all three, so the re-tap minted a fresh id. If the first send had landed, the
 * phone logged the set twice; and a set that never landed had nothing left to say so. Kept like
 * [WristEditStore]: one record, written to a sibling and renamed in, fail-soft throughout.
 *
 * One field per line: the set key and payload are free text but never hold a line break.
 */
class PendingLogSetStore(private val file: File) {

    fun load(): PendingLogSet? = runCatching {
        if (!file.isFile) return@runCatching null
        val lines = file.readText().split('\n')
        if (lines.size != 5 || lines[0] != FORMAT_V1) return@runCatching null
        PendingLogSet(
            commandId = lines[1].takeIf { it.isNotEmpty() } ?: return@runCatching null,
            sessionId = lines[2].toLongOrNull() ?: return@runCatching null,
            setKey = lines[3],
            payload = lines[4]
        )
    }.getOrNull()

    fun save(pending: PendingLogSet) {
        if ('\n' in pending.setKey || '\n' in pending.payload) return
        runCatching {
            file.parentFile?.mkdirs()
            val scratch = File(file.parentFile, file.name + TMP_SUFFIX)
            scratch.writeText(
                listOf(FORMAT_V1, pending.commandId, pending.sessionId.toString(), pending.setKey, pending.payload)
                    .joinToString("\n")
            )
            if (!scratch.renameTo(file)) {
                file.delete()
                if (!scratch.renameTo(file)) scratch.delete()
            }
        }
    }

    fun clear() {
        runCatching { file.delete() }
    }

    companion object {
        const val FILE_NAME = "wrist_pending_log_set"
        private const val FORMAT_V1 = "v1"
        private const val TMP_SUFFIX = ".tmp"
    }
}
