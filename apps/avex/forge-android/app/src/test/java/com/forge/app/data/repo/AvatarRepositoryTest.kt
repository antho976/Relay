package com.forge.app.data.repo

import android.content.Context
import android.content.ContextWrapper
import androidx.test.core.app.ApplicationProvider
import com.forge.app.core.time.Clock
import com.forge.app.data.prefs.SettingsRepository
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class AvatarRepositoryTest {

    @get:Rule
    val temporaryFolder = TemporaryFolder()

    @Test
    fun failedReplacementKeepsPreviousAvatar() = runBlocking {
        val base: Context = ApplicationProvider.getApplicationContext()
        val context = object : ContextWrapper(base) {
            override fun getFilesDir(): File = temporaryFolder.root
        }
        val repository = AvatarRepository(context, SettingsRepository(context, Clock { 0L }))
        repository.file.writeText("existing avatar")

        assertFalse(repository.setFromResource(Int.MAX_VALUE))
        assertEquals("existing avatar", repository.file.readText())
    }
}
