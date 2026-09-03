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
