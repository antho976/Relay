package com.forge.app.data.repo

import android.content.Context
import android.content.ContextWrapper
import androidx.test.core.app.ApplicationProvider
import com.forge.app.data.db.dao.BodyweightDao
import com.forge.app.data.db.entities.BodyweightEntry
import java.io.File
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ProgressPhotoRepositoryTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun corruptIndexCannotBeRewrittenAsAnEmptyLibrary() = runTest {
        val base: Context = ApplicationProvider.getApplicationContext()
        val context = object : ContextWrapper(base) {
            override fun getFilesDir(): File = temporaryFolder.root
        }
        val repository = ProgressPhotoRepository(context, EmptyBodyweightDao)
        val existingPhoto = File(repository.dir, "pp_existing.jpg").apply { writeText("photo") }
        val index = File(repository.dir, "index.json").apply { writeText("{broken") }
        val captured = temporaryFolder.newFile("capture.jpg").apply { writeText("new photo") }

        assertEquals(emptyList<ProgressPhoto>(), repository.photos())
        assertNull(repository.addCaptured(captured))
        assertEquals("{broken", index.readText())
        assertTrue(existingPhoto.exists())
    }

    @Test
    fun renamingAnAlbumOntoAnotherCaseVariantMergesIntoTheExistingName() = runTest {
        val base: Context = ApplicationProvider.getApplicationContext()
        val context = object : ContextWrapper(base) {
            override fun getFilesDir(): File = temporaryFolder.root
        }
        val repository = ProgressPhotoRepository(context, EmptyBodyweightDao)
        repository.createAlbum("Legs")
        repository.createAlbum("Arms")

        repository.renameAlbum("Arms", "legs")

        assertEquals(listOf("Legs"), repository.albums())
    }

    @Test
    fun renamingAnAlbumToADifferentCaseOfItselfChangesItsCasing() = runTest {
        val base: Context = ApplicationProvider.getApplicationContext()
        val context = object : ContextWrapper(base) {
            override fun getFilesDir(): File = temporaryFolder.root
        }
        val repository = ProgressPhotoRepository(context, EmptyBodyweightDao)
        repository.createAlbum("Legs")

        repository.renameAlbum("Legs", "LEGS")

        assertEquals(listOf("LEGS"), repository.albums())
    }

    @Test
    fun reDatingReSnapshotsTheBodyweightButKeepsATypedOne() = runTest {
        val base: Context = ApplicationProvider.getApplicationContext()
        val context = object : ContextWrapper(base) {
            override fun getFilesDir(): File = temporaryFolder.root
        }
        val day = 24L * 60 * 60 * 1000
        val now = 1_780_000_000_000L
        val then = now - 180 * day
        val dao = WeighInsDao(
            listOf(
                BodyweightEntry(id = 1, dateKey = "now", weightLb = 176.0, recordedAt = now),
                BodyweightEntry(id = 2, dateKey = "then", weightLb = 198.0, recordedAt = then)
            )
        )
        val repository = ProgressPhotoRepository(context, dao)
        listOf("pp_a.jpg", "pp_b.jpg", "pp_c.jpg").forEach { File(repository.dir, it).writeText("photo") }
        File(repository.dir, "index.json").writeText(
            """[{"file":"pp_a.jpg","takenAtMs":$now,"weightLb":176.0},""" +
                """{"file":"pp_b.jpg","takenAtMs":$now,"weightLb":170.0},""" +
                """{"file":"pp_c.jpg","takenAtMs":$now}]"""
        )
        val (snapshot, typed, none) = repository.photos().sortedBy { it.fileName }

        // Dated "now" on import, it carried today's weigh-in: the old date's weigh-in replaces it.
        assertEquals(198.0, repository.setTakenAt(snapshot, then)?.weightLb)
        // A weight that is not the old date's snapshot was typed by the user, and survives.
        assertEquals(170.0, repository.setTakenAt(typed, then)?.weightLb)
        // No weight at all: snapshot the new date.
        assertEquals(198.0, repository.setTakenAt(none, then)?.weightLb)

        val stored = repository.photos().associateBy { it.fileName }
        assertEquals(198.0, stored.getValue("pp_a.jpg").weightLb)
        assertEquals(170.0, stored.getValue("pp_b.jpg").weightLb)
        assertEquals(198.0, stored.getValue("pp_c.jpg").weightLb)
        assertTrue(stored.values.all { it.takenAtMs == then })
    }

    private class WeighInsDao(private val entries: List<BodyweightEntry>) : BodyweightDao by EmptyBodyweightDao {
        override suspend fun all(): List<BodyweightEntry> = entries
    }

    private object EmptyBodyweightDao : BodyweightDao {
        override suspend fun upsert(entry: BodyweightEntry): Long = 0L
        override fun observeRecent(limit: Int): Flow<List<BodyweightEntry>> = flowOf(emptyList())
        override suspend fun latest(): BodyweightEntry? = null
        override suspend fun earliestSince(sinceMs: Long): BodyweightEntry? = null
        override suspend fun byDateKey(dateKey: String): BodyweightEntry? = null
        override suspend fun all(): List<BodyweightEntry> = emptyList()
        override suspend fun since(sinceMs: Long): List<BodyweightEntry> = emptyList()
        override suspend fun byId(id: Long): BodyweightEntry? = null
        override suspend fun delete(id: Long) = Unit
        override suspend fun deleteAll() = Unit
    }
}
