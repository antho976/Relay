package com.forge.app.data.repo

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The profile avatar: a single app-private image file (`filesDir/avatar.jpg`), imported via the
 * Android Photo Picker. Deliberately NOT a Room table / DataStore value — it's a file, like the
 * progress photos ([ProgressPhotoRepository]). Folded into the whole-DB backup archive so it
 * survives a restore. Offline, single-user, no network.
 */
@Singleton
class AvatarRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    /** Public so the backup archive + factory reset can read/clear it. */
    val file: File get() = File(context.filesDir, FILE_NAME)

    fun exists(): Boolean = file.let { it.exists() && it.length() > 0 }

    /**
     * Import the picked image: decode downsampled to ~[MAX_PX], apply its EXIF rotation, and write a
     * compact JPEG. An avatar is only ever shown as a small circle, so storing the raw multi-MB pick
     * (which would also bloat every backup ZIP) is wasteful — re-encoding keeps it tiny. Rotation is
     * baked in and the EXIF tag dropped, so [ProgressPhotoImage] (which honours EXIF) shows it upright.
     * Returns true on success.
     */
    suspend fun set(source: Uri): Boolean = withContext(Dispatchers.IO) {
        val ok = runCatching {
            val resolver = context.contentResolver
            // Bounds pass to pick a sane inSampleSize (content streams aren't reusable → reopen each pass).
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            resolver.openInputStream(source)?.use { BitmapFactory.decodeStream(it, null, bounds) }
            var sample = 1
            val maxDim = maxOf(bounds.outWidth, bounds.outHeight)
            while (maxDim > 0 && maxDim / (sample * 2) >= MAX_PX) sample *= 2
            val decoded = resolver.openInputStream(source)?.use {
                BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample })
            } ?: return@runCatching false
            val degrees = runCatching {
                resolver.openInputStream(source)?.use { ins ->
                    when (ExifInterface(ins).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)) {
                        ExifInterface.ORIENTATION_ROTATE_90 -> 90f
                        ExifInterface.ORIENTATION_ROTATE_180 -> 180f
                        ExifInterface.ORIENTATION_ROTATE_270 -> 270f
                        else -> 0f
                    }
                } ?: 0f
            }.getOrDefault(0f)
            val upright = if (degrees == 0f) decoded
                else Bitmap.createBitmap(decoded, 0, 0, decoded.width, decoded.height, Matrix().apply { postRotate(degrees) }, true)
            file.outputStream().use { upright.compress(Bitmap.CompressFormat.JPEG, 90, it) }
            if (upright !== decoded) decoded.recycle()
            upright.recycle()
            true
        }.getOrDefault(false)
        if (!ok || file.length() == 0L) { file.delete(); false } else true
    }

    suspend fun clear() = withContext(Dispatchers.IO) { file.delete(); Unit }

    companion object {
        const val FILE_NAME = "avatar.jpg"
        /** Max stored edge — an avatar is a small circle, so this is plenty and keeps the file tiny. */
        private const val MAX_PX = 512
    }
}
