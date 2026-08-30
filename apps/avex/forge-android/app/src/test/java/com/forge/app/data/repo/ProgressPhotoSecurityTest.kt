package com.forge.app.data.repo

import androidx.test.core.app.ApplicationProvider
import com.forge.app.data.db.ForgeDatabase
import com.forge.app.data.db.inMemoryForgeDb
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.File

/**
 * `progress_photos/index.json` is metadata the app reads back as instructions, and a restored ZIP
 * can contain it.
 *
 * The restore validates ZIP ENTRY names — flat basenames, no separators — and then trusts what those
 * entries CONTAIN. The index's `file` field went straight into `File(dir, name)`, so an entry
 * reading `../../databases/forge.db` produced a "photo" whose backing file was the live database:
 * it appeared in the gallery, and the Delete action deleted it. Any app-private file an attacker
 * could name was reachable that way, from a backup file the user was invited to import.
 */
@RunWith(RobolectricTestRunner::class)
class ProgressPhotoSecurityTest {

    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()
    private val db: ForgeDatabase = inMemoryForgeDb()
    private val repo = ProgressPhotoRepository(context, db.bodyweightDao())

    @After
    fun tearDown() {
        repo.dir.deleteRecursively()
        db.close()
    }

    /** Write an index.json naming exactly [fileNames], as a hostile archive would. */
    private fun writeIndex(vararg fileNames: String) {
        repo.dir.mkdirs()
        val entries = fileNames.joinToString(",") {
            """{"file":"${it.replace("\\", "\\\\")}","takenAtMs":1000}"""
        }
        File(repo.dir, "index.json").writeText("[$entries]")
    }

    /** A real photo file, so a legitimate entry has something to point at. */
    private fun realPhoto(name: String = "pp_0123456789ab.jpg"): File =
        File(repo.dir, name).apply { parentFile?.mkdirs(); writeText("jpeg bytes") }

    @Test
    fun `a traversal entry is not a photo`() = runTest {
        val victim = File(context.filesDir, "victim.db").apply { writeText("the database") }
        writeIndex("../victim.db")

        assertEquals("a name outside the folder is not a photo", emptyList<ProgressPhoto>(), repo.photos())
        assertTrue("and nothing has touched the file it named", victim.exists())
    }

    @Test
    fun `deleting a traversal entry cannot delete the file it names`() = runTest {
        // The dangerous half: even if such an entry reached a ProgressPhoto by another route, the
        // Delete action must not follow it out of the folder.
        val victim = File(context.filesDir, "victim.db").apply { writeText("the database") }
        realPhoto()
        writeIndex("pp_0123456789ab.jpg")

        repo.delete(ProgressPhoto(fileName = "../victim.db", takenAtMs = 1000))

        assertTrue("the named file must survive", victim.exists())
        assertEquals("the database", victim.readText())
    }

    @Test
    fun `absolute paths and separators are refused`() = runTest {
        writeIndex(
            "/data/data/com.forge.app/databases/forge.db",
            "..\\..\\databases\\forge.db",
            "subdir/pp_0123456789ab.jpg"
        )
        assertEquals(emptyList<ProgressPhoto>(), repo.photos())
    }

    @Test
    fun `the index cannot name the index, or the album list`() = runTest {
        // Both exist inside the folder, so a canonical-parent check alone would admit them: deleting
        // a "photo" called index.json would take the whole library's metadata with it.
        File(repo.dir, "albums.json").apply { parentFile?.mkdirs(); writeText("[]") }
        writeIndex("index.json", "albums.json")
        assertEquals(emptyList<ProgressPhoto>(), repo.photos())
    }

    @Test
    fun `a real photo entry still loads`() = runTest {
        realPhoto()
        writeIndex("pp_0123456789ab.jpg")

        val photos = repo.photos()
        assertEquals(1, photos.size)
        assertEquals("pp_0123456789ab.jpg", photos.single().fileName)
        assertEquals(
            File(repo.dir, "pp_0123456789ab.jpg").canonicalFile,
            repo.fileForOrNull(photos.single())
        )
    }

    @Test
    fun `one hostile entry costs the user that entry, not the library`() = runTest {
        realPhoto()
        writeIndex("../victim.db", "pp_0123456789ab.jpg")

        assertEquals(1, repo.photos().size)
    }

    @Test
    fun `an entry whose file is gone is still dropped`() = runTest {
        writeIndex("pp_0123456789ab.jpg")
        assertEquals(emptyList<ProgressPhoto>(), repo.photos())
    }

    @Test
    fun `delete removes the bytes before the metadata`() = runTest {
        // "Deleted" has to mean the bytes are gone. The old order dropped the index entry first and
        // ignored File.delete()'s result, so a failure left the image on disk with nothing pointing
        // at it — invisible in the gallery, and still swept into every future backup archive.
        val file = realPhoto()
        writeIndex("pp_0123456789ab.jpg")
        val photo = repo.photos().single()

        repo.delete(photo)

        assertFalse("the image bytes must be gone", file.exists())
        assertEquals(emptyList<ProgressPhoto>(), repo.photos())
    }

    @Test
    fun `fileFor hands render-only callers something that does not exist`() {
        // They already treat a missing file as a missing image; what they must never be handed is a
        // real path outside the photo folder.
        val resolved = repo.fileFor(ProgressPhoto(fileName = "../victim.db", takenAtMs = 1000))
        assertFalse(resolved.exists())
        assertNull(repo.fileForOrNull(ProgressPhoto(fileName = "../victim.db", takenAtMs = 1000)))
    }
}
