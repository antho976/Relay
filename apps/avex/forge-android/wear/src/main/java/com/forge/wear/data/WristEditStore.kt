package com.forge.wear.data

import java.io.File

/**
 * The one wrist edit that has not been acknowledged, on disk (M-10 / H-08).
 *
 * The retry path was RAM-only: the failed edit, its command id and its payload lived on a
 * singleton, and Wear reclaims background processes aggressively — most of all during exactly the
 * window this exists for, a watch out of Bluetooth range waiting for the user to walk back. The
 * process died and the edit went with it, along with the only affordance that could have re-sent
 * it. The file it replaces its own comment described as the gap.
 *
 * One edit, not a queue: the wrist offers a single undo/rate row at a time, and a newer edit
 * supersedes an older one by definition. Written to a sibling and renamed in, so a half-written
 * line can never be read back, and fail-soft throughout — a store that cannot be written leaves
 * the in-memory recovery exactly as good as it was before.
 */
class WristEditStore(private val file: File) {

    /** The pending edit, or null when there is none (or the record cannot be read). */
    fun load(): WristEdit? = runCatching {
        if (!file.isFile) null else WearEditRecovery.decode(file.readText())
    }.getOrNull()

    /**
     * Record [edit] as pending. Called BEFORE the affordance is optimistically removed, so there is
     * no instant in which the edit exists only in a row that has already been taken away.
     */
    fun save(edit: WristEdit) {
        runCatching {
            file.parentFile?.mkdirs()
            val scratch = File(file.parentFile, file.name + TMP_SUFFIX)
            scratch.writeText(WearEditRecovery.encode(edit))
            if (!scratch.renameTo(file)) {
                file.delete()
                if (!scratch.renameTo(file)) scratch.delete()
            }
        }
    }

    /**
     * Forget the pending edit. Only ever called on a positive acknowledgement from the phone, or
     * when the user dismisses the failure — never merely because a message was handed to the
     * transport, which says nothing about whether the phone applied it.
     */
    fun clear() {
        runCatching { file.delete() }
    }

    companion object {
        const val FILE_NAME = "wrist_pending_edit"
        private const val TMP_SUFFIX = ".tmp"
    }
}
