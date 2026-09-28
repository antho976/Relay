package com.forge.app.data.repo

import androidx.room.withTransaction
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.dao.AdviceEventDao
import com.forge.app.data.db.dao.CardioDao
import com.forge.app.data.db.dao.CoachDao
import com.forge.app.data.db.dao.MoodDao
import com.forge.app.data.db.dao.RestDayDao
import com.forge.app.data.db.dao.RestEventDao
import com.forge.app.data.db.dao.SessionDao
import com.forge.app.data.db.dao.SuggestionOutcomeDao
import com.forge.app.data.db.dao.TrophyNearMissDao
import com.forge.app.data.db.dao.UnlockedTrophyDao
import com.forge.app.data.health.HealthConnectManager
import com.forge.app.data.prefs.SettingsRepository
import com.forge.app.domain.health.HcRecordKeys
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Handles the destructive reset operations in the Settings screen (#119).
 *
 * Every reset that removes rows Avex has mirrored to Health Connect takes the mirrors with them
 * (M-02): "Deletes ALL data" used to clear the local tables while Samsung Health / Fit kept every
 * Avex-written session, calorie, heart-rate, weight and body-fat record. The mirrors are keyed on
 * local row ids ([HcRecordKeys]), so each reset captures the keys BEFORE its wipe; the local
 * delete then runs first and never waits on the provider, and the external cleanup that follows
 * is best-effort and fail-soft (no provider, no grant or a provider error cannot fail the reset).
 */
@Singleton
class ResetRepository @Inject constructor(
    private val sessionDao: SessionDao,
    private val trophyDao: UnlockedTrophyDao,
    private val cardioDao: CardioDao,
    private val moodDao: MoodDao,
    private val suggestionOutcomeDao: SuggestionOutcomeDao,
    private val adviceEventDao: AdviceEventDao,
    private val restEventDao: RestEventDao,
    private val restDayDao: RestDayDao,
    private val coachDao: CoachDao,
    private val nearMissDao: TrophyNearMissDao,
    private val settingsRepo: SettingsRepository,
    private val photoRepo: ProgressPhotoRepository,
    private val avatarRepo: AvatarRepository,
    private val backupRepo: BackupRepository,
    private val health: HealthConnectManager,
    private val clock: com.forge.app.core.time.Clock,
    private val db: ForgeDatabase,
    @dagger.hilt.android.qualifiers.ApplicationContext private val context: android.content.Context
) {
    /**
     * "Deletes all sessions, sets, and exercises logged. Cannot be undone." — and the tables that
     * were *derived from* those sessions have to go with them.
     *
     * `DELETE FROM session` only reaches what declares a foreign key on `session.id`. Everything the
     * engine learned from that history — `coach_decision` / `coach_pass` (which bias EVERY future
     * program generation through `CoachGenBias`), `suggestion_outcome` (the weight-chip step
     * calibrator), `advice_event`, `trophy_near_miss`, `rest_day_entry` — has no FK and used to
     * survive. `mood_entry` survived too, with `session_id` SET NULL, and kept feeding readiness.
     * The user asked for a clean slate and got a "fresh" program silently shaped by the data they
     * had just erased.
     *
     * One transaction so a reset is all-or-nothing rather than half a wipe. The session ids are
     * read first: they are the only handle on the Health Connect mirrors, and the wipe destroys them.
     */
    suspend fun resetSessions() {
        val sessionIds = sessionDao.allIds()
        db.withTransaction {
            sessionDao.deleteAll()
            moodDao.deleteAll()
            suggestionOutcomeDao.deleteAll()
            adviceEventDao.deleteAll()
            restEventDao.deleteAll()
            restDayDao.deleteAll()
            coachDao.deleteAllDecisions()
            coachDao.deleteAllPasses()
            nearMissDao.deleteAll()
        }
        health.deleteSessionMirrors(sessionIds)
    }

    suspend fun resetTrophies() = trophyDao.deleteAll()

    /** Cardio entries plus the exercise sessions Avex mirrored for them (ids captured before the wipe). */
    suspend fun resetCardio() {
        val entryIds = cardioDao.allIds()
        cardioDao.deleteAll()
        health.deleteExerciseSessions(entryIds.map(HcRecordKeys::cardio))
    }

    /** Restore default preferences but keep onboarding/identity so the user isn't re-onboarded. */
    suspend fun resetAppSettings() = settingsRepo.resetSettingsOnly()

    /**
     * True clean slate: wipe EVERY database table (via [ForgeDatabase.clearAllTables], so no table
     * is ever forgotten as the schema grows) plus all preferences. Clearing the onboarding flag
     * routes the user back through onboarding, which regenerates the program.
     *
     * Also every copy of that data the app keeps in its own storage: the auto-backup ZIP, exports,
     * crash logs and any staged restore ([BackupRepository.deleteLocalCopies]). Backups the user
     * saved to a folder they picked are outside the app and stay theirs.
     */
    suspend fun factoryReset() {
        // Marked first, cleared last: see [finishInterruptedFactoryReset].
        withContext(Dispatchers.IO) { pendingResetMarker().createNewFile() }
        withContext(Dispatchers.IO) { db.clearAllTables() }
        eraseOutsideTheStores()
        settingsRepo.resetAll()
        withContext(Dispatchers.IO) { pendingResetMarker().delete() }
    }

    /**
     * Everything [factoryReset] erases besides the database and the settings store. It opens
     * neither, so the boot path can run it while both are still held shut by the startup gate.
     */
    private suspend fun eraseOutsideTheStores() {
        photoRepo.deleteAll() // progress photos live as files outside the DB — clear them too (#138).
        avatarRepo.clear()    // the avatar is an app-private file too.
        // The auto-backup ZIP (database, preferences AND photos), exports, crash logs and a staged
        // restore all survived "Deletes ALL data" (2026-09-26 audit, D2). Inside the marker, so an
        // interrupted reset finishes this at boot too, and a weekly backup racing the reset cannot
        // land a copy of the old data after this sweep (BackupRepository.autoBackup checks it).
        backupRepo.deleteLocalCopies()
        // Every record Avex ever wrote to Health Connect, of every type, by time range rather than
        // by key: the tables are already gone, and a range delete is scoped by the provider to
        // this app's own records, so nothing of another app's is touched. Before the preference
        // wipe so the reset's external half runs while the grants and opt-ins still describe it.
        health.deleteAllAvexRecords(clock.nowMs())
        releaseFolderGrants()
    }

    /**
     * The preference wipe forgets the backup and import folders, and a persisted grant outlives
     * the setting that named it, so the reset gives up every tree Avex holds (M-18). They are the
     * only persisted grants the app ever takes ([PersistedTreeGrants.take]).
     */
    private fun releaseFolderGrants() {
        val resolver = context.contentResolver
        resolver.persistedUriPermissions.toList().forEach { held ->
            var flags = 0
            if (held.isReadPermission) flags = flags or android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION
            if (held.isWritePermission) flags = flags or android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            if (flags != 0) runCatching { resolver.releasePersistableUriPermission(held.uri, flags) }
        }
    }

    /**
     * Finish a factory reset the process died in the middle of. Called once at boot, before any
     * screen reads the database.
     *
     * The tables are wiped before the preferences, so a process killed in between (the app swiped
     * away right after confirming; there is no progress screen) came back with onboarding marked
     * done over an empty database: the full app, with no program and none of the history. The
     * user confirmed a clean slate, so the boot completes it. One attempt only: the marker goes
     * whatever happens, so a reset that fails here can never wipe what is logged afterwards.
     *
     * This runs BEFORE the startup gate opens, and Room's open helper and the settings DataStore
     * both wait on that gate, so calling [factoryReset] here waited for itself: every later launch
     * sat on "Preparing Avex…" for good. Neither store has been opened yet, so their files are
     * deleted instead — the same file-level approach [com.forge.app.RestoreApply] takes at boot —
     * and Room and DataStore start empty when the gate lets them open.
     */
    suspend fun finishInterruptedFactoryReset() {
        val marker = pendingResetMarker()
        if (!withContext(Dispatchers.IO) { marker.exists() }) return
        try {
            // Both stores first, together: they are what keeps "onboarded" off an empty database,
            // and nothing in the rest of the sweep can then stop them going before the marker does.
            withContext(Dispatchers.IO) {
                context.deleteDatabase(DATABASE_NAME)
                java.io.File(context.filesDir, com.forge.app.RestoreApply.PREFS_PATH).delete()
                // What [SettingsRepository.resetAll] does after its clear, for the same reason.
                com.forge.app.security.ProtectionSentinel.forget(context)
            }
            eraseOutsideTheStores()
        } finally {
            withContext(Dispatchers.IO) { marker.delete() }
        }
    }

    private fun pendingResetMarker() = java.io.File(context.filesDir, PENDING_FACTORY_RESET)

    internal companion object {
        /**
         * filesDir marker held for the whole of [factoryReset]. Internal so the auto-backup can
         * refuse to publish a snapshot while a reset is erasing the data it copied.
         */
        internal const val PENDING_FACTORY_RESET = "factory_reset_pending"

        /** Must match the name `DatabaseModule` builds the live database with. */
        private const val DATABASE_NAME = "forge.db"
    }
}
