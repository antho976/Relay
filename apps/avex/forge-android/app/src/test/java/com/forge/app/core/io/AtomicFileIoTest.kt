package com.forge.app.core.io

import java.io.File
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class AtomicFileIoTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun failedReplacementPreservesPreviousContents() {
        val target = temporaryFolder.newFile("index.json")
        target.writeTextAtomically("original")

        try {
            target.writeAtomically { output ->
                output.write("partial".toByteArray())
                throw IOException("simulated full disk")
            }
            fail("write should fail")
        } catch (_: IOException) {
            // Expected.
        }

        assertEquals("original", target.readTextAtomically())
    }

    @Test
    fun readRecoversBackupLeftByInterruptedWrite() {
        val target = File(temporaryFolder.root, "albums.json")
        File(target.parentFile, "${target.name}.bak").writeText("original")

        assertEquals(true, target.existsAtomically())
        assertEquals("original", target.readTextAtomically())
        assertEquals("original", target.readText())
    }
}
