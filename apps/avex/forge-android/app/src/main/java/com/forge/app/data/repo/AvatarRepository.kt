package com.forge.app.data.repo

import android.content.Context
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The profile avatar — a single app-private image file (no Room table / no migration, same call
 * as [ProgressPhotoRepository]). Imported via the Android Photo Picker, copied bytes-as-is so EXIF
 * orientation is preserved for [com.forge.app.ui.profile.ProgressPhotoImage] to honour.
 */
@Singleton
class AvatarRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    private val file: File by lazy { File(context.filesDir, "avatar.jpg") }

    /** The avatar file if one has been set, else null. */
    fun current(): File? = file.takeIf { it.exists() && it.length() > 0 }

    suspend fun set(source: Uri): File? = withContext(Dispatchers.IO) {
        val ok = runCatching {
            context.contentResolver.openInputStream(source)?.use { input ->
                file.outputStream().use { input.copyTo(it) }
            } != null
        }.getOrDefault(false)
        if (!ok || file.length() == 0L) { file.delete(); null } else file
    }

    suspend fun clear() = withContext(Dispatchers.IO) { file.delete() }
}
