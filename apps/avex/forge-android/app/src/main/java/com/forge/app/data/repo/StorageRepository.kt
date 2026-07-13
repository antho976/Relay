package com.forge.app.data.repo

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

/** One line of the storage breakdown: a stable [key], a display [label], and its on-disk [bytes]. */
data class StorageCategory(val key: String, val label: String, val bytes: Long)

/**
 * The app's app-private footprint for the Settings → Storage page (GYMAP-68): the per-category sizes
 * (largest first, zero-sized categories dropped) plus the reclaimable [cacheBytes] the clear action frees.
 */
data class StorageBreakdown(
    val categories: List<StorageCategory> = emptyList(),
    val cacheBytes: Long = 0L
) {
    val totalBytes: Long get() = categories.sumOf { it.bytes }
}

/**
 * Measures the app's on-disk storage for the Storage settings page (GYMAP-68) and clears the transient
 * cache. Everything lives in internal storage, so no permissions are needed. This is pure file
 * arithmetic — it NEVER touches the database or the user's data; [clearCache] only deletes scratch
 * files the app regenerates on demand (snapshots, restore staging, camera temps).
 */
@Singleton
class StorageRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    // Reuse the backup repo's DB-size reader (main file + WAL + SHM) so the number can't drift from it.
    private val backupRepo: BackupRepository,
    private val photoRepo: ProgressPhotoRepository,
    private val avatarRepo: AvatarRepository,
) {

    /** Recursively sum the byte sizes of a directory tree (0 for a missing dir). */
    private fun dirSize(dir: File): Long =
        if (!dir.exists()) 0L else dir.walk().filter { it.isFile }.sumOf { it.length() }

    /** Read the current storage picture off the IO dispatcher — every entry is a File/DB read. */
    suspend fun breakdown(): StorageBreakdown = withContext(Dispatchers.IO) {
        val files = context.filesDir
        val photos = dirSize(photoRepo.dir) + (avatarRepo.file.takeIf { it.exists() }?.length() ?: 0L)
        val database = backupRepo.dbSizeBytes()
        val backup = File(files, AUTO_BACKUP_NAME).takeIf { it.exists() }?.length() ?: 0L
        // Shareable exports the user last generated (JSON/CSV/PDF) — transient, sit in filesDir root.
        val exports = files.listFiles()?.filter {
            it.isFile && it.name.startsWith("forge_") && it.name != AUTO_BACKUP_NAME &&
                it.extension in EXPORT_EXTENSIONS
        }?.sumOf { it.length() } ?: 0L
        val cache = dirSize(context.cacheDir)
        val categories = listOf(
            StorageCategory("photos", "Photos", photos),
            StorageCategory("database", "Workout data", database),
            StorageCategory("backup", "Local backup", backup),
            StorageCategory("exports", "Exports", exports),
            StorageCategory("cache", "Cache", cache),
        ).filter { it.bytes > 0 }.sortedByDescending { it.bytes }
        StorageBreakdown(categories, cache)
    }

    /**
     * Delete every file in the transient cache dir — snapshots, restore staging, camera temps — which
     * the app recreates on demand. No user data is ever touched. Returns the bytes actually reclaimed.
     */
    suspend fun clearCache(): Long = withContext(Dispatchers.IO) {
        val before = dirSize(context.cacheDir)
        context.cacheDir.listFiles()?.forEach { runCatching { it.deleteRecursively() } }
        (before - dirSize(context.cacheDir)).coerceAtLeast(0L)
    }

    private companion object {
        // Kept in sync with BackupRepository.AUTO_BACKUP_NAME (its constant is private; this is a leaf read).
        const val AUTO_BACKUP_NAME = "forge_auto_backup.zip"
        val EXPORT_EXTENSIONS = setOf("json", "csv", "pdf")
    }
}
