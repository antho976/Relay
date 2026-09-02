package com.forge.app

import java.io.File
import java.security.MessageDigest

/**
 * The READY marker of a staged restore: written last by `BackupRepository`, checked first by
 * [RestoreApply].
 *
 * Staging writes up to four `pending_restore_*` components one after another, and the boot used to
 * treat the existence of ANY of them as a complete, requested set. A process killed part-way
 * through staging — after `pending_restore.db` was created, or while its bytes were still being
 * copied — therefore left behind something the next boot renamed live: a truncated database, or a
 * complete one paired with the preferences and photos of a different backup. The restore screen
 * had never reported success, and the rollback snapshot was discarded before the replacement was
 * ever opened.
 *
 * The manifest closes that. It lists every component of the set with its size and SHA-256, and is
 * published through a temp file and an atomic rename only once every component has landed. So:
 *
 *  - no manifest → the set was never finished. It is quarantined (deleted), never applied.
 *  - a manifest whose description no longer matches the files → something was cut short or
 *    changed. Quarantined too.
 *  - a manifest that matches → this is exactly the set staging finished, and it may be applied.
 *
 * Verification hashes the files again at boot rather than trusting a flag, because a flag says
 * "someone wrote me" and a hash says "these bytes are the ones that were validated".
 */
internal object RestoreManifest {

    /** Lives in `filesDir` beside the components it describes. */
    const val NAME = "pending_restore_manifest"

    private const val FORMAT = "v1"

    /** The staging names, as `BackupRepository` writes them and [RestoreApply] reads them. */
    private val COMPONENTS = listOf(
        "pending_restore.db",
        "pending_restore_prefs.pb",
        "pending_restore_avatar.jpg",
        "pending_restore_photos"
    )

    fun file(filesDir: File): File = File(filesDir, NAME)

    /** Whether any staged component is on disk, manifest or not. */
    fun anyPending(filesDir: File): Boolean = COMPONENTS.any { File(filesDir, it).exists() }

    /**
     * Describe every pending component present and publish the description atomically.
     *
     * @return false when there is nothing to describe, a file could not be read, or the manifest
     *   could not be written — in every case the set must be treated as not staged.
     */
    fun publish(filesDir: File): Boolean = runCatching {
        val lines = describe(filesDir) ?: return false
        if (lines.isEmpty()) return false
        val target = file(filesDir)
        val scratch = File(filesDir, "$NAME.tmp")
        scratch.writeText((listOf(FORMAT) + lines).joinToString("\n"))
        if (!scratch.renameTo(target)) {
            target.delete()
            if (!scratch.renameTo(target)) {
                scratch.delete()
                return false
            }
        }
        true
    }.getOrDefault(false)

    /**
     * True only when a manifest exists and the components on disk are exactly the ones it
     * describes: same names, same sizes, same content hashes, nothing extra and nothing missing.
     */
    fun verify(filesDir: File): Boolean = runCatching {
        val f = file(filesDir)
        if (!f.isFile) return false
        val lines = f.readLines().map { it.trim() }.filter { it.isNotEmpty() }
        if (lines.firstOrNull() != FORMAT) return false
        val actual = describe(filesDir) ?: return false
        lines.drop(1) == actual
    }.getOrDefault(false)

    /** Remove the marker (and any half-written scratch). Called once a set is spent or refused. */
    fun discard(filesDir: File) {
        runCatching { file(filesDir).delete() }
        runCatching { File(filesDir, "$NAME.tmp").delete() }
    }

    /**
     * One line per file, `path<TAB>size<TAB>sha256`, in a fixed order. A photo folder is listed
     * file by file (flat, as staging writes it); an empty folder still records that it exists.
     * Null when a file could not be read, which callers treat as "cannot vouch for this set".
     */
    private fun describe(filesDir: File): List<String>? {
        val out = mutableListOf<String>()
        for (name in COMPONENTS) {
            val f = File(filesDir, name)
            if (!f.exists()) continue
            if (f.isDirectory) {
                val children = f.listFiles()?.filter { it.isFile }?.sortedBy { it.name } ?: return null
                if (children.isEmpty()) out += "$name/\t0\t-"
                for (child in children) out += line("$name/${child.name}", child)
            } else {
                out += line(name, f)
            }
        }
        return out
    }

    private fun line(path: String, f: File): String = "$path\t${f.length()}\t${sha256(f)}"

    private fun sha256(f: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        f.inputStream().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(buffer)
                if (n < 0) break
                digest.update(buffer, 0, n)
            }
        }
        return digest.digest().joinToString("") { b -> "%02x".format(b.toInt() and 0xff) }
    }
}
