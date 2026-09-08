package com.forge.app

import java.io.File

/**
 * Boot-time restore of a database, preferences, photos and avatar before live storage opens.
 *
 * The immutable set record names every component; the commit journal names outstanding swaps.
 * Original bytes (including committed WAL frames) remain in snapshots until validation succeeds.
 * A separate recovery record fixes the direction before rollback or confirmation starts. Recovery
 * copies from immutable snapshots, then publishes a settled marker before deleting any of them.
 * Every process-death boundary can therefore resume without confusing rollback with validation.
 *
 * [apply] returns true only for a landed set awaiting database validation. A failed recovery is
 * kept on disk and must block consumers; [hasUnsettledRecovery] covers unreadable legacy journals.
 * Storage work belongs on the bootstrap I/O dispatcher, never in Application's main-thread work.
 */
internal object RestoreApply {

    /** Must match `preferencesDataStore(name = "forge_settings")`. */
    private const val PREFS_PATH = "datastore/forge_settings.preferences_pb"
    /** Must match `AvatarRepository.FILE_NAME`. */
    private const val AVATAR_NAME = "avatar.jpg"
    /** Must match `ProgressPhotoRepository`'s folder. */
    private const val PHOTOS_NAME = "progress_photos"

    /** Component names, as the journal records them. */
    private const val DB = "db"
    private const val PREFS = "prefs"
    private const val PHOTOS = "photos"
    private const val AVATAR = "avatar"

    /** Commit order. The database is first and is the anchor; the rest follow it. */
    private val ORDER = listOf(DB, PREFS, PHOTOS, AVATAR)

    /**
     * The commit journal: which components are staged and still need to be renamed into place.
     *
     * Written before the first commit, shortened after each successful one, deleted when empty. Its
     * presence at boot means a restore was interrupted mid-commit and the components it names still
     * have to land.
     */
    private const val JOURNAL = "pending_restore_journal"

    /** Phase 1 puts a component's incoming bytes here, beside where they are going. */
    private const val STAGED_SUFFIX = ".restoring"

    /** Phase 2 puts a component's OUTGOING bytes here, so the swap can be undone. */
    private const val SNAPSHOT_SUFFIX = ".prerestore"

    /**
     * The set's FULL membership, published before the first live rename and untouched until
     * [confirm] or [revert].
     *
     * Two jobs, and it has to be written up front to do either. [revert] needs it because a
     * component that replaced NOTHING has no snapshot, so nothing else on disk says its restored
     * bytes belong to the set being backed out. And rollback needs it because [JOURNAL] is a list of
     * what is LEFT: it stops naming a component the moment that component lands, so a set resumed on
     * a later boot could only ever see the suffix it still had to commit. Deriving membership from
     * that suffix is what let a failed resume roll back three components out of four and leave the
     * fourth — the database — live, with its snapshot then swept as an orphan.
     *
     * Its presence with an EMPTY journal is also the protocol's third state: the set has landed and
     * nothing has yet opened the database, so the next boot owes it a validation rather than a
     * sweep. See [apply].
     */
    private const val SET_RECORD = "pending_restore_set"

    // Direction is durable before recovery mutates anything. Snapshots remain immutable until
    // a settled marker is published, so replaying a partial rollback never deletes restored originals.
    private const val RECOVERY = "pending_restore_recovery"

    /**
     * What [SET_RECORD] replaced: a post-hoc, best-effort record of the same membership, written
     * AFTER the set landed. Only ever read now to be swept, so an install interrupted across the
     * upgrade does not keep a file nothing will ever collect.
     */
    private const val LEGACY_APPLIED = "pending_restore_applied"

    /**
     * What committing one component did.
     *
     * [ALREADY] is the distinction that matters, and conflating it with [FAILED] wedged recovery
     * permanently: a crash between a component's rename and the journal being shortened leaves an
     * entry naming a staged file that is no longer there, because it has become the live file. Read
     * as a failure, that entry can never be retired — every subsequent boot finds nothing to commit,
     * reports the restore unfinished, and skips the orphan sweep because a journal still exists.
     * Read as "this one is done", the journal empties on the very next boot.
     */
    private enum class Commit { LANDED, ALREADY, FAILED }

    /**
     * @return true when a restore reached a fully-applied state on this boot — either one staged
     *   here, or one a previous boot left part-way through and this boot finished. The only case
     *   that may be reported to the user as a successful restore.
     */
    fun hasUnsettledRecovery(filesDir: File): Boolean =
        listOf(JOURNAL, SET_RECORD, RECOVERY).any { File(filesDir, it).exists() }

    fun apply(filesDir: File, liveDb: File): Boolean {
        if (File(filesDir, RECOVERY).exists()) {
            check(resumeRecovery(filesDir, liveDb)) { "Restore recovery is incomplete" }
            return false
        }
        // A journal that EXISTS but cannot be read is the one state in which doing nothing is the
        // only safe move. Its components are mid-commit, so sweeping would strand them and staging a
        // new set would interleave two restores — and we cannot tell which components they are.
        val interrupted = readJournal(filesDir) ?: return false
        // The set record is read under the same rule and for the same reason: unreadable means a set
        // exists whose membership we cannot name, and every branch below needs that membership to
        // act safely.
        val recorded = readSetRecord(filesDir) ?: return false

        // ── Resume: finish what a previous boot started. ──
        //
        // Before anything else, because these components are already committed-or-not against a
        // database that may already have been swapped.
        var resumed = false
        if (interrupted.isNotEmpty()) {
            // Membership, NOT the work list. The record names every component of the set, including
            // any an earlier boot already landed and shortened out of the journal. The fallback is
            // for a set left mid-commit by a build that predates the record and can only be as good
            // as what that build wrote down — the journal — which is why the record is published
            // before the first rename rather than derived here.
            val members = recorded.ifEmpty { interrupted }
            resumed = finishSet(filesDir, liveDb, members = members, remaining = interrupted)
            // A resumed set is spent: its components have landed, so its READY marker has done its job.
            if (resumed) RestoreManifest.discard(filesDir)
        }

        // [finishSet] leaves no journal either way, so anything still here means the clear itself
        // failed. Staging a new set on top of a record we could not retire would mix two restores.
        val settled = readJournal(filesDir)
        if (settled == null || settled.isNotEmpty()) return resumed

        // ── Awaiting validation: landed, but nothing has opened the database yet. ──
        //
        // An empty journal used to mean "no restore in flight", and that is where the set's own
        // safety net was thrown away. A set lands, `apply` returns, and the process dies before Room
        // opens: on the next boot the journal is gone, so the snapshots were swept as orphans and
        // this returned false — skipping the validate/revert branch entirely. The restored database
        // was then permanent and unbacked-out-of, having never once been proven openable.
        //
        // The record outlives the journal precisely so that state is nameable. Returning true here
        // sends the caller back into the Room open it never completed; only [confirm] or [revert]
        // retires the record, so this repeats every boot until one of them answers.
        if (setRecordFile(filesDir).exists()) return true

        // ── Orphans: a crash during staging, before any journal existed. ──
        //
        // Staging renames the pending file away, so a `.restoring` file with no journal naming it is
        // a restore that would otherwise be lost outright. Put it back under the name the next boot
        // looks for. Reached only past the branch above, so no journal AND no set record: nothing is
        // in flight and nothing is awaiting validation, and a snapshot here belongs to no set any
        // longer — see [sweepOrphans] for why that is now a narrower claim than it used to be.
        sweepOrphans(filesDir, liveDb)

        // ── The READY marker: pending files are a restore only once their manifest says so. ──
        //
        // Staging publishes the manifest after the last component, so a process killed part-way
        // leaves components with no manifest, or components that no longer match it: a truncated
        // database, or a whole one beside another backup's preferences. Such a set was never
        // reported as a success, so it is quarantined here, before staging can rename anything.
        if (RestoreManifest.anyPending(filesDir) && !RestoreManifest.verify(filesDir)) {
            quarantine(filesDir)
            return resumed
        }

        val pendingDb = File(filesDir, "pending_restore.db")
        val pendingPrefs = File(filesDir, "pending_restore_prefs.pb")
        val pendingPhotos = File(filesDir, "pending_restore_photos")
        val pendingAvatar = File(filesDir, "pending_restore_avatar.jpg")
        // Captured BEFORE staging, which renames these away.
        val hadDb = pendingDb.exists()
        val hadPrefs = pendingPrefs.exists()
        val hadPhotos = pendingPhotos.isDirectory
        val hadAvatar = pendingAvatar.exists()
        if (!hadDb && !hadPrefs && !hadPhotos && !hadAvatar) return resumed

        val livePrefs = File(filesDir, PREFS_PATH)
        val liveAvatar = File(filesDir, AVATAR_NAME)

        // ── Phase 1: stage beside each destination. Nothing is live yet. ──
        val stagedDb = if (hadDb) stageBeside(pendingDb, liveDb) else null
        val stagedPrefs = if (hadPrefs) stageBeside(pendingPrefs, livePrefs) else null
        val stagedAvatar = if (hadAvatar) stageBeside(pendingAvatar, liveAvatar) else null

        val unstageAll: () -> Unit = {
            stagedDb?.let { unstage(it, pendingDb) }
            stagedPrefs?.let { unstage(it, pendingPrefs) }
            stagedAvatar?.let { unstage(it, pendingAvatar) }
        }

        if ((hadDb && stagedDb == null) || (hadPrefs && stagedPrefs == null) || (hadAvatar && stagedAvatar == null)) {
            // Put back whatever did stage, so the next cold start retries the set as one unit, and
            // leave this boot running entirely on pre-restore data.
            unstageAll()
            return resumed
        }

        // ── Phase 2: commit, journalled and reversible. ──
        val toCommit = ORDER.filter {
            when (it) {
                DB -> stagedDb != null
                PREFS -> stagedPrefs != null
                PHOTOS -> hadPhotos
                else -> stagedAvatar != null
            }
        }
        if (!writeJournal(filesDir, toCommit)) {
            // No journal means no crash recovery for the renames about to happen, which is the whole
            // reason phase 2 has one. Swallowing this failure and committing anyway would leave the
            // unrecoverable mixed state the journal exists to prevent — so abandon the set instead
            // and retry it whole, which costs a boot and loses nothing.
            unstageAll()
            return resumed
        }

        if (!finishSet(filesDir, liveDb, members = toCommit, remaining = toCommit)) return resumed
        RestoreManifest.discard(filesDir)

        if (hadDb && !hadAvatar && hadPrefs) {
            // The restore replaced the prefs but carried no avatar → the restored state has none.
            // Clear any live avatar so a previously-seeded default cover can't outlive the (now
            // blank) avatarDefaultId — otherwise the cover shows but the picker rings nothing.
            // Moved aside like every other pre-restore file rather than deleted, so [revert] can
            // bring it back with the rest and [confirm] discards it with the rest.
            snapshot(liveAvatar)
        }
        if (!hadDb) {
            // The set landed and contains no database, so there is nothing for Room to open and no
            // later boot will ever reach [confirm] for it. Left as-is the record and its snapshots
            // would sit unclaimed forever, and the awaiting-validation branch above would re-report
            // this same restore on every subsequent boot. Prove it here instead: a set with no
            // database has already survived everything that could test it.
            confirm(filesDir, liveDb)
        }
        return hadDb
    }

    /**
     * The restored database has been opened by the application: the set is proven. Release every
     * pre-restore snapshot and the record of what the set replaced.
     */
    fun confirm(filesDir: File, liveDb: File) {
        val recovery = File(filesDir, RECOVERY)
        check(!recovery.exists()) { "Cannot confirm while rollback is pending" }
        check(writeRecovery(filesDir, "settled", ORDER)) { "Could not journal restore confirmation" }
        check(resumeRecovery(filesDir, liveDb)) { "Could not retire restore snapshots" }
    }

    /** Roll back a rejected restore. False leaves all recovery records and snapshots retryable. */
    fun revert(filesDir: File, liveDb: File): Boolean {
        if (File(filesDir, RECOVERY).exists()) return resumeRecovery(filesDir, liveDb)
        val recorded = readSetRecord(filesDir) ?: return false
        val members = (recorded + readLegacyApplied(filesDir) + ORDER.filter {
            liveFor(it, filesDir, liveDb)?.let(::snapshotOf)?.exists() == true
        }).distinct()
        if (members.isEmpty()) return true
        if (!writeRecovery(filesDir, "revert", members)) return false
        return resumeRecovery(filesDir, liveDb)
    }

    private fun writeRecovery(filesDir: File, action: String, members: List<String>): Boolean =
        runCatching {
            val scratch = File(filesDir, "$RECOVERY.tmp")
            scratch.outputStream().use { out ->
                out.write((listOf(action) + members).joinToString("\n").toByteArray())
                out.fd.sync()
            }
            check(scratch.renameTo(File(filesDir, RECOVERY))) { "Could not publish recovery direction" }
            true
        }.getOrDefault(false)

    private fun resumeRecovery(filesDir: File, liveDb: File): Boolean = runCatching {
        val record = File(filesDir, RECOVERY).readLines()
        val action = record.firstOrNull()
        check(action in setOf("revert", "rollback", "settled", "settled-revert")) { "Unknown restore recovery direction" }
        val members = record.drop(1)
        if (action == "revert" || action == "rollback") {
            for (component in members) {
                val live = liveFor(component, filesDir, liveDb) ?: continue
                val pending = pendingFor(component, filesDir) ?: continue
                val snapshot = snapshotOf(live)
                val staged = if (component == PHOTOS) pending.takeIf { it.isDirectory }
                    else stagedBeside(live)
                // If staging still exists and no original was moved aside, this component never
                // changed. Do not delete its live bytes (also handles a failed snapshot operation).
                val untouched = staged != null && !snapshot.exists()
                if (action == "rollback" && !pending.exists()) {
                    val incoming = staged ?: live.takeIf { it.exists() }
                    if (incoming != null) copyPublished(incoming, pending)
                }
                if (!untouched) {
                    if (component == DB) {
                        removeChecked(File(liveDb.path + "-wal"))
                        removeChecked(File(liveDb.path + "-shm"))
                    }
                    if (snapshot.exists()) copyPublished(snapshot, live) else removeChecked(live)
                    if (component == DB) {
                        // WAL is committed database state. SHM is an index SQLite can rebuild.
                        val wal = File(liveDb.path + "-wal")
                        snapshotOf(wal).takeIf { it.exists() }?.let { copyPublished(it, wal) }
                    }
                }
            }
            if (action == "revert") RestoreManifest.discard(filesDir)
            check(writeRecovery(filesDir, if (action == "revert") "settled-revert" else "settled", members)) { "Could not journal completed rollback" }
        }
        // A crash during cleanup resumes here, never back in the destructive recovery loop.
        for (component in members) {
            if (component != PHOTOS) liveFor(component, filesDir, liveDb)?.let(::stagedBeside)?.let(::discard)
            if (action == "revert" || action == "settled-revert")
                pendingFor(component, filesDir)?.let(::removeChecked)
        }
        for (component in ORDER) liveFor(component, filesDir, liveDb)?.let {
            removeChecked(snapshotOf(it))
        }
        removeChecked(snapshotOf(File(liveDb.path + "-wal")))
        removeChecked(snapshotOf(File(liveDb.path + "-shm")))
        removeChecked(journalFile(filesDir))
        removeChecked(setRecordFile(filesDir))
        removeChecked(File(filesDir, LEGACY_APPLIED))
        removeChecked(File(filesDir, RECOVERY))
        true
    }.getOrDefault(false)

    /** Copy first and publish beside the destination; never consume the recovery source. */
    private fun copyPublished(from: File, to: File) {
        to.parentFile?.mkdirs()
        val scratch = File(to.path + ".recovery-copy")
        removeChecked(scratch)
        if (from.isDirectory) {
            check(from.copyRecursively(scratch, overwrite = true)) { "Could not copy ${from.name}" }
        } else {
            from.inputStream().use { input ->
                scratch.outputStream().use { output -> input.copyTo(output); output.fd.sync() }
            }
        }
        if (scratch.isDirectory || to.isDirectory) removeChecked(to)
        check(scratch.renameTo(to)) { "Could not publish ${to.name}" }
    }

    private fun removeChecked(file: File) {
        if (!file.exists()) return
        check(if (file.isDirectory) file.deleteRecursively() else file.delete()) {
            "Could not remove ${file.name}"
        }
    }

    /** Remove a pending set that was never finished, marker first so a crash mid-way leaves no READY. */
    private fun quarantine(filesDir: File) {
        RestoreManifest.discard(filesDir)
        ORDER.forEach { c -> pendingFor(c, filesDir)?.let { discard(it) } }
    }

    private fun setRecordFile(filesDir: File) = File(filesDir, SET_RECORD)

    /**
     * @return the recorded membership; an empty list when no record exists; **null** when one exists
     *   but could not be read — which, exactly like an unreadable journal, means a set is in flight
     *   whose components cannot be named, and the only safe move is to touch nothing.
     */
    private fun readSetRecord(filesDir: File): List<String>? {
        val file = setRecordFile(filesDir)
        if (!file.exists()) return emptyList()
        return runCatching {
            file.readLines().map { it.trim() }.filter { it.isNotEmpty() }
        }.getOrNull()
    }

    private fun readLegacyApplied(filesDir: File): Set<String> = runCatching {
        val f = File(filesDir, LEGACY_APPLIED)
        if (!f.isFile) emptySet() else f.readLines().map { it.trim() }.filter { it.isNotEmpty() }.toSet()
    }.getOrDefault(emptySet())

    /**
     * Publish [members] as the set's membership, if it is not published already.
     *
     * NOT best-effort, which is what its predecessor was: it was written after the set had landed and
     * its failure was swallowed, so a set could go live with nothing on disk naming what it consisted
     * of — and [revert] would then leave every snapshotless component of it in place. This runs
     * BEFORE the first live rename and its caller abandons the set when it fails, so the record and
     * the mutation cannot disagree.
     *
     * An existing record is never rewritten. It is the immutable statement of what the set is; a
     * resume re-deriving it would only ever narrow it to the components still outstanding, which is
     * the defect this file exists to close.
     */
    private fun ensureSetRecord(filesDir: File, members: List<String>): Boolean {
        val target = setRecordFile(filesDir)
        if (target.exists()) return true
        if (members.isEmpty()) return true
        return runCatching {
            val scratch = File(filesDir, "$SET_RECORD.tmp")
            scratch.outputStream().use { it.write(members.joinToString("\n").toByteArray()); it.fd.sync() }
            if (!scratch.renameTo(target)) {
                scratch.delete()
                return@runCatching false
            }
            target.isFile
        }.getOrDefault(false)
    }

    /**
     * Commit [entries] in full, or leave the filesystem exactly as it was before phase 2 began.
     *
     * There is no third outcome, and that is the point: the caller returns straight into
     * `Application.onCreate` finishing, so anything left half-applied here is what the user's next
     * session runs on.
     *
     * @return true when every component landed.
     */
    private fun finishSet(
        filesDir: File,
        liveDb: File,
        members: List<String>,
        remaining: List<String>
    ): Boolean {
        // Membership first, before a single live file moves. Both ways out of phase 2 need the whole
        // set — [rollBack] to put all of it back, [revert] to back all of it out after the fact —
        // and the journal below stops naming a component the instant it lands. A set that cannot
        // publish what it consists of is a set that cannot be undone, so it does not begin.
        if (!ensureSetRecord(filesDir, members)) {
            rollBack(filesDir, liveDb, members)
            return false
        }
        // The pre-restore bytes stay beside each live file: the restored database is not proven
        // until Room has opened it, and until [confirm] says so [revert] must be able to put every
        // component of this set back.
        if (runJournal(filesDir, liveDb, remaining)) return true
        rollBack(filesDir, liveDb, members)
        return false
    }

    /**
     * Commit each component the journal names, shortening it as each lands.
     *
     * A failed shortening is survivable and deliberately not treated as fatal: the component is
     * live, the journal still names it, and the next boot resolves that entry as [Commit.ALREADY]
     * and drops it. The write that MUST succeed is the first one, and its caller checks it.
     *
     * @return true when every entry was retired.
     */
    private fun runJournal(filesDir: File, liveDb: File, entries: List<String>): Boolean {
        var remaining = entries
        for (component in entries.sortedBy { ORDER.indexOf(it) }) {
            if (commitComponent(component, filesDir, liveDb) == Commit.FAILED) return false
            remaining = remaining - component
            writeJournal(filesDir, remaining)
            // Only discard the source once its content is actually in place.
            pendingFor(component, filesDir)?.let { discard(it) }
        }
        return remaining.isEmpty()
    }

    private fun commitComponent(component: String, filesDir: File, liveDb: File): Commit = when (component) {
        DB -> {
            val wal = File(liveDb.path + "-wal")
            // No consumers are open during bootstrap. Preserve the original committed frames
            // before the base-file swap; a retry never overwrites an already captured WAL.
            val saved = runCatching {
                check(!wal.exists() || wal.isFile) { "Database WAL is not a file" }
                if (wal.exists() && !snapshotOf(wal).exists()) copyPublished(wal, snapshotOf(wal))
            }.isSuccess
            if (!saved) Commit.FAILED else commitFile(liveDb) {
                deleteOrThrow(wal)
                deleteOrThrow(File(liveDb.path + "-shm"))
            }
        }
        PREFS -> commitFile(File(filesDir, PREFS_PATH))
        AVATAR -> commitFile(File(filesDir, AVATAR_NAME))
        PHOTOS -> commitPhotos(filesDir)
        // An entry we do not recognise has no staged file and never will. Retire it rather than let
        // one unknown word keep the journal alive forever.
        else -> Commit.ALREADY
    }

    /**
     * Phase 2 for one file: move the current one aside, rename the staged one in, run any post-swap
     * cleanup.
     *
     * The snapshot goes first because it is what makes the rename undoable. If it cannot be taken,
     * the swap is refused rather than done irreversibly — a restore delayed by a boot is a far
     * smaller thing than one that cannot be backed out of half way.
     */
    private fun commitFile(live: File, afterSwap: () -> Unit = {}): Commit {
        val staged = stagedBeside(live) ?: return if (runCatching(afterSwap).isSuccess)
            Commit.ALREADY else Commit.FAILED
        if (!snapshot(live)) return Commit.FAILED
        val ok = runCatching {
            if (!staged.renameTo(live)) error("Could not move ${live.name} into place")
            afterSwap()
        }.isSuccess
        return if (ok) Commit.LANDED else Commit.FAILED
    }

    /** Photos stage AS the pending folder rather than beside the live one, so they swap directly. */
    private fun commitPhotos(filesDir: File): Commit {
        val pending = File(filesDir, "pending_restore_photos")
        if (!pending.isDirectory) return Commit.ALREADY
        val live = File(filesDir, PHOTOS_NAME)
        if (!snapshot(live)) return Commit.FAILED
        return if (pending.renameTo(live)) Commit.LANDED else Commit.FAILED
    }

    /**
     * Undo phase 2 for every component in the set, committed or not.
     *
     * A component that never committed still has its bytes under the staging name; one that did has
     * them live. Both go back to the pending name, because the set is being deferred rather than
     * thrown away — the user asked for this restore and it should be retried whole. The snapshot
     * then goes back over the top, returning the app to exactly the state it booted into. Where
     * there is no snapshot the pre-restore state was "this file does not exist", and moving the
     * restored bytes out to the pending name reproduces that.
     */
    private fun rollBack(filesDir: File, liveDb: File, entries: List<String>) {
        check(writeRecovery(filesDir, "rollback", entries)) { "Could not journal restore rollback" }
        check(resumeRecovery(filesDir, liveDb)) { "Restore rollback is incomplete" }
    }

    /**
     * Move [live] aside so its replacement can be undone.
     *
     * A rename within one directory, so the cost is a directory entry rather than a copy of a
     * multi-megabyte database. Nothing to move is success: the component simply had no pre-restore
     * state to preserve.
     */
    private fun snapshot(live: File): Boolean {
        if (!live.exists()) return true
        val snapshot = snapshotOf(live)
        if (snapshot.exists()) return true
        return move(live, snapshot)
    }

    /** The staged file beside [live], if staging left one there. */
    private fun stagedBeside(live: File): File? =
        File(live.parentFile, "${live.name}$STAGED_SUFFIX").takeIf { it.exists() }

    private fun snapshotOf(live: File) = File(live.parentFile, "${live.name}$SNAPSHOT_SUFFIX")

    private fun liveFor(component: String, filesDir: File, liveDb: File): File? = when (component) {
        DB -> liveDb
        PREFS -> File(filesDir, PREFS_PATH)
        PHOTOS -> File(filesDir, PHOTOS_NAME)
        AVATAR -> File(filesDir, AVATAR_NAME)
        else -> null
    }

    private fun pendingFor(component: String, filesDir: File): File? = when (component) {
        DB -> File(filesDir, "pending_restore.db")
        PREFS -> File(filesDir, "pending_restore_prefs.pb")
        PHOTOS -> File(filesDir, "pending_restore_photos")
        AVATAR -> File(filesDir, "pending_restore_avatar.jpg")
        else -> null
    }

    /**
     * Return every `.restoring` file to the name the next boot looks for, and drop stale snapshots.
     *
     * Only reached with no journal AND no [SET_RECORD], which together mean no restore is in flight
     * and none is awaiting validation — nothing on disk can ever claim these files. A `.restoring`
     * file here is a crash during STAGING: the pending file had been renamed away and nothing
     * recorded that it existed, so without this the restore is not merely delayed, it is gone.
     *
     * A `.prerestore` file here is an unowned leftover, which under this protocol means one written
     * by a build that predates the set record — the sweep used to run whenever the journal was empty,
     * and that is exactly how it discarded the rollback copy of a set that had landed and not yet
     * been proven. A set that landed under the current protocol still has its record, so it is
     * caught by the awaiting-validation branch in [apply] long before it reaches here.
     */
    private fun sweepOrphans(filesDir: File, liveDb: File) {
        ORDER.forEach { component ->
            val live = liveFor(component, filesDir, liveDb) ?: return@forEach
            val pending = pendingFor(component, filesDir) ?: return@forEach
            if (component != PHOTOS) {
                stagedBeside(live)?.let { if (pending.exists()) discard(it) else move(it, pending) }
            }
            snapshotOf(live).takeIf { it.exists() }?.let { discard(it) }
        }
        discard(File(filesDir, LEGACY_APPLIED))
    }

    private fun journalFile(filesDir: File) = File(filesDir, JOURNAL)

    /**
     * @return the journalled components; an empty list when there is no journal; **null** when one
     *   exists but could not be read, which the caller must treat as "a restore is in flight and I
     *   do not know which components", not as "there is nothing to do".
     */
    private fun readJournal(filesDir: File): List<String>? {
        val file = journalFile(filesDir)
        if (!file.exists()) return emptyList()
        return runCatching {
            file.readLines().map { it.trim() }.filter { it.isNotEmpty() }
        }.getOrNull()
    }

    /**
     * @return whether the journal now says exactly [remaining].
     *
     * Written to a sibling and renamed in, because a journal half-written when the process dies is
     * worse than no journal at all: the next boot would act on a truncated list and quietly skip
     * whatever was cut off. The rename is the only step that publishes it, and it is atomic.
     */
    private fun writeJournal(filesDir: File, remaining: List<String>): Boolean = runCatching {
        val file = journalFile(filesDir)
        if (remaining.isEmpty()) return@runCatching !file.exists() || file.delete()
        val scratch = File(filesDir, "$JOURNAL.tmp")
        scratch.outputStream().use { it.write(remaining.joinToString("\n").toByteArray()); it.fd.sync() }
        if (!scratch.renameTo(file)) {
            scratch.delete()
            return@runCatching false
        }
        true
    }.getOrDefault(false)

    /**
     * Phase 1 for one component: get [pending]'s bytes next to [live] without touching [live].
     *
     * Rename first to avoid copying large databases. Cross-filesystem staging falls back to a
     * copy. Bootstrap runs on IO, before opening live storage.
     *
     * @return the staged file, or null if the bytes could not be placed — in which case [live] is
     *   exactly as it was.
     */
    fun stageBeside(pending: File, live: File): File? = runCatching {
        live.parentFile?.mkdirs()
        val staged = File(live.parentFile, "${live.name}$STAGED_SUFFIX")
        if (staged.exists()) staged.delete()
        if (!pending.renameTo(staged)) pending.copyTo(staged, overwrite = true)
        staged
    }.getOrNull()

    /**
     * Abandon a staged component: put its bytes back under the name the next boot looks for.
     *
     * Called when a sibling could not be staged. Without it the staged file would sit under a
     * `.restoring` name that nothing reads again, and the restore would be silently lost.
     */
    fun unstage(staged: File, pending: File) {
        move(staged, pending)
    }

    /** Rename [from] onto [to], falling back to a copy across filesystems. Handles folders. */
    private fun move(from: File, to: File): Boolean = runCatching {
        if (from.renameTo(to)) return@runCatching true
        // Publish only a complete copy. A partial .prerestore file must never be mistaken for
        // an original snapshot after disk exhaustion or process death during the fallback.
        copyPublished(from, to)
        discard(from)
        true
    }.getOrDefault(false)

    private fun discard(target: File) {
        runCatching { if (target.isDirectory) target.deleteRecursively() else target.delete() }
    }

    /** Delete [f]; throw if it survives so the enclosing `runCatching` treats the swap as failed. */
    private fun deleteOrThrow(f: File) {
        if (f.exists() && !f.delete() && f.exists()) error("Could not delete ${f.name}")
    }
}
