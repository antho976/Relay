package com.forge.app.data.importer

import androidx.documentfile.provider.DocumentFile
import com.forge.app.core.crypto.BackupCrypto
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The Import screen's folder scan: exports are found below the top level, hidden and too-deep
 * folders are left alone, and Avex backups are told apart from every other ZIP by their bytes.
 */
@RunWith(RobolectricTestRunner::class)
class FolderScanTest {

    @get:Rule
    val tmp = TemporaryFolder()

    private fun touch(path: String) = File(tmp.root, path).apply { parentFile!!.mkdirs(); writeText("x") }

    private fun names(files: List<DocumentFile>) = files.mapNotNull { it.name }.sorted()

    @Test
    fun exportsInSubfoldersAreFoundTwoLevelsDown() {
        touch("strong.csv")
        touch("Strong/strong_workouts.csv")
        touch("Hevy/2026/hevy.csv")
        touch("a/b/c/too_deep.csv")
        touch(".thumbnails/hidden.csv")

        val found = listFilesDeep(DocumentFile.fromFile(tmp.root), maxDepth = 2, maxDirs = 40)

        assertEquals(listOf("hevy.csv", "strong.csv", "strong_workouts.csv"), names(found))
    }

    @Test
    fun theWalkStopsAtItsFolderBudget() {
        repeat(10) { touch("dir$it/file$it.csv") }
        val found = listFilesDeep(DocumentFile.fromFile(tmp.root), maxDepth = 2, maxDirs = 4)
        // The root plus three subfolders were visited: three files, not ten.
        assertEquals(3, found.size)
    }

    private fun zipWithFirstEntry(name: String): ByteArray {
        val bos = ByteArrayOutputStream()
        ZipOutputStream(bos).use { z ->
            z.putNextEntry(ZipEntry(name)); z.write(ByteArray(100)); z.closeEntry()
        }
        return bos.toByteArray().copyOf(64)
    }

    @Test
    fun aPlainAvexBackupIsRecognisedByItsFirstEntry() {
        assertEquals(false, backupHeadKind(zipWithFirstEntry("database.db")))
    }

    @Test
    fun aPasswordProtectedBackupIsRecognisedByItsMagic() {
        val key = BackupCrypto.newMasterKey("pull day forever".toCharArray(), iterations = 100_000)
        val bos = ByteArrayOutputStream()
        BackupCrypto.encryptingStream(bos, key).use { it.write(ByteArray(10)) }
        assertEquals(true, backupHeadKind(bos.toByteArray().copyOf(64)))
    }

    @Test
    fun otherZipsAndFilesAreNotBackups() {
        assertNull(backupHeadKind(zipWithFirstEntry("photos/IMG_0001.jpg")))
        assertNull(backupHeadKind(zipWithFirstEntry("database.dbx")))
        assertNull(backupHeadKind("Date,Exercise Name,Weight\n".toByteArray()))
        assertNull(backupHeadKind(ByteArray(0)))
    }
}
